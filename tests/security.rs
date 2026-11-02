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
