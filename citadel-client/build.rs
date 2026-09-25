fn main() {
    // Only embed requireAdministrator manifest in release builds so that unit tests
    // in debug mode can run without triggering OS Error 740 (Elevation Required).
    // In both debug and release, main() programmatically verifies elevation and calls
    // elevate_self() with a Windows UAC prompt.
    #[cfg(target_os = "windows")]
    {
        let is_release = std::env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
        if is_release {
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'");
        }
    }
}
