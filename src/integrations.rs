//! Right-click integrations: macOS Quick Action, Windows Send To, Linux Open With.

use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};

use plist::{Dictionary, Value};

use crate::error::{Error, Result};

pub const ACTION_NAME: &str = "Convert EML to PDF";

/// Path the right-click action should run. Prefers the `doteml` on PATH when it is
/// this same binary, so a Homebrew `bin/doteml` symlink is recorded instead of a
/// versioned Cellar path that disappears on upgrade.
pub fn exe_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(stable_exe_path(&exe, |name| which::which(name).ok()))
}

pub(crate) fn stable_exe_path(exe: &Path, lookup: impl Fn(&str) -> Option<PathBuf>) -> PathBuf {
    let same = |a: &Path, b: &Path| match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if let Some(name) = exe.file_name().and_then(|n| n.to_str())
        && let Some(on_path) = lookup(name)
        && same(&on_path, exe)
    {
        return on_path;
    }
    let parts: Vec<_> = exe.components().collect();
    if let Some(i) = parts.iter().position(|c| c.as_os_str() == "Cellar")
        && let Some(name) = exe.file_name()
    {
        let prefix: PathBuf = parts[..i].iter().collect();
        let linked = prefix.join("bin").join(name);
        if same(&linked, exe) {
            return linked;
        }
    }
    exe.to_path_buf()
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Backend("cannot find your home directory".into()))
}

/// Quote for a POSIX shell, like Python's `shlex.quote`.
pub(crate) fn sh_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@%+=:,./_-".contains(&b));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r#"'"'"'"#))
    }
}

/// Desktop Entry spec: quote, escape `" ` $ \` inside quotes, and double literal `%`.
pub(crate) fn desktop_quote(arg: &str) -> String {
    let mut out = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn uuid_v4() -> String {
    let a = RandomState::new().hash_one(std::time::SystemTime::now());
    let b = RandomState::new().hash_one(a);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

// --- macOS Quick Action ---

fn macos_workflow(home: &Path) -> PathBuf {
    home.join("Library/Services")
        .join(format!("{ACTION_NAME}.workflow"))
}

fn macos_script(exe: &Path) -> String {
    let exe = sh_quote(&exe.to_string_lossy());
    format!(
        "files=()\n\
         for f in \"$@\"; do\n  [[ \"${{f:l}}\" == *.eml ]] && files+=(\"$f\")\ndone\n\
         (( ${{#files}} )) || exit 0\n\
         if out=$({exe} \"${{files[@]}}\" 2>&1); then\n  \
         osascript -e 'display notification \"Converted to PDF\" with title \"doteml\"'\n\
         else\n  \
         osascript -e 'on run argv' -e 'display alert \"doteml failed\" message (item 1 of argv)' -e 'end run' \"$out\"\n\
         fi\n"
    )
}

fn dict(pairs: Vec<(&str, Value)>) -> Value {
    Value::Dictionary(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<Dictionary>(),
    )
}

fn strings(items: &[&str]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|s| Value::String((*s).to_string()))
            .collect(),
    )
}

pub fn install_macos() -> Result<PathBuf> {
    install_macos_at(&home()?, &exe_path()?)
}

pub fn install_macos_at(home: &Path, exe: &Path) -> Result<PathBuf> {
    let workflow = macos_workflow(home);
    let contents = workflow.join("Contents");
    if workflow.exists() {
        std::fs::remove_dir_all(&workflow)?;
    }
    std::fs::create_dir_all(&contents)?;

    let action = dict(vec![
        (
            "AMAccepts",
            dict(vec![
                ("Container", Value::String("List".into())),
                ("Optional", Value::Boolean(true)),
                ("Types", strings(&["public.item"])),
            ]),
        ),
        ("AMActionVersion", Value::String("1.0.2".into())),
        ("AMApplication", strings(&["Automator"])),
        (
            "AMBundleIdentifier",
            Value::String("com.apple.RunShellScript-Action".into()),
        ),
        ("AMCategory", strings(&["AMCategoryUtilities"])),
        (
            "ActionBundlePath",
            Value::String("/System/Library/Automator/Run Shell Script.action".into()),
        ),
        ("ActionName", Value::String("Run Shell Script".into())),
        (
            "ActionParameters",
            dict(vec![
                ("COMMAND_STRING", Value::String(macos_script(exe))),
                ("CheckedForUserDefaultShell", Value::Boolean(true)),
                ("inputMethod", Value::Integer(1.into())),
                ("shell", Value::String("/bin/zsh".into())),
                ("source", Value::String(String::new())),
            ]),
        ),
        (
            "BundleIdentifier",
            Value::String("com.apple.RunShellScript-Action".into()),
        ),
        ("CFBundleVersion", Value::String("1.0.2".into())),
        ("CanShowSelectedItemsWhenRun", Value::Boolean(false)),
        ("CanShowWhenRun", Value::Boolean(true)),
        ("Category", strings(&["AMCategoryUtilities"])),
        ("Class Name", Value::String("RunShellScriptAction".into())),
        ("InputUUID", Value::String(uuid_v4())),
        ("Keywords", strings(&["Shell", "Script"])),
        ("OutputUUID", Value::String(uuid_v4())),
        ("UUID", Value::String(uuid_v4())),
        ("UnlocalizedApplications", strings(&["Automator"])),
        ("arguments", dict(vec![])),
        ("isViewVisible", Value::Boolean(true)),
    ]);
    let document = dict(vec![
        ("AMApplicationBuild", Value::String("523".into())),
        ("AMApplicationVersion", Value::String("2.10".into())),
        ("AMDocumentVersion", Value::String("2".into())),
        (
            "actions",
            Value::Array(vec![dict(vec![("action", action)])]),
        ),
        ("connectors", dict(vec![])),
        (
            "workflowMetaData",
            dict(vec![(
                "workflowTypeIdentifier",
                Value::String("com.apple.Automator.servicesMenu".into()),
            )]),
        ),
    ]);
    let info = dict(vec![(
        "NSServices",
        Value::Array(vec![dict(vec![
            (
                "NSMenuItem",
                dict(vec![("default", Value::String(ACTION_NAME.into()))]),
            ),
            ("NSMessage", Value::String("runWorkflowAsService".into())),
            (
                "NSRequiredContext",
                dict(vec![(
                    "NSApplicationIdentifier",
                    Value::String("com.apple.finder".into()),
                )]),
            ),
            // public.email-message covers Mail's and Outlook's .eml types, which conform to it.
            (
                "NSSendFileTypes",
                strings(&["public.email-message", "com.apple.mail.email"]),
            ),
        ])]),
    )]);

    let to_io = |e: plist::Error| Error::Io(std::io::Error::other(e));
    document
        .to_file_xml(contents.join("document.wflow"))
        .map_err(to_io)?;
    info.to_file_xml(contents.join("Info.plist"))
        .map_err(to_io)?;
    Ok(workflow)
}

pub fn uninstall_macos() -> Result<()> {
    uninstall_macos_at(&home()?)
}

pub fn uninstall_macos_at(home: &Path) -> Result<()> {
    let workflow = macos_workflow(home);
    if workflow.exists() {
        std::fs::remove_dir_all(workflow)?;
    }
    Ok(())
}

// --- Windows Send To ---

fn sendto_bat(appdata: &Path) -> PathBuf {
    appdata
        .join("Microsoft")
        .join("Windows")
        .join("SendTo")
        .join(format!("{ACTION_NAME}.bat"))
}

fn appdata() -> Result<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join("AppData").join("Roaming")))
        .ok_or_else(|| Error::Backend("cannot find %APPDATA%".into()))
}

pub fn install_windows_sendto() -> Result<PathBuf> {
    install_windows_sendto_at(&appdata()?, &exe_path()?)
}

pub fn install_windows_sendto_at(appdata: &Path, exe: &Path) -> Result<PathBuf> {
    let bat = sendto_bat(appdata);
    if let Some(parent) = bat.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let exe = exe.to_string_lossy();
    // Send To passes each path already quoted, so forward %* untouched.
    let body = format!("\"{exe}\" %*\r\necho.\r\npause\r\n");
    let content = if exe.is_ascii() {
        format!("@echo off\r\n{body}")
    } else {
        format!("@echo off\r\nchcp 65001 >nul\r\n{body}")
    };
    std::fs::write(&bat, content)?;
    Ok(bat)
}

pub fn uninstall_windows_sendto() -> Result<()> {
    uninstall_windows_sendto_at(&appdata()?)
}

pub fn uninstall_windows_sendto_at(appdata: &Path) -> Result<()> {
    let bat = sendto_bat(appdata);
    if bat.exists() {
        std::fs::remove_file(bat)?;
    }
    Ok(())
}

// --- Linux "Open With" desktop entry ---

fn data_home() -> Result<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .ok_or_else(|| Error::Backend("cannot find your data directory".into()))
}

fn desktop_file(data_home: &Path) -> PathBuf {
    data_home.join("applications/doteml.desktop")
}

pub fn install_linux() -> Result<PathBuf> {
    let data_home = data_home()?;
    let desktop = install_linux_at(&data_home, &exe_path()?)?;
    if let Ok(updater) = which::which("update-desktop-database") {
        let _ = std::process::Command::new(updater)
            .arg(data_home.join("applications"))
            .output();
    }
    Ok(desktop)
}

pub fn install_linux_at(data_home: &Path, exe: &Path) -> Result<PathBuf> {
    let desktop = desktop_file(data_home);
    if let Some(parent) = desktop.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &desktop,
        format!(
            "[Desktop Entry]\nType=Application\nName={ACTION_NAME}\n\
             Comment=Convert .eml email files to PDF\nExec={} %F\n\
             MimeType=message/rfc822;\nNoDisplay=true\nTerminal=false\n",
            desktop_quote(&exe.to_string_lossy())
        ),
    )?;
    Ok(desktop)
}

pub fn uninstall_linux() -> Result<()> {
    uninstall_linux_at(&data_home()?)
}

pub fn uninstall_linux_at(data_home: &Path) -> Result<()> {
    let desktop = desktop_file(data_home);
    if desktop.exists() {
        std::fs::remove_file(desktop)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_quote_escapes_specials() {
        assert_eq!(desktop_quote(r#"a"b$c%d"#), r#""a\"b\$c%%d""#);
    }

    #[test]
    fn sh_quote_matches_shlex() {
        assert_eq!(sh_quote("/usr/local/bin/doteml"), "/usr/local/bin/doteml");
        assert_eq!(
            sh_quote("/Users/me/my bin/doteml"),
            "'/Users/me/my bin/doteml'"
        );
        assert_eq!(sh_quote("it's"), r#"'it'"'"'s'"#);
    }

    #[test]
    fn stable_path_prefers_symlink_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/doteml/0.2.0/bin");
        std::fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("doteml");
        std::fs::write(&real, "").unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        let link = dir.path().join("bin/doteml");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        std::fs::copy(&real, &link).unwrap();

        // On PATH and the same file: use the PATH entry.
        assert_eq!(stable_exe_path(&real, |_| Some(link.clone())), link);
        // Not on PATH but under Cellar: rewrite to <prefix>/bin.
        #[cfg(unix)]
        assert_eq!(stable_exe_path(&real, |_| None), link);
        // Unrelated binary on PATH: keep the real path.
        let other = dir.path().join("other");
        std::fs::write(&other, "").unwrap();
        let plain = dir.path().join("plain-doteml");
        std::fs::write(&plain, "").unwrap();
        assert_eq!(stable_exe_path(&plain, |_| Some(other.clone())), plain);
    }

    #[test]
    fn uuids_are_unique_v4() {
        let (a, b) = (uuid_v4(), uuid_v4());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
    }
}
