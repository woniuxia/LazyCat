fn main() {
    #[cfg(windows)]
    {
        let manifest_path = std::path::Path::new("lazycat.manifest");
        println!("cargo:rerun-if-changed={}", manifest_path.display());

        // 通过 tauri_build 注入自定义 manifest，避免与其默认 resource.lib 重复嵌入。
        let manifest = std::fs::read_to_string(manifest_path)
            .expect("Failed to read lazycat.manifest");
        let attributes = tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(manifest),
        );

        tauri_build::try_build(attributes).expect("failed to run tauri build script");
    }

    #[cfg(not(windows))]
    tauri_build::build();
}
