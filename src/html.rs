use std::sync::LazyLock;

use base64::Engine;
use regex::{Captures, Regex};

use crate::parser::ParsedEmail;

pub const HEADER_CSS: &str = "\
#doteml-hdr,#doteml-hdr *{box-sizing:border-box}
#doteml-hdr{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
margin:0;padding:24px 24px 0;color:#1a1a1a;line-height:1.5;background:#fff}
#doteml-hdr .eml-hdr{border-bottom:2px solid #e5e5e5;padding-bottom:16px;margin-bottom:20px}
#doteml-hdr .eml-subj{font-size:18px;font-weight:600;margin:0 0 12px;color:#111}
#doteml-hdr table{border-collapse:collapse;font-size:13px;color:#555}
#doteml-hdr td{padding:1px 0;vertical-align:top;word-break:break-word}
#doteml-hdr td.l{color:#888;font-weight:500;padding-right:12px;white-space:nowrap}
html,body,#doteml-hdr *{-webkit-print-color-adjust:exact;print-color-adjust:exact}
";

static CID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)(["'(\s=])cid:([^"'\s)>]+)"#).unwrap());
static META_REFRESH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<meta\b[^>]*http-equiv\s*=\s*["']?refresh[^>]*>"#).unwrap());
static BASE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<base\b[^>]*>").unwrap());

/// Same escaping as Python's `html.escape` (quotes included).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn csp(allow_remote: bool) -> String {
    let remote = if allow_remote { " http: https:" } else { "" };
    format!(
        "default-src 'none'; img-src data:{remote}; style-src 'unsafe-inline'{remote}; font-src data:{remote}"
    )
}

pub fn inline_cids(body: &str, eml: &ParsedEmail) -> String {
    let images: std::collections::HashMap<String, _> = eml
        .inline_images()
        .into_iter()
        .map(|(cid, att)| (cid.to_lowercase(), att))
        .collect();
    if images.is_empty() {
        return body.to_string();
    }
    CID_RE
        .replace_all(body, |caps: &Captures<'_>| {
            match images.get(&caps[2].to_lowercase()) {
                Some(att) => format!(
                    "{}data:{};base64,{}",
                    &caps[1],
                    att.content_type,
                    base64::engine::general_purpose::STANDARD.encode(&att.data)
                ),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

pub(crate) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn row(label: &str, value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    format!(
        r#"<tr><td class="l">{label}</td><td>{}</td></tr>"#,
        escape(value)
    )
}

pub fn build_html(eml: &ParsedEmail, allow_remote: bool) -> String {
    let body = inline_cids(&eml.body_html(), eml);
    let body = META_REFRESH_RE.replace_all(&body, "");
    let body = BASE_RE.replace_all(&body, "");

    let attachments = eml
        .listed_attachments()
        .iter()
        .map(|a| format!("{} ({} bytes)", a.filename, thousands(a.data.len())))
        .collect::<Vec<_>>()
        .join(", ");
    let rows = [
        row("From", &eml.from),
        row("To", &eml.to),
        row("Cc", &eml.cc),
        row("Bcc", &eml.bcc),
        row("Reply-To", &eml.reply_to),
        row("Date", &eml.date),
        row("Message-ID", &eml.message_id),
        row("Attachments", &attachments),
    ]
    .concat();

    let subject = escape(&eml.subject);
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         <title>{subject}</title>\
         <meta name=\"author\" content=\"{from}\">\
         <meta name=\"date\" content=\"{date}\">\
         <meta name=\"message-id\" content=\"{mid}\">\
         <style>{HEADER_CSS}</style></head><body>\
         <div id=\"doteml-hdr\"><div class=\"eml-hdr\">\
         <div class=\"eml-subj\">{subject}</div>\
         <table>{rows}</table>\
         </div></div>{body}</body></html>",
        csp = csp(allow_remote),
        from = escape(&eml.from),
        date = escape(&eml.date),
        mid = escape(&eml.message_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(5), "5");
        assert_eq!(thousands(1234), "1,234");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn escape_matches_python() {
        assert_eq!(
            escape(r#"<a href="x">'&'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&#x27;&amp;&#x27;&lt;/a&gt;"
        );
    }
}
