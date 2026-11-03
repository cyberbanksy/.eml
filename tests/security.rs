mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use doteml::{ConvertOptions, Format, convert};

fn pdf_opts() -> ConvertOptions {
    ConvertOptions::default()
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
fn javascript_does_not_run() {
    let (d, p) = security_eml(
        r#"<p id="x">static text</p><script>document.getElementById('x').textContent='JSRAN'</script>"#,
    );
    let out = d.path().join("o.pdf");
    convert(&p, Some(&out), &pdf_opts(), None).unwrap();
    let text = pdf_text(&out);
    assert!(text.contains("static text"), "{text}");
    assert!(!text.contains("JSRAN"));
}

#[test]
fn meta_refresh_cannot_replace_document() {
    let (d, p) = security_eml(
        r#"<meta http-equiv="refresh" content="0;url=data:text/html,HIJACKED"><p>real body</p>"#,
    );
    let out = d.path().join("o.pdf");
    convert(&p, Some(&out), &pdf_opts(), None).unwrap();
    let text = pdf_text(&out);
    assert!(text.contains("real body"), "{text}");
    assert!(!text.contains("HIJACKED"));
}

#[test]
fn remote_images_blocked_by_default() {
    let t = Tracker::start();
    let (d, p) = security_eml(&format!(r#"<img src="{}/pixel.gif"><p>hi</p>"#, t.base));
    let opts = ConvertOptions {
        format: Format::Png,
        ..Default::default()
    };
    convert(&p, Some(&d.path().join("o.png")), &opts, None).unwrap();
    assert!(t.hits.lock().unwrap().is_empty());
}

#[test]
fn remote_images_opt_in() {
    let t = Tracker::start();
    let (d, p) = security_eml(&format!(r#"<img src="{}/pixel.gif"><p>hi</p>"#, t.base));
    let opts = ConvertOptions {
        format: Format::Png,
        allow_remote: true,
        ..Default::default()
    };
    convert(&p, Some(&d.path().join("o.png")), &opts, None).unwrap();
    assert_eq!(*t.hits.lock().unwrap(), ["/pixel.gif"]);
}

#[test]
fn pdf_metadata() {
    let (d, p) = security_eml("<p>x</p>");
    let out = d.path().join("o.pdf");
    convert(&p, Some(&out), &pdf_opts(), None).unwrap();
    let info = pdf_info(&out);
    assert_eq!(info["Title"], "Security");
    assert_eq!(info["Author"], "a@example.com");
    assert_eq!(info["EmailTo"], "b@example.com");
    assert_eq!(info["EmailMessageID"], "<sec@example.com>");
    assert!(
        info["CreationDate"].starts_with("D:20240101120000"),
        "{}",
        info["CreationDate"]
    );
}

#[test]
fn body_cannot_overpaint_header() {
    let (d1, benign) = security_eml("<p>real body</p>");
    let (d2, attack) = security_eml(OVERLAY_ATTACK);
    let opts = ConvertOptions {
        format: Format::Png,
        ..Default::default()
    };
    let a = d1.path().join("a.png");
    let b = d2.path().join("b.png");
    convert(&benign, Some(&a), &opts, None).unwrap();
    convert(&attack, Some(&b), &opts, None).unwrap();
    let (pa, pb) = (png_pixels(&a), png_pixels(&b));
    assert_eq!(pa.0, pb.0);
    // The header region is pixel-identical; the attack can only paint inside its own frame.
    let header_rows = 150;
    let row_bytes = (pa.0 * 4) as usize;
    assert_eq!(
        &pa.2[..row_bytes * header_rows],
        &pb.2[..row_bytes * header_rows],
        "header pixels changed"
    );
    assert!(
        !has_red(&pb, 0, header_rows as u32),
        "red overlay reached the header"
    );
    assert!(
        has_red(&pb, header_rows as u32, pb.1),
        "attack content was not rendered at all"
    );

    let pdf = d2.path().join("b.pdf");
    convert(&attack, Some(&pdf), &ConvertOptions::default(), None).unwrap();
    // Body text is still present as text (contained, not dropped). The header's
    // text is checked via pixels above: lopdf cannot extract the header font.
    assert!(
        pdf_text(&pdf).contains("SPOOFED"),
        "body text should still be present"
    );
}

#[test]
fn oversized_eml_refused() {
    let d = tempfile::tempdir().unwrap();
    let big = d.path().join("big.eml");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(doteml::convert::MAX_EML_BYTES + 1).unwrap();
    let err = convert(&big, None, &ConvertOptions::default(), None).unwrap_err();
    assert!(matches!(err, doteml::Error::TooLarge { .. }), "{err}");
}

#[cfg(unix)]
#[test]
fn output_symlink_refused() {
    let (d, p) = security_eml("<p>x</p>");
    let target = d.path().join("precious.txt");
    std::fs::write(&target, "keep").unwrap();
    let link = d.path().join("out.html");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let opts = ConvertOptions {
        format: Format::Html,
        ..Default::default()
    };
    assert!(convert(&p, Some(&link), &opts, None).is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
}
