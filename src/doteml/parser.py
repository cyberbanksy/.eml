from __future__ import annotations

import email
import email.policy
from dataclasses import dataclass, field
from pathlib import Path


@dataclass(slots=True)
class Attachment:
    filename: str
    content_type: str
    data: bytes
    content_id: str | None = None


@dataclass(slots=True)
class ParsedEmail:
    subject: str = ""
    from_: str = ""
    to: str = ""
    cc: str = ""
    date: str = ""
    html_body: str = ""
    text_body: str = ""
    attachments: list[Attachment] = field(default_factory=list)

    @property
    def body_html(self) -> str:
        if self.html_body:
            return self.html_body
        if self.text_body:
            import html as html_mod

            escaped = html_mod.escape(self.text_body)
            return f"<pre style='white-space:pre-wrap;font-family:monospace'>{escaped}</pre>"
        return "<p><em>No content</em></p>"

    @property
    def inline_images(self) -> dict[str, Attachment]:
        return {
            a.content_id: a
            for a in self.attachments
            if a.content_id and a.content_type.startswith("image/")
        }


def parse_eml(path: Path) -> ParsedEmail:
    raw = path.read_bytes()
    msg = email.message_from_bytes(raw, policy=email.policy.default)

    parsed = ParsedEmail(
        subject=str(msg.get("Subject", "")),
        from_=str(msg.get("From", "")),
        to=str(msg.get("To", "")),
        cc=str(msg.get("Cc", "") or ""),
        date=str(msg.get("Date", "")),
    )

    if msg.is_multipart():
        for part in msg.walk():
            ct = part.get_content_type()
            disp = str(part.get("Content-Disposition", ""))
            if ct == "text/html" and "attachment" not in disp:
                content = part.get_content()
                parsed.html_body = (
                    content
                    if isinstance(content, str)
                    else content.decode("utf-8", errors="replace")
                )
            elif (
                ct == "text/plain"
                and "attachment" not in disp
                and not parsed.text_body
            ):
                content = part.get_content()
                parsed.text_body = (
                    content
                    if isinstance(content, str)
                    else content.decode("utf-8", errors="replace")
                )
            elif part.get_content_maintype() in (
                "image",
                "application",
                "audio",
                "video",
            ):
                try:
                    content = part.get_content()
                except Exception:
                    continue
                if isinstance(content, bytes):
                    cid = part.get("Content-ID", "")
                    if cid:
                        cid = cid.strip("<>")
                    parsed.attachments.append(
                        Attachment(
                            filename=part.get_filename() or "attachment",
                            content_type=ct,
                            data=content,
                            content_id=cid or None,
                        )
                    )
    else:
        ct = msg.get_content_type()
        content = msg.get_content()
        text = (
            content
            if isinstance(content, str)
            else (
                content.decode("utf-8", errors="replace")
                if isinstance(content, bytes)
                else str(content)
            )
        )
        if ct == "text/html":
            parsed.html_body = text
        elif ct == "text/plain":
            parsed.text_body = text

    return parsed
