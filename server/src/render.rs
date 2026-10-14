// 视频帧渲染: 布局由 Layout 驱动, 竖版 1080x1920(抖音) / 横版 1920x1080(经典) 两套画幅
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
use crate::sfx::Sfx;

pub const FPS: u32 = 30;

// 画幅: 竖版为默认产线, 横版保留(输出与旧版逐像素一致)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    Portrait,  // 1080x1920 竖版
    Landscape, // 1920x1080 横版
}

impl Aspect {
    // 空串/未知值回落竖版
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "landscape" => Aspect::Landscape,
            _ => Aspect::Portrait,
        }
    }
}

// 一套画幅的全部几何参数(横版数值 = 旧版硬编码, 保证横版输出不变)
pub struct Layout {
    pub canvas_w: u32,
    pub canvas_h: u32,
    pub cell: i32,            // 格距
    pub board_x0: i32,        // 棋盘左上交点
    pub board_y0: i32,
    pub wood: (i32, i32, u32, u32), // 木底板 (x, y, w, h), 内框为 +8/-16
    pub marker_gap: i32,      // 炮/兵位十字角标
    pub marker_len: i32,
    pub river_size: f32, // 楚河汉界字号与垂直偏移
    pub river_dy: i32,
    pub coord_size: f32, // 上下路数字号与偏移
    pub coord_top_dy: i32,
    pub coord_bottom_dy: i32,
    pub center_text: bool, // true: 底部信息区整体居中(竖版); false: 右侧面板(横版)
    pub title_y: i32,
    pub title_size: f32,
    pub verdict_y: i32,
    pub verdict_size: f32,
    pub main_y: i32, // 当前行棋大字行(仅竖版)
    pub main_size: f32,
    pub score_y: i32,
    pub score_size: f32,
    pub score_max_chars: usize, // 评分行折行字符数(0=不折行)
    pub info_y: i32,            // 开场/结尾信息行
    pub info_step: i32,
    pub info_size: f32,
    pub branch_card: (i32, i32, u32, u32), // 分支卡片 (x, y, w, h)
    pub moves_y: i32,                      // 着法列表(仅横版)
    pub moves_step: i32,
    pub moves_size: f32,
    pub moves_visible: usize,
    pub subtitle_band: (i32, i32, u32, u32), // 字幕条 (x, y, w, h)
    pub subtitle_size: f32,
    pub subtitle_max_chars: usize,
    pub subtitle_y1: i32, // 单行字幕 y
    pub subtitle_y2: i32, // 双行字幕首行 y
    pub subtitle_step: i32,
    pub ring_from: i32, // 出发点圆环半径
    pub ring_to: i32,   // 落点圆环半径
    pub ring_check: i32,
}

impl Layout {
    pub fn landscape() -> Self {
        Self {
            canvas_w: 1920,
            canvas_h: 1080,
            cell: 94,
            board_x0: 140,
            board_y0: 80,
            wood: (88, 28, 856, 950),
            marker_gap: 8,
            marker_len: 16,
            river_size: 44.0,
            river_dy: 22,
            coord_size: 24.0,
            coord_top_dy: 42,
            coord_bottom_dy: 18,
            center_text: false,
            title_y: 58,
            title_size: 54.0,
            verdict_y: 152,
            verdict_size: 38.0,
            main_y: 0,
            main_size: 0.0,
            score_y: 222,
            score_size: 30.0,
            score_max_chars: 0,
            info_y: 320,
            info_step: 56,
            info_size: 34.0,
            branch_card: (986, 830, 860, 150),
            moves_y: 300,
            moves_step: 48,
            moves_size: 34.0,
            moves_visible: 10,
            subtitle_band: (60, 996, 1800, 76),
            subtitle_size: 36.0,
            subtitle_max_chars: 34,
            subtitle_y1: 1014,
            subtitle_y2: 1002,
            subtitle_step: 44,
            ring_from: 46,
            ring_to: 50,
            ring_check: 52,
        }
    }

    pub fn portrait() -> Self {
        Self {
            canvas_w: 1080,
            canvas_h: 1920,
            cell: 106,
            board_x0: 116,
            board_y0: 220,
            wood: (64, 168, 952, 1058),
            marker_gap: 9,
            marker_len: 18,
            river_size: 48.0,
            river_dy: 24,
            coord_size: 26.0,
            coord_top_dy: 46,
            coord_bottom_dy: 20,
            center_text: true,
            title_y: 56,
            title_size: 46.0,
            verdict_y: 124,
            verdict_size: 32.0,
            main_y: 1250,
            main_size: 44.0,
            score_y: 1320,
            score_size: 28.0,
            score_max_chars: 30,
            info_y: 1330,
            info_step: 50,
            info_size: 30.0,
            branch_card: (100, 1390, 880, 130),
            moves_y: 0,
            moves_step: 0,
            moves_size: 0.0,
            moves_visible: 0,
            // 字幕居中于约 82% 屏高, 避开抖音信息流底部文案区, 下方留白为安全区
            subtitle_band: (80, 1572, 920, 88),
            subtitle_size: 36.0,
            subtitle_max_chars: 24,
            subtitle_y1: 1596,
            subtitle_y2: 1584,
            subtitle_step: 44,
            ring_from: 52,
            ring_to: 56,
            ring_check: 58,
        }
    }

    fn px(&self, col: usize) -> i32 {
        self.board_x0 + col as i32 * self.cell
    }

    fn py(&self, row: usize) -> i32 {
        self.board_y0 + row as i32 * self.cell
    }
}

// 旧常量: 横版画布尺寸(识别预览等仍按横版渲染)
pub const CANVAS_W: u32 = 1920;
pub const CANVAS_H: u32 = 1080;

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
    pub current_move: Option<String>, // 竖版底部主行: "第N步 · 车四进五" / "改走 将4退1"
    pub branches: Vec<String>,    // 分支卡片行
    pub info_lines: Vec<String>,  // 开场/结尾面板信息
    pub subtitle: String,         // 完整解说词(供 TTS 与时长估算)
    pub subtitles: Vec<String>,   // 按时间轮播的字幕分段
    pub sfx: Vec<(f32, Sfx)>,     // 音效标注: (占场景时长比例, 类型), 由生成循环混入
    pub check_pos: Option<(usize, usize)>, // 被将军的将帅位置(校正后棋盘坐标)
    pub anim_ratio: f32,          // 动画时长占总时长比例(0=无动画)
}

pub struct Renderer {
    pub layout: Layout,
    sprites: HashMap<char, RgbaImage>,
    title_font: FontVec,
    text_font: FontVec,
}

impl Renderer {
    pub fn new(aspect: Aspect) -> Result<Self, String> {
        let layout = match aspect {
            Aspect::Portrait => Layout::portrait(),
            Aspect::Landscape => Layout::landscape(),
        };
        let img = image::load_from_memory(PIECE_PNG).map_err(|e| format!("棋子精灵图加载失败: {e}"))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let frame_h = 1280 / 16;
        let mut sprites = HashMap::new();
        let target = (layout.cell as f32 * 0.92) as u32;
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
        Ok(Self { layout, sprites, title_font, text_font })
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
            self.draw_piece_at(canvas, piece, self.layout.px(col), self.layout.py(row));
        }
    }

    fn draw_ring(&self, canvas: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        for r in radius - 1..=radius + 1 {
            draw_hollow_circle_mut(canvas, (cx, cy), r, color);
        }
    }

    fn draw_bg(&self, canvas: &mut RgbaImage) {
        let canvas_h = self.layout.canvas_h;
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            // 轻微垂直渐变背景
            let shade = (255.0 - 12.0 * y as f32 / canvas_h as f32) as u8;
            let v = (BG[0] as u32 * shade as u32 / 255) as u8;
            *pixel = Rgba([v, v, v + 6, 255]);
            let _ = x;
        }
    }

    // 棋子位交叉点标记(炮/兵位的十字角标)
    fn draw_markers(&self, canvas: &mut RgbaImage, col: usize, row: usize) {
        let (cx, cy) = (self.layout.px(col), self.layout.py(row));
        let gap = self.layout.marker_gap;
        let len = self.layout.marker_len;
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
        let lay = &self.layout;
        // 木纹底板与内框
        draw_filled_rect_mut(canvas, Rect::at(lay.wood.0, lay.wood.1).of_size(lay.wood.2, lay.wood.3), WOOD);
        draw_hollow_rect_mut(canvas, Rect::at(lay.wood.0 + 8, lay.wood.1 + 8).of_size(lay.wood.2 - 16, lay.wood.3 - 16), WOOD_DARK);

        // 外围双线边框
        draw_hollow_rect_mut(
            canvas,
            Rect::at(lay.board_x0 - 10, lay.board_y0 - 10).of_size((lay.cell * 8 + 20) as u32, (lay.cell * 9 + 20) as u32),
            GRID,
        );

        // 横线
        for row in 0..=9 {
            let y = lay.py(row);
            draw_line_segment_mut(canvas, (lay.board_x0 as f32, y as f32), ((lay.board_x0 + lay.cell * 8) as f32, y as f32), GRID);
        }
        // 竖线(中间列在楚河汉界断开)
        for col in 0..=8 {
            let x = lay.px(col);
            if col == 0 || col == 8 {
                draw_line_segment_mut(canvas, (x as f32, lay.board_y0 as f32), (x as f32, lay.py(9) as f32), GRID);
            } else {
                draw_line_segment_mut(canvas, (x as f32, lay.board_y0 as f32), (x as f32, lay.py(4) as f32), GRID);
                draw_line_segment_mut(canvas, (x as f32, lay.py(5) as f32), (x as f32, lay.py(9) as f32), GRID);
            }
        }
        // 九宫斜线
        for (x0, y0, x1, y1) in [
            (3usize, 0usize, 5usize, 2usize),
            (5, 0, 3, 2),
            (3, 7, 5, 9),
            (5, 7, 3, 9),
        ] {
            draw_line_segment_mut(canvas, (lay.px(x0) as f32, lay.py(y0) as f32), (lay.px(x1) as f32, lay.py(y1) as f32), GRID);
        }
        // 炮位/兵位标记
        for (col, row) in [(1usize, 2), (7, 2), (1, 7), (7, 7), (0, 3), (2, 3), (4, 3), (6, 3), (8, 3), (0, 6), (2, 6), (4, 6), (6, 6), (8, 6)] {
            self.draw_markers(canvas, col, row);
        }

        // 楚河汉界
        let river_cy = (lay.py(4) + lay.py(5)) / 2 - lay.river_dy;
        let grid_text = Rgba([74, 49, 32, 210]);
        self.draw_text_center(canvas, &self.title_font, lay.river_size, grid_text, lay.px(2), river_cy, "楚  河");
        self.draw_text_center(canvas, &self.title_font, lay.river_size, grid_text, lay.px(6), river_cy, "汉  界");

        // 上下路数坐标(上黑下红)
        for col in 0..=8 {
            let num_black = format!("{}", col + 1);
            let num_red = &["九", "八", "七", "六", "五", "四", "三", "二", "一"][col];
            self.draw_text_center(canvas, &self.text_font, lay.coord_size, GRID, lay.px(col), lay.py(0) - lay.coord_top_dy, &num_black);
            self.draw_text_center(canvas, &self.text_font, lay.coord_size, Rgba([122, 42, 26, 255]), lay.px(col), lay.py(9) + lay.coord_bottom_dy, num_red);
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
        let mut canvas = RgbaImage::new(self.layout.canvas_w, self.layout.canvas_h);
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
                self.draw_ring(&mut canvas, self.layout.px(fx), self.layout.py(fy), self.layout.ring_from, Rgba([235, 84, 60, 170]));
                self.draw_ring(&mut canvas, self.layout.px(tx), self.layout.py(ty), self.layout.ring_to, RED_MARK);
            }
        }

        // 被将军的将帅警示
        if let Some((kx, ky)) = scene.check_pos {
            if !animating {
                self.draw_ring(&mut canvas, self.layout.px(kx), self.layout.py(ky), self.layout.ring_check, Rgba([255, 64, 44, 230]));
            }
        }

        self.draw_panel(&mut canvas, scene, animating);
        self.draw_subtitle(&mut canvas, scene, t);
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
        let lay = &self.layout;
        let (fx, fy, tx, ty, piece) = Self::iccs_endpoints(before, iccs);
        // 出发点标记
        self.draw_ring(canvas, lay.px(fx), lay.py(fy), lay.ring_from, Rgba([235, 84, 60, 160]));
        for row in 0..10 {
            for col in 0..9 {
                if (col, row) != (fx, fy) && before[row][col] != ' ' {
                    self.draw_piece(canvas, before, col, row);
                }
            }
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let cx = (lay.px(fx) as f32 + (lay.px(tx) - lay.px(fx)) as f32 * ease) as i32;
        let cy = (lay.py(fy) as f32 + (lay.py(ty) - lay.py(fy)) as f32 * ease) as i32;
        if piece != ' ' {
            self.draw_piece_at(canvas, piece, cx, cy);
        }
    }

    // 分支演示帧: 前半程演示备选着法, 后半程演示对方反制; 落定后标记两步着法
    fn draw_demo(&self, canvas: &mut RgbaImage, demo: &BranchDemo, before: &Board, after: &Board, t: f32, animating: bool) {
        let lay = &self.layout;
        let (alt_fx, alt_fy, alt_tx, alt_ty, _) = Self::iccs_endpoints(before, &demo.alt_iccs);
        if animating {
            match &demo.reply_iccs {
                Some(reply) if t >= 0.5 => {
                    // 第二段: 对方反制; 淡显备选落点提示这是假设走法
                    self.draw_ring(canvas, lay.px(alt_tx), lay.py(alt_ty), lay.ring_from, Rgba([235, 84, 60, 90]));
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
            self.draw_ring(canvas, lay.px(alt_fx), lay.py(alt_fy), lay.ring_from, Rgba([235, 84, 60, 170]));
            self.draw_ring(canvas, lay.px(alt_tx), lay.py(alt_ty), lay.ring_to, RED_MARK);
            if let Some(reply) = &demo.reply_iccs {
                let (r_fx, r_fy, r_tx, r_ty, _) = Self::iccs_endpoints(after, reply);
                self.draw_ring(canvas, lay.px(r_fx), lay.py(r_fy), lay.ring_from, Rgba([230, 179, 74, 170]));
                self.draw_ring(canvas, lay.px(r_tx), lay.py(r_ty), lay.ring_to, GOLD);
            }
        }
    }

    fn draw_panel(&self, canvas: &mut RgbaImage, scene: &FrameScene, animating: bool) {
        if self.layout.center_text {
            self.draw_panel_portrait(canvas, scene);
        } else {
            self.draw_panel_landscape(canvas, scene, animating);
        }
    }

    // 横版右侧面板: 标题/结论/评分 + 着法列表 + 分支卡
    fn draw_panel_landscape(&self, canvas: &mut RgbaImage, scene: &FrameScene, animating: bool) {
        let lay = &self.layout;
        let panel_x = 1000;
        // 标题与结论
        self.draw_text(canvas, &self.title_font, lay.title_size, WHITE, panel_x, lay.title_y, &scene.title);
        self.draw_text(canvas, &self.text_font, lay.verdict_size, GOLD, panel_x, lay.verdict_y, &scene.verdict);

        // 评分与胜率
        let mut score_text = scene.score_line.clone();
        if let Some(wr) = scene.winrate {
            if !score_text.contains("步杀") {
                score_text = format!("{}  胜率{}%", score_text, wr / 10);
            }
        }
        self.draw_text(canvas, &self.text_font, lay.score_size, Rgba([159, 208, 255, 255]), panel_x, lay.score_y, &score_text);

        if scene.info_lines.is_empty() {
            // 着法列表(滚动窗口)
            let total = scene.log.len();
            let start = total.saturating_sub(lay.moves_visible);
            let mut y = lay.moves_y;
            for (i, line) in scene.log.iter().enumerate().skip(start) {
                let is_current = Some(i) == scene.current_idx && !animating;
                let color = if is_current { PANEL_HIGHLIGHT } else { PANEL_TEXT };
                self.draw_text(canvas, &self.text_font, lay.moves_size, color, panel_x, y, line);
                if is_current {
                    draw_filled_rect_mut(canvas, Rect::at(panel_x - 14, y + 4).of_size(6, 34), PANEL_HIGHLIGHT);
                }
                y += lay.moves_step;
            }

            // 分支分析卡片
            if !scene.branches.is_empty() {
                let card_top = lay.branch_card.1;
                fill_rect_blend(canvas, lay.branch_card.0, card_top, lay.branch_card.2, lay.branch_card.3, Rgba([255, 255, 255, 14]));
                self.draw_text(canvas, &self.text_font, 26.0, GOLD, panel_x, card_top + 10, "换个下法试试");
                let mut by = card_top + 56;
                for line in scene.branches.iter().take(2) {
                    self.draw_text(canvas, &self.text_font, 28.0, PANEL_TEXT, panel_x, by, line);
                    by += 40;
                }
            }
        } else {
            // 开场/结尾信息面板
            let mut y = lay.info_y;
            for line in &scene.info_lines {
                self.draw_text(canvas, &self.text_font, lay.info_size, PANEL_TEXT, panel_x, y, line);
                y += lay.info_step;
            }
        }
    }

    // 竖版底部极简信息区: 标题/结论居中在上, 当前行棋大字 + 评分 + 分支卡, 无着法列表
    fn draw_panel_portrait(&self, canvas: &mut RgbaImage, scene: &FrameScene) {
        let lay = &self.layout;
        let cx = lay.canvas_w as i32 / 2;
        // 顶部标题与结论
        self.draw_text_center(canvas, &self.title_font, lay.title_size, WHITE, cx, lay.title_y, &scene.title);
        self.draw_text_center(canvas, &self.text_font, lay.verdict_size, GOLD, cx, lay.verdict_y, &scene.verdict);

        // 评分与胜率
        let mut score_text = scene.score_line.clone();
        if let Some(wr) = scene.winrate {
            if !score_text.contains("步杀") {
                score_text = format!("{}  胜率{}%", score_text, wr / 10);
            }
        }

        if scene.info_lines.is_empty() {
            // 主行: 当前行棋大字
            if let Some(mv) = &scene.current_move {
                self.draw_text_center(canvas, &self.text_font, lay.main_size, WHITE, cx, lay.main_y, mv);
            }
            // 次行: 评分(过长折两行)
            let lines = wrap_chars(&score_text, lay.score_max_chars);
            let mut y = lay.score_y;
            for line in &lines {
                self.draw_text_center(canvas, &self.text_font, lay.score_size, Rgba([159, 208, 255, 255]), cx, y, line);
                y += 36;
            }

            // 分支卡片
            if !scene.branches.is_empty() {
                let (bx, by, bw, bh) = lay.branch_card;
                fill_rect_blend(canvas, bx, by, bw, bh, Rgba([255, 255, 255, 14]));
                self.draw_text_center(canvas, &self.text_font, 26.0, GOLD, cx, by + 12, "换个下法试试");
                let mut y = by + 52;
                for line in scene.branches.iter().take(2) {
                    self.draw_text_center(canvas, &self.text_font, 28.0, PANEL_TEXT, cx, y, line);
                    y += 38;
                }
            }
        } else {
            // 开场/结尾: 评分行 + 信息行居中
            self.draw_text_center(canvas, &self.text_font, 30.0, Rgba([159, 208, 255, 255]), cx, lay.main_y, &score_text);
            let mut y = lay.info_y;
            for line in &scene.info_lines {
                self.draw_text_center(canvas, &self.text_font, lay.info_size, PANEL_TEXT, cx, y, line);
                y += lay.info_step;
            }
        }
    }

    // 字幕: 长解说词已切段, 按场景进度 t 轮播显示当前段
    fn draw_subtitle(&self, canvas: &mut RgbaImage, scene: &FrameScene, t: f32) {
        let chunks = &scene.subtitles;
        if chunks.is_empty() {
            return;
        }
        let idx = ((t * chunks.len() as f32) as usize).min(chunks.len() - 1);
        let text = &chunks[idx];
        let lay = &self.layout;
        let (bx, by, bw, bh) = lay.subtitle_band;
        fill_rect_blend(canvas, bx, by, bw, bh, Rgba([0, 0, 0, 150]));
        // 单段过长时仍折成两行兜底
        let lines = wrap_chars(text, lay.subtitle_max_chars);
        let mut y = if lines.len() > 1 { lay.subtitle_y2 } else { lay.subtitle_y1 };
        for line in lines {
            self.draw_text_center(canvas, &self.text_font, lay.subtitle_size, WHITE, lay.canvas_w as i32 / 2, y, &line);
            y += lay.subtitle_step;
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
        let mut canvas = RgbaImage::new(self.layout.canvas_w, self.layout.canvas_h);
        self.draw_bg(&mut canvas);
        self.draw_board(&mut canvas);
        self.draw_board_pieces(&mut canvas, board);
        Ok(canvas)
    }

    // 竖版封面卡: 初始局面 + 大字钩子标题(发布用)
    pub fn render_cover(&self, board: &Board, title: &str) -> RgbaImage {
        let mut canvas = RgbaImage::new(self.layout.canvas_w, self.layout.canvas_h);
        self.draw_bg(&mut canvas);
        self.draw_board(&mut canvas);
        self.draw_board_pieces(&mut canvas, board);
        let cx = self.layout.canvas_w as i32 / 2;
        // 下部标题大字(自动两行)
        let lines = wrap_chars(title, 12);
        let mut y = 1310;
        for line in &lines {
            let size = if lines.len() > 1 { 62.0 } else { 74.0 };
            self.draw_text_center(&mut canvas, &self.title_font, size, WHITE, cx, y, line);
            y += 86;
        }
        self.draw_text_center(&mut canvas, &self.text_font, 30.0, GOLD, cx, y + 24, "— 象棋残局 · 每日一局 —");
        canvas
    }

    // 棋盘木底板区域(用于裁剪预览)
    pub fn wood_rect(&self) -> (u32, u32, u32, u32) {
        (self.layout.wood.0 as u32, self.layout.wood.1 as u32, self.layout.wood.2, self.layout.wood.3)
    }
}

// 半透明矩形: 与底色做 alpha 合成。imageproc 的 draw_filled_rect_mut 是直接覆写,
// 半透明像素会原样留在画布上, 经帧编码丢掉 alpha 后会变成实心色块(分支卡/字幕条)
fn fill_rect_blend(canvas: &mut RgbaImage, x: i32, y: i32, w: u32, h: u32, color: Rgba<u8>) {
    let a = color[3] as u32;
    if a == 0 {
        return;
    }
    let inv = 255 - a;
    let mix = |fg: u8, bg: u8| ((fg as u32 * a + bg as u32 * inv) / 255) as u8;
    for yy in y..y + h as i32 {
        for xx in x..x + w as i32 {
            if xx < 0 || yy < 0 || xx >= canvas.width() as i32 || yy >= canvas.height() as i32 {
                continue;
            }
            let bg = canvas.get_pixel(xx as u32, yy as u32);
            canvas.put_pixel(xx as u32, yy as u32, Rgba([mix(color[0], bg[0]), mix(color[1], bg[1]), mix(color[2], bg[2]), 255]));
        }
    }
}

// 把长解说词切成多条字幕(每条 ≤ max 字), 优先在标点处断句, 各段时长随场景进度轮播
pub fn split_subtitle(text: &str, max: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    if max == 0 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return vec![text.to_string()];
    }
    // 先定段数使每段尽量均匀, 再在目标断点附近找标点
    let n = (chars.len() + max - 1) / max;
    let target = (chars.len() + n - 1) / n;
    const BREAKS: [char; 9] = ['，', '。', '！', '？', '；', '、', ',', '.', ' '];
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + target).min(chars.len());
        let mut cut = end;
        if end < chars.len() {
            // 从目标断点向回找最近的标点(最多回看 8 字, 避免产生碎片段)
            for i in (start + 1..=end).rev() {
                if BREAKS.contains(&chars[i - 1]) {
                    cut = i;
                    break;
                }
                if end - i >= 8 {
                    break;
                }
            }
        }
        let cut = cut.max(start + 1);
        let seg: String = chars[start..cut].iter().collect();
        let seg = seg.trim();
        if !seg.is_empty() {
            out.push(seg.to_string());
        }
        start = cut;
    }
    out
}

// 按字符数折行(最多两行, 超长部分舍去), max=0 表示不折行
fn wrap_chars(text: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return vec![text.to_string()];
    }
    let total = text.chars().count();
    if total <= max {
        return vec![text.to_string()];
    }
    let first: String = text.chars().take(max).collect();
    let second: String = text.chars().skip(max).take(max).collect();
    vec![first, second]
}

// 全局共享渲染器(字体与精灵图只加载一次; 识别预览固定横版)
static RENDERER: std::sync::OnceLock<Renderer> = std::sync::OnceLock::new();

pub fn shared() -> &'static Renderer {
    RENDERER.get_or_init(|| Renderer::new(Aspect::Landscape).expect("渲染器初始化失败"))
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

#[cfg(test)]
mod tests {
    use super::*;

    // 横版布局数值必须与旧版硬编码一致(保证横版输出逐像素不变)
    #[test]
    fn test_landscape_layout_unchanged() {
        let lay = Layout::landscape();
        assert_eq!((lay.canvas_w, lay.canvas_h), (1920, 1080));
        assert_eq!((lay.cell, lay.board_x0, lay.board_y0), (94, 140, 80));
        assert_eq!(lay.wood, (88, 28, 856, 950));
        assert_eq!((lay.ring_from, lay.ring_to, lay.ring_check), (46, 50, 52));
        assert_eq!(lay.subtitle_band, (60, 996, 1800, 76));
        assert!(!lay.center_text);
        assert_eq!(Aspect::from_str("landscape"), Aspect::Landscape);
        assert_eq!(Aspect::from_str(""), Aspect::Portrait);
        assert_eq!(Aspect::from_str("portrait"), Aspect::Portrait);
    }

    // 竖版布局: 画布 1080x1920, 棋盘水平居中且完整落在木底板内, 底部信息区不与棋盘重叠
    #[test]
    fn test_portrait_layout_bounds() {
        let lay = Layout::portrait();
        assert_eq!((lay.canvas_w, lay.canvas_h), (1080, 1920));
        let board_left = lay.board_x0;
        let board_right = lay.board_x0 + lay.cell * 8;
        assert_eq!(board_left, lay.canvas_w as i32 - board_right, "棋盘应水平居中");
        assert!(board_left >= lay.wood.0 && board_right <= (lay.wood.0 + lay.wood.2 as i32));
        let board_bottom = lay.board_y0 + lay.cell * 9;
        assert!(board_bottom <= (lay.wood.1 + lay.wood.3 as i32) as i32);
        // 底部文字区从木底板之下开始
        assert!(lay.main_y > lay.wood.1 + lay.wood.3 as i32);
        // 字幕条不越过画布底部
        assert!(lay.subtitle_band.1 + lay.subtitle_band.3 as i32 <= lay.canvas_h as i32);
        assert!(lay.center_text);
    }

    // 折行工具: 短文本单行, 长文本两行, max=0 不折行
    #[test]
    fn test_wrap_chars() {
        assert_eq!(wrap_chars("短字幕", 24), vec!["短字幕".to_string()]);
        let long = "一二三四五六七八九十壹贰叁肆伍陆柒捌玖拾甲乙丙丁戊己庚辛壬癸子丑寅卯";
        let wrapped = wrap_chars(long, 24);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].chars().count(), 24);
        assert_eq!(wrap_chars("任意", 0), vec!["任意".to_string()]);
    }

    // 视觉验证: 封面卡落盘 scripts/out/cover/ (人工查看)
    #[test]
    #[ignore]
    fn test_dump_cover() {
        let board = crate::chess::fen_to_board("3k5/9/9/9/9/9/9/4C4/9/4K3R w");
        let renderer = Renderer::new(Aspect::Portrait).unwrap();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/out/cover");
        std::fs::create_dir_all(&dir).unwrap();
        let frame = renderer.render_cover(&board, "红先!这个残局暗藏杀机, 你能几步赢?");
        let path = dir.join("cover.png");
        frame.save(&path).unwrap();
        println!("saved {}", path.display());
    }

    // 字幕分段: 短文本单段, 长文本按标点切成 ≤max 的多段, 空文本无段
    #[test]
    fn test_split_subtitle() {
        assert_eq!(split_subtitle("", 24), Vec::<String>::new());
        assert_eq!(split_subtitle("  ", 24), Vec::<String>::new());
        assert_eq!(split_subtitle("短解说", 24), vec!["短解说".to_string()]);
        assert_eq!(split_subtitle("任意文本", 0), vec!["任意文本".to_string()]);
        // 无标点长文: 均匀切段且每段不超上限
        let long = "一".repeat(49);
        let chunks = split_subtitle(&long, 24);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.chars().count() <= 24));
        // 带标点: 在标点处断句
        let text = "红方有车和炮, 黑方仅剩孤将。红方先行, 红方五步绝杀。我们一步一步推演。";
        let chunks = split_subtitle(text, 24);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|c| c.chars().count() <= 26));
        assert_eq!(chunks.concat().replace(' ', ""), text.replace(' ', ""), "切段不应丢失内容");
    }
}
