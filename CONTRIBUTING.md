# Contributing

## Setup

```bash
git clone https://github.com/cyberbanksy/.eml.git && cd .eml
pip install -e ".[dev]"
playwright install chromium
```

## Tests

```bash
pytest           # run all
pytest -v        # verbose
pytest -k e2e    # only E2E tests
```

To use an existing Chromium instead of Playwright's download, set `DOTEML_CHROMIUM_PATH=/path/to/chrome`.

## Lint

```bash
uvx ruff check src/ tests/
uvx ruff format src/ tests/
```

## Pull requests

- One feature or fix per PR
- Add tests for new functionality
- Keep commits short and imperative ("Add PNG output", not "Added PNG output support")
