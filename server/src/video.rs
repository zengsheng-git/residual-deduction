// ffmpeg 合成: 片段编码(帧管道+配音) → 拼接 → 封面
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use crate::render::CANVAS_H;
use crate::render::CANVAS_W;
use crate::render::FPS;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

fn spawn(ffmpeg: &Path, args: &[String]) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

fn run(ffmpeg: &Path, args: &[String]) -> Result<(), String> {
    let out = spawn(ffmpeg, args).output().map_err(|e| format!("ffmpeg 启动失败: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg 执行失败: {}", String::from_utf8_lossy(&out.stderr).chars().take(400).collect::<String>()));
    }
    Ok(())
}

// mp3 → wav(24kHz 单声道 s16le), 便于精确测量时长并直接送入编码器
pub fn mp3_to_wav(ffmpeg: &Path, mp3: &Path, wav: &Path) -> Result<(), String> {
    run(
        ffmpeg,
        &[
            "-y".into(),
            "-i".into(),
            mp3.to_string_lossy().into_owned(),
            "-ar".into(),
            "24000".into(),
            "-ac".into(),
            "1".into(),
            "-c:a".into(),
            "pcm_s16le".into(),
            wav.to_string_lossy().into_owned(),
        ],
    )
}

// 解析 wav 数据长度, 计算时长(秒)
pub fn wav_duration(wav: &Path) -> Result<f64, String> {
    let mut f = std::fs::File::open(wav).map_err(|e| format!("wav 打开失败: {e}"))?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header).map_err(|e| format!("wav 读取失败: {e}"))?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("非 wav 文件".to_string());
    }
    let mut byte_rate = 0u32;
    let mut data_len: Option<u32> = None;
    loop {
        let mut chunk = [0u8; 8];
        if f.read_exact(&mut chunk).is_err() {
            break;
        }
        let id = &chunk[0..4];
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        if id == b"fmt " {
            let mut fmt = vec![0u8; size as usize];
            f.read_exact(&mut fmt).map_err(|e| format!("wav fmt 读取失败: {e}"))?;
            byte_rate = u32::from_le_bytes([fmt[8], fmt[9], fmt[10], fmt[11]]);
        } else if id == b"data" {
            data_len = Some(size);
            let _ = std::io::Seek::seek(&mut f, std::io::SeekFrom::Current(size as i64));
        } else {
            let _ = std::io::Seek::seek(&mut f, std::io::SeekFrom::Current(size as i64));
        }
        if data_len.is_some() && byte_rate > 0 {
            break;
        }
    }
    match (data_len, byte_rate) {
        (Some(len), rate) if rate > 0 => Ok(len as f64 / rate as f64),
        _ => Err("wav 时长解析失败".to_string()),
    }
}

// 编码一个片段: 帧由 frames 回调逐帧产出(rgb24), 音频来自 wav
pub fn encode_segment(
    ffmpeg: &Path,
    audio_wav: &Path,
    out_mp4: &Path,
    frames: &mut dyn Iterator<Item = Vec<u8>>,
) -> Result<(), String> {
    let mut child = spawn(
        ffmpeg,
        &[
            "-y".into(),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgb24".into(),
            "-s".into(),
            format!("{CANVAS_W}x{CANVAS_H}"),
            "-r".into(),
            format!("{FPS}"),
            "-i".into(),
            "-".into(),
            "-i".into(),
            audio_wav.to_string_lossy().into_owned(),
            "-af".into(),
            "apad".into(),
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            "20".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "128k".into(),
            "-shortest".into(),
            out_mp4.to_string_lossy().into_owned(),
        ],
    )
    .stdin(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("ffmpeg 编码启动失败: {e}"))?;

    let mut stdin = child.stdin.take().ok_or("ffmpeg stdin 不可用")?;
    for frame in frames {
        if stdin.write_all(&frame).is_err() {
            break; // ffmpeg 已退出(出错), 交给 wait 收集错误
        }
    }
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| format!("ffmpeg 等待失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ffmpeg 编码失败: {}",
            String::from_utf8_lossy(&out.stderr).chars().take(400).collect::<String>()
        ));
    }
    Ok(())
}

// 拼接全部片段
pub fn concat_segments(ffmpeg: &Path, segments: &[PathBuf], out_mp4: &Path, work_dir: &Path) -> Result<(), String> {
    let list_path = work_dir.join("concat.txt");
    let mut list = String::new();
    for seg in segments {
        list.push_str(&format!("file '{}'\n", seg.to_string_lossy().replace('\\', "/")));
    }
    std::fs::write(&list_path, list).map_err(|e| format!("写入 concat 列表失败: {e}"))?;
    run(
        ffmpeg,
        &[
            "-y".into(),
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            list_path.to_string_lossy().into_owned(),
            "-c".into(),
            "copy".into(),
            out_mp4.to_string_lossy().into_owned(),
        ],
    )
}

// 提取封面
pub fn extract_thumbnail(ffmpeg: &Path, video: &Path, out_jpg: &Path, at_secs: f64) -> Result<(), String> {
    run(
        ffmpeg,
        &[
            "-y".into(),
            "-ss".into(),
            format!("{at_secs:.2}"),
            "-i".into(),
            video.to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            "-q:v".into(),
            "3".into(),
            out_jpg.to_string_lossy().into_owned(),
        ],
    )
}

// 读取视频时长(秒), 解析 ffmpeg -i 输出中的 Duration
pub fn video_duration(ffmpeg: &Path, video: &Path) -> Result<f64, String> {
    let mut cmd = spawn(ffmpeg, &["-i".into(), video.to_string_lossy().into_owned()]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = cmd.output().map_err(|e| format!("ffmpeg 启动失败: {e}"))?;
    let text = String::from_utf8_lossy(&out.stderr);
    for line in text.lines() {
        if let Some(idx) = line.find("Duration:") {
            let tail = &line[idx + 9..];
            let parts: Vec<&str> = tail.trim().split([':', '.']).collect();
            if parts.len() >= 3 {
                let h: f64 = parts[0].trim().parse().unwrap_or(0.0);
                let m: f64 = parts[1].trim().parse().unwrap_or(0.0);
                let s: f64 = parts[2].trim().parse().unwrap_or(0.0);
                return Ok(h * 3600.0 + m * 60.0 + s);
            }
        }
    }
    Err("视频时长解析失败".to_string())
}
