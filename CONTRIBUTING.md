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
existing release instead of failing; re-apply those edits if you ever run `dist generate`
again.
