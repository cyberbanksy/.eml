//! Command-line interface for `doteml` and the `eml2*` shortcuts.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::convert::{ConvertOptions, Format, convert};
use crate::render::{BackendChoice, RenderOptions, Renderer};
use crate::{browser, integrations, verify};

#[derive(Args, Debug, Clone)]
pub struct CommonOpts {
    /// .eml files to convert
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
    /// Output path (single file only)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// JPG quality
    #[arg(short, long, default_value_t = 85, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub quality: u8,
    /// Load remote images and styles (off by default: blocks trackers)
    #[arg(long)]
    pub remote_images: bool,
    /// Rendering engine
    #[arg(long, value_enum, default_value_t = BackendChoice::Auto, env = "DOTEML_BACKEND")]
    pub backend: BackendChoice,
}

#[derive(Parser, Debug)]
#[command(
    name = "doteml",
    version,
    about = "Convert .eml files to PDF, PNG, JPG, or HTML.",
    after_help = "Examples:\n  doteml message.eml\n  doteml -f png *.eml\n  doteml verify message.pdf"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Convert .eml files (default command)
    Convert {
        #[command(flatten)]
        common: CommonOpts,
        /// Output format
        #[arg(short, long, value_enum, default_value_t = Format::Pdf)]
        format: Format,
    },
    /// Check that a rendering engine is available
    Doctor {
        #[arg(long, value_enum, default_value_t = BackendChoice::Auto, env = "DOTEML_BACKEND")]
        backend: BackendChoice,
    },
    /// Check that a PDF still matches the original .eml embedded in it
    Verify {
        /// PDFs made by doteml
        #[arg(required = true)]
        pdfs: Vec<PathBuf>,
        /// Also write the embedded .eml to this file or directory
        #[arg(long)]
        extract: Option<PathBuf>,
        /// Print a JSON report
        #[arg(long)]
        json: bool,
    },
    /// Render a raw HTML file through the same pipeline (development aid)
    #[command(hide = true)]
    RenderHtml {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long, value_enum, default_value_t = Format::Png)]
        format: Format,
        #[arg(long, value_enum, default_value_t = BackendChoice::Auto, env = "DOTEML_BACKEND")]
        backend: BackendChoice,
    },
    /// Add a right-click "Convert EML to PDF" action to your OS
    Install,
    /// Remove the right-click action
    Uninstall,
}

const SUBCOMMANDS: [&str; 7] = [
    "convert",
    "doctor",
    "verify",
    "render-html",
    "install",
    "uninstall",
    "help",
];

/// Route `doteml [OPTIONS] FILES` to `convert`, like the default subcommand of the Python CLI.
pub fn route_default(mut args: Vec<OsString>) -> Vec<OsString> {
    if let Some(first) = args.get(1).and_then(|a| a.to_str()) {
        let passthrough =
            SUBCOMMANDS.contains(&first) || matches!(first, "--help" | "-h" | "--version" | "-V");
        if !passthrough {
            args.insert(1, "convert".into());
        }
    }
    args
}

pub fn main_entry() -> ExitCode {
    let cli = Cli::parse_from(route_default(std::env::args_os().collect()));
    run(cli)
}

#[derive(Parser, Debug)]
#[command(version)]
struct FormatCli {
    #[command(flatten)]
    common: CommonOpts,
}

pub fn format_entry(format: Format) -> ExitCode {
    let (name, about) = match format {
        Format::Pdf => ("eml2pdf", "Convert .eml files to PDF."),
        Format::Png => ("eml2png", "Convert .eml files to PNG."),
        Format::Jpg => ("eml2jpg", "Convert .eml files to JPG."),
        Format::Html => ("eml2html", "Convert .eml files to HTML."),
    };
    let cmd = FormatCli::command().name(name).bin_name(name).about(about);
    let matches = cmd.get_matches();
    let cli = FormatCli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    auto_setup();
    run_convert(format, cli.common)
}

/// First run on macOS: add the Finder Quick Action without a separate step.
fn auto_setup() {
    if cfg!(target_os = "macos")
        && let Ok(Some(_)) = integrations::auto_install_macos()
    {
        eprintln!(
            "doteml: added the Finder Quick Action: right-click an .eml file -> Quick Actions -> {} \
             (remove it with `doteml uninstall`)",
            integrations::ACTION_NAME
        );
    }
}

pub fn run(cli: Cli) -> ExitCode {
    if !matches!(cli.cmd, Cmd::Install | Cmd::Uninstall) {
        auto_setup();
    }
    match cli.cmd {
        Cmd::Convert { common, format } => run_convert(format, common),
        Cmd::Doctor { backend } => run_doctor(backend),
        Cmd::Verify {
            pdfs,
            extract,
            json,
        } => run_verify(&pdfs, extract.as_deref(), json),
        Cmd::RenderHtml {
            file,
            output,
            format,
            backend,
        } => run_render_html(&file, &output, format, backend),
        Cmd::Install => run_install(),
        Cmd::Uninstall => run_uninstall(),
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("Usage error: {msg}");
    ExitCode::from(2)
}

pub fn run_convert(format: Format, c: CommonOpts) -> ExitCode {
    if c.output.is_some() && c.files.len() > 1 {
        return usage_error("--output can only be used with a single file");
    }
    let opts = ConvertOptions {
        format,
        quality: c.quality,
        allow_remote: c.remote_images,
        backend: c.backend,
    };
    let mut renderer = None;
    if format != Format::Html {
        match Renderer::new(RenderOptions {
            allow_remote: c.remote_images,
            backend: c.backend,
            ..Default::default()
        }) {
            Ok(r) => renderer = Some(r),
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let mut failed = 0;
    for f in &c.files {
        match convert(f, c.output.as_deref(), &opts, renderer.as_mut()) {
            Ok(out) => println!("{} -> {}", f.display(), out.display()),
            Err(e) => {
                failed += 1; // keep going: one bad file shouldn't stop a batch
                eprintln!("Error: {}: {e}", f.display());
            }
        }
    }
    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_render_html(file: &Path, output: &Path, format: Format, backend: BackendChoice) -> ExitCode {
    use crate::render::RenderFormat;
    let fmt = match format {
        Format::Pdf => RenderFormat::Pdf,
        Format::Png => RenderFormat::Png,
        Format::Jpg => RenderFormat::Jpg,
        Format::Html => {
            eprintln!("Error: render-html needs -f pdf, png, or jpg");
            return ExitCode::from(2);
        }
    };
    let result = std::fs::read_to_string(file)
        .map_err(crate::Error::from)
        .and_then(|html| {
            let mut r = Renderer::new(RenderOptions {
                backend,
                ..Default::default()
            })?;
            r.render(&html, output, fmt, 85)
        });
    match result {
        Ok(()) => {
            println!("{} -> {}", file.display(), output.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_doctor(backend: BackendChoice) -> ExitCode {
    match Renderer::new(RenderOptions {
        backend,
        ..Default::default()
    }) {
        Ok(r) => {
            let info = r.info();
            println!("Backend:  {}", info.name);
            println!("Engine:   {}", info.executable);
            if info.name == "chrome" {
                println!(
                    "Version:  {} (need {}+)",
                    info.product,
                    browser::MIN_VERSION
                );
            } else {
                println!("Version:  {}", info.product);
            }
            println!("OK: ready to convert PDF, PNG, and JPG.");
            if cfg!(target_os = "macos")
                && let Some(home) = dirs::home_dir()
            {
                if integrations::macos_installed_at(&home) {
                    println!(
                        "Finder:   Quick Action \"{}\" installed",
                        integrations::ACTION_NAME
                    );
                } else {
                    println!("Finder:   Quick Action not installed (run `doteml install`)");
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_verify(pdfs: &[PathBuf], extract: Option<&Path>, json: bool) -> ExitCode {
    let mut code = ExitCode::SUCCESS;
    let mut reports = Vec::new();
    for pdf in pdfs {
        let report = match verify::verify_pdf(pdf) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}: {e}", pdf.display());
                code = ExitCode::from(2);
                continue;
            }
        };
        if let Some(target) = extract {
            let dest = if target.is_dir() {
                target.join(&report.embedded_file_name)
            } else {
                target.to_path_buf()
            };
            if dest.exists() {
                eprintln!(
                    "{}: refusing to overwrite {}",
                    pdf.display(),
                    dest.display()
                );
                code = ExitCode::FAILURE;
            } else if let Err(e) = std::fs::write(&dest, &report.eml) {
                eprintln!("{}: {e}", dest.display());
                code = ExitCode::FAILURE;
            } else if !json {
                println!("extracted {}", dest.display());
            }
        }
        if !report.ok && code == ExitCode::SUCCESS {
            code = ExitCode::FAILURE;
        }
        if !json {
            print_report(&report);
        }
        reports.push(report);
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_default()
        );
    }
    code
}

fn mark(ok: bool) -> &'static str {
    if ok { "OK" } else { "MISMATCH" }
}

fn print_report(r: &verify::VerifyReport) {
    println!("{}", r.pdf.display());
    println!(
        "  embedded:   {} ({} bytes)",
        r.embedded_file_name,
        crate::html::thousands(r.embedded_size)
    );
    println!(
        "  sha256:     {} recorded",
        r.recorded_sha256.as_deref().unwrap_or("(none)")
    );
    println!(
        "              {} computed  {}",
        r.computed_sha256,
        mark(r.hash_matches)
    );
    for c in &r.header_checks {
        if c.recorded.is_empty() && c.parsed.is_empty() {
            continue;
        }
        let label = c
            .key
            .trim_start_matches("Email")
            .replace("MessageID", "Message-ID");
        println!(
            "  {:<11} {}  {}",
            format!("{label}:"),
            c.recorded,
            mark(c.matches)
        );
        if !c.matches {
            println!("  {:<11} {}  (from the embedded .eml)", "", c.parsed);
        }
    }
    if r.ok {
        println!("VERIFIED: the PDF carries its source email unchanged.");
    } else {
        println!("FAILED: the PDF and its embedded email do not match.");
    }
}

fn run_install() -> ExitCode {
    let result = if cfg!(target_os = "macos") {
        integrations::install_macos().map(|p| {
            (
                p,
                "Right-click .eml files -> Quick Actions -> Convert EML to PDF",
            )
        })
    } else if cfg!(windows) {
        integrations::install_windows_sendto()
            .map(|p| (p, "Right-click .eml files -> Send to -> Convert EML to PDF"))
    } else {
        integrations::install_linux().map(|p| {
            (
                p,
                "Right-click .eml files -> Open With -> Convert EML to PDF",
            )
        })
    };
    match result {
        Ok((path, hint)) => {
            println!("Installed: {}", path.display());
            println!("{hint}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_uninstall() -> ExitCode {
    let result = if cfg!(target_os = "macos") {
        integrations::uninstall_macos()
    } else if cfg!(windows) {
        integrations::uninstall_windows_sendto()
    } else {
        integrations::uninstall_linux()
    };
    match result {
        Ok(()) => {
            println!("Removed.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(args: &[&str]) -> Vec<String> {
        route_default(args.iter().map(OsString::from).collect())
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect()
    }

    #[test]
    fn routes_files_and_options_to_convert() {
        assert_eq!(route(&["doteml", "a.eml"]), ["doteml", "convert", "a.eml"]);
        assert_eq!(
            route(&["doteml", "-f", "png", "a.eml"]),
            ["doteml", "convert", "-f", "png", "a.eml"]
        );
        assert_eq!(route(&["doteml", "doctor"]), ["doteml", "doctor"]);
        assert_eq!(route(&["doteml", "--version"]), ["doteml", "--version"]);
        assert_eq!(route(&["doteml"]), ["doteml"]);
    }
}
