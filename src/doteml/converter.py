from __future__ import annotations

from pathlib import Path

from .parser import parse_eml
from .renderer import Renderer, build_html, write_pdf_metadata

FORMATS = ("pdf", "png", "jpg", "html")


def convert(
    input_path: str | Path,
    output_path: str | Path | None = None,
    *,
    fmt: str = "pdf",
    quality: int = 85,
    allow_remote: bool = False,
    renderer: Renderer | None = None,
) -> Path:
    """Convert one .eml file. Pass a shared ``renderer`` to reuse a browser across files."""
    input_path = Path(input_path)
    if not input_path.exists():
        raise FileNotFoundError(f"File not found: {input_path}")
    if input_path.suffix.lower() != ".eml":
        raise ValueError(f"Expected .eml file, got: {input_path.suffix}")
    if fmt not in FORMATS:
        raise ValueError(f"Unsupported format: {fmt} (expected one of {FORMATS})")

    output_path = Path(output_path) if output_path else input_path.with_suffix(f".{fmt}")

    eml = parse_eml(input_path)
    html = build_html(eml, allow_remote=allow_remote)

    if fmt == "html":
        output_path.write_text(html, encoding="utf-8")
        return output_path

    if renderer is None:
        with Renderer(allow_remote=allow_remote) as r:
            r.render(html, output_path, fmt, quality=quality)
    else:
        renderer.render(html, output_path, fmt, quality=quality)

    if fmt == "pdf":
        write_pdf_metadata(output_path, eml)
    return output_path
