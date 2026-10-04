// 引擎子进程管理, 移植自参考项目 chessboard 的 engine/command.rs
use std::process::Child;
use std::process::ChildStdin;
use std::process::Command;
use std::process::Stdio;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// 启动引擎子进程。
/// 返回: (子进程, 命令写入口)
#[cfg(target_os = "windows")]
pub fn new(libs: &std::path::Path) -> (Child, ChildStdin) {
    let mut child = Command::new(libs.join("pikafish-windows.exe"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .expect("Unable to run engine");
    let stdin = child.stdin.take().unwrap();
    (child, stdin)
}

#[cfg(not(target_os = "windows"))]
pub fn new(libs: &std::path::Path) -> (Child, ChildStdin) {
    let exe = if cfg!(target_os = "macos") { "pikafish-macos" } else { "pikafish-linux" };
    let mut child = Command::new(libs.join(exe))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Unable to run engine");
    let stdin = child.stdin.take().unwrap();
    (child, stdin)
}
