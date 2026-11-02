from __future__ import annotations

import os
from pathlib import Path

import pytest
from click.testing import CliRunner

from doteml import browser
from doteml.cli import cli


def test_parse_major() -> None:
    assert browser.parse_major("HeadlessChrome/131.0.6778.85") == 131
    assert browser.parse_major("Edg/120.0.1") == 120
    assert browser.parse_major("garbage") is None


def test_env_override(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    exe = tmp_path / "chrome"
    exe.write_text("")
    monkeypatch.setenv("DOTEML_BROWSER", str(exe))
    assert browser.find_browser() == str(exe)


def test_env_override_missing(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DOTEML_BROWSER", os.path.join("nope", "chrome"))
    with pytest.raises(browser.BrowserNotFoundError, match="missing file"):
        browser.find_browser()


def test_not_found_recommends_download(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("DOTEML_BROWSER", raising=False)
    monkeypatch.setattr(browser, "candidates", lambda: [])
    with pytest.raises(browser.BrowserNotFoundError, match="google.com/chrome"):
        browser.find_browser()


def test_first_existing_candidate_wins(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    real = tmp_path / "edge"
    real.write_text("")
    monkeypatch.delenv("DOTEML_BROWSER", raising=False)
    monkeypatch.setattr(browser, "candidates", lambda: [str(tmp_path / "missing"), str(real)])
    assert browser.find_browser() == str(real)


def test_too_old_browser_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    from doteml import renderer

    class OldBrowser:
        def __init__(self, exe: str) -> None:
            self.closed = False

        def send(self, method: str, *args: object) -> dict[str, str]:
            return {"product": "HeadlessChrome/99.0.0.0"}

        def close(self) -> None:
            self.closed = True

    monkeypatch.setattr(renderer, "find_browser", lambda: "/fake/chrome")
    monkeypatch.setattr(renderer, "Browser", OldBrowser)
    with pytest.raises(browser.BrowserNotFoundError, match="too old"), renderer.Renderer():
        pass


def test_doctor_reports_missing_browser(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("DOTEML_BROWSER", raising=False)
    monkeypatch.setattr(browser, "candidates", lambda: [])
    result = CliRunner().invoke(cli, ["doctor"])
    assert result.exit_code == 1
    assert "No Chrome, Edge, or Chromium found" in result.output


@pytest.mark.timeout(60)
def test_doctor_ok() -> None:
    result = CliRunner().invoke(cli, ["doctor"])
    assert result.exit_code == 0, result.output
    assert "OK" in result.output
