// 生成调度: 剧本 → 逐场景 TTS+渲染编码 → 拼接 → 入库, 带进度事件与取消
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::attack;
use crate::chess;
use crate::chess::Board;
use crate::chess::Camp;
use crate::engine::Engine;
use crate::engine::EngineConfig;
use crate::library;
use crate::library::VideoMeta;
use crate::render::BranchDemo;
use crate::render::FrameScene;
use crate::render::Renderer;
use crate::render::CANVAS_H;
use crate::render::CANVAS_W;
use crate::render::FPS;
use crate::storyboard;
use crate::tts;
use crate::video;

#[derive(Debug, Clone, Serialize)]
pub struct GenProgress {
    pub stage: String, // analyse / tts / render / compose / done
    pub current: u32,
    pub total: u32,
    pub message: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GenParams {
    pub pieces: Vec<chess::Position>,
    pub side: char,  // 'w' 红 / 'b' 黑
    pub depth: u32,
    pub movetime: u32,
    pub branch_max: u32,
    pub voice: String,
    pub rate: i32,
    #[serde(default = "default_true")]
    pub voice_enabled: bool,     // 是否生成语音解说(关闭则字幕静音)
    #[serde(default = "default_true")]
    pub branches_enabled: bool,  // 是否包含分支推演(关闭则纯主线)
}

fn default_true() -> bool {
    true
}

// 生成一段静音 wav(网络 TTS 失败时兜底, 保证视频仍可产出)
fn write_silent_wav(path: &Path, seconds: f64) -> Result<(), String> {
    let sample_rate = 24000u32;
    let samples = (seconds * sample_rate as f64) as u32;
    let data_len = samples * 2;
    let mut bytes: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // pcm
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::write(path, bytes).map_err(|e| format!("写入静音音频失败: {e}"))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn make_id() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = {
        let days = (secs / 86400) as i64;
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        (if m <= 2 { y + 1 } else { y }, m, day)
    };
    let (hh, mm, ss) = ((secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    format!("v{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss}")
}

fn frame_to_rgb(frame: &image::RgbaImage) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((CANVAS_W * CANVAS_H * 3) as usize);
    for p in frame.pixels() {
        rgb.extend_from_slice(&[p[0], p[1], p[2]]);
    }
    rgb
}

// 构造场景帧数据
#[allow(clippy::too_many_arguments)]
fn build_frame_scenes(script: &storyboard::Script, start_board: &Board) -> (Vec<FrameScene>, Board) {
    let mut scenes: Vec<FrameScene> = Vec::new();
    let mut log: Vec<String> = Vec::new();

    // 开场
    scenes.push(FrameScene {
        board_before: chess::board_map(*start_board).into_iter().filter(|p| p.piece != ' ').collect(),
        board_after: chess::board_map(*start_board).into_iter().filter(|p| p.piece != ' ').collect(),
        move_iccs: None,
        branch_demo: None,
        camp: script.camp,
        title: script.title.clone(),
        verdict: script.verdict.clone(),
        score_line: "引擎开局推演".to_string(),
        winrate: None,
        log: vec![],
        current_idx: None,
        branches: vec![],
        info_lines: script.intro_info.clone(),
        subtitle: script.intro_comment.clone(),
        check_pos: None,
        anim_ratio: 0.0,
    });

    // 主线走子
    let mut cur = *start_board;
    for s in &script.scenes {
        let before = chess::board_from_positions(&s.board_before).unwrap_or(cur);
        let after = chess::board_move(before, &s.iccs);
        log.push(format!("{}. {}  {}", s.ply, s.chinese, s.score_text));
        let idx = log.len() - 1;

        let mut branches = Vec::new();
        for b in &s.branches {
            branches.push(format!("{}   {}", b.chinese, b.note));
        }

        let check_pos = if s.check { find_king(&after, if s.camp == 'w' { 'k' } else { 'K' }) } else { None };

        scenes.push(FrameScene {
            board_before: s.board_before.clone(),
            board_after: chess::board_map(after).into_iter().filter(|p| p.piece != ' ').collect(),
            move_iccs: Some(s.iccs.clone()),
            branch_demo: None,
            camp: s.camp,
            title: script.title.clone(),
            verdict: script.verdict.clone(),
            score_line: format!("第{}步 · 评分 {}", s.ply, s.score_text),
            winrate: s.winrate,
            log: log.clone(),
            current_idx: Some(idx),
            branches,
            info_lines: vec![],
            subtitle: s.comment.clone(),
            check_pos,
            anim_ratio: 0.35,
        });
        cur = after;

        // 分支演示场景: 回到本步走子前, 假设改走备选着法, 并演示对方的反制
        for b in &s.branches {
            let after_alt = chess::board_move(before, &b.iccs);
            let demo_final = b.reply.as_ref().map(|r| chess::board_move(after_alt, r)).unwrap_or(after_alt);
            // 演示线最后一手若形成将军, 落定帧高亮被将的将帅
            let last_mover_char = if b.reply.is_some() { if s.camp == 'w' { 'b' } else { 'w' } } else { s.camp };
            let check_pos = if attack::gives_check(&demo_final, &Camp::from_char(last_mover_char)) {
                find_king(&demo_final, if last_mover_char == 'w' { 'k' } else { 'K' })
            } else {
                None
            };
            scenes.push(FrameScene {
                board_before: s.board_before.clone(),
                board_after: chess::board_map(demo_final).into_iter().filter(|p| p.piece != ' ').collect(),
                move_iccs: None,
                branch_demo: Some(BranchDemo { alt_iccs: b.iccs.clone(), reply_iccs: b.reply.clone() }),
                camp: s.camp,
                title: script.title.clone(),
                verdict: script.verdict.clone(),
                score_line: format!("分支演示 · 假如{}方不走{} · {}", if s.camp == 'w' { "红" } else { "黑" }, s.chinese, b.note),
                winrate: None,
                log: log.clone(),
                current_idx: Some(idx),
                branches: vec![format!("改走{}   {}", b.chinese, b.note)],
                info_lines: vec![],
                subtitle: b.demo_comment.clone(),
                check_pos,
                anim_ratio: if b.reply.is_some() { 0.7 } else { 0.35 },
            });
        }
    }

    // 结尾
    let final_move_text = script.scenes.last().map(|s| format!("终局: {}", s.chinese)).unwrap_or_default();
    let mut outro_info = script.outro_info.clone();
    if let Some(last) = script.scenes.last() {
        if last.mate {
            outro_info[1] = final_move_text;
        }
    }
    scenes.push(FrameScene {
        board_before: chess::board_map(cur).into_iter().filter(|p| p.piece != ' ').collect(),
        board_after: chess::board_map(cur).into_iter().filter(|p| p.piece != ' ').collect(),
        move_iccs: None,
        branch_demo: None,
        camp: script.camp,
        title: script.title.clone(),
        verdict: script.verdict.clone(),
        score_line: "推演完成".to_string(),
        winrate: None,
        log,
        current_idx: None,
        branches: vec![],
        info_lines: outro_info,
        subtitle: script.outro_comment.clone(),
        check_pos: None,
        anim_ratio: 0.0,
    });
    (scenes, cur)
}

fn find_king(board: &Board, king: char) -> Option<(usize, usize)> {
    for y in 0..10 {
        for x in 0..9 {
            if board[y][x] == king {
                return Some((x, y));
            }
        }
    }
    None
}

const LEAD: f64 = 0.5; // 每场景开头留白
const TAIL: f64 = 0.6; // 每场景结尾留白
const MIN_DUR: f64 = 2.4;

pub fn generate(
    engine: Arc<Mutex<Engine>>,
    ffmpeg: &Path,
    base_dir: &Path,
    params: &GenParams,
    stop: &AtomicBool,
    progress: &dyn Fn(GenProgress),
) -> Result<VideoMeta, String> {
    let board = chess::board_from_positions(&params.pieces).map_err(|e| format!("局面数据无效: {e}"))?;
    if !chess::board_check(board) {
        return Err("局面未通过合法性校验, 请先核对识别结果".to_string());
    }
    let camp = Camp::from_char(params.side);
    if camp == Camp::None {
        return Err("请选择先行方".to_string());
    }

    // 1. 引擎推演剧本
    progress(GenProgress { stage: "analyse".into(), current: 0, total: 0, message: "引擎推演主线与分支...".into() });
    let script = {
        let mut eng = engine.lock().map_err(|_| "引擎被占用".to_string())?;
        eng.set_threads(4);
        let cfg = EngineConfig {
            depth: params.depth as usize,
            time: params.movetime as usize,
            multipv: if params.branches_enabled { 3 } else { 1 },
            ..Default::default()
        };
        let branch_budget = if params.branches_enabled { params.branch_max } else { 0 };
        let out = storyboard::build(&mut eng, &cfg, board, camp, stop, branch_budget)?;
        out.script
    };

    // 2. 场景帧数据
    let (frame_scenes, final_board) = build_frame_scenes(&script, &board);
    let total_scenes = frame_scenes.len() as u32;

    // 3. 输出目录
    let id = make_id();
    let work_dir = library::videos_dir(base_dir).join(&id);
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    let renderer = Renderer::new()?;

    let mut segments: Vec<PathBuf> = Vec::new();
    let mut total_duration = 0f64;

    for (i, scene) in frame_scenes.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(&work_dir);
            return Err("已取消".to_string());
        }
        let is_move = scene.move_iccs.is_some() || scene.branch_demo.is_some();
        // TTS
        progress(GenProgress { stage: "tts".into(), current: i as u32, total: total_scenes, message: format!("配音 {}/{}: {}", i + 1, total_scenes, scene.subtitle.chars().take(18).collect::<String>()) });
        let mp3_path = work_dir.join(format!("scene_{i:02}.mp3"));
        let wav_path = work_dir.join(format!("scene_{i:02}.wav"));
        let mut use_silent = !params.voice_enabled;
        if !params.voice_enabled {
            progress(GenProgress { stage: "tts".into(), current: i as u32, total: total_scenes, message: "语音解说已关闭, 本段静音".into() });
        }
        if params.voice_enabled {
            match tts::synthesize(&scene.subtitle, &params.voice, params.rate) {
                Ok(bytes) => {
                    std::fs::write(&mp3_path, &bytes).map_err(|e| format!("写入音频失败: {e}"))?;
                    video::mp3_to_wav(ffmpeg, &mp3_path, &wav_path)?;
                }
                Err(e) => {
                    tracing::warn!("TTS 失败, 使用静音兜底: {e}");
                    use_silent = true;
                    progress(GenProgress { stage: "tts".into(), current: i as u32, total: total_scenes, message: format!("配音失败, 本段将静音(视频不受影响): {e}") });
                }
            }
        }

        // 场景时长
        let speech = if use_silent { None } else { Some(video::wav_duration(&wav_path)?) };
        let dur = match speech {
            Some(d) => (LEAD + d + TAIL).max(MIN_DUR),
            None => {
                let est = 1.2 + scene.subtitle.chars().count() as f64 * 0.22;
                (LEAD + est + TAIL).max(MIN_DUR)
            }
        };
        if use_silent {
            write_silent_wav(&wav_path, dur)?;
        }
        total_duration += dur;

        // 渲染帧
        let total_frames = (dur * FPS as f64) as usize;
        let anim_frames = if is_move { ((dur * scene.anim_ratio as f64) * FPS as f64) as usize } else { 0 };
        let anim_frames = anim_frames.clamp(1, total_frames.saturating_sub(1));
        let mut frame_idx = 0usize;
        let renderer_ref = &renderer;
        let scene_ref = scene;
        let frames = std::iter::from_fn(move || {
            if frame_idx >= total_frames {
                return None;
            }
            let t = if frame_idx < anim_frames {
                frame_idx as f32 / anim_frames as f32
            } else {
                1.0
            };
            frame_idx += 1;
            let frame = renderer_ref.render_frame(scene_ref, t);
            Some(frame_to_rgb(&frame))
        });

        let seg_path = work_dir.join(format!("scene_{i:02}.mp4"));
        progress(GenProgress { stage: "render".into(), current: i as u32, total: total_scenes, message: format!("渲染编码 {}/{}", i + 1, total_scenes) });
        video::encode_segment(ffmpeg, &wav_path, &seg_path, &mut frames.filter_map(|v| if v.len() == (CANVAS_W * CANVAS_H * 3) as usize { Some(v) } else { None }))?;
        segments.push(seg_path);
    }

    // 4. 拼接 + 封面
    progress(GenProgress { stage: "compose".into(), current: total_scenes, total: total_scenes, message: "拼接片段并生成封面...".into() });
    let video_path = work_dir.join("video.mp4");
    video::concat_segments(ffmpeg, &segments, &video_path, &work_dir)?;
    let thumb_path = work_dir.join("thumb.jpg");
    let thumb_at = (total_duration * 0.45).min(4.0).max(1.0);
    video::extract_thumbnail(ffmpeg, &video_path, &thumb_path, thumb_at)?;
    let duration = video::video_duration(ffmpeg, &video_path).unwrap_or(total_duration);

    // 5. 写元数据与剧本
    let size = std::fs::metadata(&video_path).map(|m| m.len()).unwrap_or(0);
    let meta = VideoMeta {
        id: id.clone(),
        title: script.title.clone(),
        verdict: script.verdict.clone(),
        duration_secs: duration,
        size_bytes: size,
        created_ms: now_ms(),
        move_count: script.scenes.len(),
        video_path: video_path.to_string_lossy().into_owned(),
        thumb_path: thumb_path.to_string_lossy().into_owned(),
    };
    library::save_meta(base_dir, &meta)?;
    let script_json = serde_json::to_string_pretty(&script).map_err(|e| format!("剧本序列化失败: {e}"))?;
    std::fs::write(work_dir.join("script.json"), script_json).map_err(|e| format!("写入剧本失败: {e}"))?;

    // 清理中间产物(保留成品/封面/元数据/剧本)
    for seg in &segments {
        let _ = std::fs::remove_file(seg);
    }
    if let Ok(entries) = std::fs::read_dir(&work_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".mp3") || name.ends_with(".wav") || name == "concat.txt" {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    progress(GenProgress { stage: "done".into(), current: total_scenes, total: total_scenes, message: "生成完成".into() });
    let _ = final_board;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 端到端全流程: 引擎+TTS(联网)+渲染+ffmpeg
    fn test_generate_full_video() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let libs = manifest.join("../libs");
        let base = manifest.join("../scripts/out/appdata");
        std::fs::create_dir_all(&base).unwrap();

        // 车炮对双士的必胜残局
        let board = crate::chess::fen_to_board("3k4/9/9/9/9/9/9/9/4C4/3KR4 w");
        let params = GenParams {
            pieces: crate::chess::board_map(board).into_iter().filter(|p| p.piece != ' ').collect(),
            side: 'w',
            depth: 20,
            movetime: 1500,
            branch_max: 3,
            voice: "zh-CN-YunxiNeural".into(),
            rate: 0,
            voice_enabled: true,
            branches_enabled: true,
        };

        let libs_pikafish = libs.join("pikafish");
        let (child, stdin) = crate::engine::command::new(&libs_pikafish);
        let engine = Arc::new(Mutex::new(crate::engine::Engine::from_child(child, stdin, &libs_pikafish)));

        let ffmpeg = libs.join("ffmpeg/ffmpeg.exe");
        let meta = generate(
            engine,
            &ffmpeg,
            &base,
            &params,
            &AtomicBool::new(false),
            &|p| println!("[{:?} {}/{}] {}", p.stage, p.current, p.total, p.message),
        )
        .expect("生成失败");

        println!("video: {}", meta.video_path);
        println!("duration: {:.1}s size: {} bytes", meta.duration_secs, meta.size_bytes);
        assert!(std::path::Path::new(&meta.video_path).exists());
        assert!(meta.size_bytes > 100_000);
    }
}

// 分支演示场景测试
#[cfg(test)]
mod branch_demo_tests {
    use super::*;
    use crate::chess::fen_to_board;

    fn demo_script() -> (storyboard::Script, Board) {
        let board = fen_to_board("3k5/9/9/9/9/9/9/9/4C4/4KR3 w");
        let pieces = chess::board_map(board).into_iter().filter(|p| p.piece != ' ').collect();
        let script = storyboard::Script {
            title: "残局推演: 红方主动进攻".into(),
            camp: 'w',
            verdict: "红方优势明显".into(),
            intro_comment: "开场".into(),
            intro_info: vec![],
            outro_comment: "结尾".into(),
            outro_info: vec![],
            scenes: vec![storyboard::MoveScene {
                ply: 2,
                camp: 'b',
                board_before: pieces,
                iccs: "d9e9".into(),
                chinese: "将4平5".into(),
                score: -300,
                score_text: "-300 较差".into(),
                winrate: Some(400),
                capture: None,
                check: false,
                mate: false,
                comment: "将4平5".into(),
                branches: vec![storyboard::Branch {
                    iccs: "d9d8".into(),
                    chinese: "将4退1".into(),
                    gap: 320,
                    note: "明显吃亏".into(),
                    reply: Some("f0f8".into()),
                    reply_chinese: Some("车四进八".into()),
                    demo_comment: "假如黑方不走将4平5, 改走将4退1, 红方立即应以车四进八, 黑方明显吃亏。".into(),
                }],
            }],
        };
        (script, board)
    }

    // 带分支的剧本 → build_frame_scenes 应在分支节点后追加演示场景
    #[test]
    fn test_build_frame_scenes_with_branch_demo() {
        let (script, board) = demo_script();
        let (scenes, _) = build_frame_scenes(&script, &board);
        // 开场 + 主线 + 分支演示 + 结尾
        assert_eq!(scenes.len(), 4);

        let demo = &scenes[2];
        assert!(demo.move_iccs.is_none());
        let bd = demo.branch_demo.as_ref().expect("应有分支演示数据");
        assert_eq!(bd.alt_iccs, "d9d8");
        assert_eq!(bd.reply_iccs.as_deref(), Some("f0f8"));
        assert!((demo.anim_ratio - 0.7).abs() < 1e-6, "有反制时动画应占 0.7");
        assert!(demo.subtitle.contains("假如黑方不走"));
        assert!(demo.score_line.contains("假如黑方不走将4平5"));

        // 演示场景落定局面 = 根局面 + 备选防着 + 进攻方反制
        let root = chess::board_from_positions(&script.scenes[0].board_before).unwrap();
        let expected = chess::board_move(chess::board_move(root, "d9d8"), "f0f8");
        let after = chess::board_from_positions(&demo.board_after).unwrap();
        assert_eq!(after, expected);
    }

    // 视觉验证: 渲染演示场景关键帧到 scripts/out/branch_demo/ (人工查看)
    #[test]
    #[ignore]
    fn test_dump_branch_demo_frames() {
        let (script, board) = demo_script();
        let (scenes, _) = build_frame_scenes(&script, &board);
        let renderer = Renderer::new().unwrap();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/out/branch_demo");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, t) in [("1_x_anim", 0.15), ("2_x_done", 0.45), ("3_reply_anim", 0.65), ("4_reply_done", 0.9), ("5_settled", 1.0)] {
            let frame = renderer.render_frame(&scenes[2], t);
            let path = dir.join(format!("{name}.png"));
            frame.save(&path).unwrap();
            println!("saved {}", path.display());
        }
    }
}

// 追加测试: 开关关闭路径的快速验证
#[cfg(test)]
mod switch_tests {
    use super::*;

    // 静音 wav 写入 → 时长解析往返
    #[test]
    fn test_silent_wav_roundtrip() {
        let dir = std::env::temp_dir().join("residual-silent-test");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("silent.wav");
        write_silent_wav(&wav, 3.5).unwrap();
        let dur = video::wav_duration(&wav).unwrap();
        assert!((dur - 3.5).abs() < 0.01, "duration={dur}");
        std::fs::remove_file(&wav).ok();
    }

    // 分支预算为 0 时剧本不含分支(需要本地引擎)
    #[test]
    #[ignore]
    fn test_build_without_branches() {
        let libs = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../libs/pikafish");
        let (child, stdin) = crate::engine::command::new(&libs);
        let mut engine = crate::engine::Engine::from_child(child, stdin, &libs);
        engine.set_threads(4);
        let board = crate::chess::fen_to_board("3k5/9/9/9/9/9/9/9/4C4/4KR3 w");
        let cfg = crate::engine::EngineConfig { depth: 16, time: 800, ..Default::default() };
        let out = storyboard::build(&mut engine, &cfg, board, crate::chess::Camp::Red, &AtomicBool::new(false), 0)
            .expect("构建失败");
        for s in &out.script.scenes {
            assert!(s.branches.is_empty(), "预算为0时不应有分支: {:?}", s.branches);
        }
        println!("scenes={} (无分支路径 OK)", out.script.scenes.len());
    }
}
