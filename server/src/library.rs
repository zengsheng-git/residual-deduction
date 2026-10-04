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
