from __future__ import annotations

from pathlib import Path

from .parser import parse_eml
from .renderer import build_html, render_jpg, render_pdf


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

    ext = ".jpg" if fmt == "jpg" else ".pdf"
    if output_path is None:
        output_path = input_path.with_suffix(ext)
    else:
        output_path = Path(output_path)

    eml = parse_eml(input_path)
    html = build_html(eml)

    if fmt == "jpg":
        render_jpg(html, output_path, quality=quality)
    else:
        render_pdf(html, output_path)

    return output_path
