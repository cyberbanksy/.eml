use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::fsutil::{reject_symlink, write_output};
use crate::html::build_html;
use crate::parser::parse_eml_bytes;
use crate::pdfmeta::{EmlSource, write_pdf_metadata};
use crate::render::{BackendChoice, RenderFormat, RenderOptions, Renderer};

pub const FORMATS: [&str; 4] = ["pdf", "png", "jpg", "html"];

/// Inputs above this size are refused: every attachment is held in memory
/// and inline images are base64-expanded, so memory use is a multiple of it.
pub const MAX_EML_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, clap::ValueEnum)]
pub enum Format {
    #[default]
    Pdf,
    Png,
    Jpg,
    Html,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Png => "png",
            Format::Jpg => "jpg",
            Format::Html => "html",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConvertOptions {
    pub format: Format,
    pub quality: u8,
    pub allow_remote: bool,
    pub backend: BackendChoice,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            format: Format::Pdf,
            quality: 85,
            allow_remote: false,
            backend: BackendChoice::Auto,
        }
    }
}

/// Convert one `.eml` file. Pass a shared `renderer` to reuse a browser across files.
pub fn convert(
    input: &Path,
    output: Option<&Path>,
    opts: &ConvertOptions,
    renderer: Option<&mut Renderer>,
) -> Result<PathBuf> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("eml") {
        let shown = if ext.is_empty() {
            String::new()
        } else {
            format!(".{ext}")
        };
        return Err(Error::NotEml(shown));
    }
    let output = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| input.with_extension(opts.format.extension()));

    let size = std::fs::metadata(input)?.len();
    if size > MAX_EML_BYTES {
        return Err(Error::TooLarge {
            size,
            limit: MAX_EML_BYTES,
        });
    }
    reject_symlink(&output)?;

    let raw = std::fs::read(input)?;
    let eml = parse_eml_bytes(&raw);
    let html = build_html(&eml, opts.allow_remote);

    let fmt = match opts.format {
        Format::Html => {
            write_output(&output, html.as_bytes())?;
            return Ok(output);
        }
        Format::Pdf => RenderFormat::Pdf,
        Format::Png => RenderFormat::Png,
        Format::Jpg => RenderFormat::Jpg,
    };
    match renderer {
        Some(r) => r.render(&html, &output, fmt, opts.quality)?,
        None => {
            let mut r = Renderer::new(RenderOptions {
                allow_remote: opts.allow_remote,
                backend: opts.backend,
                ..Default::default()
            })?;
            r.render(&html, &output, fmt, opts.quality)?;
        }
    }
    if fmt == RenderFormat::Pdf {
        let file_name = input
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("message.eml");
        write_pdf_metadata(
            &output,
            &eml,
            EmlSource {
                bytes: &raw,
                file_name,
            },
        )?;
    }
    Ok(output)
}
