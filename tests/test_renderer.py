from __future__ import annotations

from doteml.parser import Attachment, ParsedEmail
from doteml.renderer import build_html


class TestBuildHtml:
    def test_contains_subject(self) -> None:
        eml = ParsedEmail(subject="Test Subject")
        html = build_html(eml)
        assert "Test Subject" in html

    def test_contains_from(self) -> None:
        eml = ParsedEmail(from_="sender@example.com")
        html = build_html(eml)
        assert "sender@example.com" in html

    def test_escapes_html(self) -> None:
        eml = ParsedEmail(subject="<script>alert('xss')</script>")
        html = build_html(eml)
        assert "<script>" not in html
        assert "&lt;script&gt;" in html

    def test_cc_row_present(self) -> None:
        eml = ParsedEmail(cc="cc@example.com")
        html = build_html(eml)
        assert "cc@example.com" in html
        assert "Cc" in html

    def test_cc_row_absent(self) -> None:
        eml = ParsedEmail()
        html = build_html(eml)
        assert "Cc" not in html

    def test_body_included(self) -> None:
        eml = ParsedEmail(html_body="<p>Hello world</p>")
        html = build_html(eml)
        assert "<p>Hello world</p>" in html

    def test_cid_exact_match(self) -> None:
        eml = ParsedEmail(
            html_body='<img src="cid:a"><img src="CID:ab">',
            attachments=[
                Attachment("a.png", "image/png", b"A", content_id="a"),
                Attachment("ab.png", "image/png", b"B", content_id="ab"),
            ],
        )
        html = build_html(eml)
        assert 'src="data:image/png;base64,QQ=="' in html
        assert 'src="data:image/png;base64,Qg=="' in html
        assert "cid:" not in html.lower()

    def test_strips_meta_refresh_and_base(self) -> None:
        eml = ParsedEmail(
            html_body='<meta http-equiv="refresh" content="0;url=https://evil.test">'
            '<base href="https://evil.test/"><p>ok</p>'
        )
        html = build_html(eml)
        assert "refresh" not in html
        assert "<base" not in html

    def test_csp_blocks_remote_by_default(self) -> None:
        assert "img-src data:;" in build_html(ParsedEmail())
        assert "https:" in build_html(ParsedEmail(), allow_remote=True)

    def test_metadata_rows(self) -> None:
        eml = ParsedEmail(
            message_id="<id@example.com>",
            reply_to="r@example.com",
            attachments=[Attachment("doc.pdf", "application/pdf", b"12345")],
        )
        html = build_html(eml)
        assert "&lt;id@example.com&gt;" in html
        assert "r@example.com" in html
        assert "doc.pdf (5 bytes)" in html
