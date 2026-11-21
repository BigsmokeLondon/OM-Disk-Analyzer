//! Saved settings and scan history.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::fmt::Unit;
use crate::scan::{AgeBasis, ScanOptions};
use crate::sys;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Mode {
    Size,
    Alloc,
    Files,
    Percent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SortBy {
    Size,
    Name,
    Files,
    Changed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BoldMode {
    PercentOfScan,
    PercentOfParent,
    Megabytes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub sort: SortBy,
    pub unit: Unit,
    pub gradient: bool,
    pub grad_start: [u8; 3],
    pub grad_end: [u8; 3],
    pub bold_mode: BoldMode,
    pub bold_value: f64,
    pub compressed_color: bool,
    pub follow_links: bool,
    pub ext_stats: bool,
    pub owner_stats: bool,
    pub exclude_dirs: String,
    pub include_files: String,
    pub age_basis: AgeBasis,
    pub age_days: String,
    pub priority: i32,
    pub recent: Vec<String>,
    pub show_drives: bool,
    pub tooltips: bool,
    pub show_files: bool,
    /// Start with administrator rights when the account is an administrator.
    pub start_as_admin: bool,
    /// Ask GitHub for the newest version number when the program starts.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Alloc, // 0x810220: ViewType = 3 (vt_alloc)
            sort: SortBy::Size,
            unit: Unit::Mb,
            gradient: true,
            grad_start: [0xFF, 0xFF, 0x64], // 0x810220: IndicatorBarColor1 = $0064FFFF
            grad_end: [0xFF, 0x64, 0x00],   // 0x810220: IndicatorBarColor2 = $000064FF
            bold_mode: BoldMode::PercentOfScan,
            bold_value: 15.0,
            compressed_color: true,
            follow_links: false,
            ext_stats: true,
            owner_stats: false,
            exclude_dirs: String::new(),
            include_files: String::new(),
            age_basis: AgeBasis::LastChange,
            age_days: "1, 7, 30, 182, 365, 730".into(), // 0x816710
            priority: -1,
            recent: Vec::new(),
            show_drives: true,
            tooltips: true,
            show_files: false, // DefaultShowFiles
            start_as_admin: true,
            check_updates: true,
        }
    }
}

fn settings_file() -> PathBuf {
    sys::data_dir().join("settings-v3.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(settings_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if read_only() {
            return;
        }
        let f = settings_file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(f, s);
        }
    }

    pub fn age_days_list(&self) -> Vec<u32> {
        let v: Vec<u32> = self.age_days.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|s| s.trim().parse().ok()).collect();
        if v.is_empty() {
            vec![1, 7, 30, 182, 365, 730]
        } else {
            v
        }
    }

    pub fn scan_options(&self) -> ScanOptions {
        let split = |s: &str| s.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        ScanOptions {
            follow_links: self.follow_links,
            ext_stats: self.ext_stats,
            owner_stats: self.owner_stats,
            exclude_dirs: split(&self.exclude_dirs),
            include_files: split(&self.include_files),
            age_basis: self.age_basis,
            age_days: self.age_days_list(),
            priority: self.priority,
        }
    }

    pub fn remember(&mut self, path: &str) {
        self.recent.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(15);
    }
}

/// One finished scan, in the same format the earlier version saved.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub date: f64,
    pub path: String,
    pub size: u64,
    pub alloc: u64,
    pub files: u64,
    pub dirs: u64,
}

fn history_file() -> PathBuf {
    sys::data_dir().join("history.json")
}

pub fn load_history() -> Vec<HistoryEntry> {
    std::fs::read_to_string(history_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn add_history(e: HistoryEntry) {
    if read_only() {
        return;
    }
    let mut h = load_history();
    h.push(e);
    let skip = h.len().saturating_sub(500);
    let h: Vec<_> = h.into_iter().skip(skip).collect();
    let f = history_file();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(s) = serde_json::to_string(&h) {
        let _ = std::fs::write(f, s);
    }
}

static READ_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Test pictures (--screenshot) read the saved settings and history but never change them.
pub fn set_read_only() {
    READ_ONLY.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn read_only() -> bool {
    READ_ONLY.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn clear_history() {
    if read_only() {
        return;
    }
    let _ = std::fs::remove_file(history_file());
}
