from __future__ import annotations

import plistlib
import shutil
import uuid
from pathlib import Path

WORKFLOW_NAME = "Convert EML to PDF"


def _which_doteml() -> str:
    return shutil.which("doteml") or "doteml"


# --- macOS Quick Action ---


def install_macos() -> Path:
    services_dir = Path.home() / "Library" / "Services"
    services_dir.mkdir(parents=True, exist_ok=True)
    workflow_dir = services_dir / f"{WORKFLOW_NAME}.workflow"
    contents_dir = workflow_dir / "Contents"

    if workflow_dir.exists():
        shutil.rmtree(workflow_dir)
    contents_dir.mkdir(parents=True)

    exe = _which_doteml()
    shell_script = (
        'export PATH="/usr/local/bin:/opt/homebrew/bin:$HOME/.local/bin:$PATH"\n'
        'for f in "$@"; do\n'
        '    if [[ "${f##*.}" == "eml" || "${f##*.}" == "EML" ]]; then\n'
        f'        "{exe}" "$f"\n'
        "    fi\n"
        "done\n"
        "osascript -e 'display notification \"Done\" with title \"doteml\"'\n"
    )

    def uid() -> str:
        return str(uuid.uuid4()).upper()

    document = {
        "AMApplicationBuild": "523",
        "AMApplicationVersion": "2.10",
        "AMDocumentVersion": "2",
        "actions": [
            {
                "action": {
                    "AMAccepts": {
                        "Container": "List",
                        "Optional": True,
                        "Types": ["public.item"],
                    },
                    "AMActionVersion": "1.0.2",
                    "AMApplication": ["Automator"],
                    "AMBundleIdentifier": "com.apple.RunShellScript-Action",
                    "AMCategory": ["AMCategoryUtilities"],
                    "ActionBundlePath": "/System/Library/Automator/Run Shell Script.action",
                    "ActionName": "Run Shell Script",
                    "ActionParameters": {
                        "COMMAND_STRING": shell_script,
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
        "workflowMetaData": {
            "workflowTypeIdentifier": "com.apple.Automator.servicesMenu"
        },
    }

    info = {
        "NSServices": [
            {
                "NSMenuItem": {"default": WORKFLOW_NAME},
                "NSMessage": "runWorkflowAsService",
                "NSRequiredContext": {
                    "NSApplicationIdentifier": "com.apple.finder"
                },
                "NSSendFileTypes": ["com.apple.mail.email"],
            }
        ]
    }

    with open(contents_dir / "document.wflow", "wb") as f:
        plistlib.dump(document, f)
    with open(contents_dir / "Info.plist", "wb") as f:
        plistlib.dump(info, f)

    return workflow_dir


def uninstall_macos() -> None:
    workflow = Path.home() / "Library" / "Services" / f"{WORKFLOW_NAME}.workflow"
    if workflow.exists():
        shutil.rmtree(workflow)


# --- Windows Send To ---


def install_windows_sendto() -> Path:
    import os

    sendto = Path(os.environ.get("APPDATA", "")) / "Microsoft" / "Windows" / "SendTo"
    sendto.mkdir(parents=True, exist_ok=True)

    exe = _which_doteml()
    bat = sendto / "Convert EML to PDF.bat"
    bat.write_text(
        "@echo off\r\n"
        f'for %%f in (%*) do "{exe}" "%%f"\r\n'
        "echo Done. Press any key to close.\r\n"
        "pause >nul\r\n",
        encoding="utf-8",
    )
    return bat


def uninstall_windows_sendto() -> None:
    import os

    bat = (
        Path(os.environ.get("APPDATA", ""))
        / "Microsoft"
        / "Windows"
        / "SendTo"
        / "Convert EML to PDF.bat"
    )
    if bat.exists():
        bat.unlink()


# --- Linux .desktop file ---


def install_linux() -> Path:
    apps_dir = Path.home() / ".local" / "share" / "applications"
    apps_dir.mkdir(parents=True, exist_ok=True)

    exe = _which_doteml()
    desktop = apps_dir / "doteml.desktop"
    desktop.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Convert EML to PDF\n"
        "Comment=Convert .eml email files to PDF\n"
        f"Exec={exe} %F\n"
        "MimeType=message/rfc822;\n"
        "NoDisplay=true\n"
        "Terminal=false\n",
        encoding="utf-8",
    )
    return desktop


def uninstall_linux() -> None:
    desktop = Path.home() / ".local" / "share" / "applications" / "doteml.desktop"
    if desktop.exists():
        desktop.unlink()
