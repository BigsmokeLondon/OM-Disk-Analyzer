//! "Is there a newer version?" - one small request to GitHub's public release list.
//!
//! It runs on a background thread through the `curl` program that ships with Windows 10/11 and
//! macOS, so the program needs no networking library. Only the version number is read; nothing
//! about the computer or the person is sent. It can be turned off in Options.

use std::sync::mpsc::{channel, Receiver};

pub const OWNER: &str = "BigsmokeLondon";
pub const REPO: &str = "OM-Disk-Analyzer";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Newer(Release),
    Current,
    Failed(String),
}

/// "v2.0.1" -> [2, 0, 1]; anything that isn't numbers separated by dots gives None.
fn numbers(v: &str) -> Option<Vec<u64>> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    if v.is_empty() {
        return None;
    }
    v.split('.').map(|p| p.parse::<u64>().ok()).collect()
}

/// Whether `tag` names a later version than `current` ("2.0.0" equals "v2.0").
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (numbers(tag), numbers(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

/// The text of the first `"key": "value"` pair in a piece of JSON (no escapes expected in tags).
fn first_string(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = json.find(&pat)? + pat.len();
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Turn GitHub's answer into an outcome. `status` is the HTTP status code.
pub fn interpret(status: u16, body: &str, current: &str) -> Outcome {
    match status {
        200 => {}
        404 => {
            return Outcome::Failed(
                "No published release was found. (If the repository is private, the public release list can't be read.)".into(),
            )
        }
        403 | 429 => return Outcome::Failed("GitHub is limiting requests right now. Try again later.".into()),
        s => return Outcome::Failed(format!("GitHub answered with code {s}.")),
    }
    let Some(tag) = first_string(body, "tag_name") else {
        return Outcome::Failed("The answer from GitHub had no version number.".into());
    };
    // the tag goes into a web address, so only plain characters are accepted
    if tag.is_empty() || tag.len() > 40 || !tag.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        return Outcome::Failed("The version name from GitHub looked wrong.".into());
    }
    if is_newer(&tag, current) {
        let url = format!("https://github.com/{OWNER}/{REPO}/releases/tag/{tag}");
        Outcome::Newer(Release { tag, url })
    } else {
        Outcome::Current
    }
}

/// Split curl's output ("<body>\n<status code>") into the code and the body.
fn split_status(out: &str) -> Option<(u16, &str)> {
    let (body, code) = out.trim_end().rsplit_once('\n')?;
    Some((code.trim().parse().ok()?, body))
}

fn fetch(current: &str) -> Outcome {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-s", "-L", "--max-time", "10", "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: OM-Disk-Analyzer"])
        .args(["-w", "\n%{http_code}"])
        .arg(format!("https://api.github.com/repos/{OWNER}/{REPO}/releases/latest"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    match cmd.output() {
        Ok(o) => match split_status(&String::from_utf8_lossy(&o.stdout)) {
            Some((code, body)) => interpret(code, body, current),
            None => Outcome::Failed("Could not reach GitHub. Check the internet connection.".into()),
        },
        Err(_) => Outcome::Failed("The curl program is needed to check for updates and was not found.".into()),
    }
}

/// State the window keeps between frames.
pub struct Checker {
    rx: Option<Receiver<Outcome>>,
    manual: bool,
    pub checking: bool,
    pub outcome: Option<Outcome>,
    /// Whether the result window is open.
    pub show: bool,
}

impl Checker {
    pub fn new() -> Self {
        Checker { rx: None, manual: false, checking: false, outcome: None, show: false }
    }

    /// Start a check on a background thread. A start-up check (`manual` false) only opens the
    /// window when a newer version exists; a check the person asked for always shows its result.
    pub fn start(&mut self, manual: bool) {
        if self.checking {
            self.show |= manual;
            self.manual |= manual;
            return;
        }
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.manual = manual;
        self.checking = true;
        self.show = manual;
        let current = env!("CARGO_PKG_VERSION").to_string();
        std::thread::spawn(move || {
            let _ = tx.send(fetch(&current));
        });
    }

    /// Call every frame; picks up the answer when it arrives.
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        if let Ok(o) = rx.try_recv() {
            self.show = self.manual || matches!(o, Outcome::Newer(_));
            self.outcome = Some(o);
            self.checking = false;
            self.rx = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert!(is_newer("v2.1.0", "2.0.0"));
        assert!(is_newer("2.0.1", "2.0.0"));
        assert!(is_newer("v10.0", "9.9.9"));
        assert!(!is_newer("v2.0", "2.0.0"));
        assert!(!is_newer("v1.9.9", "2.0.0"));
        assert!(!is_newer("nightly", "2.0.0"));
        assert!(!is_newer("v2.0.0-beta", "2.0.0"));
    }

    #[test]
    fn reads_the_tag_from_github_json() {
        let body = r#"{"url":"https://api.github.com/x","tag_name": "v2.1.0","name":"OM Disk Analyzer 2.1","author":{"login":"x"}}"#;
        assert_eq!(first_string(body, "tag_name").as_deref(), Some("v2.1.0"));
        assert_eq!(first_string(body, "missing"), None);
    }

    #[test]
    fn outcomes() {
        let body = r#"{"tag_name":"v2.1.0"}"#;
        assert_eq!(
            interpret(200, body, "2.0.0"),
            Outcome::Newer(Release { tag: "v2.1.0".into(), url: "https://github.com/BigsmokeLondon/OM-Disk-Analyzer/releases/tag/v2.1.0".into() })
        );
        assert_eq!(interpret(200, body, "2.1.0"), Outcome::Current);
        assert!(matches!(interpret(404, "", "2.0.0"), Outcome::Failed(m) if m.contains("private")));
        assert!(matches!(interpret(403, "", "2.0.0"), Outcome::Failed(_)));
        assert!(matches!(interpret(500, "", "2.0.0"), Outcome::Failed(m) if m.contains("500")));
        assert!(matches!(interpret(200, "{}", "2.0.0"), Outcome::Failed(_)));
    }

    #[test]
    fn hostile_tags_are_refused() {
        for bad in ["v1 2", "../x", "v1;rm", "v1&calc", "v1%20x", "v1?a=b"] {
            let body = format!("{{\"tag_name\":\"{bad}\"}}");
            assert!(matches!(interpret(200, &body, "0.1"), Outcome::Failed(_)), "{bad}");
        }
        // a quote ends the JSON string, so only the harmless part before it is ever used
        match interpret(200, r#"{"tag_name":"v1"&calc"}"#, "0.1") {
            Outcome::Newer(r) => assert!(r.url.ends_with("/releases/tag/v1")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn curl_output_is_split() {
        assert_eq!(split_status("{\"a\":1}\n200"), Some((200, "{\"a\":1}")));
        assert_eq!(split_status("\n404"), Some((404, "")));
        assert_eq!(split_status("no code here"), None);
        assert_eq!(split_status("body\nabc"), None);
    }
}
