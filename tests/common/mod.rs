#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn legoland() -> PathBuf {
    fixtures().join("legoland.eml")
}

/// Write `raw` (with `\n` turned into `\r\n`) to `<tmp>/<name>`.
pub fn write_eml(name: &str, raw: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, raw.replace('\n', "\r\n")).unwrap();
    (dir, path)
}

pub fn simple_eml() -> (TempDir, PathBuf) {
    write_eml(
        "simple.eml",
        "From: sender@example.com\nTo: recipient@example.com\nSubject: Test Email\n\
         Date: Mon, 1 Jan 2024 12:00:00 +0000\nMIME-Version: 1.0\n\
         Content-Type: text/html; charset=\"utf-8\"\n\n\
         <html><body><h1>Hello</h1><p>This is a test.</p></body></html>\n",
    )
}

pub fn plaintext_eml() -> (TempDir, PathBuf) {
    write_eml(
        "plain.eml",
        "From: alice@example.com\nTo: bob@example.com\nSubject: Plain text email\n\
         Date: Tue, 2 Jan 2024 09:30:00 +0000\nMIME-Version: 1.0\n\
         Content-Type: text/plain; charset=\"utf-8\"\n\n\
         Hello Bob,\n\nThis is a plain text email.\n\nBest,\nAlice\n",
    )
}

pub fn multipart_eml() -> (TempDir, PathBuf) {
    write_eml(
        "multipart.eml",
        "From: sender@example.com\nTo: recipient@example.com\nSubject: Multipart Email\n\
         Date: Wed, 3 Jan 2024 15:00:00 +0000\nMIME-Version: 1.0\n\
         Content-Type: multipart/alternative; boundary=\"B\"\n\n\
         --B\nContent-Type: text/plain; charset=\"us-ascii\"\n\nPlain text version\n\
         --B\nContent-Type: text/html; charset=\"us-ascii\"\n\n\
         <html><body><h1>HTML Version</h1><p>Rich content here.</p></body></html>\n\
         --B--\n",
    )
}

pub fn cc_eml() -> (TempDir, PathBuf) {
    write_eml(
        "cc.eml",
        "From: sender@example.com\nTo: recipient@example.com\n\
         Cc: cc1@example.com, cc2@example.com\nSubject: Email with CC\n\
         Date: Thu, 4 Jan 2024 10:00:00 +0000\nMIME-Version: 1.0\n\
         Content-Type: text/html; charset=\"utf-8\"\n\n<html><body><p>CC test</p></body></html>\n",
    )
}

pub fn encoded_subject_eml() -> (TempDir, PathBuf) {
    write_eml(
        "encoded.eml",
        "From: sender@example.com\nTo: recipient@example.com\n\
         Subject: =?UTF-8?B?SGVsbG8g8J+RiyBXb3JsZA==?=\n\
         Date: Fri, 5 Jan 2024 08:00:00 +0000\nMIME-Version: 1.0\n\
         Content-Type: text/html; charset=\"utf-8\"\n\n<html><body><p>Encoded subject test</p></body></html>\n",
    )
}

/// A security-test email with the given HTML body.
pub fn security_eml(body: &str) -> (TempDir, PathBuf) {
    write_eml(
        "sec.eml",
        &format!(
            "From: a@example.com\nTo: b@example.com\nSubject: Security\n\
             Date: Mon, 1 Jan 2024 12:00:00 +0000\nMessage-ID: <sec@example.com>\n\
             Content-Type: text/html; charset=\"utf-8\"\n\n{body}"
        ),
    )
}

/// All text in a PDF, page by page.
pub fn pdf_text(path: &Path) -> String {
    let doc = lopdf::Document::load(path).unwrap();
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    doc.extract_text(&pages).unwrap_or_default()
}

/// The PDF Info dictionary as decoded strings.
pub fn pdf_info(path: &Path) -> std::collections::BTreeMap<String, String> {
    let doc = lopdf::Document::load(path).unwrap();
    let id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    doc.get_dictionary(id)
        .unwrap()
        .iter()
        .filter_map(|(k, v)| {
            let v = match v {
                lopdf::Object::Integer(i) => Some(i.to_string()),
                other => doteml::pdfmeta::pdf_text_decode(other),
            }?;
            Some((String::from_utf8_lossy(k).into_owned(), v))
        })
        .collect()
}
