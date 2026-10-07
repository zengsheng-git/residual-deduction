// 视频帧渲染: 1920x1080 画布, 左侧棋盘(程序绘制+精灵棋子), 右侧信息面板, 底部字幕
use std::collections::HashMap;

use ab_glyph::Font as _;
use ab_glyph::FontVec;
use ab_glyph::ScaleFont as _;
use image::imageops::FilterType;
use image::Rgba;
use image::RgbaImage;
use imageproc::drawing::draw_filled_rect_mut;
use imageproc::drawing::draw_hollow_circle_mut;
use imageproc::drawing::draw_hollow_rect_mut;
use imageproc::drawing::draw_line_segment_mut;
use imageproc::drawing::draw_text_mut;
use imageproc::rect::Rect;
use serde::Serialize;

use crate::chess;
use crate::chess::Board;

pub const CANVAS_W: u32 = 1920;
pub const CANVAS_H: u32 = 1080;
pub const FPS: u32 = 30;

const CELL: i32 = 94;
const BOARD_X0: i32 = 140;
const BOARD_Y0: i32 = 80;
const WOOD: Rgba<u8> = Rgba([214, 179, 132, 255]);
const WOOD_DARK: Rgba<u8> = Rgba([122, 74, 36, 255]);
const GRID: Rgba<u8> = Rgba([74, 49, 32, 255]);
const BG: Rgba<u8> = Rgba([20, 20, 26, 255]);
const PANEL_TEXT: Rgba<u8> = Rgba([214, 214, 224, 255]);
const PANEL_HIGHLIGHT: Rgba<u8> = Rgba([255, 210, 122, 255]);
const GOLD: Rgba<u8> = Rgba([230, 179, 74, 255]);
const RED_MARK: Rgba<u8> = Rgba([235, 84, 60, 255]);
const WHITE: Rgba<u8> = Rgba([245, 245, 248, 255]);

const PIECE_PNG: &[u8] = include_bytes!("../../src/assets/images/piece.png");
const QITI_TTF: &[u8] = include_bytes!("../../src/assets/fonts/qiti.ttf");

// 精灵图帧序号: 0-6 红方(车马相仕帅炮兵), 7-13 黑方(车马象士将砲卒), 14-15 为角标
const SPRITE_FRAME: [(char, usize); 14] = [
    ('R', 0), ('N', 1), ('B', 2), ('A', 3), ('K', 4), ('C', 5), ('P', 6),
    ('r', 7), ('n', 8), ('b', 9), ('a', 10), ('k', 11), ('c', 12), ('p', 13),
];

// 分支演示: 在 board_before 局面上假设走 alt, 对方以 reply 反制
#[derive(Debug, Clone, Serialize)]
pub struct BranchDemo {
    pub alt_iccs: String,           // 假设的备选着法
    pub reply_iccs: Option<String>, // 对方反制着法(无则只演示备选)
}

// 一个场景的一帧所需全部数据
#[derive(Debug, Clone, Serialize)]
pub struct FrameScene {
    pub board_before: Vec<chess::Position>,
    pub board_after: Vec<chess::Position>,
    pub move_iccs: Option<String>, // Move 场景: 本步着法
    pub branch_demo: Option<BranchDemo>, // 分支演示场景(与 move_iccs 互斥)
    pub camp: char,                // 行棋方 'w'/'b'
    pub title: String,
    pub verdict: String,
    pub score_line: String,       // "评分 +850 胜势" / 其他说明
    pub winrate: Option<usize>,   // 千分比
    pub log: Vec<String>,         // "1. 炮二平五"
    pub current_idx: Option<usize>,
    pub branches: Vec<String>,    // 分支卡片行
    pub info_lines: Vec<String>,  // 开场/结尾面板信息
    pub subtitle: String,
    pub check_pos: Option<(usize, usize)>, // 被将军的将帅位置(校正后棋盘坐标)
    pub anim_ratio: f32,          // 动画时长占总时长比例(0=无动画)
}

pub struct Renderer {
    sprites: HashMap<char, RgbaImage>,
    title_font: FontVec,
    text_font: FontVec,
}

// 坐标 -> 画布交点像素
fn px(col: usize) -> i32 {
    BOARD_X0 + col as i32 * CELL
}

fn py(row: usize) -> i32 {
    BOARD_Y0 + row as i32 * CELL
}

impl Renderer {
    pub fn new() -> Result<Self, String> {
        let img = image::load_from_memory(PIECE_PNG).map_err(|e| format!("棋子精灵图加载失败: {e}"))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let frame_h = 1280 / 16;
        let mut sprites = HashMap::new();
        let target = (CELL as f32 * 0.92) as u32;
        for (piece, idx) in SPRITE_FRAME {
            let y0 = idx * frame_h as usize;
            if y0 + frame_h as usize > h as usize {
                continue;
            }
            let frame = image::imageops::resize(
                &image::imageops::crop_imm(&rgba, 0, y0 as u32, w, frame_h).to_image(),
                target,
                target,
                FilterType::CatmullRom,
            );
            sprites.insert(piece, frame);
        }

        let title_font = FontVec::try_from_vec(QITI_TTF.to_vec()).map_err(|e| format!("标题字体加载失败: {e}"))?;
        // 正文用系统黑体(中文显示更工整), 缺失时退回内置字体
        let text_font = match std::fs::read("C:/Windows/Fonts/simhei.ttf") {
            Ok(bytes) => FontVec::try_from_vec(bytes).unwrap_or_else(|_| FontVec::try_from_vec(QITI_TTF.to_vec()).unwrap()),
            Err(_) => FontVec::try_from_vec(QITI_TTF.to_vec()).map_err(|e| format!("正文字体加载失败: {e}"))?,
        };
        Ok(Self { sprites, title_font, text_font })
    }

    // 文本宽度(像素)
    fn text_width(&self, font: &FontVec, px_size: f32, text: &str) -> f32 {
        let scaled = font.as_scaled(px_size);
        text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum()
    }

    fn draw_text(&self, canvas: &mut RgbaImage, font: &FontVec, px_size: f32, color: Rgba<u8>, x: i32, y: i32, text: &str) {
        draw_text_mut(canvas, color, x, y, px_size, font, text);
    }

    fn draw_text_center(&self, canvas: &mut RgbaImage, font: &FontVec, px_size: f32, color: Rgba<u8>, cx: i32, y: i32, text: &str) {
        let width = self.text_width(font, px_size, text);
        self.draw_text(canvas, font, px_size, color, cx - (width / 2.0) as i32, y, text);
    }

    fn draw_piece_at(&self, canvas: &mut RgbaImage, piece: char, cx: i32, cy: i32) {
        if let Some(sprite) = self.sprites.get(&piece) {
            image::imageops::overlay(canvas, sprite, (cx - sprite.width() as i32 / 2) as i64, (cy - sprite.height() as i32 / 2) as i64);
        }
    }

    fn draw_piece(&self, canvas: &mut RgbaImage, board: &Board, col: usize, row: usize) {
        let piece = board[row][col];
        if piece != ' ' {
            self.draw_piece_at(canvas, piece, px(col), py(row));
        }
    }

    fn draw_ring(&self, canvas: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        for r in radius - 1..=radius + 1 {
            draw_hollow_circle_mut(canvas, (cx, cy), r, color);
        }
    }

    fn draw_bg(&self, canvas: &mut RgbaImage) {
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            // 轻微垂直渐变背景
            let shade = (255.0 - 12.0 * y as f32 / CANVAS_H as f32) as u8;
            let v = (BG[0] as u32 * shade as u32 / 255) as u8;
            *pixel = Rgba([v, v, v + 6, 255]);
            let _ = x;
        }
    }

    // 棋子位交叉点标记(炮/兵位的十字角标)
    fn draw_markers(&self, canvas: &mut RgbaImage, col: usize, row: usize) {
        let (cx, cy) = (px(col), py(row));
        let gap = 8;
        let len = 16;
        let dirs = [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)];
        for (dx, dy) in dirs {
            // 边界列省略朝外的一侧
            if (col == 0 && dx < 0) || (col == 8 && dx > 0) {
                continue;
            }
            let sx = cx + dx * gap;
            let sy = cy + dy * gap;
            draw_line_segment_mut(canvas, (sx as f32, sy as f32), ((sx + dx * len) as f32, sy as f32), GRID);
            draw_line_segment_mut(canvas, (sx as f32, sy as f32), (sx as f32, (sy + dy * len) as f32), GRID);
        }
    }

    pub fn draw_board(&self, canvas: &mut RgbaImage) {
        // 木纹底板
        let wood = Rect::at(88, 28).of_size(856, 950);
        draw_filled_rect_mut(canvas, wood, WOOD);
        draw_hollow_rect_mut(canvas, Rect::at(96, 36).of_size(840, 934), WOOD_DARK);

        // 外围双线边框
        draw_hollow_rect_mut(canvas, Rect::at(BOARD_X0 - 10, BOARD_Y0 - 10).of_size((CELL * 8 + 20) as u32, (CELL * 9 + 20) as u32), GRID);

        // 横线
        for row in 0..=9 {
            let y = py(row);
            draw_line_segment_mut(canvas, (BOARD_X0 as f32, y as f32), ((BOARD_X0 + CELL * 8) as f32, y as f32), GRID);
        }
        // 竖线(中间列在楚河汉界断开)
        for col in 0..=8 {
            let x = px(col);
            if col == 0 || col == 8 {
                draw_line_segment_mut(canvas, (x as f32, BOARD_Y0 as f32), (x as f32, py(9) as f32), GRID);
            } else {
                draw_line_segment_mut(canvas, (x as f32, BOARD_Y0 as f32), (x as f32, py(4) as f32), GRID);
                draw_line_segment_mut(canvas, (x as f32, py(5) as f32), (x as f32, py(9) as f32), GRID);
            }
        }
        // 九宫斜线
        for (x0, y0, x1, y1) in [
            (3usize, 0usize, 5usize, 2usize),
            (5, 0, 3, 2),
            (3, 7, 5, 9),
            (5, 7, 3, 9),
        ] {
            draw_line_segment_mut(canvas, (px(x0) as f32, py(y0) as f32), (px(x1) as f32, py(y1) as f32), GRID);
        }
        // 炮位/兵位标记
        for (col, row) in [(1usize, 2), (7, 2), (1, 7), (7, 7), (0, 3), (2, 3), (4, 3), (6, 3), (8, 3), (0, 6), (2, 6), (4, 6), (6, 6), (8, 6)] {
            self.draw_markers(canvas, col, row);
        }

        // 楚河汉界
        let river_cy = (py(4) + py(5)) / 2 - 22;
        let grid_text = Rgba([74, 49, 32, 210]);
        self.draw_text_center(canvas, &self.title_font, 44.0, grid_text, BOARD_X0 + CELL * 2, river_cy, "楚  河");
        self.draw_text_center(canvas, &self.title_font, 44.0, grid_text, BOARD_X0 + CELL * 6, river_cy, "汉  界");

        // 上下路数坐标(上黑下红)
        for col in 0..=8 {
            let num_black = format!("{}", col + 1);
            let num_red = &["九", "八", "七", "六", "五", "四", "三", "二", "一"][col];
            self.draw_text_center(canvas, &self.text_font, 24.0, GRID, px(col), BOARD_Y0 - 42, &num_black);
            self.draw_text_center(canvas, &self.text_font, 24.0, Rgba([122, 42, 26, 255]), px(col), py(9) + 18, num_red);
        }
    }

    fn draw_board_pieces(&self, canvas: &mut RgbaImage, board: &Board) {
        for row in 0..10 {
            for col in 0..9 {
                if board[row][col] != ' ' {
                    self.draw_piece(canvas, board, col, row);
                }
            }
        }
    }

    // 从"坐标棋子列表"还原棋盘
    fn board_from(positions: &[chess::Position]) -> Board {
        chess::board_from_positions(positions).unwrap_or([[' '; 9]; 10])
    }

    // 绘制一帧. t in [0,1]; t<1 表示走子动画进度, t>=1 为落定状态
    pub fn render_frame(&self, scene: &FrameScene, t: f32) -> RgbaImage {
        let mut canvas = RgbaImage::new(CANVAS_W, CANVAS_H);
        self.draw_bg(&mut canvas);
        self.draw_board(&mut canvas);

        let before = Self::board_from(&scene.board_before);
        let after = Self::board_from(&scene.board_after);

        let animating = t < 1.0 && (scene.move_iccs.is_some() || scene.branch_demo.is_some());
        if let Some(demo) = &scene.branch_demo {
            self.draw_demo(&mut canvas, demo, &before, &after, t, animating);
        } else if animating {
            // 动画阶段: 其他棋子取 before 局面, 移动棋子单独插值
            self.draw_move_anim(&mut canvas, &before, scene.move_iccs.as_ref().unwrap(), t);
        } else {
            // 落定状态
            self.draw_board_pieces(&mut canvas, &after);
            if let Some(iccs) = &scene.move_iccs {
                let (fx, fy, tx, ty, _) = Self::iccs_endpoints(&after, iccs);
                self.draw_ring(&mut canvas, px(fx), py(fy), 46, Rgba([235, 84, 60, 170]));
                self.draw_ring(&mut canvas, px(tx), py(ty), 50, RED_MARK);
            }
        }

        // 被将军的将帅警示
        if let Some((kx, ky)) = scene.check_pos {
            if !animating {
                self.draw_ring(&mut canvas, px(kx), py(ky), 52, Rgba([255, 64, 44, 230]));
            }
        }

        self.draw_panel(&mut canvas, scene, animating);
        self.draw_subtitle(&mut canvas, &scene.subtitle);
        canvas
    }

    // 解析 iccs 着法 → (from_x, from_y, to_x, to_y, 移动的棋子)
    fn iccs_endpoints(board: &Board, iccs: &str) -> (usize, usize, usize, usize, char) {
        let mut cs = iccs.chars();
        let fx = cs.next().unwrap() as usize - 97;
        let fy = 9 - cs.next().unwrap().to_digit(10).unwrap() as usize;
        let tx = cs.next().unwrap() as usize - 97;
        let ty = 9 - cs.next().unwrap().to_digit(10).unwrap() as usize;
        (fx, fy, tx, ty, board[fy][fx])
    }

    // 走子动画帧: 其他棋子取 before 局面, 移动棋子按 ease-out 插值, t in [0,1]
    fn draw_move_anim(&self, canvas: &mut RgbaImage, before: &Board, iccs: &str, t: f32) {
        let (fx, fy, tx, ty, piece) = Self::iccs_endpoints(before, iccs);
        // 出发点标记
        self.draw_ring(canvas, px(fx), py(fy), 46, Rgba([235, 84, 60, 160]));
        for row in 0..10 {
            for col in 0..9 {
                if (col, row) != (fx, fy) && before[row][col] != ' ' {
                    self.draw_piece(canvas, before, col, row);
                }
            }
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let cx = (px(fx) as f32 + (px(tx) - px(fx)) as f32 * ease) as i32;
        let cy = (py(fy) as f32 + (py(ty) - py(fy)) as f32 * ease) as i32;
        if piece != ' ' {
            self.draw_piece_at(canvas, piece, cx, cy);
        }
    }

    // 分支演示帧: 前半程演示备选着法, 后半程演示对方反制; 落定后标记两步着法
    fn draw_demo(&self, canvas: &mut RgbaImage, demo: &BranchDemo, before: &Board, after: &Board, t: f32, animating: bool) {
        let (alt_fx, alt_fy, alt_tx, alt_ty, _) = Self::iccs_endpoints(before, &demo.alt_iccs);
        if animating {
            match &demo.reply_iccs {
                Some(reply) if t >= 0.5 => {
                    // 第二段: 对方反制; 淡显备选落点提示这是假设走法
                    self.draw_ring(canvas, px(alt_tx), py(alt_ty), 46, Rgba([235, 84, 60, 90]));
                    let after_alt = chess::board_move(*before, &demo.alt_iccs);
                    self.draw_move_anim(canvas, &after_alt, reply, (t - 0.5) * 2.0);
                }
                _ => {
                    // 第一段: 备选着法(有反制时占前半程, 否则占整段动画)
                    let progress = if demo.reply_iccs.is_some() { t * 2.0 } else { t };
                    self.draw_move_anim(canvas, before, &demo.alt_iccs, progress);
                }
            }
        } else {
            self.draw_board_pieces(canvas, after);
            // 备选着法: 红圈; 对方反制: 金圈
            self.draw_ring(canvas, px(alt_fx), py(alt_fy), 46, Rgba([235, 84, 60, 170]));
            self.draw_ring(canvas, px(alt_tx), py(alt_ty), 50, RED_MARK);
            if let Some(reply) = &demo.reply_iccs {
                let (r_fx, r_fy, r_tx, r_ty, _) = Self::iccs_endpoints(after, reply);
                self.draw_ring(canvas, px(r_fx), py(r_fy), 46, Rgba([230, 179, 74, 170]));
                self.draw_ring(canvas, px(r_tx), py(r_ty), 50, GOLD);
            }
        }
    }

    fn draw_panel(&self, canvas: &mut RgbaImage, scene: &FrameScene, animating: bool) {
        let panel_x = 1000;
        // 标题与结论
        self.draw_text(canvas, &self.title_font, 54.0, WHITE, panel_x, 58, &scene.title);
        self.draw_text(canvas, &self.text_font, 38.0, GOLD, panel_x, 152, &scene.verdict);

        // 评分与胜率
        let mut score_text = scene.score_line.clone();
        if let Some(wr) = scene.winrate {
            if !score_text.contains("步杀") {
                score_text = format!("{}  胜率{}%", score_text, wr / 10);
            }
        }
        self.draw_text(canvas, &self.text_font, 30.0, Rgba([159, 208, 255, 255]), panel_x, 222, &score_text);

        if scene.info_lines.is_empty() {
            // 着法列表(滚动窗口, 显示最近 10 条)
            let total = scene.log.len();
            let visible = 10;
            let start = total.saturating_sub(visible);
            let mut y = 300;
            for (i, line) in scene.log.iter().enumerate().skip(start) {
                let is_current = Some(i) == scene.current_idx && !animating;
                let color = if is_current { PANEL_HIGHLIGHT } else { PANEL_TEXT };
                self.draw_text(canvas, &self.text_font, 34.0, color, panel_x, y, line);
                if is_current {
                    draw_filled_rect_mut(canvas, Rect::at(panel_x - 14, y + 4).of_size(6, 34), PANEL_HIGHLIGHT);
                }
                y += 48;
            }

            // 分支分析卡片
            if !scene.branches.is_empty() {
                let card_top = 830;
                draw_filled_rect_mut(canvas, Rect::at(panel_x - 14, card_top).of_size(860, 150), Rgba([255, 255, 255, 14]));
                self.draw_text(canvas, &self.text_font, 26.0, GOLD, panel_x, card_top + 10, "换个下法试试");
                let mut by = card_top + 56;
                for line in scene.branches.iter().take(2) {
                    self.draw_text(canvas, &self.text_font, 28.0, PANEL_TEXT, panel_x, by, line);
                    by += 40;
                }
            }
        } else {
            // 开场/结尾信息面板
            let mut y = 320;
            for line in &scene.info_lines {
                self.draw_text(canvas, &self.text_font, 34.0, PANEL_TEXT, panel_x, y, line);
                y += 56;
            }
        }
    }

    fn draw_subtitle(&self, canvas: &mut RgbaImage, text: &str) {
        if text.is_empty() {
            return;
        }
        let band = Rect::at(60, 996).of_size(1800, 76);
        draw_filled_rect_mut(canvas, band, Rgba([0, 0, 0, 150]));
        // 过长的字幕折成两行
        let max_chars = 34;
        let lines: Vec<&str> = if text.chars().count() <= max_chars {
            vec![text]
        } else {
            let split_at = text.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(text.len());
            vec![&text[..split_at], &text[split_at..]]
        };
        let mut y = if lines.len() > 1 { 1002 } else { 1014 };
        for line in lines {
            self.draw_text_center(canvas, &self.text_font, 36.0, WHITE, CANVAS_W as i32 / 2, y, line);
            y += 44;
        }
    }

    // 识别预览图(仅棋盘区域), 返回 PNG base64
    pub fn render_board_preview(&self, board: &Board) -> String {
        let frame = self.render_board_image(board).expect("preview render");
        let (rx, ry, rw, rh) = self.wood_rect();
        let crop = image::imageops::crop_imm(&frame, rx, ry, rw, rh).to_image();
        let small = image::DynamicImage::ImageRgba8(crop).resize_exact(560, 622, FilterType::Triangle);
        let mut png = std::io::Cursor::new(Vec::new());
        small.write_to(&mut png, image::ImageFormat::Png).ok();
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(png.get_ref())
    }

    // 渲染纯棋盘画面(用于识别往返测试)
    pub fn render_board_image(&self, board: &Board) -> Result<RgbaImage, String> {
        let mut canvas = RgbaImage::new(CANVAS_W, CANVAS_H);
        self.draw_bg(&mut canvas);
        self.draw_board(&mut canvas);
        self.draw_board_pieces(&mut canvas, board);
        Ok(canvas)
    }

    // 棋盘木底板区域(用于裁剪预览)
    pub fn wood_rect(&self) -> (u32, u32, u32, u32) {
        (88, 28, 856, 950)
    }
}

// 全局共享渲染器(字体与精灵图只加载一次)
static RENDERER: std::sync::OnceLock<Renderer> = std::sync::OnceLock::new();

pub fn shared() -> &'static Renderer {
    RENDERER.get_or_init(|| Renderer::new().expect("渲染器初始化失败"))
}

// 识别预览图(仅棋盘区域), 返回 PNG base64
pub fn render_board_preview(board: &Board) -> String {
    shared().render_board_preview(board)
}

// 渲染纯棋盘画面(用于识别往返测试)
#[allow(dead_code)]
pub fn render_board_image(board: &Board) -> Result<RgbaImage, String> {
    shared().render_board_image(board)
}

// 调试用: 在图上画一个矩形框
#[allow(dead_code)]
pub fn debug_draw_rect(canvas: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32) {
    imageproc::drawing::draw_hollow_rect_mut(
        canvas,
        Rect::at(x0, y0).of_size((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32),
        Rgba([255, 0, 0, 255]),
    );
}
