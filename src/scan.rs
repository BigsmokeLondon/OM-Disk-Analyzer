//! The folder scanner: walks a folder on a background thread and builds a size tree.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use crate::fmt::wildcard;
use crate::sys;

pub type NodeId = usize;

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub size: u64,
    pub own_size: u64,
    pub alloc: u64,
    pub own_alloc: u64,
    pub files: u64,
    pub own_files: u64,
    pub dirs: u64,
    pub mtime: i64,
    pub atime: i64,
    pub error: Option<String>,
    pub compressed: bool,
    pub depth: u16,
}

#[derive(Clone, Debug)]
pub struct TopFile {
    pub size: u64,
    pub alloc: u64,
    pub path: PathBuf,
    pub mtime: i64,
    pub atime: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgeBasis {
    LastChange,
    LastAccess,
    Created,
}

impl AgeBasis {
    pub fn label(self) -> &'static str {
        match self {
            AgeBasis::LastChange => "Last Change Date",
            AgeBasis::LastAccess => "Last Access Date",
            AgeBasis::Created => "Creation Date",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub follow_links: bool,
    pub ext_stats: bool,
    pub owner_stats: bool,
    pub exclude_dirs: Vec<String>,
    pub include_files: Vec<String>,
    pub age_basis: AgeBasis,
    pub age_days: Vec<u32>,
    pub priority: i32,
    /// Scanning threads (0 = automatic, at most `MAX_THREADS`).
    pub threads: usize,
    /// Count a file that has several names (hard links) once.
    pub hard_links: bool,
}

/// Age buckets from day limits: [(limit, label)], the last one open-ended.
pub fn make_buckets(days: &[u32]) -> Vec<(f64, String)> {
    let mut vals: Vec<u32> = days.iter().copied().filter(|&d| d > 0).collect();
    vals.sort_unstable();
    vals.dedup();
    fn human(d: u32) -> String {
        let plural = |n: u32, unit: &str| format!("{n} {unit}{}", if n > 1 { "s" } else { "" });
        if d % 365 == 0 {
            plural(d / 365, "year")
        } else if d == 182 || d == 183 {
            "6 months".into()
        } else if d % 30 == 0 {
            plural(d / 30, "month")
        } else if d % 7 == 0 {
            plural(d / 7, "week")
        } else {
            plural(d, "day")
        }
    }
    let mut out = Vec::new();
    let mut prev: Option<u32> = None;
    for d in vals {
        let label = match prev {
            None => format!("Less than {}", human(d)),
            Some(p) => format!("{} - {}", human(p), human(d)),
        };
        out.push((d as f64, label));
        prev = Some(d);
    }
    out.push((f64::INFINITY, prev.map(|p| format!("More than {}", human(p))).unwrap_or_else(|| "All files".into())));
    out
}

pub const TOP_N: usize = 100;

#[derive(Default)]
pub struct ScanResult {
    pub nodes: Vec<Node>,
    pub ext_stats: HashMap<String, (u64, u64, u64)>,
    pub age_buckets: Vec<(f64, String)>,
    pub age_stats: Vec<(u64, u64)>,
    pub age_basis: Option<AgeBasis>,
    pub owner_stats: HashMap<String, (u64, u64)>,
    pub top_files: Vec<TopFile>,
    pub errors: Vec<(String, String)>,
    pub error_count: u64,
    pub elapsed: f64,
    pub finished_at: i64,
    pub cancelled: bool,
    pub ext_enabled: bool,
    pub owner_enabled: bool,
    pub cluster: u64,
}

/// Live counters the window reads while a scan runs.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub stop: AtomicBool,
    pub done: AtomicBool,
    pub current: Mutex<String>,
    pub result: Mutex<Option<ScanResult>>,
}

pub fn start(path: PathBuf, cluster: u64, opts: ScanOptions) -> Arc<Progress> {
    let prog = Arc::new(Progress::default());
    let p2 = prog.clone();
    std::thread::spawn(move || {
        sys::set_thread_priority(opts.priority);
        let res = run(&path, cluster, &opts, &p2);
        *p2.result.lock().unwrap() = Some(res);
        p2.done.store(true, Ordering::SeqCst);
    });
    prog
}

/// Space a file takes on disk: whole clusters (estimated: compressed, sparse and very small
/// files stored inside the file table take less).
pub fn allocated(size: u64, cluster: u64) -> u64 {
    if cluster == 0 {
        size
    } else {
        size.div_ceil(cluster) * cluster
    }
}

struct FileFacts {
    size: u64,
    mtime: i64,
    atime: i64,
    ctime: i64,
    attrs: u32,
}

fn facts(md: &std::fs::Metadata) -> FileFacts {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        FileFacts {
            size: md.file_size(),
            mtime: sys::filetime_to_unix(md.last_write_time()),
            atime: sys::filetime_to_unix(md.last_access_time()),
            ctime: sys::filetime_to_unix(md.creation_time()),
            attrs: md.file_attributes(),
        }
    }
    #[cfg(not(windows))]
    {
        let t = |r: std::io::Result<std::time::SystemTime>| {
            r.ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        };
        FileFacts { size: md.len(), mtime: t(md.modified()), atime: t(md.accessed()), ctime: t(md.created()), attrs: 0 }
    }
}

const ATTR_REPARSE: u32 = 0x400;
const ATTR_COMPRESSED: u32 = 0x800;
const ATTR_SPARSE: u32 = 0x200;

/// Most scanning threads the options offer (the earlier version's limit).
pub const MAX_THREADS: usize = 8;

/// How many threads a scan really uses. Link-following needs one, so that the folder reached
/// first is always the same one; 0 means "automatic".
pub fn thread_count(o: &ScanOptions) -> usize {
    if o.follow_links {
        return 1;
    }
    if o.threads == 0 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, MAX_THREADS)
    } else {
        o.threads.clamp(1, MAX_THREADS)
    }
}

/// One file that has been looked at and is ready to be counted.
struct Counted {
    name: String,
    path: PathBuf,
    size: u64,
    alloc: u64,
    mtime: i64,
    atime: i64,
    ctime: i64,
}

/// A file with several names, held back until the end so it is counted once, under the first
/// name in path order (the same answer however many threads ran).
struct Pending {
    node: NodeId,
    key: (u64, u64),
    file: Counted,
}

type TopItem = (u64, PathBuf, i64, i64);

fn push_top(heap: &mut BinaryHeap<Reverse<TopItem>>, item: TopItem) {
    if heap.len() < TOP_N {
        heap.push(Reverse(item));
    } else if heap.peek().map(|Reverse(least)| item > *least).unwrap_or(false) {
        heap.pop();
        heap.push(Reverse(item));
    }
}

/// What the scan needs to know that never changes while it runs.
struct Ctx<'a> {
    o: &'a ScanOptions,
    cluster: u64,
    now: i64,
    buckets: Vec<(f64, String)>,
    exclude: Vec<String>,
    include: Vec<String>,
}

/// One worker's running totals; the workers' are added together when the scan ends.
#[derive(Default)]
struct Local {
    ext: HashMap<String, (u64, u64, u64)>,
    owner: HashMap<String, (u64, u64)>,
    age: Vec<(u64, u64)>,
    top: BinaryHeap<Reverse<TopItem>>,
    errors: Vec<(String, String)>,
    error_count: u64,
    cancelled: bool,
    links: Vec<Pending>,
}

impl Local {
    fn new(buckets: usize) -> Local {
        Local { age: vec![(0, 0); buckets], ..Default::default() }
    }

    fn error(&mut self, path: &Path, msg: String) {
        self.error_count += 1;
        if self.errors.len() < 1000 {
            self.errors.push((path.to_string_lossy().into_owned(), msg));
        }
    }

    /// Add one file to the extension, owner, age and largest-files statistics.
    fn count(&mut self, c: &Ctx, f: &Counted) {
        if c.o.ext_stats {
            let ext = match f.name.rfind('.') {
                Some(d) if d > 0 => f.name[d + 1..].to_lowercase(),
                _ => String::new(),
            };
            let r = self.ext.entry(ext).or_insert((0, 0, 0));
            r.0 += f.size;
            r.1 += 1;
            r.2 += f.alloc;
        }
        if c.o.owner_stats {
            let mut owner = sys::owner_of(&f.path);
            if owner.is_empty() {
                owner = "(unknown)".into();
            }
            let r = self.owner.entry(owner).or_insert((0, 0));
            r.0 += f.size;
            r.1 += 1;
        }
        let t = match c.o.age_basis {
            AgeBasis::LastChange => f.mtime,
            AgeBasis::LastAccess => f.atime,
            AgeBasis::Created => f.ctime,
        };
        let days = (c.now - t) as f64 / 86400.0;
        if let Some(i) = c.buckets.iter().position(|(limit, _)| days < *limit) {
            self.age[i].0 += f.size;
            self.age[i].1 += 1;
        }
        push_top(&mut self.top, (f.size, f.path.clone(), f.mtime, f.atime));
    }
}

/// A folder waiting to be read.
struct Work {
    id: NodeId,
    path: PathBuf,
    depth: u16,
}

/// Shared between the workers: the tree so far and the folders still to read.
struct Shared {
    nodes: Vec<Node>,
    queue: Vec<Work>,
    /// Workers busy reading a folder; the scan is over when this is 0 and the queue is empty.
    active: usize,
}

struct Pool {
    state: Mutex<Shared>,
    wake: Condvar,
    /// Real paths of folders already taken (link-following only).
    visited: Mutex<HashSet<PathBuf>>,
}

/// What reading one folder found.
#[derive(Default)]
struct DirOut {
    subdirs: Vec<Node>,
    own_size: u64,
    own_alloc: u64,
    own_files: u64,
    mtime: i64,
    atime: i64,
    error: Option<String>,
}

fn run(root_path: &Path, cluster: u64, o: &ScanOptions, prog: &Progress) -> ScanResult {
    let t0 = Instant::now();
    let buckets = make_buckets(&o.age_days);
    let ctx = Ctx {
        o,
        cluster,
        now: sys::now_unix(),
        buckets: buckets.clone(),
        exclude: o.exclude_dirs.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect(),
        include: o.include_files.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect(),
    };
    let mut res = ScanResult {
        age_stats: vec![(0, 0); buckets.len()],
        age_buckets: buckets,
        age_basis: Some(o.age_basis),
        ext_enabled: o.ext_stats,
        owner_enabled: o.owner_stats,
        cluster,
        ..Default::default()
    };

    let mut root = Node { name: root_path.to_string_lossy().into_owned(), path: root_path.to_path_buf(), ..Default::default() };
    if let Ok(md) = std::fs::metadata(root_path) {
        let f = facts(&md);
        root.mtime = f.mtime;
        root.atime = f.atime;
    }
    let mut visited: HashSet<PathBuf> = HashSet::new();
    if let Ok(c) = std::fs::canonicalize(root_path) {
        visited.insert(c);
    }
    let pool = Pool {
        state: Mutex::new(Shared {
            nodes: vec![root],
            queue: vec![Work { id: 0, path: root_path.to_path_buf(), depth: 0 }],
            active: 0,
        }),
        wake: Condvar::new(),
        visited: Mutex::new(visited),
    };

    let threads = thread_count(o);
    let mut locals: Vec<Local> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads).map(|_| s.spawn(|| worker(&pool, &ctx, prog))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    res.cancelled = locals.iter().any(|l| l.cancelled);
    let mut nodes = pool.state.into_inner().unwrap().nodes;

    // Files with several names: count each once, under the first name in path order.
    let mut links: Vec<Pending> = locals.iter_mut().flat_map(|l| std::mem::take(&mut l.links)).collect();
    links.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut last = Local::new(res.age_stats.len());
    for p in links {
        if !seen.insert(p.key) {
            continue;
        }
        let n = &mut nodes[p.node];
        n.own_size += p.file.size;
        n.own_alloc += p.file.alloc;
        n.own_files += 1;
        n.mtime = n.mtime.max(p.file.mtime);
        n.atime = n.atime.max(p.file.atime);
        last.count(&ctx, &p.file);
        prog.files.fetch_add(1, Ordering::Relaxed);
        prog.bytes.fetch_add(p.file.size, Ordering::Relaxed);
    }
    locals.push(last);

    // Add the workers' statistics together.
    let mut top: BinaryHeap<Reverse<TopItem>> = BinaryHeap::new();
    for l in locals {
        for (k, v) in l.ext {
            let r = res.ext_stats.entry(k).or_insert((0, 0, 0));
            r.0 += v.0;
            r.1 += v.1;
            r.2 += v.2;
        }
        for (k, v) in l.owner {
            let r = res.owner_stats.entry(k).or_insert((0, 0));
            r.0 += v.0;
            r.1 += v.1;
        }
        for (i, v) in l.age.into_iter().enumerate() {
            res.age_stats[i].0 += v.0;
            res.age_stats[i].1 += v.1;
        }
        for Reverse(item) in l.top {
            push_top(&mut top, item);
        }
        res.error_count += l.error_count;
        res.errors.extend(l.errors);
    }
    res.errors.sort();
    res.errors.truncate(1000);

    // Children always come after their parent, so one backward pass adds everything up.
    for i in (0..nodes.len()).rev() {
        let n = &mut nodes[i];
        n.size += n.own_size;
        n.alloc += n.own_alloc;
        n.files += n.own_files;
        let (size, alloc, files, dirs, mt, at, parent) = (n.size, n.alloc, n.files, n.dirs, n.mtime, n.atime, n.parent);
        if let Some(p) = parent {
            let p = &mut nodes[p];
            p.size += size;
            p.alloc += alloc;
            p.files += files;
            p.dirs += dirs + 1;
            p.mtime = p.mtime.max(mt);
            p.atime = p.atime.max(at);
        }
    }
    // The workers finish folders in any order; sort each folder's children by name so the
    // result does not depend on that.
    for i in 0..nodes.len() {
        let mut kids = std::mem::take(&mut nodes[i].children);
        kids.sort_by(|&a, &b| nodes[a].name.cmp(&nodes[b].name));
        nodes[i].children = kids;
    }
    let mut tops: Vec<TopFile> = top
        .into_iter()
        .map(|Reverse((size, path, mtime, atime))| TopFile { size, alloc: allocated(size, cluster), path, mtime, atime })
        .collect();
    tops.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    res.top_files = tops;
    res.nodes = nodes;
    res.elapsed = t0.elapsed().as_secs_f64();
    res.finished_at = sys::now_unix();
    res
}

/// One scanning thread: takes folders off the queue until none are left.
fn worker(pool: &Pool, ctx: &Ctx, prog: &Progress) -> Local {
    sys::set_thread_priority(ctx.o.priority);
    let mut local = Local::new(ctx.buckets.len());
    loop {
        let work = {
            let mut sh = pool.state.lock().unwrap();
            loop {
                if let Some(w) = sh.queue.pop() {
                    if prog.stop.load(Ordering::Relaxed) {
                        local.cancelled = true;
                        return local;
                    }
                    sh.active += 1;
                    break w;
                }
                if sh.active == 0 {
                    return local;
                }
                // Another worker may still add folders; look again when it says so.
                sh = pool.wake.wait_timeout(sh, std::time::Duration::from_millis(50)).unwrap().0;
            }
        };
        if let Ok(mut c) = prog.current.try_lock() {
            *c = work.path.to_string_lossy().into_owned();
        }
        let out = read_folder(pool, ctx, prog, &work, &mut local);
        let mut sh = pool.state.lock().unwrap();
        let n = &mut sh.nodes[work.id];
        n.own_size += out.own_size;
        n.own_alloc += out.own_alloc;
        n.own_files += out.own_files;
        n.mtime = n.mtime.max(out.mtime);
        n.atime = n.atime.max(out.atime);
        if out.error.is_some() {
            n.error = out.error;
        }
        for child in out.subdirs {
            let cid = sh.nodes.len();
            sh.queue.push(Work { id: cid, path: child.path.clone(), depth: child.depth });
            sh.nodes.push(Node { parent: Some(work.id), ..child });
            sh.nodes[work.id].children.push(cid);
        }
        sh.active -= 1;
        drop(sh);
        pool.wake.notify_all();
    }
}

/// Read one folder: count its files and list its sub-folders. Takes no lock while it reads.
fn read_folder(pool: &Pool, ctx: &Ctx, prog: &Progress, work: &Work, local: &mut Local) -> DirOut {
    let o = ctx.o;
    let mut out = DirOut::default();
    let dir = &work.path;
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            local.error(dir, msg.clone());
            out.error = Some(msg);
            return out;
        }
    };
    let depth = work.depth + 1;
    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                local.error(dir, e.to_string());
                continue;
            }
        };
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(e) => {
                local.error(&entry.path(), e.to_string());
                continue;
            }
        };
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                local.error(&entry.path(), e.to_string());
                continue;
            }
        };
        let f = facts(&md);
        let is_link = ft.is_symlink() || f.attrs & ATTR_REPARSE != 0;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let is_dir = if is_link {
            if !o.follow_links {
                continue;
            }
            path.is_dir()
        } else {
            ft.is_dir()
        };
        if is_dir {
            // With link-following on, every folder (not only links) is recorded by its real
            // path, so a folder reached both directly and through a link counts once.
            if is_link || o.follow_links {
                match std::fs::canonicalize(&path) {
                    Ok(real) if pool.visited.lock().unwrap().insert(real.clone()) => {}
                    _ => continue,
                }
            }
            if !ctx.exclude.is_empty() {
                let (n, p) = (name.to_lowercase(), path.to_string_lossy().to_lowercase());
                if ctx.exclude.iter().any(|x| wildcard(x, &n) || wildcard(x, &p)) {
                    continue;
                }
            }
            out.subdirs.push(Node {
                name,
                path,
                mtime: f.mtime,
                atime: f.atime,
                compressed: f.attrs & ATTR_COMPRESSED != 0,
                depth,
                ..Default::default()
            });
            prog.dirs.fetch_add(1, Ordering::Relaxed);
        } else {
            if !ctx.include.is_empty() && !ctx.include.iter().any(|x| wildcard(x, &name)) {
                continue;
            }
            let key = if o.hard_links && !is_link { sys::hard_link_key(&md, &path) } else { None };
            let f = if is_link { std::fs::metadata(&path).map(|m| facts(&m)).unwrap_or(f) } else { f };
            let sz = f.size;
            // 0x5eacc4: compressed or sparse files take what Windows reports as their
            // compressed size, rounded up to whole clusters; others take whole clusters.
            let al = if f.attrs & (ATTR_COMPRESSED | ATTR_SPARSE) != 0 {
                allocated(sys::compressed_size(&path).unwrap_or(sz), ctx.cluster)
            } else {
                allocated(sz, ctx.cluster)
            };
            let file = Counted { name, path, size: sz, alloc: al, mtime: f.mtime, atime: f.atime, ctime: f.ctime };
            if let Some(key) = key {
                local.links.push(Pending { node: work.id, key, file });
                continue;
            }
            out.own_size += sz;
            out.own_alloc += al;
            out.own_files += 1;
            out.mtime = out.mtime.max(file.mtime);
            out.atime = out.atime.max(file.atime);
            local.count(ctx, &file);
            prog.files.fetch_add(1, Ordering::Relaxed);
            prog.bytes.fetch_add(sz, Ordering::Relaxed);
        }
    }
    out
}

/// Files directly inside one folder (for the Details list): (name, path, size, mtime, atime).
pub fn files_in(dir: &Path) -> Vec<(String, PathBuf, u64, i64, i64)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let (Ok(ft), Ok(md)) = (e.file_type(), e.metadata()) {
                if ft.is_file() {
                    let f = facts(&md);
                    out.push((e.file_name().to_string_lossy().into_owned(), e.path(), f.size, f.mtime, f.atime));
                }
            }
        }
    }
    out
}

/// Search the scanned folders for files whose name matches a wildcard pattern.
pub fn search(
    nodes: &[Node],
    pattern: &str,
    min_size: u64,
    stop: &AtomicBool,
    out: &Mutex<Vec<(String, PathBuf, u64, i64)>>,
) {
    let pat = if pattern.contains('*') || pattern.contains('?') { pattern.to_string() } else { format!("*{}*", pattern) };
    for n in nodes {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if n.own_files == 0 {
            continue;
        }
        for (name, path, size, mtime, _) in files_in(&n.path) {
            if size >= min_size && wildcard(&pat, &name) {
                let mut o = out.lock().unwrap();
                if o.len() >= 10_000 {
                    return;
                }
                o.push((name, path, size, mtime));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn buckets_and_alloc() {
        let b = make_buckets(&[30, 90, 180, 365, 730, 1825]);
        assert_eq!(b[0].1, "Less than 1 month");
        assert_eq!(b[1].1, "1 month - 3 months");
        assert_eq!(b.last().unwrap().1, "More than 5 years");
        let d = make_buckets(&[1, 7, 30, 182, 365, 730]);
        let labels: Vec<&str> = d.iter().map(|x| x.1.as_str()).collect();
        assert_eq!(labels, ["Less than 1 day", "1 day - 1 week", "1 week - 1 month", "1 month - 6 months", "6 months - 1 year", "1 year - 2 years", "More than 2 years"]);
        assert_eq!(allocated(1, 4096), 4096);
        assert_eq!(allocated(4097, 4096), 8192);
        assert_eq!(allocated(0, 4096), 0);
    }

    #[test]
    fn allocated_edges() {
        assert_eq!(allocated(4096, 4096), 4096);
        assert_eq!(allocated(8192, 4096), 8192);
        assert_eq!(allocated(123, 0), 123);
    }

    #[test]
    fn buckets_dedupe_sort_and_empty() {
        let b = make_buckets(&[7, 1, 7, 0, 30]);
        assert_eq!(b.len(), 4);
        assert_eq!(b[0].0, 1.0);
        assert_eq!(b[2].0, 30.0);
        assert!(b[3].0.is_infinite());
        let e = make_buckets(&[]);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].1, "All files");
    }

    /// A scratch folder under the system temp dir, removed on drop.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new(name: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("omda-test-{}-{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn file(&self, rel: &str, bytes: usize) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut f = fs::File::create(&p).unwrap();
            f.write_all(&vec![b'x'; bytes]).unwrap();
            p
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn opts() -> ScanOptions {
        ScanOptions {
            follow_links: false,
            ext_stats: true,
            owner_stats: false,
            exclude_dirs: vec![],
            include_files: vec![],
            age_basis: AgeBasis::LastChange,
            age_days: vec![1, 7, 30, 182, 365, 730],
            priority: 0,
            threads: 1,
            hard_links: false,
        }
    }

    fn scan(root: &Path, o: &ScanOptions) -> ScanResult {
        run(root, 4096, o, &Progress::default())
    }

    fn find<'a>(r: &'a ScanResult, name: &str) -> &'a Node {
        r.nodes.iter().find(|n| n.name == name).unwrap_or_else(|| panic!("no node {name}"))
    }

    fn sample(name: &str) -> Tmp {
        let t = Tmp::new(name);
        t.file("a/f1.txt", 1000);
        t.file("a/b/f2.log", 5000);
        t.file("c/f3.txt", 10);
        t.file("r.txt", 200);
        t
    }

    #[test]
    fn totals_roll_up_to_parents() {
        let t = sample("totals");
        let r = scan(&t.0, &opts());
        let root = &r.nodes[0];
        assert_eq!(root.size, 6210);
        assert_eq!(root.files, 4);
        assert_eq!(root.dirs, 3);
        // 4096 (f1) + 8192 (f2) + 4096 (f3) + 4096 (r.txt)
        assert_eq!(root.alloc, 20480);
        let a = find(&r, "a");
        assert_eq!((a.size, a.files, a.dirs, a.alloc), (6000, 2, 1, 12288));
        let b = find(&r, "b");
        assert_eq!((b.size, b.own_size, b.files), (5000, 5000, 1));
        assert_eq!(root.own_size, 200);
    }

    #[test]
    fn children_come_after_parents() {
        let t = sample("order");
        let r = scan(&t.0, &opts());
        for (i, n) in r.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(p < i, "parent {p} must precede child {i}");
                assert!(r.nodes[p].children.contains(&i));
            }
            let kids: u64 = n.children.iter().map(|&c| r.nodes[c].size).sum();
            assert_eq!(n.size, n.own_size + kids);
        }
    }

    #[test]
    fn extension_stats_and_top_files() {
        let t = sample("ext");
        let r = scan(&t.0, &opts());
        assert_eq!(r.ext_stats["txt"], (1210, 3, 12288));
        assert_eq!(r.ext_stats["log"], (5000, 1, 8192));
        let sizes: Vec<u64> = r.top_files.iter().map(|f| f.size).collect();
        assert_eq!(sizes, [5000, 1000, 200, 10]);
        assert_eq!(r.top_files[0].alloc, 8192);
    }

    #[test]
    fn no_extension_and_dotfiles_group_as_empty() {
        let t = Tmp::new("noext");
        t.file("README", 5);
        t.file(".gitignore", 5);
        t.file("x.TXT", 5);
        let r = scan(&t.0, &opts());
        assert_eq!(r.ext_stats[""].1, 2);
        assert_eq!(r.ext_stats["txt"].1, 1);
    }

    #[test]
    fn top_files_keeps_only_the_largest() {
        let t = Tmp::new("topn");
        for i in 0..(TOP_N + 5) {
            t.file(&format!("f{i:03}.bin"), i + 1);
        }
        let r = scan(&t.0, &opts());
        assert_eq!(r.top_files.len(), TOP_N);
        assert_eq!(r.top_files[0].size, (TOP_N + 5) as u64);
        assert_eq!(r.top_files.last().unwrap().size, 6);
    }

    #[test]
    fn exclude_folders() {
        let t = sample("exclude");
        let mut o = opts();
        o.exclude_dirs = vec![" B ".into()];
        let r = scan(&t.0, &o);
        assert!(r.nodes.iter().all(|n| n.name != "b"));
        assert_eq!(r.nodes[0].size, 1210);
        o.exclude_dirs = vec!["*".into()];
        let r = scan(&t.0, &o);
        assert_eq!(r.nodes.len(), 1);
        assert_eq!(r.nodes[0].size, 200);
    }

    #[test]
    fn include_files_filter() {
        let t = sample("include");
        let mut o = opts();
        o.include_files = vec!["*.log".into()];
        let r = scan(&t.0, &o);
        assert_eq!(r.nodes[0].size, 5000);
        assert_eq!(r.nodes[0].files, 1);
        assert!(!r.ext_stats.contains_key("txt"));
    }

    #[test]
    fn stop_flag_cancels() {
        let t = sample("cancel");
        let p = Progress::default();
        p.stop.store(true, Ordering::SeqCst);
        let r = run(&t.0, 4096, &opts(), &p);
        assert!(r.cancelled);
        assert_eq!(r.nodes.len(), 1);
    }

    #[test]
    fn missing_folder_reports_error() {
        let t = Tmp::new("missing");
        let gone = t.0.join("nope");
        let r = scan(&gone, &opts());
        assert_eq!(r.error_count, 1);
        assert!(r.nodes[0].error.is_some());
        assert_eq!(r.nodes[0].size, 0);
    }

    #[test]
    fn age_buckets_use_modified_time() {
        let t = Tmp::new("age");
        let old = t.file("old.dat", 100);
        t.file("new.dat", 50);
        let f = fs::OpenOptions::new().write(true).open(&old).unwrap();
        f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86400)).unwrap();
        drop(f);
        let r = scan(&t.0, &opts());
        assert_eq!(r.age_buckets.len(), 7);
        assert_eq!(r.age_stats[0], (50, 1)); // less than 1 day
        assert_eq!(r.age_stats[3], (100, 1)); // 1 month - 6 months
        let total: u64 = r.age_stats.iter().map(|x| x.1).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn start_runs_in_background_and_finishes() {
        let t = sample("start");
        let prog = start(t.0.clone(), 4096, opts());
        let t0 = Instant::now();
        while !prog.done.load(Ordering::SeqCst) {
            assert!(t0.elapsed().as_secs() < 30, "scan did not finish");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let res = prog.result.lock().unwrap().take().unwrap();
        assert_eq!(res.nodes[0].size, 6210);
        assert_eq!(prog.files.load(Ordering::Relaxed), 4);
        assert_eq!(prog.dirs.load(Ordering::Relaxed), 3);
        assert_eq!(prog.bytes.load(Ordering::Relaxed), 6210);
    }

    #[test]
    fn files_in_lists_only_files() {
        let t = sample("filesin");
        let mut names: Vec<String> = files_in(&t.0).into_iter().map(|f| f.0).collect();
        names.sort();
        assert_eq!(names, ["r.txt"]);
    }

    #[test]
    fn search_matches_pattern_and_min_size() {
        let t = sample("search");
        let r = scan(&t.0, &opts());
        let stop = AtomicBool::new(false);
        let out = Mutex::new(Vec::new());
        search(&r.nodes, "f", 0, &stop, &out);
        assert_eq!(out.lock().unwrap().len(), 3); // f1.txt f2.log f3.txt
        let out = Mutex::new(Vec::new());
        search(&r.nodes, "*.txt", 500, &stop, &out);
        let v = out.lock().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "f1.txt");
    }

    /// Everything about a scan that must not depend on the number of threads.
    fn snapshot(r: &ScanResult) -> Vec<String> {
        let mut v: Vec<String> = r
            .nodes
            .iter()
            .map(|n| {
                let parent = n.parent.map(|p| r.nodes[p].path.to_string_lossy().into_owned()).unwrap_or_default();
                let kids: Vec<&str> = n.children.iter().map(|&c| r.nodes[c].name.as_str()).collect();
                format!(
                    "{} parent={parent} size={} alloc={} files={} dirs={} own={}/{}/{} depth={} kids={kids:?}",
                    n.path.display(), n.size, n.alloc, n.files, n.dirs, n.own_size, n.own_alloc, n.own_files, n.depth
                )
            })
            .collect();
        v.sort();
        let mut ext: Vec<_> = r.ext_stats.iter().collect();
        ext.sort();
        v.push(format!("ext={ext:?}"));
        v.push(format!("age={:?}", r.age_stats));
        let tops: Vec<_> = r.top_files.iter().map(|t| (t.size, t.alloc, t.path.clone())).collect();
        v.push(format!("top={tops:?}"));
        v.push(format!("errors={} {:?}", r.error_count, r.errors));
        v
    }

    /// A tree with many folders, so several threads really do share the work.
    fn big_tree(name: &str) -> Tmp {
        let t = Tmp::new(name);
        for d in 0..12 {
            for e in 0..6 {
                for f in 0..5 {
                    t.file(&format!("d{d:02}/e{e}/f{f}.{}", ["txt", "log", "bin"][f % 3]), 100 + d * 37 + e * 11 + f * 5);
                }
            }
            t.file(&format!("d{d:02}/top{d}.dat"), 5000 + d * 100);
        }
        fs::create_dir_all(t.0.join("empty/inner")).unwrap();
        t
    }

    #[test]
    fn any_number_of_threads_gives_the_same_scan() {
        let t = big_tree("threads");
        let one = snapshot(&scan(&t.0, &opts()));
        assert!(one.len() > 80);
        for n in [2, 4, 8, 0] {
            let mut o = opts();
            o.threads = n;
            for round in 0..3 {
                assert_eq!(snapshot(&scan(&t.0, &o)), one, "threads={n} round={round}");
            }
        }
    }

    #[test]
    fn children_are_sorted_by_name_and_follow_parents() {
        let t = big_tree("kids");
        let mut o = opts();
        o.threads = 4;
        let r = scan(&t.0, &o);
        for (i, n) in r.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(p < i);
            }
            let names: Vec<&String> = n.children.iter().map(|&c| &r.nodes[c].name).collect();
            let mut sorted = names.clone();
            sorted.sort();
            assert_eq!(names, sorted);
        }
        assert_eq!(r.nodes[0].dirs, 12 + 12 * 6 + 2);
    }

    #[test]
    fn exclude_and_include_work_with_threads() {
        let t = big_tree("filters");
        let mut o = opts();
        o.threads = 4;
        o.exclude_dirs = vec!["e1".into(), "e2".into()];
        o.include_files = vec!["*.txt".into()];
        let many = snapshot(&scan(&t.0, &o));
        o.threads = 1;
        assert_eq!(snapshot(&scan(&t.0, &o)), many);
        assert!(many.iter().all(|l| !l.contains("/e1") && !l.contains("\\e1")));
    }

    #[test]
    fn stop_flag_cancels_with_threads() {
        let t = big_tree("cancel4");
        let p = Progress::default();
        p.stop.store(true, Ordering::SeqCst);
        let mut o = opts();
        o.threads = 4;
        let r = run(&t.0, 4096, &o, &p);
        assert!(r.cancelled);
        assert_eq!(r.nodes.len(), 1);
    }

    #[test]
    fn thread_count_rules() {
        let mut o = opts();
        o.threads = 0;
        assert!((1..=MAX_THREADS).contains(&thread_count(&o)));
        o.threads = 99;
        assert_eq!(thread_count(&o), MAX_THREADS);
        o.threads = 3;
        assert_eq!(thread_count(&o), 3);
        o.follow_links = true;
        assert_eq!(thread_count(&o), 1);
    }

    #[test]
    fn hard_links_count_once_under_the_first_name() {
        let t = Tmp::new("hard");
        let first = t.file("a/data.bin", 10_000);
        t.file("b/other.txt", 300);
        fs::create_dir_all(t.0.join("c")).unwrap();
        fs::hard_link(&first, t.0.join("c/copy.bin")).unwrap();
        fs::hard_link(&first, t.0.join("b/alias.bin")).unwrap();
        // off: every name counts
        let r = scan(&t.0, &opts());
        assert_eq!(r.nodes[0].size, 3 * 10_000 + 300);
        assert_eq!(r.nodes[0].files, 4);
        for n in [1, 2, 4, 8] {
            let mut o = opts();
            o.hard_links = true;
            o.threads = n;
            let r = scan(&t.0, &o);
            assert_eq!(r.nodes[0].size, 10_300, "threads={n}");
            assert_eq!(r.nodes[0].alloc, 12288 + 4096);
            assert_eq!(r.nodes[0].files, 2);
            // the first name in path order keeps it: a/data.bin
            assert_eq!(find(&r, "a").size, 10_000);
            assert_eq!(find(&r, "b").size, 300);
            assert_eq!(find(&r, "c").size, 0);
            assert_eq!(r.ext_stats["bin"], (10_000, 1, 12288));
            assert_eq!(r.top_files.iter().filter(|f| f.size == 10_000).count(), 1);
            assert_eq!(r.age_stats.iter().map(|x| x.1).sum::<u64>(), 2);
        }
    }

    #[test]
    fn hard_link_counting_leaves_ordinary_files_alone() {
        let t = big_tree("hard-plain");
        let off = snapshot(&scan(&t.0, &opts()));
        let mut o = opts();
        o.hard_links = true;
        o.threads = 4;
        assert_eq!(snapshot(&scan(&t.0, &o)), off);
    }

    #[test]
    fn progress_counts_each_hard_linked_file_once() {
        let t = Tmp::new("hard-prog");
        let f = t.file("x/one.bin", 700);
        fs::hard_link(&f, t.0.join("two.bin")).unwrap();
        let mut o = opts();
        o.hard_links = true;
        let prog = Progress::default();
        let r = run(&t.0, 4096, &o, &prog);
        assert_eq!(r.nodes[0].files, 1);
        assert_eq!(prog.files.load(Ordering::Relaxed), 1);
        assert_eq!(prog.bytes.load(Ordering::Relaxed), 700);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_skipped_unless_following() {
        let t = sample("links");
        std::os::unix::fs::symlink(t.0.join("a"), t.0.join("link")).unwrap();
        let r = scan(&t.0, &opts());
        assert_eq!(r.nodes[0].size, 6210);
        let mut o = opts();
        o.follow_links = true;
        let r = scan(&t.0, &o);
        // a is reached first or via the link, never both
        assert_eq!(r.nodes[0].size, 6210);
    }
}
