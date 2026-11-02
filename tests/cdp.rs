use std::io;
use std::time::Duration;

use doteml::cdp::wait_for_port;

#[test]
fn port_file_retries_while_locked() {
    let mut attempts = 0;
    let got = wait_for_port(
        || {
            attempts += 1;
            if attempts <= 3 {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
            } else {
                Ok("9222\n/devtools/browser/abc\n".into())
            }
        },
        || true,
        String::new,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(got, (9222, "/devtools/browser/abc".to_string()));
    assert_eq!(attempts, 4);
}

#[test]
fn exited_browser_reports_log() {
    let err = wait_for_port(
        || Err(io::ErrorKind::NotFound.into()),
        || false,
        || "boom".into(),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Browser exited on startup:\nboom"));
}
