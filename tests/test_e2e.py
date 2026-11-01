from __future__ import annotations

from pathlib import Path

import pytest

from doteml.converter import convert


@pytest.mark.timeout(60)
class TestE2EPdf:
    def test_simple_to_pdf(self, simple_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "output.pdf"
        result = convert(simple_eml, output)
        assert result == output
        assert output.exists()
        assert output.stat().st_size > 0
        content = output.read_bytes()
        assert content[:5] == b"%PDF-"

    def test_plaintext_to_pdf(self, plaintext_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "plain.pdf"
        result = convert(plaintext_eml, output)
        assert result.exists()
        assert output.read_bytes()[:5] == b"%PDF-"

    def test_multipart_to_pdf(self, multipart_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "multi.pdf"
        result = convert(multipart_eml, output)
        assert result.exists()
        assert output.read_bytes()[:5] == b"%PDF-"

    def test_legoland_to_pdf(self, legoland_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "legoland.pdf"
        result = convert(legoland_eml, output)
        assert result.exists()
        assert output.stat().st_size > 1000
        assert output.read_bytes()[:5] == b"%PDF-"

    def test_default_output_path(self, simple_eml: Path) -> None:
        result = convert(simple_eml)
        expected = simple_eml.with_suffix(".pdf")
        assert result == expected
        assert expected.exists()
        expected.unlink()

    def test_pdf_has_pages(self, simple_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "pages.pdf"
        convert(simple_eml, output)
        from pypdf import PdfReader
        reader = PdfReader(output)
        assert len(reader.pages) >= 1

    def test_cc_email_to_pdf(self, eml_with_cc: Path, tmp_path: Path) -> None:
        output = tmp_path / "cc.pdf"
        result = convert(eml_with_cc, output)
        assert result.exists()


@pytest.mark.timeout(60)
class TestE2EJpg:
    def test_simple_to_jpg(self, simple_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "output.jpg"
        result = convert(simple_eml, output, fmt="jpg")
        assert result == output
        assert output.exists()
        assert output.stat().st_size > 0
        content = output.read_bytes()
        assert content[:2] == b"\xff\xd8"  # JPEG magic bytes

    def test_legoland_to_jpg(self, legoland_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "legoland.jpg"
        result = convert(legoland_eml, output, fmt="jpg")
        assert result.exists()
        assert output.stat().st_size > 1000

    def test_default_jpg_path(self, simple_eml: Path) -> None:
        result = convert(simple_eml, fmt="jpg")
        expected = simple_eml.with_suffix(".jpg")
        assert result == expected
        assert expected.exists()
        expected.unlink()


@pytest.mark.timeout(60)
class TestE2EPng:
    def test_simple_to_png(self, simple_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "output.png"
        result = convert(simple_eml, output, fmt="png")
        assert result == output
        assert output.exists()
        assert output.stat().st_size > 0
        content = output.read_bytes()
        assert content[:4] == b"\x89PNG"

    def test_legoland_to_png(self, legoland_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "legoland.png"
        result = convert(legoland_eml, output, fmt="png")
        assert result.exists()
        assert output.stat().st_size > 1000

    def test_default_png_path(self, simple_eml: Path) -> None:
        result = convert(simple_eml, fmt="png")
        expected = simple_eml.with_suffix(".png")
        assert result == expected
        assert expected.exists()
        expected.unlink()


@pytest.mark.timeout(60)
class TestE2EHtml:
    def test_simple_to_html(self, simple_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "output.html"
        result = convert(simple_eml, output, fmt="html")
        assert result == output
        assert output.exists()
        content = output.read_text()
        assert "<!DOCTYPE html>" in content
        assert "Test Email" in content

    def test_legoland_to_html(self, legoland_eml: Path, tmp_path: Path) -> None:
        output = tmp_path / "legoland.html"
        result = convert(legoland_eml, output, fmt="html")
        assert result.exists()
        content = output.read_text()
        assert "LEGOLAND" in content or "legoland" in content.lower()

    def test_default_html_path(self, simple_eml: Path) -> None:
        result = convert(simple_eml, fmt="html")
        expected = simple_eml.with_suffix(".html")
        assert result == expected
        assert expected.exists()
        expected.unlink()


@pytest.mark.timeout(60)
class TestE2EErrors:
    def test_missing_file(self, tmp_path: Path) -> None:
        with pytest.raises(FileNotFoundError):
            convert(tmp_path / "nonexistent.eml")

    def test_wrong_extension(self, tmp_path: Path) -> None:
        bad = tmp_path / "test.txt"
        bad.write_text("not an eml")
        with pytest.raises(ValueError, match="Expected .eml"):
            convert(bad)


@pytest.mark.timeout(60)
class TestE2ECli:
    def test_cli_convert(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli

        output = tmp_path / "cli_out.pdf"
        runner = CliRunner()
        result = runner.invoke(cli, [str(simple_eml), "-o", str(output)])
        assert result.exit_code == 0, result.output
        assert output.exists()

    def test_cli_version(self) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli

        runner = CliRunner()
        result = runner.invoke(cli, ["--version"])
        assert result.exit_code == 0
        assert "doteml" in result.output

    def test_cli_jpg(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli

        output = tmp_path / "cli_out.jpg"
        runner = CliRunner()
        result = runner.invoke(cli, [str(simple_eml), "-o", str(output), "-f", "jpg"])
        assert result.exit_code == 0, result.output
        assert output.exists()

    def test_cli_png_format(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli

        output = tmp_path / "cli_out.png"
        runner = CliRunner()
        result = runner.invoke(cli, [str(simple_eml), "-o", str(output), "-f", "png"])
        assert result.exit_code == 0, result.output
        assert output.exists()

    def test_cli_html_format(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli

        output = tmp_path / "cli_out.html"
        runner = CliRunner()
        result = runner.invoke(cli, [str(simple_eml), "-o", str(output), "-f", "html"])
        assert result.exit_code == 0, result.output
        assert output.exists()


@pytest.mark.timeout(60)
class TestE2EEntryPoints:
    def test_eml2png_entrypoint(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli_png

        output = tmp_path / "ep.png"
        runner = CliRunner()
        result = runner.invoke(cli_png, [str(simple_eml), "-o", str(output)])
        assert result.exit_code == 0, result.output
        assert output.exists()

    def test_eml2jpg_entrypoint(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli_jpg

        output = tmp_path / "ep.jpg"
        runner = CliRunner()
        result = runner.invoke(cli_jpg, [str(simple_eml), "-o", str(output)])
        assert result.exit_code == 0, result.output
        assert output.exists()

    def test_eml2html_entrypoint(self, simple_eml: Path, tmp_path: Path) -> None:
        from click.testing import CliRunner
        from doteml.cli import cli_html

        output = tmp_path / "ep.html"
        runner = CliRunner()
        result = runner.invoke(cli_html, [str(simple_eml), "-o", str(output)])
        assert result.exit_code == 0, result.output
        assert output.exists()
