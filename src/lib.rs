//! Convert `.eml` email files to PDF, PNG, JPG, or HTML.

pub mod browser;
pub mod cdp;
pub mod cli;
pub mod convert;
pub mod error;
pub mod html;
pub mod integrations;
pub mod parser;
pub mod pdfmeta;
pub mod render;
pub mod verify;

pub use convert::{ConvertOptions, Format, convert};
pub use error::{Error, Result};
pub use parser::{Attachment, ParsedEmail, parse_eml, parse_eml_bytes};
pub use render::{BackendChoice, RenderOptions, Renderer};
