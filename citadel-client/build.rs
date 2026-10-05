fn main() {
    #[cfg(target_os = "windows")]
    {
        // Embed the requireAdministrator manifest ONLY into distributable
        // (release) builds. Cargo links build-script resources into every unit
        // of the package — including `cargo test` harnesses — which otherwise
        // makes the test executables unlaunchable without elevation
        // (os error 740). Debug/test builds stay manifest-free; the binary's
        // own UAC auto-elevation loop in main.rs still enforces admin at
        // runtime. (`cargo test --release` remains elevated-only.)
        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile == "release" {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("citadel.ico");
            res.set_manifest_file("citadel-client.manifest");
            res.set("FileDescription", "Citadel Assessment Lockdown Client");
            res.set("ProductName", "Citadel Examination Platform");
            res.set("OriginalFilename", "citadel-client.exe");
            res.set("CompanyName", "Citadel Assessment Systems");
            res.set("LegalCopyright", "Copyright (C) 2026 Citadel Security Technologies");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to embed Windows manifest: {}", e);
            }
        }
    }
}
