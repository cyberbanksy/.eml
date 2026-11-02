use std::collections::BTreeMap;
use std::path::Path;

use mail_parser::{
    Address, HeaderName, Message, MessageParser, MessagePart, MimeHeaders, PartType,
};

use crate::error::Result;
use crate::html::escape;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedEmail {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub reply_to: String,
    pub date: String,
    pub message_id: String,
    pub html_body: String,
    pub text_body: String,
    pub attachments: Vec<Attachment>,
    /// Parsed Date header, kept for PDF metadata.
    pub date_parts: Option<mail_parser::DateTime>,
}

impl ParsedEmail {
    pub fn body_html(&self) -> String {
        if !self.html_body.is_empty() {
            return self.html_body.clone();
        }
        if !self.text_body.is_empty() {
            return format!(
                "<pre style='white-space:pre-wrap;font-family:monospace'>{}</pre>",
                escape(&self.text_body)
            );
        }
        "<p><em>No content</em></p>".to_string()
    }

    pub fn inline_images(&self) -> BTreeMap<String, &Attachment> {
        self.attachments
            .iter()
            .filter(|a| a.content_type.starts_with("image/"))
            .filter_map(|a| a.content_id.clone().map(|cid| (cid, a)))
            .collect()
    }

    /// Attachments a reader should be told about (excludes inline cid images).
    pub fn listed_attachments(&self) -> Vec<&Attachment> {
        let inline = self.inline_images();
        self.attachments
            .iter()
            .filter(|a| !inline.values().any(|i| std::ptr::eq(*i, *a)))
            .collect()
    }
}

pub fn parse_eml(path: &Path) -> Result<ParsedEmail> {
    Ok(parse_eml_bytes(&std::fs::read(path)?))
}

pub fn parse_eml_bytes(raw: &[u8]) -> ParsedEmail {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return ParsedEmail {
            text_body: String::from_utf8_lossy(raw).into_owned(),
            ..Default::default()
        };
    };

    let mut parsed = ParsedEmail {
        subject: msg.subject().unwrap_or_default().to_string(),
        from: address_header(&msg, HeaderName::From),
        to: address_header(&msg, HeaderName::To),
        cc: address_header(&msg, HeaderName::Cc),
        bcc: address_header(&msg, HeaderName::Bcc),
        reply_to: address_header(&msg, HeaderName::ReplyTo),
        date: raw_header(&msg, HeaderName::Date),
        message_id: msg
            .message_id()
            .map(|id| format!("<{id}>"))
            .unwrap_or_default(),
        date_parts: msg.date().cloned(),
        ..Default::default()
    };

    let mut leaves = Vec::new();
    walk(&msg, 0, &mut leaves);
    for id in leaves {
        classify(&msg.parts[id], &mut parsed);
    }
    parsed
}

/// Collect leaf part ids without descending into attached messages.
fn walk(msg: &Message<'_>, id: usize, out: &mut Vec<usize>) {
    let Some(part) = msg.parts.get(id) else {
        return;
    };
    match &part.body {
        PartType::Multipart(children) => {
            for child in children {
                walk(msg, *child as usize, out);
            }
        }
        _ => out.push(id),
    }
}

fn content_type(part: &MessagePart<'_>) -> String {
    part.content_type()
        .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("")).to_ascii_lowercase())
        .unwrap_or_else(|| "text/plain".to_string())
}

fn classify(part: &MessagePart<'_>, parsed: &mut ParsedEmail) {
    let ct = content_type(part);
    let filename = part.attachment_name().map(str::to_string);
    let disposition = part
        .content_disposition()
        .map(|d| d.ctype().to_ascii_lowercase());
    let is_attachment = disposition.as_deref() == Some("attachment")
        || (filename.is_some() && !ct.starts_with("image/"));
    let is_message = matches!(part.body, PartType::Message(_));

    if !is_attachment && !is_message && ct == "text/html" && parsed.html_body.is_empty() {
        parsed.html_body = text_of(part);
    } else if !is_attachment && !is_message && ct == "text/plain" && parsed.text_body.is_empty() {
        parsed.text_body = text_of(part);
    } else {
        let default_name = if is_message || ct.starts_with("message/") {
            "forwarded.eml"
        } else {
            "attachment"
        };
        parsed.attachments.push(Attachment {
            filename: filename.unwrap_or_else(|| default_name.to_string()),
            content_type: ct,
            data: part.contents().to_vec(),
            content_id: part
                .content_id()
                .map(|s| s.trim().trim_matches(['<', '>']).to_string())
                .filter(|s| !s.is_empty()),
        });
    }
}

fn text_of(part: &MessagePart<'_>) -> String {
    match &part.body {
        PartType::Text(t) | PartType::Html(t) => t.to_string(),
        _ => String::from_utf8_lossy(part.contents()).into_owned(),
    }
}

fn raw_header(msg: &Message<'_>, name: HeaderName<'_>) -> String {
    msg.header_raw(name).map(unfold).unwrap_or_default()
}

/// Collapse folded header lines and trim, like Python's `str(msg[name])`.
fn unfold(raw: &str) -> String {
    raw.split(['\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn address_header(msg: &Message<'_>, name: HeaderName<'_>) -> String {
    let Some(value) = msg.header(name.clone()) else {
        return String::new();
    };
    match value.as_address() {
        Some(addr) => format_address(addr),
        None => value
            .as_text()
            .map(str::to_string)
            .unwrap_or_else(|| raw_header(msg, name)),
    }
}

fn format_addr(name: Option<&str>, email: Option<&str>) -> String {
    let name = name.map(|n| {
        if n.contains([',', ';', ':', '<', '>', '@', '"']) {
            format!("\"{}\"", n.replace('"', "\\\""))
        } else {
            n.to_string()
        }
    });
    match (name, email) {
        (Some(n), Some(e)) => format!("{n} <{e}>"),
        (None, Some(e)) => e.to_string(),
        (Some(n), None) => n,
        (None, None) => String::new(),
    }
}

fn format_address(addr: &Address<'_>) -> String {
    match addr {
        Address::List(list) => list
            .iter()
            .map(|a| format_addr(a.name(), a.address()))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Address::Group(groups) => groups
            .iter()
            .map(|g| {
                let inner = g
                    .addresses
                    .iter()
                    .map(|a| format_addr(a.name(), a.address()))
                    .collect::<Vec<_>>()
                    .join(", ");
                match &g.name {
                    Some(n) => format!("{n}: {inner};"),
                    None => inner,
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}
