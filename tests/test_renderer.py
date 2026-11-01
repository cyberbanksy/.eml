from __future__ import annotations

from doteml.parser import ParsedEmail
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
