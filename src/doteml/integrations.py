"""Right-click integrations for macOS (Quick Action), Windows (Send To) and Linux (Open With)."""

from __future__ import annotations

import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

ACTION_NAME = "Convert EML to PDF"


# --- macOS Quick Action ---


def _macos_workflow() -> Path:
    return Path.home() / "Library" / "Services" / f"{ACTION_NAME}.workflow"


def _macos_script() -> str:
    py = shlex.quote(sys.executable)
    return (
        "files=()\n"
        'for f in "$@"; do\n'
        '  [[ "${f:l}" == *.eml ]] && files+=("$f")\n'
        "done\n"
        "(( ${#files} )) || exit 0\n"
        f'if out=$({py} -m doteml "${{files[@]}}" 2>&1); then\n'
        '  osascript -e \'display notification "Converted to PDF" with title "doteml"\'\n'
        "else\n"
        "  osascript -e 'on run argv' "
        "-e 'display alert \"doteml failed\" message (item 1 of argv)' "
        "-e 'end run' \"$out\"\n"
        "fi\n"
    )


def install_macos() -> Path:
    workflow = _macos_workflow()
    contents = workflow / "Contents"
    if workflow.exists():
        shutil.rmtree(workflow)
    contents.mkdir(parents=True)

    def uid() -> str:
        return str(uuid.uuid4()).upper()

    document = {
        "AMApplicationBuild": "523",
        "AMApplicationVersion": "2.10",
        "AMDocumentVersion": "2",
        "actions": [
            {
                "action": {
                    "AMAccepts": {"Container": "List", "Optional": True, "Types": ["public.item"]},
                    "AMActionVersion": "1.0.2",
                    "AMApplication": ["Automator"],
                    "AMBundleIdentifier": "com.apple.RunShellScript-Action",
                    "AMCategory": ["AMCategoryUtilities"],
                    "ActionBundlePath": "/System/Library/Automator/Run Shell Script.action",
                    "ActionName": "Run Shell Script",
                    "ActionParameters": {
                        "COMMAND_STRING": _macos_script(),
                        "CheckedForUserDefaultShell": True,
                        "inputMethod": 1,
                        "shell": "/bin/zsh",
                        "source": "",
                    },
                    "BundleIdentifier": "com.apple.RunShellScript-Action",
                    "CFBundleVersion": "1.0.2",
                    "CanShowSelectedItemsWhenRun": False,
                    "CanShowWhenRun": True,
                    "Category": ["AMCategoryUtilities"],
                    "Class Name": "RunShellScriptAction",
                    "InputUUID": uid(),
                    "Keywords": ["Shell", "Script"],
                    "OutputUUID": uid(),
                    "UUID": uid(),
                    "UnlocalizedApplications": ["Automator"],
                    "arguments": {},
                    "isViewVisible": True,
                }
            }
        ],
        "connectors": {},
        "workflowMetaData": {"workflowTypeIdentifier": "com.apple.Automator.servicesMenu"},
    }
    info = {
        "NSServices": [
            {
                "NSMenuItem": {"default": ACTION_NAME},
                "NSMessage": "runWorkflowAsService",
                "NSRequiredContext": {"NSApplicationIdentifier": "com.apple.finder"},
                # public.email-message covers Mail's and Outlook's .eml types, which conform to it.
                "NSSendFileTypes": ["public.email-message", "com.apple.mail.email"],
            }
        ]
    }

    with open(contents / "document.wflow", "wb") as f:
        plistlib.dump(document, f)
    with open(contents / "Info.plist", "wb") as f:
        plistlib.dump(info, f)
    return workflow


def uninstall_macos() -> None:
    shutil.rmtree(_macos_workflow(), ignore_errors=True)


# --- Windows Send To ---


def _sendto_bat() -> Path:
    appdata = os.environ.get("APPDATA") or str(Path.home() / "AppData" / "Roaming")
    return Path(appdata) / "Microsoft" / "Windows" / "SendTo" / f"{ACTION_NAME}.bat"


def install_windows_sendto() -> Path:
    bat = _sendto_bat()
    bat.parent.mkdir(parents=True, exist_ok=True)
    # Send To passes each path already quoted, so forward %* untouched.
    body = f'"{sys.executable}" -m doteml %*\r\necho.\r\npause\r\n'
    try:
        # cmd.exe reads batch files in the console (OEM) code page.
        bat.write_bytes(("@echo off\r\n" + body).encode("oem"))
    except (LookupError, UnicodeEncodeError):
        bat.write_bytes(("@echo off\r\nchcp 65001 >nul\r\n" + body).encode("utf-8"))
    return bat


def uninstall_windows_sendto() -> None:
    _sendto_bat().unlink(missing_ok=True)


# --- Linux "Open With" desktop entry ---


def _desktop_file() -> Path:
    data = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
    return Path(data) / "applications" / "doteml.desktop"


def _desktop_quote(arg: str) -> str:
    # Desktop Entry spec: quote args, escape " ` $ \ inside quotes, and double literal %.
    escaped = "".join("\\" + c if c in '"`$\\' else c for c in arg).replace("%", "%%")
    return f'"{escaped}"'


def install_linux() -> Path:
    desktop = _desktop_file()
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={ACTION_NAME}\n"
        "Comment=Convert .eml email files to PDF\n"
        f"Exec={_desktop_quote(sys.executable)} -m doteml %F\n"
        "MimeType=message/rfc822;\n"
        "NoDisplay=true\n"
        "Terminal=false\n",
        encoding="utf-8",
    )
    if updater := shutil.which("update-desktop-database"):
        subprocess.run([updater, str(desktop.parent)], check=False, capture_output=True)
    return desktop


def uninstall_linux() -> None:
    _desktop_file().unlink(missing_ok=True)
