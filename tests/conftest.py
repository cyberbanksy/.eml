from __future__ import annotations

import email.mime.image
import email.mime.multipart
import email.mime.text
from pathlib import Path

import pytest

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture
def fixtures_dir() -> Path:
    return FIXTURES


@pytest.fixture
def legoland_eml() -> Path:
    return FIXTURES / "legoland.eml"


@pytest.fixture
def simple_eml(tmp_path: Path) -> Path:
    path = tmp_path / "simple.eml"
    path.write_text(
        "From: sender@example.com\r\n"
        "To: recipient@example.com\r\n"
        "Subject: Test Email\r\n"
        "Date: Mon, 1 Jan 2024 12:00:00 +0000\r\n"
        "MIME-Version: 1.0\r\n"
        'Content-Type: text/html; charset="utf-8"\r\n'
        "\r\n"
        "<html><body><h1>Hello</h1><p>This is a test.</p></body></html>\r\n"
    )
    return path


@pytest.fixture
def plaintext_eml(tmp_path: Path) -> Path:
    path = tmp_path / "plain.eml"
    path.write_text(
        "From: alice@example.com\r\n"
        "To: bob@example.com\r\n"
        "Subject: Plain text email\r\n"
        "Date: Tue, 2 Jan 2024 09:30:00 +0000\r\n"
        "MIME-Version: 1.0\r\n"
        'Content-Type: text/plain; charset="utf-8"\r\n'
        "\r\n"
        "Hello Bob,\r\n\r\nThis is a plain text email.\r\n\r\nBest,\r\nAlice\r\n"
    )
    return path


@pytest.fixture
def multipart_eml(tmp_path: Path) -> Path:
    msg = email.mime.multipart.MIMEMultipart("alternative")
    msg["From"] = "sender@example.com"
    msg["To"] = "recipient@example.com"
    msg["Subject"] = "Multipart Email"
    msg["Date"] = "Wed, 3 Jan 2024 15:00:00 +0000"

    text_part = email.mime.text.MIMEText("Plain text version", "plain")
    html_part = email.mime.text.MIMEText(
        "<html><body><h1>HTML Version</h1><p>Rich content here.</p></body></html>",
        "html",
    )
    msg.attach(text_part)
    msg.attach(html_part)

    path = tmp_path / "multipart.eml"
    path.write_bytes(msg.as_bytes())
    return path


@pytest.fixture
def eml_with_cc(tmp_path: Path) -> Path:
    path = tmp_path / "cc.eml"
    path.write_text(
        "From: sender@example.com\r\n"
        "To: recipient@example.com\r\n"
        "Cc: cc1@example.com, cc2@example.com\r\n"
        "Subject: Email with CC\r\n"
        "Date: Thu, 4 Jan 2024 10:00:00 +0000\r\n"
        "MIME-Version: 1.0\r\n"
        'Content-Type: text/html; charset="utf-8"\r\n'
        "\r\n"
        "<html><body><p>CC test</p></body></html>\r\n"
    )
    return path


@pytest.fixture
def encoded_subject_eml(tmp_path: Path) -> Path:
    path = tmp_path / "encoded.eml"
    path.write_text(
        "From: sender@example.com\r\n"
        "To: recipient@example.com\r\n"
        "Subject: =?UTF-8?B?SGVsbG8g8J+RiyBXb3JsZA==?=\r\n"
        "Date: Fri, 5 Jan 2024 08:00:00 +0000\r\n"
        "MIME-Version: 1.0\r\n"
        'Content-Type: text/html; charset="utf-8"\r\n'
        "\r\n"
        "<html><body><p>Encoded subject test</p></body></html>\r\n"
    )
    return path
