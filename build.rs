fn main() {
    // Embed the application icon and manifest as Windows resources
    // (only applies to Windows targets)
    #[cfg(target_os = "windows")]
    {
        // Only embed if the .ico file exists (skip gracefully during CI/first build)
        let ico_path = "assets/icon.ico";
        if std::path::Path::new(ico_path).exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path);
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=winresource compile failed: {e}");
            }
        } else {
            eprintln!(
                "cargo:warning=assets/icon.ico not found — \
                 taskbar icon will use default. \
                 Convert assets/icon.png to icon.ico to fix."
            );
        }
    }
}
