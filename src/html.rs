use std::sync::LazyLock;

use base64::Engine;
use regex::{Captures, Regex};

use crate::parser::ParsedEmail;

/// Inline `cid:` images larger than this are left unresolved instead of being
/// base64-embedded, which would multiply memory use several times over.
pub const MAX_INLINE_IMAGE_BYTES: usize = 25 * 1024 * 1024;

/// Id of the frame that holds the untrusted message body.
pub const BODY_FRAME_ID: &str = "doteml-body";

pub const HEADER_CSS: &str = "\
html,body{margin:0;background:#fff}
#doteml-hdr,#doteml-hdr *{box-sizing:border-box}
#doteml-hdr{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
margin:0;padding:24px 24px 0;color:#1a1a1a;line-height:1.5;background:#fff}
#doteml-hdr .eml-hdr{border-bottom:2px solid #e5e5e5;padding-bottom:16px;margin-bottom:20px}
#doteml-hdr .eml-subj{font-size:18px;font-weight:600;margin:0 0 12px;color:#111}
#doteml-hdr table{border-collapse:collapse;font-size:13px;color:#555}
#doteml-hdr td{padding:1px 0;vertical-align:top;word-break:break-word}
#doteml-hdr td.l{color:#888;font-weight:500;padding-right:12px;white-space:nowrap}
#doteml-body{display:block;width:100%;height:150px;border:0;margin:0;padding:0}
html,body,#doteml-hdr *{-webkit-print-color-adjust:exact;print-color-adjust:exact}
";

/// Styles for the body frame itself (the message's own CSS applies on top).
const BODY_CSS: &str =
    "html,body{margin:0}*{-webkit-print-color-adjust:exact;print-color-adjust:exact}";

/// Sizes the body frame to its content when the HTML is opened in a browser.
/// During PDF/PNG rendering scripts are off and the backend runs [`RESIZE_SCRIPT`].
const RESIZE_ON_LOAD: &str = "\
var f=document.getElementById('doteml-body');\
function fit(){try{var d=f.contentDocument;\
f.style.height=Math.max(d.documentElement.scrollHeight,d.body?d.body.scrollHeight:0)+'px'}catch(e){}}\
f.addEventListener('load',fit);fit();";

/// Run by the renderer after load: fits the body frame to its content and
/// returns the full document height in CSS pixels.
pub const RESIZE_SCRIPT: &str = "\
(function(){var f=document.getElementById('doteml-body');\
if(f){var d=f.contentDocument;\
f.style.height=Math.max(d.documentElement.scrollHeight,d.body?d.body.scrollHeight:0)+'px'}\
return Math.ceil(Math.max(document.documentElement.scrollHeight,document.body?document.body.scrollHeight:0))})()";

/// Reports the body frame's load state: `complete:<child nodes>` once it is ready.
pub const FRAME_STATE_SCRIPT: &str = "\
(function(){var f=document.getElementById('doteml-body');if(!f)return 'noframe';\
try{var d=f.contentDocument;if(!d)return 'nodoc';\
return d.readyState+':'+(d.body?d.body.childNodes.length:-1)}catch(e){return 'err:'+e}})()";

static CID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)(["'(\s=])cid:([^"'\s)>]+)"#).unwrap());
static META_REFRESH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<meta\b[^>]*http-equiv\s*=\s*["']?refresh[^>]*>"#).unwrap());
static BASE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<base\b[^>]*>").unwrap());
static MIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+$").unwrap());

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

/// Header text as shown on the page: escaped, with Unicode bidi controls removed
/// so `evil.com` cannot be made to read as something else.
pub fn display(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        .collect();
    escape(&cleaned)
}

pub fn csp(allow_remote: bool, nonce: &str) -> String {
    let remote = if allow_remote { " http: https:" } else { "" };
    format!(
        "default-src 'none'; frame-src about:; script-src 'nonce-{nonce}'; \
         img-src data:{remote}; style-src 'unsafe-inline'{remote}; font-src data:{remote}"
    )
}

fn safe_mime(content_type: &str) -> &str {
    if MIME_RE.is_match(content_type) {
        content_type
    } else {
        "application/octet-stream"
    }
}

pub fn inline_cids(body: &str, eml: &ParsedEmail) -> String {
    let images: std::collections::HashMap<String, _> = eml
        .inline_images()
        .into_iter()
        .filter(|(_, att)| att.data.len() <= MAX_INLINE_IMAGE_BYTES)
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
                    escape(safe_mime(&att.content_type)),
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
        display(value)
    )
}

/// A fresh 128-bit CSP nonce from the operating system's secure RNG.
fn nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("operating system RNG unavailable");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The trusted header block from the email's metadata.
pub fn header_block(eml: &ParsedEmail) -> String {
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
    format!(
        "<div id=\"doteml-hdr\"><div class=\"eml-hdr\"><div class=\"eml-subj\">{}</div>\
         <table>{rows}</table></div></div>",
        display(&eml.subject)
    )
}

/// The untrusted message body as a standalone document (goes inside the body frame).
pub fn body_document(eml: &ParsedEmail) -> String {
    let body = inline_cids(&eml.body_html(), eml);
    let body = META_REFRESH_RE.replace_all(&body, "");
    let body = BASE_RE.replace_all(&body, "");
    format!("<style>{BODY_CSS}</style>{body}")
}

/// A page with doteml's header block and the message body in an isolated,
/// script-free frame, so nothing in the message can draw over or restyle the header.
pub fn build_html(eml: &ParsedEmail, allow_remote: bool) -> String {
    let nonce = nonce();
    let subject = display(&eml.subject);
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         <title>{subject}</title>\
         <meta name=\"author\" content=\"{from}\">\
         <meta name=\"date\" content=\"{date}\">\
         <meta name=\"message-id\" content=\"{mid}\">\
         <style>{HEADER_CSS}</style></head><body>\
         {header}\
         <iframe id=\"{BODY_FRAME_ID}\" title=\"Message body\" sandbox=\"allow-same-origin\" \
         srcdoc=\"{srcdoc}\"></iframe>\
         <script nonce=\"{nonce}\">{RESIZE_ON_LOAD}</script>\
         </body></html>",
        csp = csp(allow_remote, &nonce),
        from = display(&eml.from),
        date = display(&eml.date),
        mid = display(&eml.message_id),
        header = header_block(eml),
        srcdoc = escape(&body_document(eml)),
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

    #[test]
    fn display_strips_bidi_controls() {
        assert_eq!(display("bank\u{202E}moc.live"), "bankmoc.live");
        assert_eq!(display("a\u{2066}b\u{2069}c"), "abc");
    }

    #[test]
    fn nonces_are_random_hex() {
        let (a, b) = (nonce(), nonce());
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn safe_mime_rejects_injection() {
        assert_eq!(safe_mime("image/png"), "image/png");
        assert_eq!(safe_mime("image/svg+xml"), "image/svg+xml");
        assert_eq!(
            safe_mime(r#"image/png" onerror="x"#),
            "application/octet-stream"
        );
    }
}
