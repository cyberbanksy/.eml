<div align="center">

# .eml

**Convert `.eml` files to PDF, PNG, JPG, or HTML — from the terminal or a right-click.**

[![CI](https://img.shields.io/github/actions/workflow/status/cyberbanksy/.eml/ci.yml?branch=main&style=flat-square)](https://github.com/cyberbanksy/.eml/actions)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-3776ab?style=flat-square)](https://python.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

</div>

---

- **Faithful rendering** — headless Chromium, the same engine as Chrome, so marketing-grade HTML email looks right.
- **Private by default** — JavaScript off, remote images and tracking pixels blocked, redirects stripped.
- **Verifiable** — sender, recipients, date, and Message-ID are printed on the page *and* written into the PDF's metadata.
- **Right-click on every OS** — Finder Quick Action, Windows *Send to*, Linux *Open With*.
- **No Cairo / Pango / GTK** — just `pip` and a one-time Chromium download.

## Install

```bash
pipx install git+https://github.com/cyberbanksy/.eml
playwright install chromium
```

<details>
<summary>uv / pip</summary>

```bash
uv tool install git+https://github.com/cyberbanksy/.eml
```

```bash
pip install git+https://github.com/cyberbanksy/.eml
```

Then run `playwright install chromium` once. On a bare Linux server, use `playwright install --with-deps chromium` to pull in the shared libraries Chromium needs.

</details>

## Usage

```bash
doteml message.eml              # -> message.pdf
doteml -f png message.eml       # -> message.png
doteml -f jpg -q 70 *.eml       # batch, JPG quality 70
doteml -f html message.eml      # standalone HTML, images inlined
doteml --remote-images news.eml # allow remote images (sends requests to the sender's servers)
```

`eml2pdf`, `eml2png`, `eml2jpg`, and `eml2html` are installed as shortcuts.

## Right-click integration

```bash
doteml install     # remove with: doteml uninstall
```

| OS | Where it appears |
|----|------------------|
| **macOS** | Right-click → Quick Actions → **Convert EML to PDF** |
| **Windows** | Right-click → Send to → **Convert EML to PDF** (no admin needed) |
| **Linux** | Right-click → Open With → **Convert EML to PDF** |

The action calls the same Python that installed it, so it works even when `doteml` isn't on Finder's or Explorer's `PATH`.

## Output formats

| Format | Good for |
|--------|----------|
| **PDF** | Archiving, legal/HR records, uploading to portals — carries email metadata |
| **PNG** | Lossless image of the full message |
| **JPG** | Smaller image for chat apps and forms that only accept photos |
| **HTML** | Viewing or editing in a browser; inline images embedded |

## What's preserved

Every output starts with a header block: **Subject, From, To, Cc, Bcc, Reply-To, Date, Message-ID**, and a list of attachments (name and size). Inline `cid:` images are embedded.

PDFs also carry this in their document properties, so it survives even if the page is cropped:

```
Title / Subject   message subject
Author            From
Keywords          Message-ID
CreationDate      the email's Date header
EmailFrom, EmailTo, EmailCc, EmailDate, EmailMessageID
```

Attachments themselves aren't embedded — the conversion is intentionally lossy. Keep the `.eml` if you need them.

## Options

```
doteml [OPTIONS] FILES...

  -f, --format [pdf|png|jpg|html]  Output format  [default: pdf]
  -o, --output PATH                Output path (single file only)
  -q, --quality 1-100              JPG quality  [default: 85]
      --remote-images              Load remote images and styles
      --version                    Show version
      --help                       Show help
```

A failed file is reported and the batch continues; the exit code is non-zero if anything failed.

## Development

```bash
git clone https://github.com/cyberbanksy/.eml.git && cd .eml
pip install -e ".[dev]"
playwright install chromium
pytest
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
