mod common;

use common::*;
use doteml::{ConvertOptions, Error, Format, convert};

fn opts(format: Format) -> ConvertOptions {
    ConvertOptions {
        format,
        ..Default::default()
    }
}

fn magic(path: &std::path::Path, n: usize) -> Vec<u8> {
    std::fs::read(path).unwrap()[..n].to_vec()
}

#[test]
fn simple_to_pdf() {
    let (d, p) = simple_eml();
    let out = d.path().join("output.pdf");
    assert_eq!(
        convert(&p, Some(&out), &opts(Format::Pdf), None).unwrap(),
        out
    );
    assert_eq!(magic(&out, 5), b"%PDF-");
}

#[test]
fn plaintext_to_pdf() {
    let (d, p) = plaintext_eml();
    let out = d.path().join("plain.pdf");
    convert(&p, Some(&out), &opts(Format::Pdf), None).unwrap();
    assert_eq!(magic(&out, 5), b"%PDF-");
}

#[test]
fn multipart_to_pdf() {
    let (d, p) = multipart_eml();
    let out = d.path().join("multi.pdf");
    convert(&p, Some(&out), &opts(Format::Pdf), None).unwrap();
    assert_eq!(magic(&out, 5), b"%PDF-");
}

#[test]
fn legoland_to_pdf() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.pdf");
    convert(&legoland(), Some(&out), &opts(Format::Pdf), None).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);
    assert_eq!(magic(&out, 5), b"%PDF-");
}

#[test]
fn default_output_path() {
    let (_d, p) = simple_eml();
    let out = convert(&p, None, &opts(Format::Pdf), None).unwrap();
    assert_eq!(out, p.with_extension("pdf"));
    assert!(out.exists());
}

#[test]
fn pdf_has_pages() {
    let (d, p) = simple_eml();
    let out = d.path().join("pages.pdf");
    convert(&p, Some(&out), &opts(Format::Pdf), None).unwrap();
    assert!(!lopdf::Document::load(&out).unwrap().get_pages().is_empty());
}

#[test]
fn cc_email_to_pdf() {
    let (d, p) = cc_eml();
    let out = d.path().join("cc.pdf");
    convert(&p, Some(&out), &opts(Format::Pdf), None).unwrap();
    assert!(out.exists());
}

#[test]
fn simple_to_jpg() {
    let (d, p) = simple_eml();
    let out = d.path().join("output.jpg");
    convert(&p, Some(&out), &opts(Format::Jpg), None).unwrap();
    assert_eq!(magic(&out, 2), [0xFF, 0xD8]);
}

#[test]
fn legoland_to_jpg() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.jpg");
    convert(&legoland(), Some(&out), &opts(Format::Jpg), None).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);
}

#[test]
fn default_jpg_path() {
    let (_d, p) = simple_eml();
    let out = convert(&p, None, &opts(Format::Jpg), None).unwrap();
    assert_eq!(out, p.with_extension("jpg"));
    assert!(out.exists());
}

#[test]
fn simple_to_png() {
    let (d, p) = simple_eml();
    let out = d.path().join("output.png");
    convert(&p, Some(&out), &opts(Format::Png), None).unwrap();
    assert_eq!(magic(&out, 4), b"\x89PNG");
}

#[test]
fn legoland_to_png() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.png");
    convert(&legoland(), Some(&out), &opts(Format::Png), None).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);
}

#[test]
fn default_png_path() {
    let (_d, p) = simple_eml();
    let out = convert(&p, None, &opts(Format::Png), None).unwrap();
    assert_eq!(out, p.with_extension("png"));
    assert!(out.exists());
}

#[test]
fn simple_to_html() {
    let (d, p) = simple_eml();
    let out = d.path().join("output.html");
    convert(&p, Some(&out), &opts(Format::Html), None).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("<!DOCTYPE html>"));
    assert!(text.contains("Test Email"));
}

#[test]
fn legoland_to_html() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.html");
    convert(&legoland(), Some(&out), &opts(Format::Html), None).unwrap();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .to_lowercase()
            .contains("legoland")
    );
}

#[test]
fn default_html_path() {
    let (_d, p) = simple_eml();
    let out = convert(&p, None, &opts(Format::Html), None).unwrap();
    assert_eq!(out, p.with_extension("html"));
}

#[test]
fn missing_file_errors() {
    let d = tempfile::tempdir().unwrap();
    let err = convert(
        &d.path().join("nonexistent.eml"),
        None,
        &opts(Format::Pdf),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn wrong_extension_errors() {
    let d = tempfile::tempdir().unwrap();
    let bad = d.path().join("test.txt");
    std::fs::write(&bad, "not an eml").unwrap();
    let err = convert(&bad, None, &opts(Format::Pdf), None).unwrap_err();
    assert!(matches!(err, Error::NotEml(_)));
    assert!(err.to_string().starts_with("Expected .eml"));
}
