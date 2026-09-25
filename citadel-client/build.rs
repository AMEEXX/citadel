fn main() {
    // Force MSVC linker to embed requestedExecutionLevel="requireAdministrator"
    // into the PE manifest resource so Windows automatically triggers UAC prompt on launch.
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'");
    }
}
