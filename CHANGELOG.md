# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## 0.2.3 - 2026-11-09

### Added

- `doteml doctor --fix` installs the macOS Quick Action, or shows it again if it is hidden.
- Releases are published to crates.io, so `cargo install doteml` works.

### Fixed

- The macOS Quick Action now appears in Finder's right-click menu: doteml marks it as shown in the services preferences, as Automator does, instead of leaving it off under System Settings → Extensions. Actions installed by 0.2.1 or 0.2.2 are switched on the next time doteml runs, unless you turned them off. `doteml doctor` reports when the action is hidden.
- Chrome backend: an email taller than 65,535 px written as JPG left an empty file and reported success; doteml now stops with a clear error (JPEG cannot hold more), and treats any empty render from the browser as a failure.
- WebKit backend: the same JPG case, and PNGs WebKit cannot encode (around 200,000 px tall), now say why and suggest `-f pdf` or `--backend chrome` instead of "cannot encode WebKit snapshot".
- Passing a directory or a file with no extension names what was given instead of printing an empty "got:".
- The README's Homebrew lines paste cleanly in zsh; `brew trust` is required by Homebrew 7.
- Debug builds (`cargo run`, `cargo test`) no longer install a Quick Action that points at `target/debug`.

### Changed

- lopdf 0.47.

## 0.2.2 - 2026-11-05

### Fixed

- Release builds for Apple Silicon Macs run on the `macos-15` runner; GitHub no longer accepts `macos-14`, so 0.2.1 never published binaries.

## 0.2.1 - 2026-11-05

### Security

- Generate the page's CSP nonce from the operating system's secure RNG.

### Added

- On macOS, the Finder Quick Action is set up automatically on first run, and `doteml doctor` shows whether it is installed.
- Fuzz targets for email parsing and PDF verification (`cargo fuzz`), run in CI.
- Release archives ship their SLSA build-provenance bundle (`*.intoto.jsonl`) and a CycloneDX SBOM.
- macOS binaries are signed and notarized when signing secrets are configured.

## 0.2.0 - 2026-11-05

### Added

- Rewritten in Rust as one small binary, `doteml`, with `eml2pdf`, `eml2png`, `eml2jpg` and `eml2html` shortcuts.
- Native WebKit rendering on macOS, with no browser needed; Chrome, Chromium or Edge elsewhere.
- Self-verifying PDFs: the original `.eml` and its SHA-256 are embedded, and `doteml verify` checks them.
- Right-click integration: a macOS Quick Action, Windows "Send to", and a Linux desktop entry (`doteml install`).
- Installers: shell, PowerShell, MSI and Homebrew.

### Security

- The message body renders in a sandboxed, script-free frame under a strict CSP, so it cannot overlay or restyle the header block.
- Remote content is blocked unless `--allow-remote` is given.
