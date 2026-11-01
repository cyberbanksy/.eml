from __future__ import annotations

import base64
import html as html_mod
from pathlib import Path

from .parser import ParsedEmail

_HEADER_CSS = """\
*{box-sizing:border-box}
.eml-hdr-wrap{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;margin:0;padding:24px 24px 0;color:#1a1a1a;line-height:1.5}
.eml-hdr{border-bottom:2px solid #e5e5e5;padding-bottom:16px;margin-bottom:20px}
.eml-subj{font-size:18px;font-weight:600;margin-bottom:12px;color:#111}
.eml-meta{font-size:13px;color:#555}
.eml-meta table{border-collapse:collapse}
.eml-meta td{padding:1px 0}
.eml-meta .l{color:#888;font-weight:500;padding-right:12px;white-space:nowrap;vertical-align:top}
"""


def build_html(eml: ParsedEmail) -> str:
    body = eml.body_html
    for cid, att in eml.inline_images.items():
        data_uri = (
            f"data:{att.content_type};base64,{base64.b64encode(att.data).decode()}"
        )
        body = body.replace(f"cid:{cid}", data_uri)

    cc_row = ""
    if eml.cc:
        cc_row = (
            '<tr><td class="l">Cc</td><td>'
            + html_mod.escape(eml.cc)
            + "</td></tr>"
        )

    subject = html_mod.escape(eml.subject)
    header = (
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">"
        f"<title>{subject}</title>"
        f"<style>{_HEADER_CSS}</style></head><body>"
        '<div class="eml-hdr-wrap"><div class="eml-hdr">'
        f'<div class="eml-subj">{subject}</div>'
        '<div class="eml-meta"><table>'
        f'<tr><td class="l">From</td><td>{html_mod.escape(eml.from_)}</td></tr>'
        f'<tr><td class="l">To</td><td>{html_mod.escape(eml.to)}</td></tr>'
        f"{cc_row}"
        f'<tr><td class="l">Date</td><td>{html_mod.escape(eml.date)}</td></tr>'
        "</table></div></div></div>"
    )

    return header + body + "</body></html>"


def _find_chromium() -> str | None:
    candidates = [
        Path("/opt/pw-browsers/chromium-1243/chrome-linux64/chrome"),
        Path("/opt/pw-browsers/chromium"),
    ]
    for c in candidates:
        if c.exists():
            return str(c)
    return None


def _launch_browser(pw: object) -> object:
    exe = _find_chromium()
    kwargs: dict = {}
    if exe:
        kwargs["executable_path"] = exe
    return pw.chromium.launch(**kwargs)  # type: ignore[union-attr]


def render_pdf(html: str, output: Path) -> None:
    from playwright.sync_api import sync_playwright

    with sync_playwright() as p:
        browser = _launch_browser(p)
        page = browser.new_page()
        page.set_content(html, wait_until="networkidle")
        page.pdf(
            path=str(output),
            format="Letter",
            print_background=True,
            margin={
                "top": "0.4in",
                "bottom": "0.4in",
                "left": "0.4in",
                "right": "0.4in",
            },
        )
        browser.close()


def render_jpg(html: str, output: Path, *, quality: int = 85) -> None:
    from playwright.sync_api import sync_playwright

    with sync_playwright() as p:
        browser = _launch_browser(p)
        page = browser.new_page(viewport={"width": 800, "height": 600})
        page.set_content(html, wait_until="networkidle")
        page.screenshot(
            path=str(output), full_page=True, type="jpeg", quality=quality
        )
        browser.close()
