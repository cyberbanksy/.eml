//! Find a Chromium-based browser (Chrome, Edge, Chromium, Brave) already installed.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const MIN_VERSION: u32 = 110;
pub const DOWNLOAD_URL: &str = "https://www.google.com/chrome/";

const LINUX_NAMES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
    "brave-browser",
];

const MAC_APPS: &[&str] = &[
    "Google Chrome.app/Contents/MacOS/Google Chrome",
    "Chromium.app/Contents/MacOS/Chromium",
    "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "Brave Browser.app/Contents/MacOS/Brave Browser",
];

const WIN_EXES: &[&str] = &[
    r"Google\Chrome\Application\chrome.exe",
    r"Microsoft\Edge\Application\msedge.exe",
    r"Chromium\Application\chrome.exe",
    r"BraveSoftware\Brave-Browser\Application\brave.exe",
];

/// Chromium that `playwright install chromium` may have downloaded.
fn playwright_cache() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = std::env::var_os("PLAYWRIGHT_BROWSERS_PATH")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if cfg!(target_os = "macos") {
        roots.extend(dirs::home_dir().map(|h| h.join("Library/Caches/ms-playwright")));
    } else if cfg!(windows) {
        roots.extend(dirs::data_local_dir().map(|d| d.join("ms-playwright")));
    } else {
        roots.extend(dirs::home_dir().map(|h| h.join(".cache/ms-playwright")));
    }
    let inner: &[&str] = &[
        "chrome-linux64/chrome",
        "chrome-linux/chrome",
        "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
        "chrome-mac-arm64/Chromium.app/Contents/MacOS/Chromium",
        "chrome-win64/chrome.exe",
        "chrome-win/chrome.exe",
    ];
    let mut found = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|n| n.starts_with("chromium-"))
            })
            .collect();
        dirs.sort_by(|a, b| b.cmp(a));
        for dir in dirs {
            found.extend(inner.iter().map(|i| dir.join(i)));
        }
    }
    found
}

pub fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(target_os = "macos") {
        let mut bases = vec![PathBuf::from("/Applications")];
        bases.extend(dirs::home_dir().map(|h| h.join("Applications")));
        for base in bases {
            paths.extend(MAC_APPS.iter().map(|app| base.join(app)));
        }
    } else if cfg!(windows) {
        for var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                paths.extend(WIN_EXES.iter().map(|exe| Path::new(&base).join(exe)));
            }
        }
    } else {
        paths.extend(LINUX_NAMES.iter().filter_map(|n| which::which(n).ok()));
    }
    paths.extend(playwright_cache());
    paths
}

pub fn find_browser() -> Result<PathBuf> {
    find_browser_in(std::env::var_os("DOTEML_BROWSER").as_deref(), &candidates())
}

pub fn find_browser_in(override_: Option<&OsStr>, candidates: &[PathBuf]) -> Result<PathBuf> {
    if let Some(path) = override_.filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(Error::BrowserNotFound(format!(
                "DOTEML_BROWSER points to a missing file: {}",
                path.display()
            )));
        }
        return Ok(path);
    }
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .ok_or_else(|| {
            Error::BrowserNotFound(format!(
                "No Chrome, Edge, or Chromium found.\n  Install Google Chrome: {DOWNLOAD_URL}\n  \
             or point doteml at a browser: DOTEML_BROWSER=/path/to/chrome"
            ))
        })
}

/// `"HeadlessChrome/131.0.6778.85"` -> `131`.
pub fn parse_major(product: &str) -> Option<u32> {
    product.split_once('/')?.1.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_major_cases() {
        assert_eq!(parse_major("HeadlessChrome/131.0.6778.85"), Some(131));
        assert_eq!(parse_major("Edg/120.0.1"), Some(120));
        assert_eq!(parse_major("garbage"), None);
    }
}
