// LLM 解说润色: 把模板生成的解说词批量交给 OpenAI 兼容接口改写成真人语气。
// 未启用/未配置/调用失败时保持模板原文, 不阻塞出片。
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

use crate::storyboard::Script;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolishConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: String, // OpenAI 兼容接口, 如 https://api.deepseek.com 或 https://api.openai.com/v1
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

impl PolishConfig {
    // 是否具备调用润色接口的完整配置
    pub fn ready(&self) -> bool {
        self.enabled
            && !self.base_url.trim().is_empty()
            && !self.api_key.trim().is_empty()
            && !self.model.trim().is_empty()
    }
}

// 接口端点: 允许用户填根地址或完整路径
fn endpoint(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .build()
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max_chars).collect();
        format!("{cut}…")
    }
}

const SYSTEM_PROMPT: &str = "你是一位象棋残局视频的解说撰稿人。用户会给你一个 JSON 字符串数组, 按顺序对应视频各场景的解说词草稿(由模板生成)。请逐条改写成自然、口语化、像真人棋手临场讲解的解说词。硬性要求: 1) 草稿中的着法记法(如 炮二平五、将4进1)、步数等数字、红方/黑方等事实必须原样保留, 不得改动或增删; 2) 严禁出现 引擎、AI、算法、程序、软件 等词, 全程以真人视角讲解; 3) 不得引入草稿中没有的棋评结论, 只改表达方式; 4) 每条长度与草稿相近(用于字幕与配音), 多用短句; 5) 只输出一个 JSON 字符串数组, 与输入等长且顺序一致, 不要输出任何解释、不要使用代码块。";

// 收集全部待润色文本: [标题, 开场, 结尾, 各场景解说..., 各分支演示词...]
fn collect_texts(script: &Script) -> Vec<String> {
    let mut texts = vec![script.title.clone(), script.intro_comment.clone(), script.outro_comment.clone()];
    for s in &script.scenes {
        texts.push(s.comment.clone());
        for b in &s.branches {
            texts.push(b.demo_comment.clone());
        }
    }
    texts
}

// 按收集时的顺序回写
fn apply_texts(script: &mut Script, texts: &[String]) {
    let mut it = texts.iter();
    if let Some(t) = it.next() {
        script.title = t.clone();
    }
    if let Some(t) = it.next() {
        script.intro_comment = t.clone();
    }
    if let Some(t) = it.next() {
        script.outro_comment = t.clone();
    }
    for s in &mut script.scenes {
        if let Some(t) = it.next() {
            s.comment = t.clone();
        }
        for b in &mut s.branches {
            if let Some(t) = it.next() {
                b.demo_comment = t.clone();
            }
        }
    }
}

// 提取响应中所有顶层 JSON 数组片段(跳过字符串内的括号与转义), 供逐一尝试解析。
// 模型常在数组前后附加说明文字, 甚至文字里带方括号, 不能简单按首尾括号截取。
fn top_level_arrays(content: &str) -> Vec<&str> {
    let bytes = content.as_bytes();
    let mut out: Vec<&str> = Vec::new();
    let mut start: Option<usize> = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b']' if depth > 0 => {
                depth -= 1;
                if depth == 0 && let Some(s) = start.take() {
                    out.push(&content[s..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

// 从响应中解析字符串数组: 依次尝试每个顶层候选, 第一个能解析为字符串数组的即为结果
fn parse_json_array(content: &str) -> Result<Vec<String>, String> {
    let candidates = top_level_arrays(content);
    if candidates.is_empty() {
        if content.contains('[') {
            return Err("JSON 数组未闭合, 响应可能被截断".into());
        }
        return Err("响应中未找到 JSON 数组".into());
    }
    let mut last_err = String::new();
    for slice in &candidates {
        match serde_json::from_str::<Vec<String>>(slice) {
            Ok(items) => return Ok(items),
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(format!("解析润色结果失败: {last_err}"))
}

// 批量润色。返回润色失败的段数(0=全部成功), Err 仅在取消时出现。
// 失败的段保留模板原文, 其余段正常回写。
pub fn polish(script: &mut Script, cfg: &PolishConfig, stop: &AtomicBool) -> Result<usize, String> {
    if !cfg.ready() {
        return Ok(0);
    }
    if stop.load(Ordering::Relaxed) {
        return Err("已取消".into());
    }
    let texts = collect_texts(script);
    if texts.is_empty() {
        return Ok(0);
    }

    // 分小段请求: 一次性改写全部条目时, 长响应容易被模型 max_tokens 截断(JSON 未闭合)
    let url = endpoint(&cfg.base_url);
    let agent = http_agent();

    let mut polished: Vec<Option<String>> = vec![None; texts.len()];
    let mut failed: Vec<String> = Vec::new();
    for (ci, chunk) in texts.chunks(CHUNK_SIZE).enumerate() {
        if stop.load(Ordering::Relaxed) {
            return Err("已取消".into());
        }
        match polish_chunk_with_retry(&agent, &url, cfg, chunk, stop) {
            Ok(items) => {
                for (k, item) in items.into_iter().enumerate() {
                    polished[ci * CHUNK_SIZE + k] = Some(item);
                }
            }
            Err(e) => {
                tracing::warn!("润色第 {} 段失败, 该段保留模板原文: {e}", ci + 1);
                failed.push(format!("第{}段 {e}", ci + 1));
            }
        }
    }

    let final_texts: Vec<String> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| polished[i].clone().unwrap_or_else(|| t.clone()))
        .collect();
    // 逐条记录润色明细(仅记录发生变化的), 便于在日志中确认模型是否真正生效
    for (i, (before, after)) in texts.iter().zip(final_texts.iter()).enumerate() {
        if before != after {
            tracing::info!("润色 #{:>2}: {} → {}", i + 1, truncate(before, 36), truncate(after, 48));
        }
    }
    apply_texts(script, &final_texts);
    Ok(failed.len())
}

const CHUNK_SIZE: usize = 5; // 每次请求改写的条数上限
const MAX_ATTEMPTS: usize = 3; // 单段请求的最大尝试次数(网络抖动重试)

// 带重试的单段润色: 连接超时/响应异常等瞬时失败按短退避重试
fn polish_chunk_with_retry(
    agent: &ureq::Agent,
    url: &str,
    cfg: &PolishConfig,
    texts: &[String],
    stop: &AtomicBool,
) -> Result<Vec<String>, String> {
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        if stop.load(Ordering::Relaxed) {
            return Err("已取消".into());
        }
        match polish_chunk(agent, url, cfg, texts) {
            Ok(items) => return Ok(items),
            Err(e) => {
                last_err = e;
                if attempt < MAX_ATTEMPTS {
                    tracing::warn!("润色请求失败(第 {attempt}/{MAX_ATTEMPTS} 次), 稍后重试: {last_err}");
                    std::thread::sleep(std::time::Duration::from_millis(600 * attempt as u64));
                }
            }
        }
    }
    Err(format!("重试 {MAX_ATTEMPTS} 次后仍失败: {last_err}"))
}

fn polish_chunk(agent: &ureq::Agent, url: &str, cfg: &PolishConfig, texts: &[String]) -> Result<Vec<String>, String> {
    let body = serde_json::json!({
        "model": cfg.model.trim(),
        "temperature": 0.7,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": serde_json::to_string(texts).map_err(|e| e.to_string())?},
        ],
    });

    let response = agent
        .post(url)
        .set("Authorization", &format!("Bearer {}", cfg.api_key.trim()))
        .send_json(body)
        .map_err(|e| format!("请求润色接口失败: {e}"))?;
    let value: serde_json::Value = response.into_json().map_err(|e| format!("解析润色响应失败: {e}"))?;
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("润色响应缺少内容")?;
    let polished = parse_json_array(content)?;
    if polished.len() != texts.len() {
        return Err(format!("润色返回条数不符: 期望 {}, 实际 {}", texts.len(), polished.len()));
    }
    // 兜底: 即使模型漏改, 也不允许机器词进入成片
    Ok(polished
        .into_iter()
        .map(|s| s.replace("引擎", "").replace("AI", "").replace("算法", ""))
        .collect())
}

// 设置页"测试连接": 用一条示例解说词验证接口连通性与模型可用性, 返回模型改写结果
pub fn test_connection(cfg: &PolishConfig) -> Result<String, String> {
    let mut probe = cfg.clone();
    probe.enabled = true; // 测试时强制启用, 便于在打开开关前验证配置
    if !probe.ready() {
        return Err("请先填写接口地址、API Key 和模型名称".into());
    }
    let agent = http_agent();
    let url = endpoint(&probe.base_url);
    let sample = "炮二平五, 吃掉黑马, 将军!";
    let items = polish_chunk(&agent, &url, &probe, &[sample.to_string()])?;
    items.into_iter().next().ok_or_else(|| "模型返回为空".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess;
    use crate::chess::fen_to_board;

    fn demo_script() -> Script {
        let board = fen_to_board("3k5/9/9/9/9/9/9/9/4C4/4KR3 w");
        let pieces = chess::board_map(board).into_iter().filter(|p| p.piece != ' ').collect();
        Script {
            title: "残局推演: 红方主动进攻".into(),
            camp: 'w',
            verdict: "红方优势明显".into(),
            intro_comment: "开场解说".into(),
            intro_info: vec![],
            outro_comment: "结尾解说".into(),
            outro_info: vec![],
            polished: false,
            polish_model: String::new(),
            scenes: vec![crate::storyboard::MoveScene {
                ply: 1,
                camp: 'w',
                board_before: pieces,
                iccs: "e1e5".into(),
                chinese: "炮一进四".into(),
                score: 300,
                score_text: "+300 较优".into(),
                winrate: Some(600),
                capture: None,
                check: false,
                mate: false,
                comment: "主着解说".into(),
                branches: vec![crate::storyboard::Branch {
                    iccs: "f0f5".into(),
                    chinese: "车四进五".into(),
                    gap: 320,
                    note: "明显吃亏".into(),
                    reply: None,
                    reply_chinese: None,
                    demo_comment: "分支演示解说".into(),
                }],
            }],
        }
    }

    #[test]
    fn test_collect_apply_roundtrip() {
        let mut script = demo_script();
        let texts = collect_texts(&script);
        assert_eq!(texts, vec!["残局推演: 红方主动进攻", "开场解说", "结尾解说", "主着解说", "分支演示解说"]);

        let shifted: Vec<String> = texts.iter().map(|t| format!("{t}!")).collect();
        apply_texts(&mut script, &shifted);
        assert_eq!(script.title, "残局推演: 红方主动进攻!");
        assert_eq!(script.intro_comment, "开场解说!");
        assert_eq!(script.outro_comment, "结尾解说!");
        assert_eq!(script.scenes[0].comment, "主着解说!");
        assert_eq!(script.scenes[0].branches[0].demo_comment, "分支演示解说!");
    }

    #[test]
    fn test_parse_json_array_with_fence() {
        let content = "好的, 以下是润色结果:\n```json\n[\"a\", \"b\"]\n```";
        assert_eq!(parse_json_array(content).unwrap(), vec!["a".to_string(), "b".to_string()]);
        assert!(parse_json_array("没有数组").is_err());
    }

    #[test]
    fn test_parse_json_array_with_trailing_prose() {
        // 数组后跟说明文字, 且文字里带方括号(此前会报 trailing characters)
        let content = "[\"a\", \"b\"]\n以上是 [共2条] 润色结果。";
        assert_eq!(parse_json_array(content).unwrap(), vec!["a".to_string(), "b".to_string()]);
        // 数组前的文字带括号也不能干扰
        let content = "结果[1]: [\"a\", \"b\"]";
        assert_eq!(parse_json_array(content).unwrap(), vec!["a".to_string(), "b".to_string()]);
        // 对象包裹的数组
        let content = "{\"items\": [\"a\", \"b\"]}";
        assert_eq!(parse_json_array(content).unwrap(), vec!["a".to_string(), "b".to_string()]);
        // 字符串内的括号不参与配对
        let content = "[\"带[括号]的文本\", \"b\"]";
        assert_eq!(parse_json_array(content).unwrap(), vec!["带[括号]的文本".to_string(), "b".to_string()]);
        // 截断(只有 [ 没有 ] )
        assert!(parse_json_array("[\"a\", \"b").is_err());
    }

    #[test]
    fn test_polish_disabled_keeps_original() {
        let mut script = demo_script();
        let original = script.scenes[0].comment.clone();
        let failed = polish(&mut script, &PolishConfig::default(), &AtomicBool::new(false)).unwrap();
        assert_eq!(failed, 0);
        assert_eq!(script.scenes[0].comment, original);
    }
}
