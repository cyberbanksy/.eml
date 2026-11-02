//! Write email metadata into the PDF and embed the original `.eml`.

use std::path::Path;

use lopdf::{Dictionary, Document, Object, Stream, StringFormat};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::parser::ParsedEmail;

pub struct EmlSource<'a> {
    pub bytes: &'a [u8],
    /// Base name of the input file, e.g. `message.eml`.
    pub file_name: &'a str,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `D:YYYYMMDDHHmmSS+HH'mm'` from the email's Date header.
pub(crate) fn pdf_date(date: Option<&mail_parser::DateTime>) -> Option<String> {
    let d = date?;
    if d.year == 0 || d.month == 0 || d.day == 0 {
        return None;
    }
    let sign = if d.tz_before_gmt { '-' } else { '+' };
    Some(format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}{sign}{:02}'{:02}'",
        d.year, d.month, d.day, d.hour, d.minute, d.second, d.tz_hour, d.tz_minute
    ))
}

/// PDF text string: ASCII as a literal, anything else as UTF-16BE with a BOM.
pub(crate) fn pdf_text(s: &str) -> Object {
    if s.is_ascii() {
        return Object::String(s.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut bytes = vec![0xFE, 0xFF];
    for unit in s.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

pub fn pdf_text_decode(o: &Object) -> Option<String> {
    let Object::String(bytes, _) = o else {
        return None;
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Some(String::from_utf8_lossy(rest).into_owned());
    }
    // PDFDocEncoding is close enough to Latin-1 for the values we write (ASCII).
    Some(bytes.iter().map(|&b| b as char).collect())
}

fn ascii_safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn write_pdf_metadata(pdf: &Path, eml: &ParsedEmail, source: EmlSource<'_>) -> Result<()> {
    let mut doc = Document::load(pdf)?;
    let hash = sha256_hex(source.bytes);
    let created = pdf_date(eml.date_parts.as_ref());

    // Embedded original: EmbeddedFile stream -> Filespec -> /Names /EmbeddedFiles + /AF.
    let mut params = Dictionary::new();
    params.set("Size", Object::Integer(source.bytes.len() as i64));
    if let Some(d) = &created {
        params.set("ModDate", Object::string_literal(d.as_str()));
    }
    let mut stream_dict = Dictionary::new();
    stream_dict.set("Type", Object::Name(b"EmbeddedFile".to_vec()));
    stream_dict.set("Subtype", Object::Name(b"message/rfc822".to_vec()));
    stream_dict.set("Params", Object::Dictionary(params));
    let mut stream = Stream::new(stream_dict, source.bytes.to_vec());
    let _ = stream.compress();
    let stream_id = doc.add_object(stream);

    let mut ef = Dictionary::new();
    ef.set("F", Object::Reference(stream_id));
    ef.set("UF", Object::Reference(stream_id));
    let mut spec = Dictionary::new();
    spec.set("Type", Object::Name(b"Filespec".to_vec()));
    spec.set("F", Object::string_literal(ascii_safe(source.file_name)));
    spec.set("UF", pdf_text(source.file_name));
    spec.set(
        "Desc",
        Object::string_literal(
            "Original .eml source; SHA-256 in /EmlSHA256 of the Info dictionary",
        ),
    );
    spec.set("AFRelationship", Object::Name(b"Source".to_vec()));
    spec.set("EF", Object::Dictionary(ef));
    let spec_id = doc.add_object(spec);

    {
        let catalog = doc.catalog_mut()?;
        let mut tree = Dictionary::new();
        tree.set(
            "Names",
            Object::Array(vec![pdf_text(source.file_name), Object::Reference(spec_id)]),
        );
        let mut names = match catalog.get(b"Names") {
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        names.set("EmbeddedFiles", Object::Dictionary(tree));
        catalog.set("Names", Object::Dictionary(names));
        catalog.set("AF", Object::Array(vec![Object::Reference(spec_id)]));
    }

    // Info dictionary.
    let mut values: Vec<(&str, Object)> = Vec::new();
    let mut text = |key: &'static str, value: &str| {
        if !value.is_empty() {
            values.push((key, pdf_text(value)));
        }
    };
    text("Title", &eml.subject);
    text("Author", &eml.from);
    text("Subject", &eml.subject);
    text("Keywords", &eml.message_id);
    text("Creator", "doteml");
    text("EmailFrom", &eml.from);
    text("EmailTo", &eml.to);
    text("EmailCc", &eml.cc);
    text("EmailDate", &eml.date);
    text("EmailMessageID", &eml.message_id);
    text("EmlSHA256", &hash);
    text("EmlFileName", source.file_name);
    if let Some(d) = &created {
        values.push(("CreationDate", Object::string_literal(d.as_str())));
    }
    values.push(("EmlSize", Object::Integer(source.bytes.len() as i64)));

    let info_id = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) if doc.get_dictionary(*id).is_ok() => *id,
        _ => {
            let id = doc.add_object(Dictionary::new());
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
    };
    let info = doc.get_dictionary_mut(info_id)?;
    for (key, value) in values {
        info.set(key, value);
    }

    // Write beside the original, then swap, so a failure never leaves a truncated PDF.
    let tmp = pdf.with_extension("pdf.tmp");
    doc.save(&tmp)?;
    if std::fs::rename(&tmp, pdf).is_err() {
        std::fs::remove_file(pdf)?;
        std::fs::rename(&tmp, pdf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trip_with_emoji() {
        for s in ["plain", "Ends Tonight 🚨", "José"] {
            assert_eq!(pdf_text_decode(&pdf_text(s)).as_deref(), Some(s));
        }
    }
}
