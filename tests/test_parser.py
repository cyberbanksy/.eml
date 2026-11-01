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
        assert "LEGOLAND" in result.from_
        assert result.subject != ""
        assert result.html_body != "" or result.text_body != ""

    def test_subject_decoded(self, legoland_eml: Path) -> None:
        result = parse_eml(legoland_eml)
        assert (
            "Ends Tonight" in result.subject
            or "Save BIG" in result.subject
            or "Tickets" in result.subject
        )

    def test_has_date(self, legoland_eml: Path) -> None:
        result = parse_eml(legoland_eml)
        assert "2026" in result.date or "Nov" in result.date

    def test_message_id(self, legoland_eml: Path) -> None:
        assert parse_eml(legoland_eml).message_id


def _write(tmp_path: Path, raw: str) -> Path:
    path = tmp_path / "m.eml"
    path.write_bytes(raw.replace("\n", "\r\n").encode())
    return path


class TestParseEdgeCases:
    def test_unknown_charset_does_not_crash(self, tmp_path: Path) -> None:
        path = _write(
            tmp_path,
            "From: a@example.com\nSubject: x\n"
            "Content-Type: text/plain; charset=x-nonsense\n\nhello world\n",
        )
        assert "hello world" in parse_eml(path).text_body

    def test_forwarded_message_does_not_replace_body(self, tmp_path: Path) -> None:
        path = _write(
            tmp_path,
            "From: a@example.com\nSubject: Fwd\nMIME-Version: 1.0\n"
            'Content-Type: multipart/mixed; boundary="B"\n\n'
            "--B\nContent-Type: text/html\n\n<p>outer body</p>\n"
            "--B\nContent-Type: message/rfc822\n\n"
            "From: b@example.com\nSubject: inner\nContent-Type: text/html\n\n<p>INNER body</p>\n"
            "--B--\n",
        )
        result = parse_eml(path)
        assert "outer body" in result.html_body
        assert "INNER" not in result.html_body
        assert [a.filename for a in result.attachments] == ["forwarded.eml"]

    def test_attachments_are_listed(self, tmp_path: Path) -> None:
        path = _write(
            tmp_path,
            "From: a@example.com\nSubject: att\nMIME-Version: 1.0\n"
            'Content-Type: multipart/mixed; boundary="B"\n\n'
            "--B\nContent-Type: text/plain\n\nbody\n"
            "--B\nContent-Type: text/calendar\n"
            'Content-Disposition: attachment; filename="invite.ics"\n\nBEGIN:VCALENDAR\n'
            "--B\nContent-Type: application/pdf\n"
            'Content-Disposition: attachment; filename="doc.pdf"\n'
            "Content-Transfer-Encoding: base64\n\nJVBERi0=\n"
            "--B--\n",
        )
        result = parse_eml(path)
        assert result.text_body.strip() == "body"
        assert [a.filename for a in result.listed_attachments] == ["invite.ics", "doc.pdf"]

    def test_extra_headers(self, tmp_path: Path) -> None:
        path = _write(
            tmp_path,
            "From: a@example.com\nTo: b@example.com\nBcc: c@example.com\n"
            "Reply-To: r@example.com\nMessage-ID: <abc@example.com>\nSubject: h\n\nx\n",
        )
        result = parse_eml(path)
        assert result.bcc == "c@example.com"
        assert result.reply_to == "r@example.com"
        assert result.message_id == "<abc@example.com>"
