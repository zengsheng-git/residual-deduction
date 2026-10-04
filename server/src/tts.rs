// Edge TTS(微软朗读接口)同步客户端: 文本 → mp3 字节流
use sha2::Digest;
use tungstenite::Message;

const TRUSTED_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const WSS_URL: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0";
const ORIGIN: &str = "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold";
const GEC_VERSION: &str = "1-140.0.3485.54";

// Sec-MS-GEC: 时间戳(取整到5分钟)+Token 的 SHA256 大写十六进制
fn sec_ms_gec() -> String {
    let mut ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u64)
        .unwrap_or(0);
    ticks += 11_644_473_600;
    ticks -= ticks % 300;
    ticks *= 10_000_000;
    let mut hasher = sha2::Sha256::new();
    hasher.update(format!("{ticks}{TRUSTED_TOKEN}"));
    format!("{:X}", hasher.finalize())
}

fn request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("{:032x}", nanos as u64 ^ (std::process::id() as u64) << 32)
}

fn now_timestamp() -> String {
    // Edge 期望的 JS 风格 UTC 时间串: "Wed Oct 01 2025 08:00:00 GMT+0000 (Coordinated Universal Time)"
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let (hh, mm, ss) = ((secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    const WEEK: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01 是周四
    const MONTH: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let weekday = (days % 7 + 7) % 7;
    format!(
        "{} {:02} {:02} {} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
        WEEK[weekday as usize],
        MONTH[(m - 1) as usize],
        d,
        y,
        hh,
        mm,
        ss
    )
}

// Howard Hinnant 的 days_from_civil 逆变换: 天数 → (年, 月, 日)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn ssml_for(text: &str, voice: &str, rate: i32) -> String {
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='zh-CN'><voice name='{voice}'><prosody rate='{rate:+}%'>{text}</prosody></voice></speak>"
    )
}

// 单句合成
fn synthesize_once(text: &str, voice: &str, rate: i32) -> Result<Vec<u8>, String> {
    use tungstenite::client::IntoClientRequest;

    // tungstenite 的 rustls 依赖未选定加密提供器, 需手动安装(ring)
    let _ = rustls::crypto::ring::default_provider().install_default();

    let url = format!("{WSS_URL}?TrustedClientToken={TRUSTED_TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version={GEC_VERSION}", sec_ms_gec());
    let mut request = url.into_client_request().map_err(|e| format!("TTS 请求构造失败: {e}"))?;
    request.headers_mut().insert("User-Agent", UA.parse().unwrap());
    request.headers_mut().insert("Origin", ORIGIN.parse().unwrap());

    let (mut socket, _resp) = tungstenite::connect(request).map_err(|e| format!("TTS 连接失败: {e}"))?;

    let config = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}",
        now_timestamp()
    );
    socket.send(Message::Text(config.into())).map_err(|e| format!("TTS 发送配置失败: {e}"))?;

    let ssml = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{}",
        request_id(),
        now_timestamp(),
        ssml_for(text, voice, rate)
    );
    socket.send(Message::Text(ssml.into())).map_err(|e| format!("TTS 发送文本失败: {e}"))?;

    let mut audio: Vec<u8> = Vec::new();
    loop {
        let msg = socket.read().map_err(|e| format!("TTS 读取失败: {e}"))?;
        match msg {
            Message::Text(t) => {
                if t.contains("Path:turn.end") {
                    break;
                }
            }
            Message::Binary(b) => {
                if b.len() > 2 {
                    let header_len = u16::from_be_bytes([b[0], b[1]]) as usize;
                    if b.len() > 2 + header_len {
                        audio.extend_from_slice(&b[2 + header_len..]);
                    }
                }
            }
            Message::Close(c) => return Err(format!("TTS 连接被关闭: {c:?}")),
            _ => {}
        }
    }
    let _ = socket.close(None);
    if audio.len() < 512 {
        return Err("TTS 返回音频过短".to_string());
    }
    Ok(audio)
}

// 单句合成, 带重试: Edge TWS 接口偶发瞬断(10054)与限频, 退避后重连通常可恢复
fn synthesize_with_retry(text: &str, voice: &str, rate: i32, attempts: usize) -> Result<Vec<u8>, String> {
    let mut last_err = String::new();
    for attempt in 1..=attempts {
        match synthesize_once(text, voice, rate) {
            Ok(bytes) => return Ok(bytes),
            Err(e) => {
                last_err = e;
                tracing::warn!("TTS 第{}次尝试失败: {}", attempt, last_err);
                if attempt < attempts {
                    std::thread::sleep(std::time::Duration::from_millis(600 * attempt as u64));
                }
            }
        }
    }
    Err(last_err)
}

// 把长文本按句切分后逐句合成, 返回拼接的 mp3(同为 24kHz 48kbps 单声道, 帧级拼接可靠)。
// 单句在重试后仍失败时跳过该句(局部降级), 只有全部失败才报错。
pub fn synthesize(text: &str, voice: &str, rate: i32) -> Result<Vec<u8>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let chunks = split_text(text, 90);
    let mut all: Vec<u8> = Vec::new();
    let mut failed = 0usize;
    let mut last_err = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        match synthesize_with_retry(chunk, voice, rate, 3) {
            Ok(mp3) => {
                all.extend_from_slice(&mp3);
            }
            Err(e) => {
                failed += 1;
                last_err = e;
                tracing::warn!("TTS 句段 {}/{} 合成失败, 跳过: {}", i + 1, chunks.len(), last_err);
            }
        }
        // 请求间稍作间隔, 降低被限频的概率
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    if all.is_empty() {
        return Err(format!("TTS 全部句段合成失败: {last_err}"));
    }
    if failed > 0 {
        tracing::warn!("TTS 部分句段失败({failed}/{}), 已跳过", chunks.len());
    }
    Ok(all)
}

// 按句读切分, 每段不超过 max_chars 个字符
fn split_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut len = 0usize;
    for sentence in split_sentences(text) {
        if len + sentence.chars().count() <= max_chars {
            current.push_str(&sentence);
            len += sentence.chars().count();
        } else {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                len = 0;
            }
            if sentence.chars().count() <= max_chars {
                current.push_str(&sentence);
                len = sentence.chars().count();
            } else {
                // 超长句子硬切
                let mut buf = String::new();
                for c in sentence.chars() {
                    buf.push(c);
                    if buf.chars().count() >= max_chars {
                        chunks.push(std::mem::take(&mut buf));
                    }
                }
                if !buf.is_empty() {
                    current = buf;
                    len = current.chars().count();
                }
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    for c in text.chars() {
        buf.push(c);
        if matches!(c, '。' | '！' | '？' | '；' | '!' | '?' | ';') {
            parts.push(std::mem::take(&mut buf));
        }
    }
    if !buf.is_empty() {
        parts.push(buf);
    }
    parts
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // 需要外网; cargo test -- --ignored 运行
    fn test_synthesize() {
        let mp3 = super::synthesize("欢迎来到残局推演。红方先行, 我们一步一步来看。", "zh-CN-YunxiNeural", 0).expect("合成失败");
        std::fs::write(std::env::temp_dir().join("tts-test.mp3"), &mp3).unwrap();
        println!("mp3 bytes: {}", mp3.len());
        assert!(mp3.len() > 2000);
    }
}
