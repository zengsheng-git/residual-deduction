// 视频库: 生成成品的存储与索引
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VideoMeta {
    pub id: String,
    pub title: String,
    pub verdict: String,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub created_ms: u64,
    pub move_count: usize,
    pub video_path: String,
    pub thumb_path: String,
    #[serde(default)] // 旧版 meta.json 无此字段
    pub polished: bool, // 解说词经过 AI 润色
}

pub fn videos_dir(base: &Path) -> PathBuf {
    base.join("videos")
}

pub fn list(base: &Path) -> Vec<VideoMeta> {
    let dir = videos_dir(base);
    let mut metas = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else { return metas };
    for entry in entries.flatten() {
        let meta_file = entry.path().join("meta.json");
        if let Ok(bytes) = fs::read(&meta_file)
            && let Ok(meta) = serde_json::from_slice::<VideoMeta>(&bytes)
        {
            metas.push(meta);
        }
    }
    metas.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    metas
}

pub fn get(base: &Path, id: &str) -> Result<serde_json::Value, String> {
    let path = videos_dir(base).join(id).join("script.json");
    let bytes = fs::read(&path).map_err(|e| format!("剧本文件读取失败: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("剧本解析失败: {e}"))
}

pub fn delete(base: &Path, id: &str) -> Result<(), String> {
    // 防路径穿越
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err("非法的视频 id".to_string());
    }
    let dir = videos_dir(base).join(id);
    if !dir.exists() {
        return Err("视频不存在".to_string());
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("删除失败: {e}"))
}

pub fn save_meta(base: &Path, meta: &VideoMeta) -> Result<(), String> {
    let dir = videos_dir(base).join(&meta.id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let json = serde_json::to_string_pretty(meta).map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(dir.join("meta.json"), json).map_err(|e| format!("写入元数据失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 旧版 meta.json(无 polished 字段)必须能正常读取
    #[test]
    fn test_meta_backward_compat() {
        let json = r#"{
            "id": "v20261007-120000",
            "title": "残局挑战: 红方3步绝杀",
            "verdict": "红方3步绝杀",
            "duration_secs": 60.0,
            "size_bytes": 1000000,
            "created_ms": 0,
            "move_count": 3,
            "video_path": "video.mp4",
            "thumb_path": "thumb.jpg"
        }"#;
        let meta: VideoMeta = serde_json::from_str(json).unwrap();
        assert!(!meta.polished);
    }

    #[test]
    fn test_meta_polished_roundtrip() {
        let meta = VideoMeta {
            id: "v1".into(),
            title: "t".into(),
            verdict: "v".into(),
            duration_secs: 1.0,
            size_bytes: 1,
            created_ms: 0,
            move_count: 1,
            video_path: "a".into(),
            thumb_path: "b".into(),
            polished: true,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: VideoMeta = serde_json::from_str(&json).unwrap();
        assert!(back.polished);
    }
}
