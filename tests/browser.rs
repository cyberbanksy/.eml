use std::ffi::OsStr;
use std::path::Path;

use doteml::browser::find_browser_in;
use doteml::render::check_version;

#[test]
fn env_override_used() {
    let d = tempfile::tempdir().unwrap();
    let exe = d.path().join("chrome");
    std::fs::write(&exe, "").unwrap();
    assert_eq!(find_browser_in(Some(exe.as_os_str()), &[]).unwrap(), exe);
}

#[test]
fn env_override_missing_errors() {
    let err = find_browser_in(Some(OsStr::new("nope/chrome")), &[]).unwrap_err();
    assert!(err.to_string().contains("missing file"));
}

#[test]
fn not_found_recommends_download() {
    let err = find_browser_in(None, &[]).unwrap_err().to_string();
    assert!(err.contains("No Chrome, Edge, or Chromium found"));
    assert!(err.contains("google.com/chrome"));
}

#[test]
fn first_existing_candidate_wins() {
    let d = tempfile::tempdir().unwrap();
    let real = d.path().join("edge");
    std::fs::write(&real, "").unwrap();
    let found = find_browser_in(None, &[d.path().join("missing"), real.clone()]).unwrap();
    assert_eq!(found, real);
}

#[test]
fn too_old_browser_rejected() {
    let err = check_version("HeadlessChrome/99.0.0.0", Path::new("/fake/chrome")).unwrap_err();
    assert!(err.to_string().contains("too old"));
    assert!(check_version("Chrome/153.0.1.2", Path::new("/x")).is_ok());
}
