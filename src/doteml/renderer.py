from __future__ import annotations

import base64
import email.utils
import html as html_mod
import math
import re
import tempfile
from pathlib import Path
from typing import Any

from .browser import DOWNLOAD_URL, MIN_VERSION, BrowserNotFoundError, find_browser, parse_major
from .cdp import Browser
from .parser import ParsedEmail

_HEADER_CSS = """\
#doteml-hdr,#doteml-hdr *{box-sizing:border-box}
#doteml-hdr{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
margin:0;padding:24px 24px 0;color:#1a1a1a;line-height:1.5;background:#fff}
#doteml-hdr .eml-hdr{border-bottom:2px solid #e5e5e5;padding-bottom:16px;margin-bottom:20px}
#doteml-hdr .eml-subj{font-size:18px;font-weight:600;margin:0 0 12px;color:#111}
#doteml-hdr table{border-collapse:collapse;font-size:13px;color:#555}
#doteml-hdr td{padding:1px 0;vertical-align:top;word-break:break-word}
#doteml-hdr td.l{color:#888;font-weight:500;padding-right:12px;white-space:nowrap}
"""

_CID_RE = re.compile(r"""(["'(\s=])cid:([^"'\s)>]+)""", re.IGNORECASE)
_META_REFRESH_RE = re.compile(r"<meta\b[^>]*http-equiv\s*=\s*[\"']?refresh[^>]*>", re.IGNORECASE)
_BASE_RE = re.compile(r"<base\b[^>]*>", re.IGNORECASE)
_REMOTE_TYPES = ("Image", "Stylesheet", "Font")


def _csp(allow_remote: bool) -> str:
    remote = " http: https:" if allow_remote else ""
    return (
        "default-src 'none'; "
        f"img-src data:{remote}; "
        f"style-src 'unsafe-inline'{remote}; "
        f"font-src data:{remote}"
    )


def _inline_cids(body: str, eml: ParsedEmail) -> str:
    images = {cid.lower(): att for cid, att in eml.inline_images.items()}
    if not images:
        return body

    def sub(m: re.Match[str]) -> str:
        att = images.get(m.group(2).lower())
        if att is None:
            return m.group(0)
        b64 = base64.b64encode(att.data).decode()
        return f"{m.group(1)}data:{att.content_type};base64,{b64}"

    return _CID_RE.sub(sub, body)


def _row(label: str, value: str) -> str:
    if not value:
        return ""
    return f'<tr><td class="l">{label}</td><td>{html_mod.escape(value)}</td></tr>'


def build_html(eml: ParsedEmail, *, allow_remote: bool = False) -> str:
    body = _inline_cids(eml.body_html, eml)
    body = _BASE_RE.sub("", _META_REFRESH_RE.sub("", body))

    attachments = ", ".join(f"{a.filename} ({len(a.data):,} bytes)" for a in eml.listed_attachments)
    rows = "".join(
        [
            _row("From", eml.from_),
            _row("To", eml.to),
            _row("Cc", eml.cc),
            _row("Bcc", eml.bcc),
            _row("Reply-To", eml.reply_to),
            _row("Date", eml.date),
            _row("Message-ID", eml.message_id),
            _row("Attachments", attachments),
        ]
    )

    subject = html_mod.escape(eml.subject)
    head = (
        '<!DOCTYPE html>\n<html><head><meta charset="utf-8">'
        f'<meta http-equiv="Content-Security-Policy" content="{_csp(allow_remote)}">'
        f"<title>{subject}</title>"
        f'<meta name="author" content="{html_mod.escape(eml.from_)}">'
        f'<meta name="date" content="{html_mod.escape(eml.date)}">'
        f'<meta name="message-id" content="{html_mod.escape(eml.message_id)}">'
        f"<style>{_HEADER_CSS}</style></head><body>"
        '<div id="doteml-hdr"><div class="eml-hdr">'
        f'<div class="eml-subj">{subject}</div>'
        f"<table>{rows}</table>"
        "</div></div>"
    )
    return head + body + "</body></html>"


def _pdf_date(value: str) -> str | None:
    try:
        dt = email.utils.parsedate_to_datetime(value)
    except (TypeError, ValueError):
        return None
    off = dt.strftime("%z") or "+0000"
    return dt.strftime("D:%Y%m%d%H%M%S") + f"{off[0]}{off[1:3]}'{off[3:5]}'"


def write_pdf_metadata(path: Path, eml: ParsedEmail) -> None:
    from pypdf import PdfReader, PdfWriter

    writer = PdfWriter(clone_from=PdfReader(path))
    meta = {
        "/Title": eml.subject,
        "/Author": eml.from_,
        "/Subject": eml.subject,
        "/Keywords": eml.message_id,
        "/Creator": "doteml",
        "/EmailFrom": eml.from_,
        "/EmailTo": eml.to,
        "/EmailCc": eml.cc,
        "/EmailDate": eml.date,
        "/EmailMessageID": eml.message_id,
    }
    created = _pdf_date(eml.date)
    if created:
        meta["/CreationDate"] = created
    writer.add_metadata({k: v for k, v in meta.items() if v})
    with open(path, "wb") as f:
        writer.write(f)


class Renderer:
    """Drives one headless browser for a batch of conversions."""

    def __init__(self, *, allow_remote: bool = False, timeout_ms: int = 15_000) -> None:
        self.allow_remote = allow_remote
        self.timeout_ms = timeout_ms
        self._browser: Browser | None = None
        self._loaded: set[str] = set()
        self.executable = ""
        self.product = ""

    def __enter__(self) -> Renderer:
        self.executable = find_browser()
        self._browser = Browser(self.executable)
        self.product = self._browser.send("Browser.getVersion").get("product", "")
        major = parse_major(self.product)
        if major is not None and major < MIN_VERSION:
            self._browser.close()
            raise BrowserNotFoundError(
                f"{self.product} at {self.executable} is too old (need {MIN_VERSION}+).\n"
                f"  Update it, or install Google Chrome: {DOWNLOAD_URL}"
            )
        return self

    def __exit__(self, *exc: object) -> None:
        if self._browser:
            self._browser.close()

    def _on_event(self, session: str, doc_url: str, message: dict[str, Any]) -> None:
        method = message["method"]
        if method == "Page.loadEventFired":
            self._loaded.add(session)
        elif method == "Fetch.requestPaused":
            assert self._browser is not None
            params = message["params"]
            url = params["request"]["url"]
            allowed = (
                url == doc_url
                or url.startswith(("data:", "about:"))
                or (
                    self.allow_remote
                    and url.startswith(("http:", "https:"))
                    and params.get("resourceType") in _REMOTE_TYPES
                )
            )
            if allowed:
                self._browser.notify(
                    "Fetch.continueRequest", {"requestId": params["requestId"]}, session
                )
            else:
                self._browser.notify(
                    "Fetch.failRequest",
                    {"requestId": params["requestId"], "errorReason": "BlockedByClient"},
                    session,
                )

    def render(self, html: str, output: Path, fmt: str, *, quality: int = 85) -> None:
        if fmt not in ("pdf", "png", "jpg"):
            raise ValueError(f"Unsupported render format: {fmt}")
        browser = self._browser
        assert browser is not None, "use Renderer as a context manager"

        with tempfile.TemporaryDirectory(prefix="doteml-", ignore_cleanup_errors=True) as tmp:
            doc = Path(tmp) / "email.html"
            doc.write_text(html, encoding="utf-8")
            doc_url = doc.as_uri()

            target = browser.send("Target.createTarget", {"url": "about:blank"})["targetId"]
            session = browser.send("Target.attachToTarget", {"targetId": target, "flatten": True})[
                "sessionId"
            ]
            browser.on(session, lambda m: self._on_event(session, doc_url, m))
            try:
                browser.send("Fetch.enable", {"patterns": [{"urlPattern": "*"}]}, session)
                browser.send("Page.enable", {}, session)
                browser.send("Emulation.setScriptExecutionDisabled", {"value": True}, session)
                browser.send(
                    "Emulation.setDeviceMetricsOverride",
                    {"width": 800, "height": 600, "deviceScaleFactor": 1, "mobile": False},
                    session,
                )
                browser.send("Page.navigate", {"url": doc_url}, session)
                # Render whatever loaded; a slow remote image shouldn't fail the job.
                browser.pump_until(lambda: session in self._loaded, self.timeout_ms / 1000)

                if fmt == "pdf":
                    margin = 0.4
                    data = browser.send(
                        "Page.printToPDF",
                        {
                            "printBackground": True,
                            "paperWidth": 8.5,
                            "paperHeight": 11,
                            "marginTop": margin,
                            "marginBottom": margin,
                            "marginLeft": margin,
                            "marginRight": margin,
                        },
                        session,
                    )["data"]
                else:
                    metrics = browser.send("Page.getLayoutMetrics", {}, session)
                    size = metrics.get("cssContentSize") or metrics["contentSize"]
                    clip = {
                        "x": 0,
                        "y": 0,
                        "width": max(1, math.ceil(size["width"])),
                        "height": max(1, math.ceil(size["height"])),
                        "scale": 1,
                    }
                    params: dict[str, Any] = {
                        "format": "png" if fmt == "png" else "jpeg",
                        "captureBeyondViewport": True,
                        "clip": clip,
                    }
                    if fmt == "jpg":
                        params["quality"] = quality
                    data = browser.send("Page.captureScreenshot", params, session)["data"]
                output.write_bytes(base64.b64decode(data))
            finally:
                browser.off(session)
                self._loaded.discard(session)
                browser.send("Target.closeTarget", {"targetId": target})
