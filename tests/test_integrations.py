from __future__ import annotations

import plistlib
import sys
from pathlib import Path

import pytest

from doteml import integrations


@pytest.fixture(autouse=True)
def fake_home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(Path, "home", lambda: tmp_path)
    monkeypatch.setenv("APPDATA", str(tmp_path / "AppData"))
    monkeypatch.delenv("XDG_DATA_HOME", raising=False)
    monkeypatch.setattr(integrations.shutil, "which", lambda _name: None)
    return tmp_path


def test_macos_quick_action_plists() -> None:
    workflow = integrations.install_macos()
    with open(workflow / "Contents" / "Info.plist", "rb") as f:
        info = plistlib.load(f)
    with open(workflow / "Contents" / "document.wflow", "rb") as f:
        doc = plistlib.load(f)

    service = info["NSServices"][0]
    assert service["NSMenuItem"]["default"] == "Convert EML to PDF"
    assert "public.email-message" in service["NSSendFileTypes"]
    script = doc["actions"][0]["action"]["ActionParameters"]["COMMAND_STRING"]
    assert "-m doteml" in script

    integrations.uninstall_macos()
    assert not workflow.exists()


def test_windows_sendto_forwards_all_args() -> None:
    bat = integrations.install_windows_sendto()
    content = bat.read_bytes().decode("utf-8", errors="replace")
    assert f'"{sys.executable}" -m doteml %*' in content
    assert "for %%" not in content

    integrations.uninstall_windows_sendto()
    assert not bat.exists()


def test_linux_desktop_entry_quotes_exec(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(integrations.sys, "executable", "/opt/my env/bin/python")
    desktop = integrations.install_linux()
    content = desktop.read_text()
    assert 'Exec="/opt/my env/bin/python" -m doteml %F' in content
    assert "MimeType=message/rfc822;" in content

    integrations.uninstall_linux()
    assert not desktop.exists()


def test_desktop_quote_escapes_specials() -> None:
    assert integrations._desktop_quote('a"b$c%d') == '"a\\"b\\$c%%d"'
