fn main() {
    // 在 Windows 上嵌入自定义 manifest
    #[cfg(windows)]
    {
        let manifest_path = std::path::Path::new("lazycat.manifest");
        if manifest_path.exists() {
            println!("cargo:rerun-if-changed=lazycat.manifest");

            // 获取当前目录的绝对路径
            let current_dir = std::env::current_dir().unwrap();
            let manifest_abs = current_dir.join(manifest_path);

            // 创建临时 .rc 文件引用我们的 manifest
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let rc_path = std::path::Path::new(&out_dir).join("embed_manifest.rc");

            // 写入 RC 文件内容，使用普通路径格式（不带 \\?\ 前缀）
            let manifest_str = manifest_abs.to_str().unwrap().replace("\\\\?\\", "");
            std::fs::write(
                &rc_path,
                format!(
                    "#pragma code_page(65001)\n\
                     1 24 \"{}\"",
                    manifest_str
                )
            ).expect("Failed to write RC file");

            // 使用 embed-resource crate 编译 RC 文件
            embed_resource::compile(&rc_path, embed_resource::NONE);
        }
    }

    // 运行标准 Tauri 构建
    tauri_build::build()
}
