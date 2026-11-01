from __future__ import annotations

import http.server
import threading
from collections.abc import Iterator
from pathlib import Path

import pytest
from pypdf import PdfReader

from doteml.converter import convert

pytestmark = pytest.mark.timeout(60)


def _eml(tmp_path: Path, body: str) -> Path:
    path = tmp_path / "sec.eml"
    path.write_text(
        "From: a@example.com\r\nTo: b@example.com\r\nSubject: Security\r\n"
        "Date: Mon, 1 Jan 2024 12:00:00 +0000\r\nMessage-ID: <sec@example.com>\r\n"
        'Content-Type: text/html; charset="utf-8"\r\n\r\n' + body
    )
    return path


def _pdf_text(path: Path) -> str:
    return "".join(page.extract_text() for page in PdfReader(path).pages)


@pytest.fixture
def tracker() -> Iterator[tuple[str, list[str]]]:
    hits: list[str] = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            hits.append(self.path)
            self.send_response(404)
            self.end_headers()

        def log_message(self, *args: object) -> None:
            pass

    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_port}", hits
    server.shutdown()


def test_javascript_does_not_run(tmp_path: Path) -> None:
    eml = _eml(
        tmp_path,
        '<p id="x">static text</p>'
        "<script>document.getElementById('x').textContent='JSRAN'</script>",
    )
    out = convert(eml, tmp_path / "o.pdf")
    text = _pdf_text(out)
    assert "static text" in text
    assert "JSRAN" not in text


def test_meta_refresh_cannot_replace_document(tmp_path: Path) -> None:
    eml = _eml(
        tmp_path,
        '<meta http-equiv="refresh" content="0;url=data:text/html,HIJACKED"><p>real body</p>',
    )
    text = _pdf_text(convert(eml, tmp_path / "o.pdf"))
    assert "real body" in text
    assert "HIJACKED" not in text


def test_remote_images_blocked_by_default(tmp_path: Path, tracker: tuple[str, list[str]]) -> None:
    base, hits = tracker
    eml = _eml(tmp_path, f'<img src="{base}/pixel.gif"><p>hi</p>')
    convert(eml, tmp_path / "o.png", fmt="png")
    assert hits == []


def test_remote_images_opt_in(tmp_path: Path, tracker: tuple[str, list[str]]) -> None:
    base, hits = tracker
    eml = _eml(tmp_path, f'<img src="{base}/pixel.gif"><p>hi</p>')
    convert(eml, tmp_path / "o.png", fmt="png", allow_remote=True)
    assert hits == ["/pixel.gif"]


def test_pdf_metadata(tmp_path: Path) -> None:
    out = convert(_eml(tmp_path, "<p>x</p>"), tmp_path / "o.pdf")
    meta = PdfReader(out).metadata
    assert meta is not None
    assert meta.title == "Security"
    assert meta.author == "a@example.com"
    assert meta["/EmailTo"] == "b@example.com"
    assert meta["/EmailMessageID"] == "<sec@example.com>"
    assert meta["/CreationDate"].startswith("D:20240101120000")
