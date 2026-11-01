from __future__ import annotations

import contextlib
import platform
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import click

from . import __version__
from .converter import FORMATS

_GROUP_FLAGS = {"--help", "-h", "--version"}


class DefaultGroup(click.Group):
    """Route ``doteml [OPTIONS] FILES`` to the ``convert`` subcommand."""

    def parse_args(self, ctx: click.Context, args: list[str]) -> list[str]:
        if args and args[0] not in self.commands and args[0] not in _GROUP_FLAGS:
            args = ["convert", *args]
        return super().parse_args(ctx, args)


def _common_options(func: Callable[..., Any]) -> Callable[..., Any]:
    func = click.option(
        "--remote-images",
        is_flag=True,
        help="Load remote images and styles (off by default: blocks trackers).",
    )(func)
    func = click.option(
        "-q",
        "--quality",
        type=click.IntRange(1, 100),
        default=85,
        show_default=True,
        help="JPG quality.",
    )(func)
    func = click.option(
        "-o", "--output", type=click.Path(path_type=Path), help="Output path (single file only)."
    )(func)
    return click.argument(
        "files", nargs=-1, required=True, type=click.Path(exists=True, path_type=Path)
    )(func)


def _run(
    files: tuple[Path, ...], output: Path | None, fmt: str, quality: int, remote_images: bool
) -> None:
    from .converter import convert as do_convert
    from .renderer import Renderer

    if output and len(files) > 1:
        raise click.UsageError("--output can only be used with a single file")

    failed = 0
    with contextlib.ExitStack() as stack:
        renderer = None
        if fmt != "html":
            try:
                renderer = stack.enter_context(Renderer(allow_remote=remote_images))
            except RuntimeError as e:
                raise click.ClickException(str(e)) from e
        for f in files:
            try:
                result = do_convert(
                    f,
                    output,
                    fmt=fmt,
                    quality=quality,
                    allow_remote=remote_images,
                    renderer=renderer,
                )
                click.echo(f"{f} -> {result}")
            except Exception as e:  # keep going: one bad file shouldn't stop a batch
                failed += 1
                click.echo(f"Error: {f}: {e}", err=True)
    if failed:
        sys.exit(1)


@click.group(cls=DefaultGroup)
@click.version_option(__version__, prog_name="doteml")
def cli() -> None:
    """Convert .eml files to PDF, PNG, JPG, or HTML.

    \b
    Examples:
      doteml message.eml
      doteml -f png *.eml
    """


@cli.command()
@_common_options
@click.option(
    "-f",
    "--format",
    "fmt",
    type=click.Choice(FORMATS),
    default="pdf",
    show_default=True,
    help="Output format.",
)
def convert(
    files: tuple[Path, ...], output: Path | None, quality: int, remote_images: bool, fmt: str
) -> None:
    """Convert .eml files (default command)."""
    _run(files, output, fmt, quality, remote_images)


@cli.command()
def install() -> None:
    """Add a right-click "Convert EML to PDF" action to your OS."""
    from . import integrations

    system = platform.system()
    if system == "Darwin":
        path = integrations.install_macos()
        hint = "Right-click .eml files -> Quick Actions -> Convert EML to PDF"
    elif system == "Windows":
        path = integrations.install_windows_sendto()
        hint = "Right-click .eml files -> Send to -> Convert EML to PDF"
    elif system == "Linux":
        path = integrations.install_linux()
        hint = "Right-click .eml files -> Open With -> Convert EML to PDF"
    else:
        raise click.ClickException(f"No integration available for {system}")
    click.echo(f"Installed: {path}")
    click.echo(hint)


@cli.command()
def uninstall() -> None:
    """Remove the right-click action."""
    from . import integrations

    removers = {
        "Darwin": integrations.uninstall_macos,
        "Windows": integrations.uninstall_windows_sendto,
        "Linux": integrations.uninstall_linux,
    }
    system = platform.system()
    if system not in removers:
        raise click.ClickException(f"No integration available for {system}")
    removers[system]()
    click.echo("Removed.")


def _make_format_cli(fmt: str) -> click.Command:
    @click.command(help=f"Convert .eml files to {fmt.upper()}.")
    @_common_options
    @click.version_option(__version__, prog_name=f"eml2{fmt}")
    def format_cli(
        files: tuple[Path, ...], output: Path | None, quality: int, remote_images: bool
    ) -> None:
        _run(files, output, fmt, quality, remote_images)

    return format_cli


cli_pdf = _make_format_cli("pdf")
cli_png = _make_format_cli("png")
cli_jpg = _make_format_cli("jpg")
cli_html = _make_format_cli("html")
