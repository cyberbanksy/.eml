from __future__ import annotations

from pathlib import Path

from doteml.parser import parse_eml


class TestParseSimple:
    def test_subject(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert result.subject == "Test Email"

    def test_from(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert "sender@example.com" in result.from_

    def test_to(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert "recipient@example.com" in result.to

    def test_date(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert "2024" in result.date

    def test_html_body(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert "<h1>Hello</h1>" in result.html_body

    def test_body_html_property(self, simple_eml: Path) -> None:
        result = parse_eml(simple_eml)
        assert "<h1>Hello</h1>" in result.body_html


class TestParsePlaintext:
    def test_text_body(self, plaintext_eml: Path) -> None:
        result = parse_eml(plaintext_eml)
        assert "plain text email" in result.text_body

    def test_no_html(self, plaintext_eml: Path) -> None:
        result = parse_eml(plaintext_eml)
        assert result.html_body == ""

    def test_body_html_wraps_text(self, plaintext_eml: Path) -> None:
        result = parse_eml(plaintext_eml)
        assert "<pre" in result.body_html
        assert "plain text email" in result.body_html


class TestParseMultipart:
    def test_prefers_html(self, multipart_eml: Path) -> None:
        result = parse_eml(multipart_eml)
        assert "<h1>HTML Version</h1>" in result.html_body

    def test_also_has_text(self, multipart_eml: Path) -> None:
        result = parse_eml(multipart_eml)
        assert "Plain text version" in result.text_body


class TestParseCc:
    def test_cc_field(self, eml_with_cc: Path) -> None:
        result = parse_eml(eml_with_cc)
        assert "cc1@example.com" in result.cc
        assert "cc2@example.com" in result.cc


class TestParseEncoded:
    def test_decoded_subject(self, encoded_subject_eml: Path) -> None:
        result = parse_eml(encoded_subject_eml)
        assert "Hello" in result.subject


class TestParseLegoland:
    def test_parses_real_email(self, legoland_eml: Path) -> None:
        result = parse_eml(legoland_eml)
        assert "LEGOLAND" in result.from_ or "legoland" in result.from_.lower() or "email.legoland.com" in result.from_.lower() or result.from_ != ""
        assert result.subject != ""
        assert result.html_body != "" or result.text_body != ""

    def test_subject_decoded(self, legoland_eml: Path) -> None:
        result = parse_eml(legoland_eml)
        assert "Ends Tonight" in result.subject or "Save BIG" in result.subject or "Tickets" in result.subject

    def test_has_date(self, legoland_eml: Path) -> None:
        result = parse_eml(legoland_eml)
        assert "2026" in result.date or "Nov" in result.date
