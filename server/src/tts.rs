// Edge TTS(微软朗读接口)同步客户端: 文本 → mp3 字节流
// Sec-MS-GEC v2 令牌: 微软 2025 起新增校验, 令牌 = "{CS64}-{SHA256变体}" 双段大写十六进制,
// 算法参考 rany2/edge-tts(GPL-3.0)移植, 仅用于本学习研究项目
use tungstenite::Message;

const TRUSTED_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const WSS_URL: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
const ORIGIN: &str = "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold";
const GEC_VERSION: &str = "2-143.0.3650.75";

const WIN_EPOCH: u64 = 11_644_473_600;

// ---------- Sec-MS-GEC v2 令牌(定制哈希, 勿用标准库替换) ----------

// 标准 SHA-256 轮常量
const K: [u32; 64] = [
    0x428A2F98, 0x71374491, 0xB5C0FBCF, 0xE9B5DBA5, 0x3956C25B, 0x59F111F1, 0x923F82A4, 0xAB1C5ED5,
    0xD807AA98, 0x12835B01, 0x243185BE, 0x550C7DC3, 0x72BE5D74, 0x80DEB1FE, 0x9BDC06A7, 0xC19BF174,
    0xE49B69C1, 0xEFBE4786, 0x0FC19DC6, 0x240CA1CC, 0x2DE92C6F, 0x4A7484AA, 0x5CB0A9DC, 0x76F988DA,
    0x983E5152, 0xA831C66D, 0xB00327C8, 0xBF597FC7, 0xC6E00BF3, 0xD5A79147, 0x06CA6351, 0x14292967,
    0x27B70A85, 0x2E1B2138, 0x4D2C6DFC, 0x53380D13, 0x650A7354, 0x766A0ABB, 0x81C2C92E, 0x92722C85,
    0xA2BFE8A1, 0xA81A664B, 0xC24B8B70, 0xC76C51A3, 0xD192E819, 0xD6990624, 0xF40E3585, 0x106AA070,
    0x19A4C116, 0x1E376C08, 0x2748774C, 0x34B0BCB5, 0x391C0CB3, 0x4ED8AA4A, 0x5B9CCA4F, 0x682E6FF3,
    0x748F82EE, 0x78A5636F, 0x84C87814, 0x8CC70208, 0x90BEFFFA, 0xA4506CEB, 0xBEF9A3F7, 0xC67178F2,
];

// 标准初始向量(仅用于最终相加)
const SHA256_INITIAL: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

// 变体压缩起始状态(非标准 IV)
const SHA256_STATE: [u32; 8] = [
    0xFD9FBEA2, 0x91243CE7, 0xBE499094, 0xCD2E1E0A, 0x6C006F49, 0x04BD35AB, 0xFEB0A7E4, 0x03EFFBD2,
];

// 虚拟首块的预计算调度表(words 16..30)
const SHA256_SCHEDULE: [u32; 14] = [
    0xEB24BF9E, 0x1558691F, 0xA0C09EE2, 0x0CFA32BA, 0xA622EE8C, 0xD1DD9491, 0xC0DD25A0,
    0xF9E70AB6, 0xF0C31573, 0x7058CE4E, 0x7188E606, 0x00E5AB33, 0xD258B060, 0x72766564,
];

const SHA256_ADJUSTMENT: u32 = 0xC9DC06A7; // 替换 K[14]
const SIGMA0_ADJUSTMENT: u32 = 0x059C0B80;
const SIGMA1_ADJUSTMENT: u32 = 0x000B92C0;

fn rol16(v: u32) -> u32 {
    v.rotate_left(16)
}

fn sigma0(v: u32) -> u32 {
    v.rotate_right(7) ^ v.rotate_right(18) ^ (v >> 3)
}

fn sigma1(v: u32) -> u32 {
    v.rotate_right(17) ^ v.rotate_right(19) ^ (v >> 10)
}

// 标准 SHA-256 消息调度展开
fn expand_schedule(words: &mut Vec<u32>) {
    while words.len() < 64 {
        let n = words.len();
        let x = words[n - 2];
        let y = words[n - 15];
        let w = sigma1(x)
            .wrapping_add(words[n - 7])
            .wrapping_add(sigma0(y))
            .wrapping_add(words[n - 16]);
        words.push(w);
    }
}

// 标准 SHA-256 单块压缩
fn sha256_block(state: &[u32; 8], block: &[u8]) -> [u32; 8] {
    let mut w: Vec<u32> = (0..16).map(|i| u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap())).collect();
    expand_schedule(&mut w);
    let mut s = *state;
    for (i, wi) in w.iter().enumerate() {
        let t1 = s[7]
            .wrapping_add(big_sigma1(s[4]))
            .wrapping_add(s[6] ^ (s[4] & (s[5] ^ s[6])))
            .wrapping_add(K[i])
            .wrapping_add(*wi);
        let t2 = big_sigma0(s[0]).wrapping_add((s[0] & s[1]) | (s[2] & (s[0] | s[1])));
        s = [t1.wrapping_add(t2), s[0], s[1], s[2], s[3].wrapping_add(t1), s[4], s[5], s[6]];
    }
    let mut out = [0u32; 8];
    for i in 0..8 {
        out[i] = state[i].wrapping_add(s[i]);
    }
    out
}

fn big_sigma0(v: u32) -> u32 {
    v.rotate_right(2) ^ v.rotate_right(13) ^ v.rotate_right(22)
}

fn big_sigma1(v: u32) -> u32 {
    v.rotate_right(6) ^ v.rotate_right(11) ^ v.rotate_right(25)
}

// 虚拟首块的初始状态: 用预计算调度表与变体常量, 从 rounds 14 恢复
fn variant_init_state(prefix7: &[u8]) -> [u32; 8] {
    debug_assert_eq!(prefix7.len(), 7);
    let mut words: Vec<u32> = vec![0; 14];
    words.push(u32::from_be_bytes([0, prefix7[0], prefix7[1], prefix7[2]]));
    words.push(u32::from_be_bytes([prefix7[3], prefix7[4], prefix7[5], prefix7[6]]));
    for index in 16..30 {
        let mut value = SHA256_SCHEDULE[index - 16];
        value = value.wrapping_add(if index == 16 { sigma1(words[14]) ^ SIGMA1_ADJUSTMENT } else { sigma1(words[index - 2]) });
        if index >= 21 {
            value = value.wrapping_add(words[index - 7]);
        }
        if index >= 29 {
            value = value.wrapping_add(sigma0(words[14]) ^ SIGMA0_ADJUSTMENT);
        }
        words.push(value);
    }
    words.push(
        sigma1(words[28])
            .wrapping_add(words[23])
            .wrapping_add(sigma0(words[15]))
            .wrapping_add(words[14])
            .wrapping_add(SHA256_ADJUSTMENT.wrapping_sub(K[14])),
    );
    expand_schedule(&mut words);
    // rounds 14..63, 常量 K14 换为调整值; 起始状态为 SHA256_STATE
    let mut s = SHA256_STATE;
    for (i, wi) in words.iter().enumerate().skip(14) {
        let konst = if i == 14 { SHA256_ADJUSTMENT } else { K[i] };
        let t1 = s[7]
            .wrapping_add(big_sigma1(s[4]))
            .wrapping_add(s[6] ^ (s[4] & (s[5] ^ s[6])))
            .wrapping_add(konst)
            .wrapping_add(*wi);
        let t2 = big_sigma0(s[0]).wrapping_add((s[0] & s[1]) | (s[2] & (s[0] | s[1])));
        s = [t1.wrapping_add(t2), s[0], s[1], s[2], s[3].wrapping_add(t1), s[4], s[5], s[6]];
    }
    let mut out = [0u32; 8];
    for i in 0..8 {
        out[i] = SHA256_INITIAL[i].wrapping_add(s[i]);
    }
    out
}

// 变体 SHA-256: 虚拟首块(前 7 字节 + 57 零字节) + 常规块(标准轮常量)
fn variant_sha256(data: &[u8]) -> [u8; 32] {
    debug_assert!(data.len() >= 7);
    let length = 57 + data.len();
    let mut padded = data[7..].to_vec();
    padded.push(0x80);
    let pad_zeros = ((55i64 - length as i64).rem_euclid(64)) as usize;
    padded.extend(std::iter::repeat_n(0u8, pad_zeros));
    padded.extend_from_slice(&((length as u64) * 8).to_be_bytes());

    let mut state = variant_init_state(&data[..7]);
    for block in padded.chunks(64) {
        state = sha256_block(&state, block);
    }
    let mut out = [0u8; 32];
    for (i, v) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

// CS64: 基于摘要派生密钥的定制 64 位摘要
const CS64_COEFFICIENTS: [(u32, u32); 14] = [
    (0xE82E91B1, 0xB32700D0),
    (0x44DE24C6, 0x88DFF51D),
    (0x3BEB65BC, 0x29514038),
    (0xC89C5933, 0xB328CBDF),
    (0x8D1EC211, 0xD405B4D5),
    (0x314EF5D5, 0x26E55F3C),
    (0xE92491B1, 0x607801C6),
    (0xF0A5B9D5, 0x82B2C1FB),
    (0x06064CF3, 0xFA51A3DA),
    (0xDA79FA60, 0x207CC464),
    (0xBED20560, 0x3C3C74F3),
    (0x89F9C9F8, 0xD2D14B61),
    (0x35FA38C6, 0xBB04A2C1),
    (0xCB49E3C6, 0xC5626167),
];
const CS64_FACTOR: u32 = 0x82893933;
const CS64_ADJUSTMENT0: u32 = 0xF2F90B1A;
const CS64_ADJUSTMENT1: u32 = 0x0B1A0000;

fn cs64_digest(data: &[u8], digest: &[u8; 32]) -> (u32, u32) {
    let mut key_ab = [0u32; 2];
    for (i, k) in key_ab.iter_mut().enumerate() {
        *k = u32::from_le_bytes(digest[i * 4..i * 4 + 4].try_into().unwrap()) | 1;
    }
    let [key_a, key_b] = key_ab;
    let factors = [key_a.wrapping_mul(CS64_FACTOR), key_b.wrapping_mul(CS64_FACTOR)];
    let mut terms: Vec<u32> = CS64_COEFFICIENTS
        .iter()
        .enumerate()
        .map(|(index, (coefficient, _))| coefficient.wrapping_mul(factors[index % 2]))
        .collect();
    let mut offsets: Vec<u32> = CS64_COEFFICIENTS.iter().map(|(_, offset)| *offset).collect();

    // words_le4_even(b"\x00" + data)
    let mut bytes = Vec::with_capacity(data.len() + 1);
    bytes.push(0u8);
    bytes.extend_from_slice(data);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    let mut words: Vec<u32> = bytes.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    if words.len() % 2 != 0 {
        words.push(0);
    }
    for (index, word) in words.iter().enumerate() {
        let mut term = word.wrapping_mul(if index % 2 == 0 { key_a } else { key_b });
        let mut offset = rol16(*word).wrapping_mul(if index % 2 == 0 { 0xEF05_69FB } else { 0xC317_13DB });
        if index == 0 {
            term = term.wrapping_add(CS64_ADJUSTMENT0.wrapping_mul(factors[0]));
            offset = offset.wrapping_add(CS64_ADJUSTMENT1);
        }
        terms.push(term);
        offsets.push(offset);
    }

    let mut swap_acc: u32 = 0;
    let mut swap_last: u32 = 0;
    let mut state: u32 = 0;
    let mut acc: u32 = 0;
    for pair in (0..terms.len()).step_by(2) {
        let (x, y) = (terms[pair], terms[pair + 1]);
        let t = rol16(offsets[pair].wrapping_add(x));
        let q = t.wrapping_mul(0x76F5_0F34);
        swap_acc = swap_acc.wrapping_add(rol16(q).wrapping_mul(0xC3EF_EA97).wrapping_add(q));
        let t = rol16(offsets[pair + 1].wrapping_add(y));
        let q = t.wrapping_mul(0x1881_CE3C);
        swap_last = rol16(q).wrapping_mul(0x35BD_1EC9).wrapping_add(q);
        swap_acc = swap_acc.wrapping_add(swap_last);

        state = state.wrapping_mul(key_a).wrapping_add(x);
        state = rol16(state).wrapping_mul(0xCF98_B111);
        state = rol16(state).wrapping_mul(0x8708_5B9F);
        state = rol16(state).wrapping_mul(0x12CE_B96D);
        state = rol16(state).wrapping_mul(0x257E_1D83);
        acc = acc.wrapping_add(state);
        let t = state.wrapping_mul(key_b).wrapping_add(y);
        let t = rol16(t).wrapping_mul(0xA274_16F5);
        let t = rol16(t).wrapping_mul(0xD383_96FF);
        state = rol16(rol16(t).wrapping_mul(0x7C93_2B89)).wrapping_mul(0xBFA4_9F69);
        acc = acc.wrapping_add(state);
    }
    (swap_last ^ state, swap_acc ^ acc)
}

// Sec-MS-GEC v2: "{CS64:016X}-{SHA256变体:064X}", 时间窗 5 分钟
fn sec_ms_gec() -> String {
    let mut ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u64)
        .unwrap_or(0);
    ticks += WIN_EPOCH;
    ticks -= ticks % 300;
    // 参考实现与 Edge 客户端一样走 f64 乘法, 大数有精度舍入, 必须保持一致
    let ticks = ((ticks as f64) * 1e7) as u64;
    token_for_ticks(ticks)
}

fn token_for_ticks(ticks: u64) -> String {
    let str_to_hash = format!("{ticks}{TRUSTED_TOKEN}");
    let digest = variant_sha256(str_to_hash.as_bytes());
    let (w0, w1) = cs64_digest(str_to_hash.as_bytes(), &digest);
    // pack("<II") 后取 hex: 每个字按小端字节序展开
    let cs64: String = [w0, w1]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .map(|b| format!("{b:02X}"))
        .collect();
    let sha = digest.iter().map(|b| format!("{b:02X}")).collect::<String>();
    format!("{cs64}-{sha}")
}

// 32 位大写十六进制随机串(muid Cookie)
fn make_muid() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let pid = std::process::id() as u64;
    let a = now.as_secs() ^ (pid << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let b = ((now.subsec_nanos() as u64) << 32) ^ pid.wrapping_mul(0x2545_F491_4F6C_DD1D);
    format!("{a:016X}{b:016X}")
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

    let url = format!(
        "{WSS_URL}?TrustedClientToken={TRUSTED_TOKEN}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={GEC_VERSION}",
        request_id(),
        sec_ms_gec()
    );
    let mut request = url.into_client_request().map_err(|e| format!("TTS 请求构造失败: {e}"))?;
    {
        let headers = request.headers_mut();
        headers.insert("User-Agent", UA.parse().unwrap());
        headers.insert("Origin", ORIGIN.parse().unwrap());
        headers.insert("Pragma", "no-cache".parse().unwrap());
        headers.insert("Cache-Control", "no-cache".parse().unwrap());
        headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        headers.insert("Cookie", format!("muid={};", make_muid()).parse().unwrap());
    }

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

    // Sec-MS-GEC v2 令牌移植对照: 固定输入的期望值由 rany2/edge-tts drm.py 参考实现生成
    #[test]
    fn test_sec_ms_gec_v2_vector() {
        let data = format!("{}{}", 1790000123000000000u64, super::TRUSTED_TOKEN);
        let digest = super::variant_sha256(data.as_bytes());
        let digest_hex: String = digest.iter().map(|b| format!("{b:02X}")).collect();
        assert_eq!(
            digest_hex,
            "8CB7B0432AC1AAC2C941FBB06A81E7EB7ABA499B8FAC161CC4A5F24DF7EC73BD",
            "变体 SHA256 不匹配"
        );
        let (w0, w1) = super::cs64_digest(data.as_bytes(), &digest);
        assert_eq!((w0, w1), (0x50BF_0D91, 0x6BC6_0A75), "CS64 字不匹配");
        // 完整令牌(小端打包 hex)
        assert_eq!(
            super::token_for_ticks(1790000123000000000),
            "910DBF50750AC66B-8CB7B0432AC1AAC2C941FBB06A81E7EB7ABA499B8FAC161CC4A5F24DF7EC73BD",
            "令牌不匹配"
        );
    }
}
