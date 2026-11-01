<div align="center">

# .eml

**Convert `.eml` files to PDF, PNG, JPG, or HTML. No system dependencies. One command.**

[![CI](https://img.shields.io/github/actions/workflow/status/cyberbanksy/.eml/ci.yml?branch=main&style=flat-square)](https://github.com/cyberbanksy/.eml/actions)
[![PyPI](https://img.shields.io/pypi/v/doteml?style=flat-square)](https://pypi.org/project/doteml/)
[![Python 3.12+](https://img.shields.io/badge/python-3.12+-3776ab?style=flat-square)](https://python.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

</div>

---

Other converters need Cairo, Pango, or WeasyPrint. **doteml** uses Playwright — one `pip install`, works everywhere.

## Install

```bash
pip install doteml
playwright install chromium
```

<details>
<summary>pipx / uv</summary>

```bash
pipx install doteml && playwright install chromium
```

```bash
uv tool install doteml && playwright install chromium
```

</details>

## Usage

```bash
doteml message.eml                   # PDF (default)
doteml message.eml -f png            # PNG
doteml message.eml -f jpg            # JPG
doteml message.eml -f html           # HTML
doteml *.eml                         # batch
```

Shorthand commands are also installed:

```bash
eml2pdf message.eml
eml2png message.eml
eml2jpg message.eml
eml2html message.eml
```

## Output formats

| Format | Best for |
|--------|----------|
| **PDF** | Archiving, printing, sharing — universal |
| **PNG** | Lossless screenshots, uploads requiring images |
| **JPG** | Smaller files, photo-oriented platforms |
| **HTML** | Browser viewing, further editing |

## Right-click integration

One command installs OS-native right-click support:

```bash
doteml install
```

| OS | What it does |
|----|-------------|
| **macOS** | Adds Finder Quick Action (right-click → Quick Actions → Convert EML to PDF) |
| **Windows** | Adds Send To shortcut (right-click → Send To → Convert EML to PDF) |
| **Linux** | Adds .desktop action for Nautilus / Dolphin / Thunar |

Remove with `doteml uninstall`.

## Options

```
doteml [OPTIONS] FILES...

  -o, --output PATH     Output path (single file only)
  -f, --format FORMAT   pdf | png | jpg | html (default: pdf)
  -q, --quality INT     JPG quality 1-100 (default: 85)
      --version         Show version
      --help            Show help
```

## How it works

1. Parses `.eml` with Python's `email` module (handles multipart, encodings, inline images)
2. Builds an HTML document with email headers (From, To, Subject, Date, CC)
3. Embeds inline `cid:` images as data URIs
4. Renders via headless Chromium to PDF, PNG, or JPG — or saves the HTML directly

## Why not WeasyPrint?

WeasyPrint needs Cairo, Pango, and GDK-PixBuf as system dependencies. On macOS that means Homebrew. On Windows it's painful. On some Linux distros it conflicts with system packages.

Playwright bundles its own Chromium — `playwright install chromium` and you're done. It also renders complex HTML emails more accurately (same engine as Chrome).

## Development

```bash
git clone https://github.com/cyberbanksy/.eml.git && cd .eml
pip install -e ".[dev]"
playwright install chromium
pytest
```

## License

MIT
