//! The folder scanner: walks a folder on a background thread and builds a size tree.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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

fn run(root_path: &Path, cluster: u64, o: &ScanOptions, prog: &Progress) -> ScanResult {
    let t0 = Instant::now();
    let now = sys::now_unix();
    let buckets = make_buckets(&o.age_days);
    let mut res = ScanResult {
        age_stats: vec![(0, 0); buckets.len()],
        age_buckets: buckets.clone(),
        age_basis: Some(o.age_basis),
        ext_enabled: o.ext_stats,
        owner_enabled: o.owner_stats,
        cluster,
        ..Default::default()
    };
    let exclude: Vec<String> = o.exclude_dirs.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    let include: Vec<String> = o.include_files.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();

    let mut root = Node { name: root_path.to_string_lossy().into_owned(), path: root_path.to_path_buf(), ..Default::default() };
    if let Ok(md) = std::fs::metadata(root_path) {
        let f = facts(&md);
        root.mtime = f.mtime;
        root.atime = f.atime;
    }
    let mut nodes = vec![root];
    let mut stack: Vec<NodeId> = vec![0];
    let mut visited: HashSet<PathBuf> = HashSet::new();
    if let Ok(c) = std::fs::canonicalize(root_path) {
        visited.insert(c);
    }
    let mut top: BinaryHeap<Reverse<(u64, PathBuf, i64, i64)>> = BinaryHeap::new();
    let err = |res: &mut ScanResult, path: &Path, msg: String| {
        res.error_count += 1;
        if res.errors.len() < 1000 {
            res.errors.push((path.to_string_lossy().into_owned(), msg));
        }
    };

    while let Some(id) = stack.pop() {
        if prog.stop.load(Ordering::Relaxed) {
            res.cancelled = true;
            break;
        }
        let dir = nodes[id].path.clone();
        if let Ok(mut c) = prog.current.try_lock() {
            *c = dir.to_string_lossy().into_owned();
        }
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                nodes[id].error = Some(msg.clone());
                err(&mut res, &dir, msg);
                continue;
            }
        };
        let depth = nodes[id].depth + 1;
        for entry in rd {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    err(&mut res, &dir, e.to_string());
                    continue;
                }
            };
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    err(&mut res, &entry.path(), e.to_string());
                    continue;
                }
            };
            let md = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    err(&mut res, &entry.path(), e.to_string());
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
                        Ok(real) if visited.insert(real.clone()) => {}
                        _ => continue,
                    }
                }
                if !exclude.is_empty() {
                    let (n, p) = (name.to_lowercase(), path.to_string_lossy().to_lowercase());
                    if exclude.iter().any(|x| wildcard(x, &n) || wildcard(x, &p)) {
                        continue;
                    }
                }
                let child = Node {
                    name,
                    path,
                    parent: Some(id),
                    mtime: f.mtime,
                    atime: f.atime,
                    compressed: f.attrs & ATTR_COMPRESSED != 0,
                    depth,
                    ..Default::default()
                };
                let cid = nodes.len();
                nodes.push(child);
                nodes[id].children.push(cid);
                stack.push(cid);
                prog.dirs.fetch_add(1, Ordering::Relaxed);
            } else {
                if !include.is_empty() && !include.iter().any(|x| wildcard(x, &name)) {
                    continue;
                }
                let f = if is_link { std::fs::metadata(&path).map(|m| facts(&m)).unwrap_or(f) } else { f };
                let (sz, mt) = (f.size, f.mtime);
                // 0x5eacc4: compressed or sparse files take what Windows reports as their
                // compressed size, rounded up to whole clusters; others take whole clusters.
                let al = if f.attrs & (ATTR_COMPRESSED | ATTR_SPARSE) != 0 {
                    allocated(sys::compressed_size(&path).unwrap_or(sz), cluster)
                } else {
                    allocated(sz, cluster)
                };
                let n = &mut nodes[id];
                n.own_size += sz;
                n.own_alloc += al;
                n.own_files += 1;
                n.mtime = n.mtime.max(mt);
                n.atime = n.atime.max(f.atime);
                if o.ext_stats {
                    let ext = match name.rfind('.') {
                        Some(d) if d > 0 => name[d + 1..].to_lowercase(),
                        _ => String::new(),
                    };
                    let r = res.ext_stats.entry(ext).or_insert((0, 0, 0));
                    r.0 += sz;
                    r.1 += 1;
                    r.2 += al;
                }
                if o.owner_stats {
                    let mut owner = sys::owner_of(&path);
                    if owner.is_empty() {
                        owner = "(unknown)".into();
                    }
                    let r = res.owner_stats.entry(owner).or_insert((0, 0));
                    r.0 += sz;
                    r.1 += 1;
                }
                let t = match o.age_basis {
                    AgeBasis::LastChange => mt,
                    AgeBasis::LastAccess => f.atime,
                    AgeBasis::Created => f.ctime,
                };
                let days = (now - t) as f64 / 86400.0;
                if let Some(i) = buckets.iter().position(|(limit, _)| days < *limit) {
                    res.age_stats[i].0 += sz;
                    res.age_stats[i].1 += 1;
                }
                if top.len() < TOP_N {
                    top.push(Reverse((sz, path, mt, f.atime)));
                } else if top.peek().map(|Reverse(t)| sz > t.0).unwrap_or(false) {
                    top.pop();
                    top.push(Reverse((sz, path, mt, f.atime)));
                }
                prog.files.fetch_add(1, Ordering::Relaxed);
                prog.bytes.fetch_add(sz, Ordering::Relaxed);
            }
        }
    }

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
    let mut tops: Vec<TopFile> = top
        .into_iter()
        .map(|Reverse((size, path, mtime, atime))| TopFile { size, alloc: allocated(size, cluster), path, mtime, atime })
        .collect();
    tops.sort_by(|a, b| b.size.cmp(&a.size));
    res.top_files = tops;
    res.nodes = nodes;
    res.elapsed = t0.elapsed().as_secs_f64();
    res.finished_at = sys::now_unix();
    res
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
