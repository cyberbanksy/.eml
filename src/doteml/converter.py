from __future__ import annotations

from pathlib import Path

from .parser import parse_eml
from .renderer import build_html, render_jpg, render_pdf, render_png

FORMATS = ("pdf", "png", "jpg", "html")
_EXT_MAP = {"pdf": ".pdf", "png": ".png", "jpg": ".jpg", "html": ".html"}


def convert(
    input_path: str | Path,
    output_path: str | Path | None = None,
    *,
    fmt: str = "pdf",
    quality: int = 85,
) -> Path:
    input_path = Path(input_path)
    if not input_path.exists():
        raise FileNotFoundError(f"File not found: {input_path}")
    if input_path.suffix.lower() != ".eml":
        raise ValueError(f"Expected .eml file, got: {input_path.suffix}")
    if fmt not in FORMATS:
        raise ValueError(f"Unsupported format: {fmt} (expected one of {FORMATS})")

    if output_path is None:
        output_path = input_path.with_suffix(_EXT_MAP[fmt])
    else:
        output_path = Path(output_path)

    eml = parse_eml(input_path)
    html = build_html(eml)

    if fmt == "pdf":
        render_pdf(html, output_path)
    elif fmt == "png":
        render_png(html, output_path)
    elif fmt == "jpg":
        render_jpg(html, output_path, quality=quality)
    elif fmt == "html":
        output_path.write_text(html, encoding="utf-8")

    return output_path
