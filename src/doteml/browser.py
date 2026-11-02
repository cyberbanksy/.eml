"""Find a Chromium-based browser (Chrome, Edge, Chromium, Brave) already installed."""

from __future__ import annotations

import glob
import os
import platform
import shutil
from pathlib import Path

MIN_VERSION = 110
DOWNLOAD_URL = "https://www.google.com/chrome/"

_LINUX_NAMES = (
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
    "brave-browser",
)

_MAC_APPS = (
    "Google Chrome.app/Contents/MacOS/Google Chrome",
    "Chromium.app/Contents/MacOS/Chromium",
    "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "Brave Browser.app/Contents/MacOS/Brave Browser",
)

_WIN_EXES = (
    r"Google\Chrome\Application\chrome.exe",
    r"Microsoft\Edge\Application\msedge.exe",
    r"Chromium\Application\chrome.exe",
    r"BraveSoftware\Brave-Browser\Application\brave.exe",
)


class BrowserNotFoundError(RuntimeError):
    pass


def _playwright_cache() -> list[str]:
    """Chromium that `playwright install chromium` may have downloaded."""
    roots = [os.environ.get("PLAYWRIGHT_BROWSERS_PATH", "")]
    system = platform.system()
    if system == "Darwin":
        roots.append(str(Path.home() / "Library" / "Caches" / "ms-playwright"))
    elif system == "Windows":
        roots.append(os.path.join(os.environ.get("LOCALAPPDATA", ""), "ms-playwright"))
    else:
        roots.append(str(Path.home() / ".cache" / "ms-playwright"))
    patterns = (
        "chromium-*/chrome-linux*/chrome",
        "chromium-*/chrome-mac*/Chromium.app/Contents/MacOS/Chromium",
        "chromium-*/chrome-mac*/Google Chrome for Testing.app/Contents/MacOS/"
        "Google Chrome for Testing",
        "chromium-*/chrome-win*/chrome.exe",
    )
    found: list[str] = []
    for root in filter(None, roots):
        for pattern in patterns:
            found += sorted(glob.glob(os.path.join(root, pattern)), reverse=True)
    return found


def candidates() -> list[str]:
    system = platform.system()
    paths: list[str] = []
    if system == "Darwin":
        for base in ("/Applications", str(Path.home() / "Applications")):
            paths += [os.path.join(base, app) for app in _MAC_APPS]
    elif system == "Windows":
        for env in ("PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"):
            base = os.environ.get(env)
            if base:
                paths += [os.path.join(base, exe) for exe in _WIN_EXES]
    else:
        paths += [p for name in _LINUX_NAMES if (p := shutil.which(name))]
    return paths + _playwright_cache()


def find_browser() -> str:
    override = os.environ.get("DOTEML_BROWSER")
    if override:
        if not os.path.isfile(override):
            raise BrowserNotFoundError(f"DOTEML_BROWSER points to a missing file: {override}")
        return override
    for path in candidates():
        if os.path.isfile(path):
            return path
    raise BrowserNotFoundError(
        "No Chrome, Edge, or Chromium found.\n"
        f"  Install Google Chrome: {DOWNLOAD_URL}\n"
        "  or point doteml at a browser: DOTEML_BROWSER=/path/to/chrome"
    )


def parse_major(product: str) -> int | None:
    """'HeadlessChrome/131.0.6778.85' -> 131."""
    _, _, version = product.partition("/")
    major = version.split(".", 1)[0]
    return int(major) if major.isdigit() else None
