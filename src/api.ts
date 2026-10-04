// 与 Rust 端交互的类型与命令封装
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface PiecePosition {
    piece: string;
    pos: string;
}

export interface Recognition {
    board: PiecePosition[];
    fen: string;
    legal: boolean;
    issues: string[];
    pieces_count: number;
    preview_base64: string | null;
}

export interface VideoMeta {
    id: string;
    title: string;
    verdict: string;
    duration_secs: number;
    size_bytes: number;
    created_ms: number;
    move_count: number;
    video_path: string;
    thumb_path: string;
}

export interface GenProgress {
    stage: "analyse" | "tts" | "render" | "compose" | "done" | string;
    current: number;
    total: number;
    message: string;
}

export interface GenParams {
    pieces: PiecePosition[];
    side: string; // 'w' 红 / 'b' 黑
    depth: number;
    movetime: number;
    branch_max: number;
    voice: string;
    rate: number;
    voice_enabled: boolean;    // 是否生成语音解说
    branches_enabled: boolean; // 是否包含分支推演
}

export interface AppSettings {
    voice: string;
    rate: number;
    depth: number;
    movetime: number;
    branch_max: number;
    voice_enabled: boolean;
    branches_enabled: boolean;
    autoplay_sound: boolean;
}

export const VOICES: { value: string; label: string }[] = [
    { value: "zh-CN-YunxiNeural", label: "云希 · 男声解说" },
    { value: "zh-CN-YunyangNeural", label: "云扬 · 男声播报" },
    { value: "zh-CN-XiaoxiaoNeural", label: "晓晓 · 女声" },
    { value: "zh-CN-XiaoyiNeural", label: "晓伊 · 女声" },
];

export async function recognizeImage(bytes: Uint8Array): Promise<Recognition> {
    return invoke("recognize_image", { bytes });
}

export async function generateVideo(params: GenParams): Promise<void> {
    await invoke("generate_video", { params });
}

export async function stopGeneration(): Promise<void> {
    await invoke("stop_generation");
}

export async function listVideos(): Promise<VideoMeta[]> {
    return invoke("list_videos");
}

export async function getVideoScript(id: string): Promise<unknown> {
    return invoke("get_video_script", { id });
}

export async function deleteVideo(id: string): Promise<void> {
    await invoke("delete_video", { id });
}

export async function openVideosFolder(): Promise<void> {
    await invoke("open_videos_folder");
}

export async function getSettings(): Promise<AppSettings> {
    return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke("save_settings", { newSettings: settings });
}

export function listenProgress(cb: (p: GenProgress) => void) {
    return listen<GenProgress>("gen://progress", (e) => cb(e.payload));
}

export function listenDone(cb: (m: VideoMeta) => void) {
    return listen<VideoMeta>("gen://done", (e) => cb(e.payload));
}

export function listenError(cb: (msg: string) => void) {
    return listen<string>("gen://error", (e) => cb(e.payload));
}

// 资源协议地址(视频/封面)
export { convertFileSrc } from "@tauri-apps/api/core";
