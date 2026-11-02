mod common;

use common::*;
use doteml::pdfmeta::sha256_hex;
use doteml::verify::extract_embedded_eml;
use doteml::{ConvertOptions, convert};

#[test]
fn embeds_eml_and_hash() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.pdf");
    convert(&legoland(), Some(&out), &ConvertOptions::default(), None).unwrap();
    let original = std::fs::read(legoland()).unwrap();

    let doc = lopdf::Document::load(&out).unwrap();
    let (name, bytes) = extract_embedded_eml(&doc).unwrap().expect("embedded eml");
    assert_eq!(name, "legoland.eml");
    assert_eq!(bytes, original);

    let info = pdf_info(&out);
    assert_eq!(info["EmlSHA256"], sha256_hex(&original));
    assert_eq!(info["EmlSize"], original.len().to_string());
}

#[test]
fn utf16_strings_round_trip() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.pdf");
    convert(&legoland(), Some(&out), &ConvertOptions::default(), None).unwrap();
    let info = pdf_info(&out);
    assert!(info["Title"].contains('🚨'), "{}", info["Title"]);
    assert!(
        info["Author"].starts_with("LEGOLAND®"),
        "{}",
        info["Author"]
    );
}
