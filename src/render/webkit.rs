//! macOS backend: renders with the system WebKit, so no Chrome is needed.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBitmapImageFileType,
    NSBitmapImageRep, NSImage, NSImageCompressionFactor, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSData, NSDate, NSDefaultRunLoopMode, NSDictionary, NSError, NSNumber, NSObject,
    NSObjectProtocol, NSPoint, NSProcessInfo, NSRect, NSRunLoop, NSSize, NSString, NSURL,
};
use objc2_web_kit::{
    WKContentRuleList, WKContentRuleListStore, WKNavigation, WKNavigationAction,
    WKNavigationActionPolicy, WKNavigationDelegate, WKPDFConfiguration, WKSnapshotConfiguration,
    WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};

use super::{Backend, BackendInfo, RenderFormat, RenderOptions};
use crate::error::{Error, Result};
use crate::fsutil::{private_tempdir, write_output};
use crate::html::{FRAME_STATE_SCRIPT, RESIZE_SCRIPT};

const WIDTH: f64 = 800.0;
const INITIAL_HEIGHT: f64 = 600.0;
const PDF_TIMEOUT: Duration = Duration::from_secs(30);
/// Far off-screen so the (transparent) host window is never seen.
const WINDOW_ORIGIN: NSPoint = NSPoint {
    x: -20000.0,
    y: -20000.0,
};
/// WebKit only paints views that are in an ordered-in window; alpha 0 keeps it invisible.
const ORDER_WINDOW_FRONT: bool = true;
const DOC_NAME: &str = "email.html";

/// Filled by a completion handler: the value, or WebKit's error message.
type Slot<T> = Rc<RefCell<Option<std::result::Result<T, String>>>>;

/// Stage tracing for diagnosing WebKit on headless machines (`DOTEML_DEBUG=1`).
fn trace(stage: &str) {
    if std::env::var_os("DOTEML_DEBUG").is_some() {
        eprintln!("[webkit] {stage}");
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> *const c_void;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// WebKit needs a window-server session; over SSH or from launchd there is none and
/// AppKit would hang or abort instead of returning an error.
fn has_gui_session() -> bool {
    // SAFETY: plain C calls; the dictionary is released right away.
    let dict = unsafe { CGSessionCopyCurrentDictionary() };
    if dict.is_null() {
        return false;
    }
    unsafe { CFRelease(dict) };
    true
}

fn backend_err(msg: impl Into<String>) -> Error {
    Error::Backend(msg.into())
}

fn ns_error(err: *mut NSError) -> String {
    // SAFETY: WebKit hands us either null or a valid NSError for the duration of the callback.
    match unsafe { err.as_ref() } {
        Some(e) => e.localizedDescription().to_string(),
        None => "unknown error".into(),
    }
}

#[derive(Default)]
struct NavState {
    finished: Cell<bool>,
    failed: RefCell<Option<String>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and NavDelegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DotemlNavigationDelegate"]
    #[ivars = NavState]
    struct NavDelegate;

    unsafe impl NSObjectProtocol for NavDelegate {}

    unsafe impl WKNavigationDelegate for NavDelegate {
        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish(&self, _web_view: &WKWebView, _navigation: Option<&WKNavigation>) {
            self.ivars().finished.set(true);
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail(&self, _web_view: &WKWebView, _nav: Option<&WKNavigation>, error: &NSError) {
            self.record_failure(error);
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_provisional(
            &self,
            _web_view: &WKWebView,
            _nav: Option<&WKNavigation>,
            error: &NSError,
        ) {
            self.record_failure(error);
        }

        // Defense in depth for redirects the HTML sanitizer missed: only our staged file loads.
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_policy(
            &self,
            _web_view: &WKWebView,
            action: &WKNavigationAction,
            handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            // SAFETY: plain property read on the action WebKit passed us.
            let url = unsafe { action.request() }
                .URL()
                .and_then(|u| u.absoluteString())
                .map(|s| s.to_string())
                .unwrap_or_default();
            // Empty/about: covers the body frame's internal srcdoc navigation; data: is
            // inert; file: only for our own staged document; anything else is cancelled.
            let base = std::env::var("DOTEML_WEBKIT_BASE_URL").unwrap_or_default();
            let allow = url.is_empty()
                || url.starts_with("about:")
                || url.starts_with("data:")
                || (!base.is_empty() && url == base)
                || (url.starts_with("file://")
                    && url.split('#').next().is_some_and(|u| u.ends_with(DOC_NAME)));
            trace(&format!(
                "navigation {} {url:?}",
                if allow { "allow" } else { "cancel" }
            ));
            handler.call((if allow {
                WKNavigationActionPolicy::Allow
            } else {
                WKNavigationActionPolicy::Cancel
            },));
        }
    }
);

impl NavDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(NavState::default());
        // SAFETY: NSObject's init is always safe to call on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    fn record_failure(&self, error: &NSError) {
        // Only the main document's failure matters; later cancelled redirects are expected.
        if !self.ivars().finished.get() {
            *self.ivars().failed.borrow_mut() = Some(error.localizedDescription().to_string());
            self.ivars().finished.set(true);
        }
    }

    fn reset(&self) {
        self.ivars().finished.set(false);
        *self.ivars().failed.borrow_mut() = None;
    }
}

/// Run the main run loop until `done()` or the timeout. WebKit and AppKit deliver every
/// completion handler on the main run loop, and we never call `NSApplication::run`.
fn pump_until(done: impl Fn() -> bool, timeout: Duration) -> bool {
    let run_loop = NSRunLoop::mainRunLoop();
    let deadline = Instant::now() + timeout;
    while !done() {
        if Instant::now() >= deadline {
            return false;
        }
        let until = NSDate::dateWithTimeIntervalSinceNow(0.05);
        // SAFETY: these are immutable framework constants.
        unsafe {
            run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &until);
        }
    }
    true
}

fn rect(width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
}

/// WebKit content-rule url-filters don't support `|`, so each scheme gets its own rule.
fn content_rules(allow_remote: bool) -> &'static str {
    if allow_remote {
        // Remote images, styles and fonts load; anything else over the network is blocked.
        r#"[{"trigger":{"url-filter":"^https?://","resource-type":["document","script","raw","svg-document","media","popup"]},"action":{"type":"block"}},{"trigger":{"url-filter":"^wss?://"},"action":{"type":"block"}}]"#
    } else {
        r#"[{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}},{"trigger":{"url-filter":"^wss?://"},"action":{"type":"block"}}]"#
    }
}

pub(crate) struct WebKitBackend {
    mtm: MainThreadMarker,
    window: Retained<NSWindow>,
    webview: Retained<WKWebView>,
    delegate: Retained<NavDelegate>,
    opts: RenderOptions,
    _rule_store_dir: tempfile::TempDir,
}

impl WebKitBackend {
    pub fn new(opts: RenderOptions) -> Result<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| backend_err("the webkit backend must be used from the main thread"))?;
        trace("main thread ok");
        if !has_gui_session() {
            return Err(backend_err(
                "no graphical session (SSH or background process); use --backend chrome",
            ));
        }
        trace("gui session ok");

        // WebKit needs AppKit initialised, but a CLI must not show a Dock icon or steal focus.
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
        app.finishLaunching();
        trace("NSApplication launched");

        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        unsafe {
            config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            // Disables the page's own scripts; our evaluateJavaScript height probe still runs.
            // DOTEML_WEBKIT_ALLOW_JS is a diagnostic knob only.
            if std::env::var_os("DOTEML_WEBKIT_ALLOW_JS").is_none() {
                config
                    .defaultWebpagePreferences()
                    .setAllowsContentJavaScript(false);
            }
        }

        let rule_store_dir = private_tempdir("doteml-rules-")?;
        let rules = compile_rules(mtm, rule_store_dir.path(), opts.allow_remote, opts.timeout)?;
        trace("content rules compiled");
        unsafe { config.userContentController().addContentRuleList(&rules) };

        let webview = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                rect(WIDTH, INITIAL_HEIGHT),
                &config,
            )
        };
        let delegate = NavDelegate::new(mtm);
        unsafe { webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        trace("web view created");

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(WINDOW_ORIGIN, NSSize::new(WIDTH, INITIAL_HEIGHT)),
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setContentView(Some(&webview));
        if ORDER_WINDOW_FRONT {
            window.setAlphaValue(0.0);
            window.orderFrontRegardless();
            trace("window ordered in");
        }

        Ok(Self {
            mtm,
            window,
            webview,
            delegate,
            opts,
            _rule_store_dir: rule_store_dir,
        })
    }

    /// Load the page from memory. A page loaded from a file:// URL through WebKit's
    /// restricted file access never renders its srcdoc body frame, so no file is staged.
    fn load(&self, html: &str) -> Result<()> {
        self.delegate.reset();
        trace("load: start");
        self.resize(INITIAL_HEIGHT);
        // DOTEML_WEBKIT_BASE_URL is a diagnostic knob: WebKit treats documents with a
        // real base origin differently from opaque about:blank ones.
        let base = std::env::var("DOTEML_WEBKIT_BASE_URL")
            .ok()
            .and_then(|u| NSURL::URLWithString(&NSString::from_str(&u)));
        unsafe {
            self.webview
                .loadHTMLString_baseURL(&NSString::from_str(html), base.as_deref())
        };
        // Render whatever loaded on timeout, like the Chrome backend.
        let state = self.delegate.ivars();
        pump_until(|| state.finished.get(), self.opts.timeout);
        trace("load: done or timed out");
        if let Some(msg) = state.failed.borrow().clone() {
            return Err(backend_err(format!(
                "WebKit could not load the email: {msg}"
            )));
        }
        Ok(())
    }

    fn resize(&self, height: f64) {
        self.webview.setFrame(rect(WIDTH, height));
        self.window.setContentSize(NSSize::new(WIDTH, height));
    }

    /// Fits the body frame to its content and returns the full document height in CSS
    /// pixels (page JS is off; this injected probe still runs).
    fn content_height(&self) -> f64 {
        let result: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None));
        let done = Rc::new(Cell::new(false));
        let (r, d) = (result.clone(), done.clone());
        let block = RcBlock::new(move |value: *mut AnyObject, _err: *mut NSError| {
            // SAFETY: null or a valid object for the duration of the callback.
            if let Some(n) = unsafe { value.as_ref() }.and_then(|v| v.downcast_ref::<NSNumber>()) {
                r.set(Some(n.as_f64()));
            }
            d.set(true);
        });
        let script = NSString::from_str(RESIZE_SCRIPT);
        unsafe {
            self.webview
                .evaluateJavaScript_completionHandler(&script, Some(&block))
        };
        pump_until(|| done.get(), Duration::from_secs(5));
        trace(&format!("height probe returned {:?}", result.get()));
        result.get().filter(|h| *h >= 1.0).unwrap_or(INITIAL_HEIGHT)
    }

    /// Evaluate a script that returns a string (page JS is off; injected scripts still run).
    fn eval_string(&self, script: &str) -> Option<String> {
        let result: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let done = Rc::new(Cell::new(false));
        let (r, d) = (result.clone(), done.clone());
        let block = RcBlock::new(move |value: *mut AnyObject, err: *mut NSError| {
            // SAFETY: null or a valid object for the duration of the callback.
            match unsafe { value.as_ref() }.and_then(|v| v.downcast_ref::<NSString>()) {
                Some(s) => *r.borrow_mut() = Some(s.to_string()),
                None => *r.borrow_mut() = Some(format!("error: {}", ns_error(err))),
            }
            d.set(true);
        });
        unsafe {
            self.webview
                .evaluateJavaScript_completionHandler(&NSString::from_str(script), Some(&block))
        };
        pump_until(|| done.get(), Duration::from_secs(5));
        result.borrow().clone()
    }

    /// The main frame's load event can fire before the srcdoc body frame has laid out;
    /// wait until the frame reports a complete document with content.
    fn wait_for_frame(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = self.eval_string(FRAME_STATE_SCRIPT).unwrap_or_default();
            trace(&format!("frame state {state}"));
            let mut parts = state.split(':');
            let ready = parts.next() == Some("complete")
                && parts
                    .next()
                    .and_then(|n| n.parse::<i64>().ok())
                    .is_some_and(|n| n >= 1);
            if ready || Instant::now() >= deadline {
                return;
            }
            pump_until(|| false, Duration::from_millis(50));
        }
    }

    fn snapshot(&self, height: f64, fmt: RenderFormat, quality: u8) -> Result<Vec<u8>> {
        let cfg = unsafe { WKSnapshotConfiguration::new(self.mtm) };
        unsafe {
            cfg.setRect(rect(WIDTH, height));
            cfg.setSnapshotWidth(Some(&NSNumber::new_f64(WIDTH)));
            cfg.setAfterScreenUpdates(true);
        }
        let out: Slot<Retained<NSImage>> = Rc::new(RefCell::new(None));
        let o = out.clone();
        let block = RcBlock::new(move |image: *mut NSImage, err: *mut NSError| {
            // SAFETY: WebKit passes a valid (or null) autoreleased image; retain it to keep it.
            let image = unsafe { Retained::retain(image) };
            *o.borrow_mut() = Some(image.ok_or_else(|| ns_error(err)));
        });
        unsafe {
            self.webview
                .takeSnapshotWithConfiguration_completionHandler(Some(&cfg), &block)
        };
        trace("snapshot: requested");
        if !pump_until(|| out.borrow().is_some(), self.opts.timeout) {
            return Err(backend_err("WebKit snapshot timed out"));
        }
        let image = out.borrow_mut().take().unwrap().map_err(backend_err)?;
        encode_image(&image, fmt, quality)
    }

    fn create_pdf(&self, height: f64) -> Result<Vec<u8>> {
        let cfg = unsafe { WKPDFConfiguration::new(self.mtm) };
        unsafe { cfg.setRect(rect(WIDTH, height)) };
        let out: Slot<Vec<u8>> = Rc::new(RefCell::new(None));
        let o = out.clone();
        let block = RcBlock::new(move |data: *mut NSData, err: *mut NSError| {
            // SAFETY: copy the bytes out inside the callback; never keep the pointer.
            let bytes = unsafe { data.as_ref() }.map(|d| d.to_vec());
            *o.borrow_mut() = Some(bytes.ok_or_else(|| ns_error(err)));
        });
        unsafe {
            self.webview
                .createPDFWithConfiguration_completionHandler(Some(&cfg), &block)
        };
        trace("createPDF: requested");
        if !pump_until(|| out.borrow().is_some(), PDF_TIMEOUT) {
            return Err(backend_err("WebKit PDF export timed out"));
        }
        out.borrow_mut().take().unwrap().map_err(backend_err)
    }
}

fn compile_rules(
    mtm: MainThreadMarker,
    store_dir: &Path,
    allow_remote: bool,
    timeout: Duration,
) -> Result<Retained<WKContentRuleList>> {
    // A private store in a temp dir: nothing is written under ~/Library.
    let store_url =
        NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&store_dir.to_string_lossy()), true);
    let store = unsafe { WKContentRuleListStore::storeWithURL(Some(&store_url), mtm) }
        .ok_or_else(|| backend_err("cannot create a WebKit content rule store"))?;

    let out: Slot<Retained<WKContentRuleList>> = Rc::new(RefCell::new(None));
    let o = out.clone();
    let block = RcBlock::new(move |list: *mut WKContentRuleList, err: *mut NSError| {
        // SAFETY: WebKit passes a valid (or null) rule list; retain it beyond the callback.
        let list = unsafe { Retained::retain(list) };
        *o.borrow_mut() = Some(list.ok_or_else(|| ns_error(err)));
    });
    unsafe {
        store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
            Some(&NSString::from_str("doteml")),
            Some(&NSString::from_str(content_rules(allow_remote))),
            Some(&block),
        );
    }
    if !pump_until(|| out.borrow().is_some(), timeout) {
        return Err(backend_err("timed out compiling WebKit content rules"));
    }
    out.borrow_mut()
        .take()
        .unwrap()
        .map_err(|e| backend_err(format!("WebKit content rules: {e}")))
}

fn encode_image(image: &NSImage, fmt: RenderFormat, quality: u8) -> Result<Vec<u8>> {
    let tiff = image
        .TIFFRepresentation()
        .ok_or_else(|| backend_err("empty WebKit snapshot"))?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)
        .ok_or_else(|| backend_err("cannot decode WebKit snapshot"))?;
    let data = match fmt {
        RenderFormat::Jpg => {
            let factor = NSNumber::new_f64(f64::from(quality) / 100.0);
            let props: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
                &[unsafe { NSImageCompressionFactor }],
                &[&*factor as &AnyObject],
            );
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::JPEG, &props) }
        }
        _ => {
            let props: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }
        }
    };
    data.map(|d| d.to_vec())
        .ok_or_else(|| backend_err("cannot encode WebKit snapshot"))
}

impl Backend for WebKitBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "webkit",
            executable: "WebKit.framework".into(),
            product: format!(
                "WebKit (macOS {})",
                NSProcessInfo::processInfo().operatingSystemVersionString()
            ),
        }
    }

    fn render(&mut self, html: &str, output: &Path, fmt: RenderFormat, quality: u8) -> Result<()> {
        self.load(html)?;

        self.wait_for_frame();
        let height = self.content_height();
        self.resize(height);
        pump_until(|| false, Duration::from_millis(100)); // let layout settle at the new size

        let result = match fmt {
            RenderFormat::Png | RenderFormat::Jpg => self
                .snapshot(height, fmt, quality)
                .and_then(|bytes| write_output(output, &bytes)),
            // A single page as tall as the email. AppKit's paginated print path
            // (NSPrintOperation) blocks forever in a CLI, so it is not used.
            RenderFormat::Pdf => self
                .create_pdf(height)
                .and_then(|bytes| write_output(output, &bytes)),
        };
        // Release the document before the staging directory goes away.
        unsafe {
            self.webview
                .loadHTMLString_baseURL(&NSString::from_str(""), None)
        };
        result
    }
}

impl Drop for WebKitBackend {
    fn drop(&mut self) {
        unsafe { self.webview.setNavigationDelegate(None) };
        self.window.orderOut(None);
        self.window.close();
    }
}
