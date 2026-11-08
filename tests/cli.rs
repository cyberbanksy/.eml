mod common;

use assert_cmd::Command;
use common::*;
use predicates::str::contains;

/// Every CLI run is bounded so a hung renderer fails the test instead of the CI job.
fn bin(name: &str) -> Command {
    let mut cmd = Command::cargo_bin(name).unwrap();
    cmd.timeout(std::time::Duration::from_secs(120));
    cmd
}

fn doteml() -> Command {
    bin("doteml")
}

#[test]
fn cli_convert() {
    let (d, p) = simple_eml();
    let out = d.path().join("cli_out.pdf");
    doteml().arg(&p).arg("-o").arg(&out).assert().success();
    assert!(out.exists());
}

#[test]
fn cli_version() {
    doteml()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("doteml"));
}

#[test]
fn cli_jpg() {
    let (d, p) = simple_eml();
    let out = d.path().join("cli_out.jpg");
    doteml()
        .arg(&p)
        .args(["-f", "jpg", "-o"])
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn cli_png_format() {
    let (d, p) = simple_eml();
    let out = d.path().join("cli_out.png");
    doteml()
        .arg(&p)
        .args(["-f", "png", "-o"])
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn cli_html_format() {
    let (d, p) = simple_eml();
    let out = d.path().join("cli_out.html");
    doteml()
        .arg(&p)
        .args(["-f", "html", "-o"])
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn options_before_files() {
    let (d, p) = simple_eml();
    let out = d.path().join("first.png");
    doteml()
        .args(["-f", "png", "-o"])
        .arg(&out)
        .arg(&p)
        .assert()
        .success();
    assert_eq!(&std::fs::read(&out).unwrap()[..4], b"\x89PNG");
}

#[test]
fn batch_continues_after_failure() {
    let (d, p) = simple_eml();
    let bad = d.path().join("bad.txt");
    std::fs::write(&bad, "nope").unwrap();
    doteml()
        .args(["-f", "html"])
        .arg(&bad)
        .arg(&p)
        .assert()
        .code(1);
    assert!(p.with_extension("html").exists());
}

#[test]
fn quality_out_of_range_is_usage_error() {
    let (_d, p) = simple_eml();
    doteml().args(["-q", "0"]).arg(&p).assert().code(2);
}

#[test]
fn output_with_many_files_is_usage_error() {
    let (d, p) = simple_eml();
    doteml()
        .arg("-o")
        .arg(d.path().join("x.pdf"))
        .arg(&p)
        .arg(&p)
        .assert()
        .code(2);
}

#[test]
fn doctor_ok() {
    doteml()
        .args(["doctor", "--backend", "chrome"])
        .assert()
        .success()
        .stdout(contains("OK"));
}

#[test]
fn doctor_reports_missing_browser() {
    doteml()
        .args(["doctor", "--backend", "chrome"])
        .env("DOTEML_BROWSER", "nope/chrome")
        .assert()
        .code(1)
        .stderr(contains("missing file"));
}

#[test]
fn webkit_backend_unavailable_off_macos() {
    if cfg!(target_os = "macos") {
        return;
    }
    doteml()
        .args(["doctor", "--backend", "webkit"])
        .assert()
        .code(1)
        .stderr(contains("only available on macOS"));
}

#[test]
fn verify_cli_reports_verified() {
    let d = tempfile::tempdir().unwrap();
    let pdf = d.path().join("legoland.pdf");
    doteml()
        .arg(legoland())
        .arg("-o")
        .arg(&pdf)
        .assert()
        .success();
    doteml()
        .arg("verify")
        .arg(&pdf)
        .assert()
        .success()
        .stdout(contains("VERIFIED"));
    let extracted = d.path().join("back.eml");
    doteml()
        .args(["verify", "--json", "--extract"])
        .arg(&extracted)
        .arg(&pdf)
        .assert()
        .success()
        .stdout(contains("\"ok\": true"));
    assert_eq!(
        std::fs::read(extracted).unwrap(),
        std::fs::read(legoland()).unwrap()
    );
}

#[test]
fn verify_cli_rejects_foreign_pdf() {
    let (d, p) = simple_eml();
    let pdf = d.path().join("x.pdf");
    doteml().arg(&p).arg("-o").arg(&pdf).assert().success();
    let mut doc = lopdf::Document::load(&pdf).unwrap();
    doc.catalog_mut().unwrap().remove(b"Names");
    doc.save(&pdf).unwrap();
    doteml()
        .arg("verify")
        .arg(&pdf)
        .assert()
        .code(2)
        .stderr(contains("no embedded .eml"));
}

#[test]
fn eml2pdf_entrypoint() {
    let (d, p) = simple_eml();
    let out = d.path().join("ep.pdf");
    bin("eml2pdf")
        .arg(&p)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert_eq!(&std::fs::read(&out).unwrap()[..5], b"%PDF-");
}

#[test]
fn eml2png_entrypoint() {
    let (d, p) = simple_eml();
    let out = d.path().join("ep.png");
    bin("eml2png")
        .arg(&p)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn eml2jpg_entrypoint() {
    let (d, p) = simple_eml();
    let out = d.path().join("ep.jpg");
    bin("eml2jpg")
        .arg(&p)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn eml2html_entrypoint() {
    let (d, p) = simple_eml();
    let out = d.path().join("ep.html");
    bin("eml2html")
        .arg(&p)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists());
}

#[test]
fn eml2_version_names_itself() {
    bin("eml2jpg")
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("eml2jpg"));
}

#[test]
fn render_html_renders_a_raw_page() {
    let d = tempfile::tempdir().unwrap();
    let page = d.path().join("page.html");
    std::fs::write(
        &page,
        "<html><body><div style=\"height:900px;background:red\">x</div></body></html>",
    )
    .unwrap();
    let out = d.path().join("page.png");
    doteml()
        .args(["render-html", "--backend", "chrome", "-o"])
        .arg(&out)
        .arg(&page)
        .assert()
        .success();
    assert!(png_height(&out) >= 900, "{}", png_height(&out));
}

#[test]
fn chrome_refuses_jpg_taller_than_jpeg_allows() {
    let (d, p) = tall_eml();
    let out = d.path().join("tall.jpg");
    doteml()
        .args(["--backend", "chrome", "-f", "jpg", "-o"])
        .arg(&out)
        .arg(&p)
        .assert()
        .failure()
        .stderr(contains("65,535"));
    assert!(!out.exists(), "no empty or partial file is left behind");
}
