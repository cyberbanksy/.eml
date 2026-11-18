use std::io;
use std::time::{Duration, Instant};

use doteml::cdp::{HANDOFF_GRACE, still_starting, wait_for_port};

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

#[test]
fn running_launcher_is_starting() {
    let mut handoff = None;
    assert!(still_starting(None, &mut handoff));
    assert!(handoff.is_none());
}

#[test]
fn failed_launcher_is_not_starting() {
    assert!(!still_starting(Some(false), &mut None));
}

#[test]
fn clean_exit_waits_for_relaunched_browser() {
    // Elevated Chrome relaunches itself and the launcher exits 0 before the port appears.
    let mut handoff = None;
    assert!(still_starting(Some(true), &mut handoff));
    assert!(handoff.is_some());
    let mut expired = Some(Instant::now() - HANDOFF_GRACE);
    assert!(!still_starting(Some(true), &mut expired));
}

#[test]
fn handoff_then_port_connects() {
    let mut reads = 0;
    let mut handoff = None;
    let got = wait_for_port(
        || {
            reads += 1;
            if reads < 5 {
                Err(io::ErrorKind::NotFound.into())
            } else {
                Ok("9333\n/devtools/browser/x\n".into())
            }
        },
        || still_starting(Some(true), &mut handoff),
        String::new,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(got.0, 9333);
}
