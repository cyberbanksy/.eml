# Security policy

doteml renders untrusted email, so security reports are very welcome.

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through GitHub:
[Security → Report a vulnerability](https://github.com/cyberbanksy/.eml/security/advisories/new).

Include the `.eml` (or a minimal one) that triggers the problem, the command you ran,
your OS, and the backend (`doteml doctor` prints it). You should get a reply within
7 days. Once a fix is released, the advisory is published with credit, unless you
prefer to stay anonymous.

## Scope

In scope: anything a crafted email can do beyond producing its own rendering, e.g.
fetching remote content without `--allow-remote`, reading local files, running script
in the header frame, spoofing the header block, writing outside the output path, or
forging a PDF that `doteml verify` accepts.

## Verifying a release

Every release archive has a GitHub build-provenance attestation and a CycloneDX SBOM
(`*.cdx.xml`), and each binary embeds its dependency list (`cargo audit bin doteml`):

```bash
gh attestation verify doteml-x86_64-unknown-linux-gnu.tar.xz --repo cyberbanksy/.eml
```

macOS binaries from signed releases carry a Developer ID signature and are notarized by Apple:

```bash
codesign --verify --strict --verbose=2 "$(command -v doteml)"
```
