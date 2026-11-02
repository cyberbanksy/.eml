mod common;

use common::*;
use doteml::parse_eml;

#[test]
fn simple_subject() {
    let (_d, p) = simple_eml();
    assert_eq!(parse_eml(&p).unwrap().subject, "Test Email");
}

#[test]
fn simple_from() {
    let (_d, p) = simple_eml();
    assert!(parse_eml(&p).unwrap().from.contains("sender@example.com"));
}

#[test]
fn simple_to() {
    let (_d, p) = simple_eml();
    assert!(parse_eml(&p).unwrap().to.contains("recipient@example.com"));
}

#[test]
fn simple_date() {
    let (_d, p) = simple_eml();
    assert!(parse_eml(&p).unwrap().date.contains("2024"));
}

#[test]
fn simple_html_body() {
    let (_d, p) = simple_eml();
    assert!(parse_eml(&p).unwrap().html_body.contains("<h1>Hello</h1>"));
}

#[test]
fn simple_body_html() {
    let (_d, p) = simple_eml();
    assert!(
        parse_eml(&p)
            .unwrap()
            .body_html()
            .contains("<h1>Hello</h1>")
    );
}

#[test]
fn plain_text_body() {
    let (_d, p) = plaintext_eml();
    assert!(
        parse_eml(&p)
            .unwrap()
            .text_body
            .contains("plain text email")
    );
}

#[test]
fn plain_no_html() {
    let (_d, p) = plaintext_eml();
    assert_eq!(parse_eml(&p).unwrap().html_body, "");
}

#[test]
fn plain_body_html_wraps_pre() {
    let (_d, p) = plaintext_eml();
    let body = parse_eml(&p).unwrap().body_html();
    assert!(body.contains("<pre"));
    assert!(body.contains("plain text email"));
}

#[test]
fn multipart_prefers_html() {
    let (_d, p) = multipart_eml();
    assert!(
        parse_eml(&p)
            .unwrap()
            .html_body
            .contains("<h1>HTML Version</h1>")
    );
}

#[test]
fn multipart_keeps_text() {
    let (_d, p) = multipart_eml();
    assert!(
        parse_eml(&p)
            .unwrap()
            .text_body
            .contains("Plain text version")
    );
}

#[test]
fn cc_field() {
    let (_d, p) = cc_eml();
    let cc = parse_eml(&p).unwrap().cc;
    assert!(cc.contains("cc1@example.com"));
    assert!(cc.contains("cc2@example.com"));
}

#[test]
fn encoded_subject_decoded() {
    let (_d, p) = encoded_subject_eml();
    let subject = parse_eml(&p).unwrap().subject;
    assert!(subject.contains("Hello"));
    assert!(subject.contains('👋'));
}

#[test]
fn legoland_parses() {
    let e = parse_eml(&legoland()).unwrap();
    assert!(e.from.contains("LEGOLAND"));
    assert!(!e.subject.is_empty());
    assert!(!e.html_body.is_empty() || !e.text_body.is_empty());
}

#[test]
fn legoland_from_utf8_alias() {
    let e = parse_eml(&legoland()).unwrap();
    assert!(
        e.from.starts_with("LEGOLAND® California Resort"),
        "{}",
        e.from
    );
}

#[test]
fn legoland_subject() {
    let s = parse_eml(&legoland()).unwrap().subject;
    assert!(s.contains("Ends Tonight") && s.contains("Tickets"), "{s}");
}

#[test]
fn legoland_date() {
    let d = parse_eml(&legoland()).unwrap().date;
    assert!(d.contains("2026") || d.contains("Nov"), "{d}");
}

#[test]
fn legoland_message_id() {
    let id = parse_eml(&legoland()).unwrap().message_id;
    assert!(id.starts_with('<') && id.ends_with('>'), "{id}");
}

#[test]
fn unknown_charset_does_not_crash() {
    let (_d, p) = write_eml(
        "m.eml",
        "From: a@example.com\nSubject: x\nContent-Type: text/plain; charset=x-nonsense\n\nhello world\n",
    );
    assert!(parse_eml(&p).unwrap().text_body.contains("hello world"));
}

#[test]
fn forwarded_message_is_attachment() {
    let (_d, p) = write_eml(
        "m.eml",
        "From: a@example.com\nSubject: Fwd\nMIME-Version: 1.0\n\
         Content-Type: multipart/mixed; boundary=\"B\"\n\n\
         --B\nContent-Type: text/html\n\n<p>outer body</p>\n\
         --B\nContent-Type: message/rfc822\n\n\
         From: b@example.com\nSubject: inner\nContent-Type: text/html\n\n<p>INNER body</p>\n\
         --B--\n",
    );
    let e = parse_eml(&p).unwrap();
    assert!(e.html_body.contains("outer body"));
    assert!(!e.html_body.contains("INNER"));
    let names: Vec<_> = e.attachments.iter().map(|a| a.filename.as_str()).collect();
    assert_eq!(names, ["forwarded.eml"]);
}

#[test]
fn attachments_listed_in_order() {
    let (_d, p) = write_eml(
        "m.eml",
        "From: a@example.com\nSubject: att\nMIME-Version: 1.0\n\
         Content-Type: multipart/mixed; boundary=\"B\"\n\n\
         --B\nContent-Type: text/plain\n\nbody\n\
         --B\nContent-Type: text/calendar\n\
         Content-Disposition: attachment; filename=\"invite.ics\"\n\nBEGIN:VCALENDAR\n\
         --B\nContent-Type: application/pdf\n\
         Content-Disposition: attachment; filename=\"doc.pdf\"\n\
         Content-Transfer-Encoding: base64\n\nJVBERi0=\n\
         --B--\n",
    );
    let e = parse_eml(&p).unwrap();
    assert_eq!(e.text_body.trim(), "body");
    let names: Vec<_> = e
        .listed_attachments()
        .iter()
        .map(|a| a.filename.clone())
        .collect();
    assert_eq!(names, ["invite.ics", "doc.pdf"]);
}

#[test]
fn extra_headers_bcc_reply_to_message_id() {
    let (_d, p) = write_eml(
        "m.eml",
        "From: a@example.com\nTo: b@example.com\nBcc: c@example.com\n\
         Reply-To: r@example.com\nMessage-ID: <abc@example.com>\nSubject: h\n\nx\n",
    );
    let e = parse_eml(&p).unwrap();
    assert_eq!(e.bcc, "c@example.com");
    assert_eq!(e.reply_to, "r@example.com");
    assert_eq!(e.message_id, "<abc@example.com>");
}
