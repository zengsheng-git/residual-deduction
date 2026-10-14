// 音效合成与混入: 程序生成走子/吃子/将军/绝杀音效(24kHz mono s16le, 与场景 wav 同格式), 零素材依赖
use std::io::Read;
use std::path::Path;

use serde::Serialize;

pub const SR: u32 = 24000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Sfx {
    Click,   // 走子落子嗒声
    Capture, // 吃子: 嗒声 + 低频重击
    Check,   // 将军: 双音上扬提示
    Mate,    // 绝杀/困毙: 三连升调收尾
}

// 合成一种音效
pub fn render(kind: Sfx) -> Vec<i16> {
    match kind {
        Sfx::Click => click(),
        Sfx::Capture => capture(),
        Sfx::Check => check(),
        Sfx::Mate => mate(),
    }
}

const AMP: f32 = 32767.0;

// 把采样写入 f32 缓冲(可叠加), 越界截断
fn tone(buf: &mut [f32], start_secs: f32, freq: f32, dur_secs: f32, tau: f32, amp: f32) {
    let n = (dur_secs * SR as f32) as usize;
    let s0 = (start_secs * SR as f32) as usize;
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / SR as f32;
        buf[idx] += (t * freq * std::f32::consts::TAU).sin() * (-t / tau).exp() * amp;
    }
}

// 噪声瞬态(xorshift 伪随机, 不引入外部 crate)
fn noise(buf: &mut [f32], start_secs: f32, dur_secs: f32, tau: f32, amp: f32) {
    let n = (dur_secs * SR as f32) as usize;
    let s0 = (start_secs * SR as f32) as usize;
    let mut seed = 0x2545_F491u32;
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let r = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let t = i as f32 / SR as f32;
        buf[idx] += r * (-t / tau).exp() * amp;
    }
}

fn to_i16(buf: &[f32]) -> Vec<i16> {
    buf.iter().map(|v| (v.clamp(-1.0, 1.0) * AMP) as i16).collect()
}

// 木质落子: 高频噪声瞬态 + 中低频主体, 约 70ms
fn click() -> Vec<i16> {
    let mut buf = vec![0f32; (0.07 * SR as f32) as usize];
    noise(&mut buf, 0.0, 0.02, 0.004, 0.5);
    tone(&mut buf, 0.0, 170.0, 0.07, 0.02, 0.6);
    tone(&mut buf, 0.0, 900.0, 0.03, 0.008, 0.25);
    to_i16(&buf)
}

// 吃子: 落子声 + 更低沉的长重击, 约 220ms
fn capture() -> Vec<i16> {
    let mut buf = vec![0f32; (0.22 * SR as f32) as usize];
    noise(&mut buf, 0.0, 0.02, 0.004, 0.55);
    tone(&mut buf, 0.0, 150.0, 0.1, 0.03, 0.7);
    tone(&mut buf, 0.005, 85.0, 0.2, 0.06, 0.8);
    to_i16(&buf)
}

// 将军: 落子声后两声上扬提示音, 约 450ms
fn check() -> Vec<i16> {
    let mut buf = vec![0f32; (0.45 * SR as f32) as usize];
    noise(&mut buf, 0.0, 0.02, 0.004, 0.5);
    tone(&mut buf, 0.0, 170.0, 0.07, 0.02, 0.6);
    tone(&mut buf, 0.06, 880.0, 0.12, 0.05, 0.4);
    tone(&mut buf, 0.2, 1245.0, 0.16, 0.07, 0.4);
    to_i16(&buf)
}

// 绝杀: 三连升调收尾, 约 600ms
fn mate() -> Vec<i16> {
    let mut buf = vec![0f32; (0.6 * SR as f32) as usize];
    tone(&mut buf, 0.0, 660.0, 0.14, 0.06, 0.45);
    tone(&mut buf, 0.15, 880.0, 0.14, 0.06, 0.45);
    tone(&mut buf, 0.3, 1175.0, 0.22, 0.09, 0.5);
    to_i16(&buf)
}

// 写 wav(pcm/mono/16bit), 与 video::wav_duration 及本模块 read_wav_samples 兼容
pub fn write_wav(path: &Path, samples: &[i16]) -> Result<(), String> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // pcm
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SR.to_le_bytes());
    bytes.extend_from_slice(&(SR * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).map_err(|e| format!("写入 wav 失败: {e}"))
}

// 读取本产线生成的场景 wav(pcm/mono/16bit/{SR}), 格式不符返回 Err(调用方跳过混入)
pub fn read_wav_samples(path: &Path) -> Result<Vec<i16>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("wav 打开失败: {e}"))?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header).map_err(|e| format!("wav 头读取失败: {e}"))?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("非 wav 文件".to_string());
    }
    let mut samples: Vec<i16> = Vec::new();
    let mut has_data = false;
    loop {
        let mut chunk = [0u8; 8];
        if f.read_exact(&mut chunk).is_err() {
            break;
        }
        let id = &chunk[0..4];
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as usize;
        if id == b"fmt " {
            let mut fmt = vec![0u8; size];
            f.read_exact(&mut fmt).map_err(|e| format!("wav fmt 读取失败: {e}"))?;
            if fmt.len() >= 16 {
                let format = u16::from_le_bytes([fmt[0], fmt[1]]);
                let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
                if format != 1 || channels != 1 || bits != 16 || rate != SR {
                    return Err(format!("wav 格式不符(fmt={format} 声道={channels} 位深={bits} 采样率={rate})"));
                }
            }
        } else if id == b"data" {
            let mut data = vec![0u8; size];
            f.read_exact(&mut data).map_err(|e| format!("wav data 读取失败: {e}"))?;
            samples = data.chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]])).collect();
            has_data = true;
        } else {
            std::io::Seek::seek(&mut f, std::io::SeekFrom::Current(size as i64))
                .map_err(|e| format!("wav 跳过未知块失败: {e}"))?;
        }
    }
    if !has_data {
        return Err("wav 缺少 data 块".to_string());
    }
    Ok(samples)
}

// 把音效叠加到采样指定偏移处(超出末尾截断), 幅度夹紧防削波
pub fn mix_at(samples: &mut [i16], offset: usize, effect: &[i16]) {
    for (i, e) in effect.iter().enumerate() {
        let idx = offset + i;
        if idx >= samples.len() {
            break;
        }
        let sum = samples[idx] as i32 + *e as i32;
        samples[idx] = sum.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }
}

// 把场景音效(时长占比, 类型)按 dur 换算采样偏移并混入 wav 文件
pub fn apply(path: &Path, effects: &[(f32, Sfx)], dur: f64) -> Result<(), String> {
    if effects.is_empty() || dur <= 0.0 {
        return Ok(());
    }
    let mut samples = read_wav_samples(path)?;
    for (frac, kind) in effects {
        let offset = ((frac.clamp(0.0, 1.0) as f64 * dur * SR as f64) as usize).min(samples.len().saturating_sub(1));
        mix_at(&mut samples, offset, &render(*kind));
    }
    write_wav(path, &samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 合成信号非空、峰值有声且不越界
    #[test]
    fn test_render_bounded() {
        for kind in [Sfx::Click, Sfx::Capture, Sfx::Check, Sfx::Mate] {
            let s = render(kind);
            assert!(!s.is_empty());
            let peak = s.iter().map(|v| v.abs()).max().unwrap();
            assert!(peak > 1000, "{kind:?} 信号不应接近静音");
        }
        // 咔哒声约 70ms
        let click = render(Sfx::Click);
        let expect = (0.07 * SR as f32) as usize;
        assert!((click.len() as i64 - expect as i64).abs() <= 2, "len={}", click.len());
    }

    // wav 写读往返
    #[test]
    fn test_wav_roundtrip() {
        let dir = std::env::temp_dir().join("residual-sfx-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rt.wav");
        let samples: Vec<i16> = (0..2400i32).map(|i| (i * 7 % 30000 - 15000) as i16).collect();
        write_wav(&path, &samples).unwrap();
        assert_eq!(read_wav_samples(&path).unwrap(), samples);
        std::fs::remove_file(&path).ok();
    }

    // 混入叠加并在 i16 边界夹紧, 越界截断不 panic
    #[test]
    fn test_mix_at_clamps() {
        let mut s = vec![0i16; 10];
        mix_at(&mut s, 0, &[100, -100]);
        assert_eq!(&s[..2], &[100, -100]);
        mix_at(&mut s, 0, &[32767, -32767]);
        assert_eq!(s[0], 32767);
        assert_eq!(s[1], -32768);
        mix_at(&mut s, 8, &[1, 2, 3, 4]);
        assert_eq!(s[9], 2);
    }
}
