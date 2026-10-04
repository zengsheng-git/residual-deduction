fn main() {
    #[cfg(target_os = "windows")]
    {
        // 将 onnxruntime 动态库拷贝到可执行文件同级目录, 避免按名称搜索时
        // 命中 system32 中随系统分发的旧版 onnxruntime.dll
        let profile_dir = std::path::PathBuf::from(
            std::env::var("OUT_DIR").expect("OUT_DIR not set"),
        )
        .ancestors()
        .nth(3)
        .map(|p| p.to_path_buf())
        .expect("cannot resolve target dir");

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let dll_src = std::path::Path::new(&manifest_dir).join("../libs/windows-cpu");
        if let Ok(entries) = std::fs::read_dir(dll_src) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().ends_with(".dll") {
                    let dst = profile_dir.join(&name);
                    if let Err(e) = std::fs::copy(entry.path(), &dst) {
                        // 文件被占用(正在运行的进程)时忽略, 下次构建再同步
                        println!("cargo:warning=copy {} failed: {}", name.to_string_lossy(), e);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-fapple-link-rtlib");

    tauri_build::build()
}
