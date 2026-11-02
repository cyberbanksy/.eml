//! Check that a doteml PDF still matches the `.eml` embedded inside it.

use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::parser::parse_eml_bytes;
use crate::pdfmeta::{pdf_text_decode, sha256_hex};

#[derive(Debug, Serialize)]
pub struct HeaderCheck {
    pub key: String,
    pub recorded: String,
    pub parsed: String,
    pub matches: bool,
}

#[derive(Debug, Serialize)]
pub struct VerifyReport {
    pub pdf: PathBuf,
    pub embedded_file_name: String,
    pub embedded_size: usize,
    pub recorded_sha256: Option<String>,
    pub computed_sha256: String,
    pub hash_matches: bool,
    pub header_checks: Vec<HeaderCheck>,
    pub ok: bool,
    #[serde(skip)]
    pub eml: Vec<u8>,
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

fn dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match deref(doc, obj) {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

/// (name, filespec) pairs from the EmbeddedFiles name tree (tolerates one level of /Kids).
fn name_tree<'a>(doc: &'a Document, node: &'a Dictionary, out: &mut Vec<(String, &'a Dictionary)>) {
    if let Ok(Object::Array(names)) = node.get(b"Names").map(|o| deref(doc, o)) {
        for pair in names.chunks(2) {
            if let [name, spec] = pair
                && let Some(spec) = dict(doc, spec)
            {
                out.push((pdf_text_decode(deref(doc, name)).unwrap_or_default(), spec));
            }
        }
    }
    if let Ok(Object::Array(kids)) = node.get(b"Kids").map(|o| deref(doc, o)) {
        for kid in kids {
            if let Some(kid) = dict(doc, kid) {
                name_tree(doc, kid, out);
            }
        }
    }
}

/// The embedded `.eml` (file name, bytes), if this PDF has one.
pub fn extract_embedded_eml(doc: &Document) -> Result<Option<(String, Vec<u8>)>> {
    let catalog = doc.catalog()?;
    let Some(tree) = catalog
        .get(b"Names")
        .ok()
        .and_then(|n| dict(doc, n))
        .and_then(|n| n.get(b"EmbeddedFiles").ok())
        .and_then(|t| dict(doc, t))
    else {
        return Ok(None);
    };
    let mut entries = Vec::new();
    name_tree(doc, tree, &mut entries);
    let chosen = entries
        .iter()
        .find(|(_, spec)| matches!(spec.get(b"AFRelationship"), Ok(Object::Name(n)) if n == b"Source"))
        .or_else(|| entries.first());
    let Some((name, spec)) = chosen else {
        return Ok(None);
    };
    let Some(ef) = spec.get(b"EF").ok().and_then(|o| dict(doc, o)) else {
        return Ok(None);
    };
    let stream_ref = ef.get(b"F").or_else(|_| ef.get(b"UF"))?;
    let Object::Stream(stream) = deref(doc, stream_ref) else {
        return Ok(None);
    };
    let bytes = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    let name = spec
        .get(b"UF")
        .ok()
        .and_then(|o| pdf_text_decode(deref(doc, o)))
        .unwrap_or_else(|| name.clone());
    Ok(Some((name, bytes)))
}

pub fn verify_pdf(path: &Path) -> Result<VerifyReport> {
    let doc = Document::load(path)?;
    let Some((name, bytes)) = extract_embedded_eml(&doc)? else {
        return Err(Error::Verify(
            "no embedded .eml (not a doteml PDF, or it was stripped)".into(),
        ));
    };
    let info = doc.trailer.get(b"Info").ok().and_then(|o| dict(&doc, o));
    let recorded = |key: &[u8]| -> Option<String> {
        info.and_then(|i| i.get(key).ok())
            .and_then(|o| pdf_text_decode(deref(&doc, o)))
    };

    let computed = sha256_hex(&bytes);
    let recorded_sha = recorded(b"EmlSHA256");
    let hash_matches = recorded_sha
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case(&computed));

    let eml = parse_eml_bytes(&bytes);
    let header_checks: Vec<HeaderCheck> = [
        ("EmailFrom", &eml.from),
        ("EmailTo", &eml.to),
        ("EmailCc", &eml.cc),
        ("EmailDate", &eml.date),
        ("EmailMessageID", &eml.message_id),
    ]
    .into_iter()
    .map(|(key, parsed)| {
        let rec = recorded(key.as_bytes()).unwrap_or_default();
        HeaderCheck {
            key: key.into(),
            matches: &rec == parsed,
            recorded: rec,
            parsed: parsed.clone(),
        }
    })
    .collect();

    let ok = hash_matches && header_checks.iter().all(|c| c.matches);
    Ok(VerifyReport {
        pdf: path.to_path_buf(),
        embedded_file_name: name,
        embedded_size: bytes.len(),
        recorded_sha256: recorded_sha,
        computed_sha256: computed,
        hash_matches,
        header_checks,
        ok,
        eml: bytes,
    })
}
