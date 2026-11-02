use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("File not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("Expected .eml file, got: {0}")]
    NotEml(String),
    #[error("{0}")]
    BrowserNotFound(String),
    #[error("{0}")]
    Cdp(String),
    #[error("{0}")]
    Backend(String),
    #[error("PDF: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Verify(String),
}

pub type Result<T> = std::result::Result<T, Error>;
