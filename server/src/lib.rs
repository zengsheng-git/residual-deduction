// 应用入口: Tauri 装配、共享状态与全部命令
mod attack;
mod chess;
mod common;
mod config;
mod debug_sprite;
mod engine;
mod generator;
mod library;
mod logger;
mod narrator;
mod polish;
mod recognize;
mod render;
mod storyboard;
mod tts;
mod video;
mod yolo;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::RwLock;

use engine::Engine;
use generator::GenParams;
use generator::GenProgress;
use library::VideoMeta;
use tauri::AppHandle;
use tauri::Emitter as _;
use tauri::Manager as _;

struct SharedState {
    engine: Arc<Mutex<Engine>>,
    job: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    base_dir: PathBuf,
    ffmpeg: PathBuf,
    settings: Arc<RwLock<config::Settings>>,
    settings_dir: PathBuf,
}

static SHARED_STATE: OnceLock<SharedState> = OnceLock::new();

fn state() -> &'static SharedState {
    SHARED_STATE.get().expect("shared state not initialized")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let _guard = logger::init_tracer(tracing::Level::INFO, &app.path().app_data_dir().unwrap());

            let libs_dir = app.path().resolve("../libs", tauri::path::BaseDirectory::Resource).unwrap();

            // ort 以 load-dynamic 方式加载 onnxruntime
            #[cfg(target_os = "windows")]
            {
                let dylib = libs_dir.join("windows-cpu/onnxruntime.dll");
                if dylib.exists() && std::env::var_os("ORT_DYLIB_PATH").is_none() {
                    // 设置一次动态库路径, 需在首次推理前完成
                    unsafe { std::env::set_var("ORT_DYLIB_PATH", &dylib) };
                }
            }

            let pikafish_dir = libs_dir.join("pikafish");
            let (child, stdin) = engine::command::new(&pikafish_dir);
            let mut engine = Engine::from_child(child, stdin, &pikafish_dir);
            engine.set_threads(4);
            engine.set_hash(256);

            let ffmpeg = libs_dir.join("ffmpeg/ffmpeg.exe");
            if !ffmpeg.exists() {
                return Err(format!("未找到 ffmpeg: {}", ffmpeg.display()).into());
            }

            let settings_dir = app.path().app_config_dir().unwrap();
            let settings = config::Settings::load(&settings_dir);

            let _ = SHARED_STATE.get_or_init(|| SharedState {
                engine: Arc::new(Mutex::new(engine)),
                job: Arc::new(Mutex::new(None)),
                base_dir: app.path().app_data_dir().unwrap(),
                ffmpeg,
                settings: Arc::new(RwLock::new(settings)),
                settings_dir,
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            recognize_image,
            generate_video,
            stop_generation,
            list_videos,
            get_video_script,
            delete_video,
            open_videos_folder,
            get_settings,
            save_settings,
            test_polish_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------- 识别 ----------

#[tauri::command]
fn recognize_image(bytes: Vec<u8>) -> Result<recognize::Recognition, String> {
    let img = image::load_from_memory(&bytes).map_err(|e| format!("图片解码失败: {e}"))?;
    recognize::recognize(img.to_rgba8(), true)
}

// ---------- 生成 ----------

#[tauri::command]
fn generate_video(app: AppHandle, params: GenParams) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut job = state().job.lock().map_err(|_| "任务状态异常".to_string())?;
        if job.is_some() {
            return Err("已有生成任务进行中".to_string());
        }
        *job = Some(stop.clone());
    }

    let engine = state().engine.clone();
    let ffmpeg = state().ffmpeg.clone();
    let base_dir = state().base_dir.clone();
    let job_flag = state().job.clone();
    let polish_cfg = state().settings.read().unwrap().polish.clone();

    // 参数合并当前设置(未显式提供时)
    let params = {
        let settings = state().settings.read().unwrap();
        GenParams {
            voice: if params.voice.is_empty() { settings.voice.clone() } else { params.voice.clone() },
            rate: params.rate,
            depth: if params.depth == 0 { settings.depth } else { params.depth },
            movetime: if params.movetime == 0 { settings.movetime } else { params.movetime },
            branch_max: if params.branch_max == 0 { settings.branch_max } else { params.branch_max },
            ..params
        }
    };

    std::thread::spawn(move || {
        // 捕获生成线程的 panic, 保证 done/error 事件必定发出, 前端不会卡在"生成中"
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generator::generate(engine, &ffmpeg, &base_dir, &params, &stop, &polish_cfg, &|p: GenProgress| {
                let _ = app.emit("gen://progress", &p);
            })
        }));
        let result = match result {
            Ok(r) => r,
            Err(_) => Err("生成过程出现内部错误, 请查看日志重试".to_string()),
        };
        // 任务结束, 释放任务槽允许下一次生成
        job_flag.lock().ok().and_then(|mut j| j.take());
        match result {
            Ok(meta) => {
                let _ = app.emit("gen://done", &meta);
            }
            Err(e) => {
                let _ = app.emit("gen://error", e);
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_generation() -> Result<(), String> {
    let job = state().job.lock().map_err(|_| "任务状态异常".to_string())?;
    if let Some(stop) = job.as_ref() {
        stop.store(true, Ordering::Relaxed);
    }
    Ok(())
}

// ---------- 视频库 ----------

#[tauri::command]
fn list_videos() -> Vec<VideoMeta> {
    library::list(&state().base_dir)
}

#[tauri::command]
fn get_video_script(id: String) -> Result<serde_json::Value, String> {
    library::get(&state().base_dir, &id)
}

#[tauri::command]
fn delete_video(id: String) -> Result<(), String> {
    library::delete(&state().base_dir, &id)
}

#[tauri::command]
fn open_videos_folder() -> Result<(), String> {
    let dir = library::videos_dir(&state().base_dir);
    let _ = std::fs::create_dir_all(&dir);
    tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|e| e.to_string())
}

// ---------- 设置 ----------

#[tauri::command]
fn get_settings() -> config::Settings {
    state().settings.read().unwrap().clone()
}

#[tauri::command]
fn save_settings(new_settings: config::Settings) -> Result<config::Settings, String> {
    new_settings.save(&state().settings_dir)?;
    *state().settings.write().unwrap() = new_settings.clone();
    Ok(new_settings)
}

// 设置页"测试连接": 用示例解说词验证润色接口
#[tauri::command]
fn test_polish_connection(cfg: polish::PolishConfig) -> Result<String, String> {
    polish::test_connection(&cfg)
}
