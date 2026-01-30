fn main() {
    slint_build::compile("ui/appwindow.slint").unwrap();

    // Windows: embed app icon into the exe
    #[cfg(target_os = "windows")]
    {
        if std::path::Path::new("assets/app.ico").exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/app.ico");

            // Try to find Windows SDK toolkit path
            if let Some(sdk_path) = find_windows_sdk_bin() {
                res.set_toolkit_path(&sdk_path);
            }

            if let Err(e) = res.compile() {
                println!("cargo:warning=Failed to embed icon: {}", e);
            }
        }
    }
}

/// Find Windows SDK bin directory containing rc.exe
#[cfg(target_os = "windows")]
fn find_windows_sdk_bin() -> Option<String> {
    // Common Windows SDK paths
    let sdk_base = r"C:\Program Files (x86)\Windows Kits\10\bin";

    if let Ok(entries) = std::fs::read_dir(sdk_base) {
        // Find the latest SDK version
        let mut versions: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("10."))
            .collect();

        versions.sort();
        versions.reverse();

        for version in versions {
            let x64_path = format!(r"{}\{}\x64", sdk_base, version);
            let rc_path = format!(r"{}\rc.exe", x64_path);
            if std::path::Path::new(&rc_path).exists() {
                return Some(x64_path);
            }
        }
    }

    None
}
