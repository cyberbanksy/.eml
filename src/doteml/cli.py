from __future__ import annotations

import sys
from pathlib import Path

import click

from . import __version__
from .converter import FORMATS


class DefaultGroup(click.Group):
    def resolve_command(self, ctx: click.Context, args: list[str]) -> tuple:
        cmd_name = args[0] if args else None
        if cmd_name and cmd_name not in self.commands and not cmd_name.startswith("-"):
            args = ["convert"] + list(args)
        return super().resolve_command(ctx, args)


@click.group(cls=DefaultGroup)
@click.version_option(__version__, prog_name="doteml")
def cli() -> None:
    """Convert .eml files to PDF, PNG, JPG, or HTML."""


@cli.command()
@click.argument(
    "files", nargs=-1, required=True, type=click.Path(exists=True, path_type=Path)
)
@click.option(
    "-o", "--output", type=click.Path(path_type=Path), help="Output path (single file only)"
)
@click.option(
    "-f",
    "--format",
    "fmt",
    type=click.Choice(list(FORMATS)),
    default="pdf",
    help="Output format",
)
@click.option("-q", "--quality", type=int, default=85, help="JPG quality (1-100)")
def convert(
    files: tuple[Path, ...], output: Path | None, fmt: str, quality: int
) -> None:
    """Convert .eml files to PDF, PNG, JPG, or HTML."""
    from .converter import convert as do_convert

    if output and len(files) > 1:
        click.echo("Error: --output can only be used with a single file", err=True)
        sys.exit(1)

    for f in files:
        try:
            result = do_convert(f, output, fmt=fmt, quality=quality)
            click.echo(f"{f} -> {result}")
        except Exception as e:
            click.echo(f"Error: {f}: {e}", err=True)
            sys.exit(1)


@cli.command()
def install() -> None:
    """Install OS integration (right-click menu)."""
    import platform

    os_name = platform.system()
    if os_name == "Darwin":
        from .integrations import install_macos

        path = install_macos()
        click.echo(f"Installed macOS Quick Action: {path}")
        click.echo("Right-click .eml files -> Quick Actions -> Convert EML to PDF")
    elif os_name == "Windows":
        from .integrations import install_windows_sendto

        path = install_windows_sendto()
        click.echo(f"Installed Windows Send To shortcut: {path}")
        click.echo("Right-click .eml files -> Send To -> Convert EML to PDF")
    elif os_name == "Linux":
        from .integrations import install_linux

        path = install_linux()
        click.echo(f"Installed desktop action: {path}")
        click.echo("Right-click .eml files in your file manager to convert.")
    else:
        click.echo(f"No integration available for {os_name}", err=True)
        sys.exit(1)


@cli.command()
def uninstall() -> None:
    """Remove OS integration."""
    import platform

    os_name = platform.system()
    if os_name == "Darwin":
        from .integrations import uninstall_macos

        uninstall_macos()
        click.echo("Removed macOS Quick Action.")
    elif os_name == "Windows":
        from .integrations import uninstall_windows_sendto

        uninstall_windows_sendto()
        click.echo("Removed Windows Send To shortcut.")
    elif os_name == "Linux":
        from .integrations import uninstall_linux

        uninstall_linux()
        click.echo("Removed desktop action.")
    else:
        click.echo(f"No integration available for {os_name}", err=True)
        sys.exit(1)


def _make_format_cli(default_fmt: str) -> click.Command:
    @click.command()
    @click.argument(
        "files", nargs=-1, required=True, type=click.Path(exists=True, path_type=Path)
    )
    @click.option(
        "-o", "--output", type=click.Path(path_type=Path), help="Output path (single file only)"
    )
    @click.option("-q", "--quality", type=int, default=85, help="JPG quality (1-100)")
    @click.version_option(__version__, prog_name=f"eml2{default_fmt}")
    def format_cli(files: tuple[Path, ...], output: Path | None, quality: int) -> None:
        from .converter import convert as do_convert

        if output and len(files) > 1:
            click.echo("Error: --output can only be used with a single file", err=True)
            sys.exit(1)

        for f in files:
            try:
                result = do_convert(f, output, fmt=default_fmt, quality=quality)
                click.echo(f"{f} -> {result}")
            except Exception as e:
                click.echo(f"Error: {f}: {e}", err=True)
                sys.exit(1)

    format_cli.help = f"Convert .eml files to {default_fmt.upper()}."
    return format_cli


cli_pdf = _make_format_cli("pdf")
cli_png = _make_format_cli("png")
cli_jpg = _make_format_cli("jpg")
cli_html = _make_format_cli("html")
