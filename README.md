<div align="center">

# .eml

**Turn `.eml` files into PDF, PNG, JPG, or HTML — from the terminal or a right-click. One small binary.**

[![CI](https://img.shields.io/github/actions/workflow/status/cyberbanksy/.eml/ci.yml?branch=main&style=flat-square)](https://github.com/cyberbanksy/.eml/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

</div>

---

- **Self-verifying PDFs** — the original `.eml` is embedded inside the PDF with its SHA-256, and `doteml verify` proves the two still match.
- **Nothing else to install** — a single Rust binary. Renders with the engine your OS already has: WebKit on macOS, Edge on Windows, Chrome/Chromium on Linux.
- **Private by default** — JavaScript off, remote images and tracking pixels blocked, redirects stripped.
- **Right-click on every OS** — Finder Quick Action, Windows *Send to*, Linux *Open With*.
- **Faithful** — real browser engines, so marketing-grade HTML email looks the way it did in your inbox.

## Install

```bash
cargo install --git https://github.com/cyberbanksy/.eml doteml
doteml doctor
```

Prebuilt binaries, a Homebrew formula, and shell/PowerShell/MSI installers are produced by the release workflow once a version is tagged.

## Usage

```bash
doteml message.eml              # -> message.pdf
doteml -f png message.eml       # -> message.png
doteml -f jpg -q 70 *.eml       # batch, JPG quality 70
doteml -f html message.eml      # standalone HTML, images inlined
doteml --remote-images news.eml # allow remote images (contacts the sender's servers)
doteml verify message.pdf       # check the PDF against the email embedded in it
```

`eml2pdf`, `eml2png`, `eml2jpg`, and `eml2html` are installed as shortcuts.

## Self-verifying PDFs

Every PDF carries:

- the **original `.eml`** as an attachment (visible in Acrobat, Preview, and `pdfdetach`),
- its **SHA-256** and size in the document properties,
- the email's **From, To, Cc, Date, and Message-ID** in the document properties, and printed on the page.

```text
$ doteml verify legoland.pdf
legoland.pdf
  embedded:   legoland.eml (39,005 bytes)
  sha256:     41beb691…3e1868 recorded
              41beb691…3e1868 computed  OK
  From:       LEGOLAND® California Resort <california@email.legoland.com>  OK
  Date:       Sun, 01 Nov 2026 16:46:52 -0600  OK
  Message-ID: <c0271ae6-…@fra3s50mta0889.xt.local>  OK
VERIFIED: the PDF carries its source email unchanged.
```

`--extract <path>` writes the embedded `.eml` back out; `--json` prints a machine-readable report. Exit codes: `0` verified, `1` mismatch, `2` not a doteml PDF.

What this proves: the PDF, the email inside it, and the headers it shows are consistent with each other, which catches corruption and naive edits. It is not a signature — someone can always regenerate a consistent PDF from a different email.

## Rendering engines

| OS | Default engine | Needs |
|----|----------------|-------|
| **macOS** | WebKit (built in) | nothing |
| **Windows** | Microsoft Edge (built in) | nothing |
| **Linux** | Chrome, Chromium, Edge, or Brave | one of those, version 110+ |

`doteml doctor` shows which engine it found. Force one with `--backend chrome` or `--backend webkit` (or `DOTEML_BACKEND`), and point at a specific browser with `DOTEML_BROWSER=/path/to/chrome`. HTML output needs no engine. On a Mac without a logged-in GUI session (SSH, launchd), use `--backend chrome`.

## Right-click integration

```bash
doteml install     # remove with: doteml uninstall
```

| OS | Where it appears |
|----|------------------|
| **macOS** | Right-click → Quick Actions → **Convert EML to PDF** |
| **Windows** | Right-click → Send to → **Convert EML to PDF** (no admin needed) |
| **Linux** | Right-click → Open With → **Convert EML to PDF** |

The action runs this binary by its full path, so it works even when `doteml` isn't on Finder's or Explorer's `PATH`.

## What's preserved

Every output starts with a header block: **Subject, From, To, Cc, Bcc, Reply-To, Date, Message-ID**, and a list of attachments (name and size). Inline `cid:` images are embedded. Headers are shown exactly as written in the email.

Attachments other than the email itself aren't rendered — keep the `.eml` (or extract it from the PDF) if you need them.

## Options

```text
doteml [OPTIONS] <FILES>...

  -f, --format <pdf|png|jpg|html>   Output format [default: pdf]
  -o, --output <PATH>               Output path (single file only)
  -q, --quality <1-100>             JPG quality [default: 85]
      --remote-images               Load remote images and styles
      --backend <auto|chrome|webkit> Rendering engine [default: auto]
  -V, --version / -h, --help
```

A failed file is reported and the batch continues; the exit code is non-zero if anything failed.

## Development

```bash
git clone https://github.com/cyberbanksy/.eml.git && cd .eml
cargo test
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
