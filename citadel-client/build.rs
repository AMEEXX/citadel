fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest_file("citadel-client.manifest");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to embed Windows manifest: {}", e);
        }
    }
}
