from __future__ import annotations

import base64
import contextlib
import email.utils
import html as html_mod
import os
import re
from pathlib import Path
from typing import TYPE_CHECKING

from .parser import ParsedEmail

if TYPE_CHECKING:
    from playwright.sync_api import Browser, Playwright, Route

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
_REMOTE_TYPES = ("image", "stylesheet", "font")


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
    """Holds one headless Chromium for a batch of conversions."""

    def __init__(self, *, allow_remote: bool = False, timeout_ms: int = 15_000) -> None:
        self.allow_remote = allow_remote
        self.timeout_ms = timeout_ms
        self._pw: Playwright | None = None
        self._browser: Browser | None = None

    def __enter__(self) -> Renderer:
        from playwright.sync_api import Error, sync_playwright

        self._pw = sync_playwright().start()
        exe = os.environ.get("DOTEML_CHROMIUM_PATH")
        try:
            self._browser = self._pw.chromium.launch(executable_path=exe or None)
        except Error as e:
            self._pw.stop()
            if "Executable doesn't exist" in str(e):
                raise RuntimeError("Chromium not found. Run: playwright install chromium") from e
            raise
        return self

    def __exit__(self, *exc: object) -> None:
        if self._browser:
            self._browser.close()
        if self._pw:
            self._pw.stop()

    def _route(self, route: Route) -> None:
        local = route.request.url.startswith(("data:", "about:"))
        remote_ok = self.allow_remote and route.request.resource_type in _REMOTE_TYPES
        if local or remote_ok:
            route.continue_()
        else:
            route.abort()

    def render(self, html: str, output: Path, fmt: str, *, quality: int = 85) -> None:
        from playwright.sync_api import TimeoutError as PlaywrightTimeout

        assert self._browser is not None, "use Renderer as a context manager"
        context = self._browser.new_context(
            java_script_enabled=False, viewport={"width": 800, "height": 600}
        )
        try:
            context.route("**/*", self._route)
            page = context.new_page()
            # Render whatever loaded; a slow remote image shouldn't fail the job.
            with contextlib.suppress(PlaywrightTimeout):
                page.set_content(html, wait_until="load", timeout=self.timeout_ms)
            if fmt == "pdf":
                margin = "0.4in"
                page.pdf(
                    path=str(output),
                    format="Letter",
                    print_background=True,
                    margin={"top": margin, "bottom": margin, "left": margin, "right": margin},
                )
            elif fmt == "png":
                page.screenshot(path=str(output), full_page=True, type="png")
            elif fmt == "jpg":
                page.screenshot(path=str(output), full_page=True, type="jpeg", quality=quality)
            else:
                raise ValueError(f"Unsupported render format: {fmt}")
        finally:
            context.close()
