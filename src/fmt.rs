//! Number, size and date formatting.

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Unit {
    Kb,
    Mb,
    Gb,
    Auto,
}

impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Unit::Kb => "KB",
            Unit::Mb => "MB",
            Unit::Gb => "GB",
            Unit::Auto => "Auto",
        }
    }
}

/// 1234567 -> "1,234,567"
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// One decimal place with thousands separators: 79621.06 -> "79,621.1"
pub fn dec1(v: f64) -> String {
    let r = (v * 10.0).round() as u64;
    format!("{}.{}", thousands(r / 10), r % 10)
}

pub fn size(n: u64, unit: Unit) -> String {
    let f = n as f64;
    match unit {
        Unit::Kb => format!("{} KB", dec1(f / 1024.0)),
        Unit::Mb => format!("{} MB", dec1(f / 1048576.0)),
        Unit::Gb => format!("{} GB", dec1(f / 1073741824.0)),
        Unit::Auto => {
            let mut v = f;
            for u in ["Bytes", "KB", "MB", "GB", "TB"] {
                if v < 1024.0 || u == "TB" {
                    return if u == "Bytes" { format!("{} Bytes", thousands(n)) } else { format!("{} {}", dec1(v), u) };
                }
                v /= 1024.0;
            }
            unreachable!()
        }
    }
}

/// Three significant digits: 126 GB, 47.1 GB, 1.72 TB (drive list, status bar).
pub fn short(n: u64) -> String {
    let mut v = n as f64;
    for u in ["Bytes", "KB", "MB", "GB", "TB", "PB"] {
        if v < 1000.0 || u == "PB" {
            if u == "Bytes" {
                return format!("{} {}", n, u);
            }
            return if v >= 100.0 {
                format!("{:.0} {}", v, u)
            } else if v >= 10.0 {
                format!("{:.1} {}", v, u)
            } else {
                format!("{:.2} {}", v, u)
            };
        }
        v /= 1024.0;
    }
    unreachable!()
}

pub fn pct(part: u64, whole: u64) -> String {
    if whole == 0 {
        "0.0 %".into()
    } else {
        format!("{:.1} %", 100.0 * part as f64 / whole as f64)
    }
}

pub fn date(t: i64) -> String {
    use chrono::TimeZone;
    if t <= 0 {
        return String::new();
    }
    match chrono::Local.timestamp_opt(t, 0) {
        chrono::LocalResult::Single(d) | chrono::LocalResult::Ambiguous(d, _) => d.format("%d/%m/%Y").to_string(),
        _ => String::new(),
    }
}

pub fn date_time(t: i64) -> String {
    use chrono::TimeZone;
    if t <= 0 {
        return String::new();
    }
    match chrono::Local.timestamp_opt(t, 0) {
        chrono::LocalResult::Single(d) | chrono::LocalResult::Ambiguous(d, _) => {
            d.format("%d/%m/%Y %H:%M").to_string()
        }
        _ => String::new(),
    }
}

/// Simple wildcard match (`*` and `?`), case-insensitive.
pub fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

pub const EXT_KINDS: [(&str, &str); 8] = [
    ("Image", "jpg jpeg png gif bmp tif tiff webp heic raw svg ico psd"),
    ("Video", "mp4 mkv avi mov wmv flv webm m4v mpg mpeg"),
    ("Audio", "mp3 wav flac aac ogg m4a wma"),
    ("Document", "pdf doc docx xls xlsx ppt pptx odt ods txt rtf md csv"),
    ("Archive", "zip rar 7z gz tar bz2 xz cab iso"),
    ("Program", "exe dll msi sys bat cmd ps1 com pyd"),
    ("Source code", "py pyc js ts html css json xml c h cpp hpp rs go java cs"),
    ("Database", "db sqlite mdb accdb ldf mdf"),
];

pub fn ext_kind(ext: &str) -> String {
    if ext.is_empty() {
        return "No extension".into();
    }
    for (kind, list) in EXT_KINDS {
        if list.split(' ').any(|e| e == ext) {
            return kind.into();
        }
    }
    format!("{} file", ext.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats() {
        assert_eq!(thousands(515452), "515,452");
        assert_eq!(size(83_488_774_554, Unit::Mb), "79,621.1 MB");
        assert_eq!(short(126 * 1073741824), "126 GB");
        assert!(wildcard("*.log", "Setup.LOG"));
        assert!(!wildcard("a?c", "abdc"));
    }

    #[test]
    fn thousands_edges() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn dec1_rounds() {
        assert_eq!(dec1(0.0), "0.0");
        assert_eq!(dec1(0.96), "1.0");
        assert_eq!(dec1(1234.56), "1,234.6");
    }

    #[test]
    fn size_units() {
        assert_eq!(size(1024, Unit::Kb), "1.0 KB");
        assert_eq!(size(1073741824, Unit::Gb), "1.0 GB");
        assert_eq!(size(512, Unit::Auto), "512 Bytes");
        assert_eq!(size(1536, Unit::Auto), "1.5 KB");
        assert_eq!(size(5 * 1048576, Unit::Auto), "5.0 MB");
        assert!(size(u64::MAX, Unit::Auto).ends_with("TB"));
    }

    #[test]
    fn short_three_digits() {
        assert_eq!(short(0), "0 Bytes");
        assert_eq!(short(999), "999 Bytes");
        assert_eq!(short(47 * 1024 * 1024 * 1024 + 107374182), "47.1 GB");
        assert_eq!(short(1024 * 1024 * 1024 * 1024 * 3 / 2), "1.50 TB");
    }

    #[test]
    fn percent() {
        assert_eq!(pct(1, 0), "0.0 %");
        assert_eq!(pct(1, 4), "25.0 %");
        assert_eq!(pct(5, 5), "100.0 %");
    }

    #[test]
    fn wildcard_edges() {
        assert!(wildcard("*", ""));
        assert!(wildcard("*", "anything"));
        assert!(!wildcard("?", ""));
        assert!(wildcard("a*b*c", "aXXbYYc"));
        assert!(!wildcard("a*b*c", "aXXbYY"));
        assert!(wildcard("*.TXT", "notes.txt"));
        assert!(wildcard("", ""));
        assert!(!wildcard("", "x"));
    }

    #[test]
    fn extension_kinds() {
        assert_eq!(ext_kind(""), "No extension");
        assert_eq!(ext_kind("jpg"), "Image");
        assert_eq!(ext_kind("rs"), "Source code");
        assert_eq!(ext_kind("xyz"), "XYZ file");
    }

    #[test]
    fn dates_blank_for_unset() {
        assert_eq!(date(0), "");
        assert_eq!(date_time(-5), "");
        assert_eq!(date(1_700_000_000).len(), 10);
    }
}
