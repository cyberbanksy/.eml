# Contributing

## Setup

```bash
git clone https://github.com/cyberbanksy/.eml.git && cd .eml
cargo build
cargo run -- doctor
```

You need Rust 1.88+ and, for the Chrome backend tests, Chrome/Chromium/Edge (set `DOTEML_BROWSER=/path/to/chrome` to pick one).

## Tests

```bash
cargo test                 # everything for this OS
cargo test --test e2e      # one file
```

The test suite renders real emails, including `tests/fixtures/legoland.eml`. `tests/webkit.rs` only compiles on macOS, where it drives the built binary with `--backend webkit` (WebKit must run on the process main thread, which `cargo test` worker threads aren't). Type-check the macOS code from Linux with:

```bash
rustup target add aarch64-apple-darwin
cargo clippy --target aarch64-apple-darwin --all-targets -- -D warnings
```

## Fuzzing

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run eml fuzz/corpus/eml tests/fixtures   # email parsing and page building
cargo +nightly fuzz run pdf                                  # embedded-.eml extraction from PDFs
```

CI runs each target for a minute on every push.

## Lint

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
```

## Pull requests

- One feature or fix per PR
- Add tests for new functionality
- Keep commits short and imperative ("Add PNG output", not "Added PNG output support")

## Releases

Tag `vX.Y.Z` (matching `Cargo.toml`) and push the tag, or publish a release with a new
`vX.Y.Z` tag on `main` from GitHub's Releases page; cargo-dist builds the archives,
installers and the MSI, creates the GitHub Release, and commits `Formula/doteml.rb` to
`main` so `brew tap cyberbanksy/eml https://github.com/cyberbanksy/.eml` works. The
`publish-homebrew-formula` job in `.github/workflows/release.yml` is hand-adjusted for
that same-repo tap (`ref: main`, `GITHUB_TOKEN`), and the `host` job uploads into an
existing release instead of failing. The `build-local-artifacts` job also sets
`CODESIGN_OPTIONS: runtime` (hardened runtime, needed for notarization), and the global
build uploads `steps.cargo-cyclonedx.outputs.paths` (cargo-dist writes `output`). For
least privilege the workflow's default permission is `contents: read`, with `contents: write`
only on `host` and `publish-homebrew-formula`; actions are pinned to commit SHAs; and each
build job copies its attestation bundle to `doteml-<target>.intoto.jsonl` so it ships with
the release. Re-apply those edits if you ever run `dist generate` again.

### macOS signing and notarization

Releases are signed and notarized only when these repository secrets exist (Settings →
Secrets and variables → Actions); without them the binaries ship unsigned.

| Secret | Value |
|---|---|
| `CODESIGN_CERTIFICATE` | Your *Developer ID Application* certificate exported as `.p12`, base64-encoded (`base64 -i cert.p12 \| pbcopy`) |
| `CODESIGN_CERTIFICATE_PASSWORD` | The `.p12` export password |
| `CODESIGN_IDENTITY` | The certificate's name, e.g. `Developer ID Application: Jane Doe (TEAMID1234)` |
| `APPLE_API_KEY` | Contents of an App Store Connect API key (`AuthKey_XXXX.p8`, role *Developer*) |
| `APPLE_API_KEY_ID` | That key's ID |
| `APPLE_API_ISSUER` | The issuer ID shown above the key list in App Store Connect |

cargo-dist signs during the build; `.github/workflows/notarize.yml` then submits the
macOS archives to Apple once the release workflow succeeds (or run it by hand with a tag).
