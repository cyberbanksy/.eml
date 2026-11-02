use doteml::html::build_html;
use doteml::{Attachment, ParsedEmail};

fn html(e: &ParsedEmail) -> String {
    build_html(e, false)
}

#[test]
fn contains_subject() {
    assert!(
        html(&ParsedEmail {
            subject: "Test Subject".into(),
            ..Default::default()
        })
        .contains("Test Subject")
    );
}

#[test]
fn contains_from() {
    assert!(
        html(&ParsedEmail {
            from: "sender@example.com".into(),
            ..Default::default()
        })
        .contains("sender@example.com")
    );
}

#[test]
fn escapes_html() {
    let h = html(&ParsedEmail {
        subject: "<script>alert('xss')</script>".into(),
        ..Default::default()
    });
    assert!(!h.contains("<script>"));
    assert!(h.contains("&lt;script&gt;"));
}

#[test]
fn cc_row_present() {
    let h = html(&ParsedEmail {
        cc: "cc@example.com".into(),
        ..Default::default()
    });
    assert!(h.contains("cc@example.com"));
    assert!(h.contains("Cc"));
}

#[test]
fn cc_row_absent() {
    assert!(!html(&ParsedEmail::default()).contains("Cc"));
}

#[test]
fn body_included() {
    let h = html(&ParsedEmail {
        html_body: "<p>Hello world</p>".into(),
        ..Default::default()
    });
    assert!(h.contains("<p>Hello world</p>"));
}

#[test]
fn cid_exact_match() {
    let e = ParsedEmail {
        html_body: r#"<img src="cid:a"><img src="CID:ab">"#.into(),
        attachments: vec![
            Attachment {
                filename: "a.png".into(),
                content_type: "image/png".into(),
                data: b"A".to_vec(),
                content_id: Some("a".into()),
            },
            Attachment {
                filename: "ab.png".into(),
                content_type: "image/png".into(),
                data: b"B".to_vec(),
                content_id: Some("ab".into()),
            },
        ],
        ..Default::default()
    };
    let h = html(&e);
    assert!(h.contains(r#"src="data:image/png;base64,QQ==""#));
    assert!(h.contains(r#"src="data:image/png;base64,Qg==""#));
    assert!(!h.to_lowercase().contains("cid:"));
}

#[test]
fn strips_meta_refresh_and_base() {
    let h = html(&ParsedEmail {
        html_body: r#"<meta http-equiv="refresh" content="0;url=https://evil.test"><base href="https://evil.test/"><p>ok</p>"#.into(),
        ..Default::default()
    });
    assert!(!h.contains("refresh"));
    assert!(!h.contains("<base"));
}

#[test]
fn csp_blocks_remote_by_default() {
    assert!(build_html(&ParsedEmail::default(), false).contains("img-src data:;"));
    assert!(build_html(&ParsedEmail::default(), true).contains("https:"));
}

#[test]
fn metadata_rows() {
    let e = ParsedEmail {
        message_id: "<id@example.com>".into(),
        reply_to: "r@example.com".into(),
        attachments: vec![Attachment {
            filename: "doc.pdf".into(),
            content_type: "application/pdf".into(),
            data: b"12345".to_vec(),
            content_id: None,
        }],
        ..Default::default()
    };
    let h = html(&e);
    assert!(h.contains("&lt;id@example.com&gt;"));
    assert!(h.contains("r@example.com"));
    assert!(h.contains("doc.pdf (5 bytes)"));
}
