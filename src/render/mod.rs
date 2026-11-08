//! Rendering backends: Chrome/Edge over DevTools everywhere, WebKit on macOS.

use std::path::Path;
use std::time::Duration;

use crate::browser::{DOWNLOAD_URL, MIN_VERSION, parse_major};
use crate::error::{Error, Result};

mod chrome;
#[cfg(target_os = "macos")]
mod webkit;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, clap::ValueEnum)]
pub enum BackendChoice {
    /// WebKit on macOS, Chrome/Edge elsewhere
    #[default]
    Auto,
    Chrome,
    Webkit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderFormat {
    Pdf,
    Png,
    Jpg,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    pub allow_remote: bool,
    pub backend: BackendChoice,
    /// How long to wait for the page (and any allowed remote images) to load.
    pub timeout: Duration,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            allow_remote: false,
            backend: BackendChoice::Auto,
            timeout: Duration::from_secs(15),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackendInfo {
    pub name: &'static str,
    pub executable: String,
    pub product: String,
}

pub(crate) trait Backend {
    fn info(&self) -> BackendInfo;
    fn render(&mut self, html: &str, output: &Path, fmt: RenderFormat, quality: u8) -> Result<()>;
}

/// One browser engine, reused across a batch of conversions.
pub struct Renderer {
    backend: Box<dyn Backend>,
}

impl Renderer {
    pub fn new(opts: RenderOptions) -> Result<Renderer> {
        let backend: Box<dyn Backend> = match opts.backend {
            BackendChoice::Chrome => Box::new(chrome::ChromeBackend::new(opts)?),
            BackendChoice::Webkit => webkit_backend(opts)?,
            BackendChoice::Auto => {
                if cfg!(target_os = "macos") {
                    match webkit_backend(opts) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("webkit backend unavailable ({e}); falling back to chrome");
                            Box::new(chrome::ChromeBackend::new(opts)?)
                        }
                    }
                } else {
                    Box::new(chrome::ChromeBackend::new(opts)?)
                }
            }
        };
        Ok(Renderer { backend })
    }

    pub fn info(&self) -> BackendInfo {
        self.backend.info()
    }

    pub fn render(
        &mut self,
        html: &str,
        output: &Path,
        fmt: RenderFormat,
        quality: u8,
    ) -> Result<()> {
        self.backend.render(html, output, fmt, quality)
    }
}

/// JPEG stores each dimension in 16 bits, so taller images cannot be encoded.
const JPEG_MAX_HEIGHT: f64 = 65_535.0;

/// Refuse an image the format cannot hold, before spending time rendering it.
pub(crate) fn check_image_height(fmt: RenderFormat, height: f64) -> Result<()> {
    if fmt == RenderFormat::Jpg && height > JPEG_MAX_HEIGHT {
        return Err(Error::Backend(format!(
            "the email is {height:.0} px tall; JPG allows at most 65,535 px (use -f png or -f pdf)"
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn webkit_backend(opts: RenderOptions) -> Result<Box<dyn Backend>> {
    Ok(Box::new(webkit::WebKitBackend::new(opts)?))
}

#[cfg(not(target_os = "macos"))]
fn webkit_backend(_opts: RenderOptions) -> Result<Box<dyn Backend>> {
    Err(Error::Backend(
        "the webkit backend is only available on macOS".into(),
    ))
}

pub fn check_version(product: &str, executable: &Path) -> Result<()> {
    match parse_major(product) {
        Some(major) if major < MIN_VERSION => Err(Error::BrowserNotFound(format!(
            "{product} at {} is too old (need {MIN_VERSION}+).\n  Update it, or install Google Chrome: {DOWNLOAD_URL}",
            executable.display()
        ))),
        _ => Ok(()),
    }
}

/// `file://` URL for a local path (forward slashes, percent-encoded, Windows drive aware).
pub(crate) fn file_url(path: &Path) -> String {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut s = path.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    let mut out = String::from("file://");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpg_height_limit() {
        assert!(check_image_height(RenderFormat::Jpg, 65_535.0).is_ok());
        let err = check_image_height(RenderFormat::Jpg, 65_536.0).unwrap_err();
        assert!(err.to_string().contains("65,535"));
        assert!(check_image_height(RenderFormat::Png, 200_000.0).is_ok());
        assert!(check_image_height(RenderFormat::Pdf, 200_000.0).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn file_url_unix() {
        assert_eq!(
            file_url(Path::new("/tmp/a b/é.html")),
            "file:///tmp/a%20b/%C3%A9.html"
        );
    }

    #[cfg(windows)]
    #[test]
    fn file_url_windows() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\A B\x.html")),
            "file:///C:/Users/A%20B/x.html"
        );
    }
}
