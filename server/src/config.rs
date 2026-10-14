// 应用设置(存于应用配置目录 settings.json)
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

pub const VOICES: [(&str, &str); 4] = [
    ("zh-CN-YunxiNeural", "云希 · 男声解说"),
    ("zh-CN-YunyangNeural", "云扬 · 男声播报"),
    ("zh-CN-XiaoxiaoNeural", "晓晓 · 女声"),
    ("zh-CN-XiaoyiNeural", "晓伊 · 女声"),
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub voice: String,
    pub rate: i32,          // 语速百分比 -50~100
    pub depth: u32,         // 引擎搜索深度
    pub movetime: u32,      // 引擎搜索时限(ms)
    pub branch_max: u32,    // 分支分析节点上限
    pub voice_enabled: bool,    // 默认生成语音解说
    pub branches_enabled: bool, // 默认包含分支推演
    pub autoplay_sound: bool,
    #[serde(default = "default_aspect")]
    pub aspect: String, // 默认画幅: portrait 竖版9:16 / landscape 横版16:9
    #[serde(default = "default_true")]
    pub sfx_enabled: bool, // 走子/吃子/将军音效
    #[serde(default = "default_true")]
    pub bgm_enabled: bool, // 背景音乐(music 目录随机选用)
    #[serde(default = "default_bgm_volume")]
    pub bgm_volume: u32, // BGM 音量 0~100
    #[serde(default)]
    pub polish: crate::polish::PolishConfig, // AI 解说润色(可选)
}

fn default_aspect() -> String {
    "portrait".to_string()
}

fn default_true() -> bool {
    true
}

fn default_bgm_volume() -> u32 {
    20
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            voice: VOICES[0].0.to_string(),
            rate: 0,
            depth: 24,
            movetime: 3000,
            branch_max: 4,
            voice_enabled: true,
            branches_enabled: true,
            autoplay_sound: true,
            aspect: default_aspect(),
            sfx_enabled: true,
            bgm_enabled: true,
            bgm_volume: default_bgm_volume(),
            polish: Default::default(),
        }
    }
}

impl Settings {
    pub fn load(base: &Path) -> Self {
        let path = base.join("settings.json");
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => s,
                Err(_) => Self::default(),
            },
            Err(_) => {
                let s = Self::default();
                let _ = s.save(base);
                s
            }
        }
    }

    pub fn save(&self, base: &Path) -> Result<(), String> {
        let path = base.join("settings.json");
        fs::create_dir_all(base).map_err(|e| format!("创建配置目录失败: {e}"))?;
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("序列化失败: {e}"))?;
        fs::write(path, json).map_err(|e| format!("写入设置失败: {e}"))
    }
}
