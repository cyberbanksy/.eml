//! WebKit backend end-to-end tests. WebKit must run on the process main thread, which
//! `cargo test` worker threads are not, so every test drives the built binary.
#![cfg(target_os = "macos")]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assert_cmd::Command;
use common::*;
use predicates::str::contains;

fn webkit() -> Command {
    let mut cmd = Command::cargo_bin("doteml").unwrap();
    cmd.args(["--backend", "webkit"])
        .env("DOTEML_DEBUG", "1")
        .timeout(Duration::from_secs(120));
    cmd
}

/// Like `assert()`, but echo the binary's stderr so WebKit stage traces show in CI logs.
trait AssertLogged {
    fn assert_logged(self) -> assert_cmd::assert::Assert;
}

impl AssertLogged for &mut Command {
    fn assert_logged(self) -> assert_cmd::assert::Assert {
        let a = self.assert();
        eprintln!(
            "--- stderr ---\n{}",
            String::from_utf8_lossy(&a.get_output().stderr)
        );
        a
    }
}

struct Tracker {
    base: String,
    hits: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl Tracker {
    fn start() -> Tracker {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (h, s) = (hits.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                if let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(100)) {
                    h.lock().unwrap().push(req.url().to_string());
                    let _ = req.respond(tiny_http::Response::empty(404));
                }
            }
        });
        Tracker { base, hits, stop }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[test]
fn webkit_legoland_pdf() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.pdf");
    webkit()
        .arg(legoland())
        .arg("-o")
        .arg(&out)
        .assert_logged()
        .success();
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1000);
    let doc = lopdf::Document::load(&out).unwrap();
    assert!(!doc.get_pages().is_empty());
    assert!(pdf_info(&out).contains_key("EmlSHA256"));
}

#[test]
fn webkit_legoland_png() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.png");
    webkit()
        .arg(legoland())
        .args(["-f", "png", "-o"])
        .arg(&out)
        .assert_logged()
        .success();
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"\x89PNG"));
    assert!(bytes.len() > 1000, "{} bytes", bytes.len());
    assert!(png_height(&out) > 1000, "png height {}", png_height(&out));
}

#[test]
fn webkit_simple_jpg() {
    let (d, p) = simple_eml();
    let out = d.path().join("simple.jpg");
    webkit()
        .arg(&p)
        .args(["-f", "jpg", "-o"])
        .arg(&out)
        .assert_logged()
        .success();
    assert!(std::fs::read(&out).unwrap().starts_with(&[0xFF, 0xD8]));
}

#[test]
fn webkit_js_does_not_run() {
    let (d, p) = security_eml(
        r#"<p id="x">static text</p><script>document.getElementById('x').textContent='JSRAN'</script>"#,
    );
    let out = d.path().join("o.pdf");
    webkit().arg(&p).arg("-o").arg(&out).assert().success();
    let text = pdf_text(&out);
    assert!(text.contains("static text"), "{text}");
    assert!(!text.contains("JSRAN"));
}

#[test]
fn webkit_blocks_remote_by_default() {
    let t = Tracker::start();
    let (d, p) = security_eml(&format!(r#"<img src="{}/pixel.gif"><p>hi</p>"#, t.base));
    webkit()
        .arg(&p)
        .args(["-f", "png", "-o"])
        .arg(d.path().join("o.png"))
        .assert_logged()
        .success();
    assert!(t.hits.lock().unwrap().is_empty());
}

#[test]
fn webkit_remote_opt_in() {
    let t = Tracker::start();
    let (d, p) = security_eml(&format!(r#"<img src="{}/pixel.gif"><p>hi</p>"#, t.base));
    webkit()
        .arg(&p)
        .args(["-f", "png", "--remote-images", "-o"])
        .arg(d.path().join("o.png"))
        .assert_logged()
        .success();
    assert_eq!(*t.hits.lock().unwrap(), ["/pixel.gif"]);
}

#[test]
fn webkit_doctor() {
    Command::cargo_bin("doteml")
        .unwrap()
        .args(["doctor", "--backend", "webkit"])
        .timeout(Duration::from_secs(60))
        .assert_logged()
        .success()
        .stdout(contains("webkit"));
}

#[test]
fn webkit_body_cannot_overpaint_header() {
    let (d1, benign) = security_eml("<p>real body</p>");
    let (d2, attack) = security_eml(OVERLAY_ATTACK);
    let a = d1.path().join("a.png");
    let b = d2.path().join("b.png");
    webkit()
        .args(["-f", "png", "-o"])
        .arg(&a)
        .arg(&benign)
        .assert_logged()
        .success();
    webkit()
        .args(["-f", "png", "-o"])
        .arg(&b)
        .arg(&attack)
        .assert_logged()
        .success();
    let (pa, pb) = (png_pixels(&a), png_pixels(&b));
    assert_eq!(pa.0, pb.0);
    let header_rows = 150u32;
    let row_bytes = (pa.0 * 4) as usize;
    assert_eq!(
        &pa.2[..row_bytes * header_rows as usize],
        &pb.2[..row_bytes * header_rows as usize]
    );
    assert!(!has_red(&pb, 0, header_rows));
    assert!(has_red(&pb, header_rows, pb.1));
}

/// Diagnostic: which page variants load the body frame under WebKit. Prints a table
/// to the CI log and fails only if none of them work.
#[test]
fn webkit_frame_variants() {
    use doteml::html::build_html;
    let eml = doteml::ParsedEmail {
        subject: "Variants".into(),
        html_body: "<div style=\"height:800px;background:#ff0000\">body</div>".into(),
        ..Default::default()
    };
    let base = build_html(&eml, false);
    let csp_start = base
        .find("<meta http-equiv=\"Content-Security-Policy\"")
        .unwrap();
    let csp_end = base[csp_start..].find('>').unwrap() + csp_start + 1;
    let csp_meta = &base[csp_start..csp_end];
    let no_csp = base.replacen(csp_meta, "", 1);
    let self_csp = base.replacen(
        "frame-src about:;",
        "frame-src 'self' about: data: blob:;",
        1,
    );
    let no_sandbox = base.replacen(" sandbox=\"allow-same-origin\"", "", 1);
    let no_csp_no_sandbox = no_csp.replacen(" sandbox=\"allow-same-origin\"", "", 1);
    let child_src = base.replacen(
        "frame-src about:;",
        "frame-src about:; child-src about:;",
        1,
    );
    let variants = [
        ("baseline", base.clone()),
        ("no-csp", no_csp),
        ("csp-self", self_csp),
        ("no-sandbox", no_sandbox),
        ("no-csp-no-sandbox", no_csp_no_sandbox),
        ("child-src", child_src),
    ];
    let d = tempfile::tempdir().unwrap();
    let mut worked = Vec::new();
    for (name, html) in &variants {
        let page = d.path().join(format!("{name}.html"));
        std::fs::write(&page, html).unwrap();
        let out = d.path().join(format!("{name}.png"));
        webkit()
            .args(["render-html", "-f", "png", "-o"])
            .arg(&out)
            .arg(&page)
            .assert_logged()
            .success();
        let h = png_height(&out);
        eprintln!("VARIANT {name}: png height {h}");
        if h > 900 {
            worked.push(*name);
        }
    }
    eprintln!("VARIANTS THAT LOADED THE FRAME: {worked:?}");
    assert!(!worked.is_empty(), "no variant loaded the body frame");
}
