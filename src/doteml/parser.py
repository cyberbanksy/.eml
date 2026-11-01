from __future__ import annotations

import email
import email.policy
import html as html_mod
from dataclasses import dataclass, field
from email.message import Message
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
    bcc: str = ""
    reply_to: str = ""
    date: str = ""
    message_id: str = ""
    html_body: str = ""
    text_body: str = ""
    attachments: list[Attachment] = field(default_factory=list)

    @property
    def body_html(self) -> str:
        if self.html_body:
            return self.html_body
        if self.text_body:
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

    @property
    def listed_attachments(self) -> list[Attachment]:
        """Attachments a reader should be told about (excludes inline cid images)."""
        return [a for a in self.attachments if a not in self.inline_images.values()]


def _decode_text(part: Message) -> str:
    try:
        content = part.get_content()
        if isinstance(content, str):
            return content
    except (LookupError, UnicodeError, ValueError):
        pass
    payload = part.get_payload(decode=True) or b""
    charset = part.get_content_charset() or "utf-8"
    try:
        return payload.decode(charset, errors="replace")
    except LookupError:
        return payload.decode("latin-1")


def _header(msg: Message, name: str) -> str:
    try:
        return str(msg.get(name, "") or "")
    except Exception:
        raw = msg.get_all(name, failobj=[])
        return " ".join(str(v) for v in raw)


def _walk(msg: Message):
    """Yield leaf parts, without descending into attached messages."""
    if msg.is_multipart() and msg.get_content_maintype() == "multipart":
        for sub in msg.get_payload():
            yield from _walk(sub)
    else:
        yield msg


def parse_eml(path: Path) -> ParsedEmail:
    msg = email.message_from_bytes(path.read_bytes(), policy=email.policy.default)

    parsed = ParsedEmail(
        subject=_header(msg, "Subject"),
        from_=_header(msg, "From"),
        to=_header(msg, "To"),
        cc=_header(msg, "Cc"),
        bcc=_header(msg, "Bcc"),
        reply_to=_header(msg, "Reply-To"),
        date=_header(msg, "Date"),
        message_id=_header(msg, "Message-ID"),
    )

    for part in _walk(msg):
        ct = part.get_content_type()
        is_attachment = part.get_content_disposition() == "attachment" or (
            part.get_filename() is not None and not ct.startswith("image/")
        )

        if ct == "text/html" and not is_attachment and not parsed.html_body:
            parsed.html_body = _decode_text(part)
        elif ct == "text/plain" and not is_attachment and not parsed.text_body:
            parsed.text_body = _decode_text(part)
        else:
            if part.get_content_maintype() == "message":
                inner = part.get_payload()
                data = inner[0].as_bytes() if isinstance(inner, list) and inner else part.as_bytes()
                default_name = "forwarded.eml"
            else:
                data = part.get_payload(decode=True) or b""
                default_name = "attachment"
            cid = (part.get("Content-ID") or "").strip().strip("<>") or None
            parsed.attachments.append(
                Attachment(
                    filename=part.get_filename() or default_name,
                    content_type=ct,
                    data=data,
                    content_id=cid,
                )
            )

    return parsed
