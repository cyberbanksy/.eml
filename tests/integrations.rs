use std::path::Path;

use doteml::integrations::*;

#[test]
fn macos_quick_action_plists() {
    let home = tempfile::tempdir().unwrap();
    let exe = Path::new("/Users/me/my bin/doteml");
    let workflow = install_macos_at(home.path(), exe).unwrap();
    let info = plist::Value::from_file(workflow.join("Contents/Info.plist")).unwrap();
    let doc = plist::Value::from_file(workflow.join("Contents/document.wflow")).unwrap();

    let service = &info.as_dictionary().unwrap()["NSServices"]
        .as_array()
        .unwrap()[0];
    let service = service.as_dictionary().unwrap();
    let menu = service["NSMenuItem"].as_dictionary().unwrap()["default"]
        .as_string()
        .unwrap();
    assert_eq!(menu, "Convert EML to PDF");
    let types: Vec<_> = service["NSSendFileTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_string().unwrap())
        .collect();
    assert!(types.contains(&"public.email-message"));

    let action = &doc.as_dictionary().unwrap()["actions"].as_array().unwrap()[0];
    let script = action.as_dictionary().unwrap()["action"]
        .as_dictionary()
        .unwrap()["ActionParameters"]
        .as_dictionary()
        .unwrap()["COMMAND_STRING"]
        .as_string()
        .unwrap();
    assert!(
        script.contains("'/Users/me/my bin/doteml' \"${files[@]}\""),
        "{script}"
    );

    uninstall_macos_at(home.path()).unwrap();
    assert!(!workflow.exists());
}

#[test]
fn windows_sendto_forwards_all_args() {
    let appdata = tempfile::tempdir().unwrap();
    let bat = install_windows_sendto_at(
        appdata.path(),
        Path::new(r"C:\Program Files\doteml\doteml.exe"),
    )
    .unwrap();
    let content = std::fs::read_to_string(&bat).unwrap();
    assert!(
        content.contains(r#""C:\Program Files\doteml\doteml.exe" %*"#),
        "{content}"
    );
    assert!(!content.contains("for %%"));
    assert!(content.contains("\r\n"));
    uninstall_windows_sendto_at(appdata.path()).unwrap();
    assert!(!bat.exists());
}

#[test]
fn windows_sendto_non_ascii_path_uses_utf8_codepage() {
    let appdata = tempfile::tempdir().unwrap();
    let bat =
        install_windows_sendto_at(appdata.path(), Path::new(r"C:\Users\José\doteml.exe")).unwrap();
    assert!(std::fs::read_to_string(bat).unwrap().contains("chcp 65001"));
}

#[test]
fn linux_desktop_entry_quotes_exec() {
    let data = tempfile::tempdir().unwrap();
    let desktop = install_linux_at(data.path(), Path::new("/opt/my env/bin/doteml")).unwrap();
    let content = std::fs::read_to_string(&desktop).unwrap();
    assert!(
        content.contains(r#"Exec="/opt/my env/bin/doteml" %F"#),
        "{content}"
    );
    assert!(content.contains("MimeType=message/rfc822;"));
    uninstall_linux_at(data.path()).unwrap();
    assert!(!desktop.exists());
}
