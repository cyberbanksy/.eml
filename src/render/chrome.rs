use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use super::{
    Backend, BackendInfo, RenderFormat, RenderOptions, check_image_height, check_version, file_url,
};
use crate::browser::find_browser;
use crate::cdp::Browser;
use crate::error::{Error, Result};
use crate::fsutil::{private_tempdir, write_output, write_private};
use crate::html::RESIZE_SCRIPT;

const REMOTE_TYPES: [&str; 3] = ["Image", "Stylesheet", "Font"];
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct ChromeBackend {
    browser: Browser,
    opts: RenderOptions,
    executable: String,
    product: String,
}

impl ChromeBackend {
    pub fn new(opts: RenderOptions) -> Result<Self> {
        let exe = find_browser()?;
        let mut browser = Browser::launch(&exe, LAUNCH_TIMEOUT)?;
        let version = browser.send("Browser.getVersion", json!({}), None)?;
        let product = version["product"].as_str().unwrap_or_default().to_string();
        check_version(&product, &exe)?;
        Ok(Self {
            browser,
            opts,
            executable: exe.display().to_string(),
            product,
        })
    }
}

fn allowed(url: &str, doc_url: &str, resource_type: &str, allow_remote: bool) -> bool {
    url == doc_url
        || url.starts_with("data:")
        || url.starts_with("about:")
        || (allow_remote
            && (url.starts_with("http:") || url.starts_with("https:"))
            && REMOTE_TYPES.contains(&resource_type))
}

impl Backend for ChromeBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "chrome",
            executable: self.executable.clone(),
            product: self.product.clone(),
        }
    }

    fn render(&mut self, html: &str, output: &Path, fmt: RenderFormat, quality: u8) -> Result<()> {
        let dir = private_tempdir("doteml-")?;
        let doc = dir.path().join("email.html");
        write_private(&doc, html.as_bytes())?;
        let doc_url = file_url(&doc);
        let b = &mut self.browser;

        let target = b.send("Target.createTarget", json!({"url": "about:blank"}), None)?;
        let target_id = target["targetId"].as_str().unwrap_or_default().to_string();
        let attached = b.send(
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
            None,
        )?;
        let session = attached["sessionId"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let loaded = Rc::new(Cell::new(false));
        {
            let loaded = loaded.clone();
            let doc_url = doc_url.clone();
            let allow_remote = self.opts.allow_remote;
            b.on(
                &session,
                Box::new(move |msg: &Value| match msg["method"].as_str() {
                    Some("Page.loadEventFired") => {
                        loaded.set(true);
                        vec![]
                    }
                    Some("Fetch.requestPaused") => {
                        let p = &msg["params"];
                        let id = p["requestId"].clone();
                        let url = p["request"]["url"].as_str().unwrap_or_default();
                        let rtype = p["resourceType"].as_str().unwrap_or_default();
                        if allowed(url, &doc_url, rtype, allow_remote) {
                            vec![("Fetch.continueRequest".into(), json!({"requestId": id}))]
                        } else {
                            vec![(
                                "Fetch.failRequest".into(),
                                json!({"requestId": id, "errorReason": "BlockedByClient"}),
                            )]
                        }
                    }
                    _ => vec![],
                }),
            );
        }

        let result = (|| -> Result<Vec<u8>> {
            let s = Some(session.as_str());
            b.send(
                "Fetch.enable",
                json!({"patterns": [{"urlPattern": "*"}]}),
                s,
            )?;
            b.send("Page.enable", json!({}), s)?;
            b.send(
                "Emulation.setScriptExecutionDisabled",
                json!({"value": true}),
                s,
            )?;
            b.send(
                "Emulation.setDeviceMetricsOverride",
                json!({"width": 800, "height": 600, "deviceScaleFactor": 1, "mobile": false}),
                s,
            )?;
            b.send("Page.navigate", json!({"url": doc_url}), s)?;
            // Render whatever loaded; a slow remote image shouldn't fail the job.
            let flag = loaded.clone();
            b.pump_until(move || flag.get(), self.opts.timeout)?;
            // Page scripts are off, but DevTools can still evaluate: fit the body frame.
            b.send(
                "Runtime.evaluate",
                json!({"expression": RESIZE_SCRIPT, "returnByValue": true}),
                s,
            )?;

            let data = match fmt {
                RenderFormat::Pdf => {
                    let m = 0.4;
                    b.send(
                        "Page.printToPDF",
                        json!({"printBackground": true, "paperWidth": 8.5, "paperHeight": 11,
                               "marginTop": m, "marginBottom": m, "marginLeft": m, "marginRight": m}),
                        s,
                    )?
                }
                RenderFormat::Png | RenderFormat::Jpg => {
                    let metrics = b.send("Page.getLayoutMetrics", json!({}), s)?;
                    let size = if metrics["cssContentSize"].is_object() {
                        &metrics["cssContentSize"]
                    } else {
                        &metrics["contentSize"]
                    };
                    let w = size["width"].as_f64().unwrap_or(800.0).ceil().max(1.0);
                    let h = size["height"].as_f64().unwrap_or(600.0).ceil().max(1.0);
                    check_image_height(fmt, h)?;
                    let mut params = json!({
                        "format": if fmt == RenderFormat::Png { "png" } else { "jpeg" },
                        "captureBeyondViewport": true,
                        "clip": {"x": 0, "y": 0, "width": w, "height": h, "scale": 1},
                    });
                    if fmt == RenderFormat::Jpg {
                        params["quality"] = json!(quality);
                    }
                    // Chrome occasionally has no frame ready on a busy machine; retry once.
                    match b.send("Page.captureScreenshot", params.clone(), s) {
                        Err(Error::Cdp(msg)) if msg.contains("Unable to capture screenshot") => {
                            b.pump_until(|| false, Duration::from_millis(500))?;
                            b.send("Page.captureScreenshot", params, s)?
                        }
                        other => other?,
                    }
                }
            };
            // Chrome answers an image it cannot encode with empty data, not an error.
            let encoded = data["data"].as_str().unwrap_or_default();
            if encoded.is_empty() {
                return Err(Error::Cdp("the browser returned an empty render".into()));
            }
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| Error::Cdp(format!("Bad render output: {e}")))
        })();

        b.off(&session);
        let _ = b.send("Target.closeTarget", json!({"targetId": target_id}), None);
        write_output(output, &result?)?;
        Ok(())
    }
}
