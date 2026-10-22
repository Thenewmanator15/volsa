//! The application manifest the build puts into `voxa.exe` on Windows: what it asks of the
//! system, read from the file the build script hands the resource compiler.

/// The manifest, as the build script gives it.
const MANIFEST: &str = include_str!("../voxa.exe.manifest");

/// The text between `<name` and the `>` that closes that element's first tag.
fn tag<'a>(name: &str, within: &'a str) -> Option<&'a str> {
    let start = within.find(&format!("<{name}"))?;
    let rest = within.get(start..)?;
    rest.find('>').and_then(|end| rest.get(..=end))
}

#[test]
fn the_manifest_asks_for_todays_common_controls() {
    let controls = tag("assemblyIdentity", MANIFEST)
        .filter(|found| found.contains(r#"name="Microsoft.Windows.Common-Controls""#));
    let controls = controls.expect("a dependency on the common controls");
    for wanted in [
        r#"type="win32""#,
        r#"version="6.0.0.0""#,
        r#"publicKeyToken="6595b64144ccf1df""#,
        r#"processorArchitecture="*""#,
    ] {
        assert!(controls.contains(wanted), "{controls} lacks {wanted}");
    }
    assert!(MANIFEST.contains("<dependentAssembly>"), "{MANIFEST}");
}

#[test]
fn the_manifest_says_voxa_follows_each_screens_scale_as_winit_does() {
    // winit asks for the same itself as the window opens; the manifest makes it so from the
    // start, for a message shown before the window.
    let awareness = MANIFEST
        .split("<dpiAwareness")
        .nth(1)
        .and_then(|rest| rest.split("</dpiAwareness>").next());
    let awareness = awareness.expect("a dpiAwareness element");
    assert!(
        awareness.ends_with(">PerMonitorV2, PerMonitor"),
        "{awareness}"
    );
    assert!(
        awareness.contains("http://schemas.microsoft.com/SMI/2016/WindowsSettings"),
        "{awareness}"
    );
    // Windows before 10 1607 read only the older element.
    assert!(MANIFEST.contains(">true/pm</dpiAware>"), "{MANIFEST}");
}

#[test]
fn the_manifest_names_windows_10_and_11_and_asks_for_no_more_rights_than_the_person_has() {
    assert!(
        MANIFEST.contains(r#"<supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>"#),
        "{MANIFEST}"
    );
    assert!(
        MANIFEST.contains(r#"<requestedExecutionLevel level="asInvoker" uiAccess="false"/>"#),
        "{MANIFEST}"
    );
    assert!(MANIFEST.trim_start().starts_with("<?xml"), "{MANIFEST}");
    assert!(MANIFEST.trim_end().ends_with("</assembly>"), "{MANIFEST}");
}
