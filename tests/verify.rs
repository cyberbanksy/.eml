mod common;

use common::*;
use doteml::verify::verify_pdf;
use doteml::{ConvertOptions, convert};
use lopdf::Object;

fn legoland_pdf() -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("legoland.pdf");
    convert(&legoland(), Some(&out), &ConvertOptions::default(), None).unwrap();
    (d, out)
}

#[test]
fn verify_ok() {
    let (_d, pdf) = legoland_pdf();
    let report = verify_pdf(&pdf).unwrap();
    assert!(report.hash_matches);
    assert!(
        report.header_checks.iter().all(|c| c.matches),
        "{:?}",
        report.header_checks
    );
    assert!(report.ok);
}

#[test]
fn verify_detects_tampered_eml() {
    let (_d, pdf) = legoland_pdf();
    let mut doc = lopdf::Document::load(&pdf).unwrap();
    let ids: Vec<_> = doc
        .objects
        .iter()
        .filter(|(_, o)| matches!(o, Object::Stream(s) if matches!(s.dict.get(b"Type"), Ok(Object::Name(n)) if n == b"EmbeddedFile")))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(ids.len(), 1);
    if let Object::Stream(s) = doc.get_object_mut(ids[0]).unwrap() {
        let mut bytes = s.decompressed_content().unwrap();
        let pos = bytes.windows(9).position(|w| w == b"Bricktacu").unwrap();
        bytes[pos] = b'X';
        s.set_plain_content(bytes);
    }
    doc.save(&pdf).unwrap();
    let report = verify_pdf(&pdf).unwrap();
    assert!(!report.hash_matches);
    assert!(!report.ok);
}

#[test]
fn verify_rejects_foreign_pdf() {
    let (d, p) = simple_eml();
    let out = d.path().join("plain.pdf");
    convert(&p, Some(&out), &ConvertOptions::default(), None).unwrap();
    let mut doc = lopdf::Document::load(&out).unwrap();
    doc.catalog_mut().unwrap().remove(b"Names");
    doc.save(&out).unwrap();
    let err = verify_pdf(&out).unwrap_err();
    assert!(err.to_string().contains("no embedded .eml"));
}
