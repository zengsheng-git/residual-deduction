// 静态残局截图的识别流程: 整图粗定位棋盘 → 裁剪精识别 → 方向校正 → 合法性校验
use image::RgbaImage;
use serde::Serialize;

use crate::attack;
use crate::chess;
use crate::chess::Board;
use crate::chess::Camp;
use crate::common;
use crate::yolo;

#[derive(Debug, Serialize)]
pub struct Recognition {
    pub board: Vec<chess::Position>, // 校正后的棋子列表(红方在下)
    pub fen: String,                 // 以红方(w)行棋生成的 FEN, 行棋方由用户确认
    pub legal: bool,                 // 通过布局合法性校验
    pub issues: Vec<String>,         // 校验提示
    pub pieces_count: usize,
    pub preview_base64: Option<String>, // 校正后棋盘的预览图(PNG base64)
}

pub fn recognize(origin: RgbaImage, with_preview: bool) -> Result<Recognition, String> {
    let (origin_w, origin_h) = origin.dimensions();
    if origin_w < 64 || origin_h < 64 {
        return Err("图片尺寸过小".to_string());
    }

    // 第一遍: 整图压缩到模型输入, 粗定位棋盘
    let pass1 = yolo::predict(origin.clone()).map_err(|e| format!("模型推理失败: {e}"))?;
    let (cx, cy, cw, ch) = common::board_crop_bound(origin_w, origin_h, &pass1)?;

    // 第二遍: 裁剪棋盘+半格边距区域, 精确识别棋子
    let crop = image::imageops::crop_imm(&origin, cx as u32, cy as u32, cw as u32, ch as u32).to_image();
    let detections = yolo::predict(crop).map_err(|e| format!("模型推理失败: {e}"))?;

    let (bottom_camp, mut board) = common::detections_to_board(&detections)?;
    if bottom_camp == Camp::None {
        return Err("未识别到将/帅, 请确认截图包含完整棋盘".to_string());
    }
    // 统一方向: 红方在下
    chess::board_fix(&bottom_camp, &mut board);

    let pieces_count = board.iter().flatten().filter(|&&p| p != ' ').count();
    if pieces_count < 4 {
        return Err("识别到的棋子过少, 请上传更清晰的残局截图".to_string());
    }

    let mut issues = Vec::new();
    let legal = chess::board_check(board);
    if !legal {
        issues.push("棋子布局未通过合法性校验, 识别可能有误, 请核对预览图".to_string());
    }
    if attack::in_check(&board, &Camp::Red) || attack::in_check(&board, &Camp::Black) {
        issues.push("识别局面存在将军状态, 请确认行棋方".to_string());
    }

    let board_positions = chess::board_map(board);
    let preview_base64 = if with_preview {
        Some(crate::render::render_board_preview(&board))
    } else {
        None
    };

    Ok(Recognition {
        fen: chess::board_fen(&Camp::Red, board),
        legal,
        issues,
        pieces_count,
        board: board_positions.into_iter().filter(|p| p.piece != ' ').collect(),
        preview_base64,
    })
}

// 供生成流程使用: 直接把识别出的棋子列表还原为棋盘
#[allow(dead_code)]
pub fn board_from_recognition(board: &[chess::Position]) -> Result<Board, String> {
    chess::board_from_positions(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::fen_to_board;

    // 用渲染器生成一张棋盘图, 做识别往返测试
    #[test]
    #[ignore] // 需要模型推理, 默认跳过; cargo test -- --ignored 运行
    fn test_recognize_rendered_board() {
        let board = fen_to_board("3k5/9/9/9/9/9/9/4C4/9/4K3R w");
        let png = crate::render::render_board_image(&board).unwrap();
        let rec = recognize(png, false).expect("识别失败");
        let back = chess::board_from_positions(&rec.board).unwrap();
        assert_eq!(back, board, "往返识别结果不一致: {:?}", rec.board);
    }
}

// 调试: 渲染→识别的中间产物落盘 (cargo test debug_recog -- --ignored --nocapture)
#[cfg(test)]
mod debug_tests {
    use crate::chess::fen_to_board;

    #[test]
    #[ignore]
    fn debug_recog() {
        let board = fen_to_board("3k5/9/9/9/9/9/9/4C4/9/4K3R w");
        let img = crate::render::render_board_image(&board).unwrap();
        img.save("../scripts/out/dbg-full.png").unwrap();

        // 第一遍
        let d1 = crate::yolo::predict(img.clone()).unwrap();
        for d in &d1 {
            println!("pass1: {} @ ({:.0},{:.0})-({:.0},{:.0}) conf={:.2}", d.label, d.x0, d.y0, d.x1, d.y1, d.confidence);
        }
        // 画框保存
        let mut annotated = img.clone();
        for d in &d1 {
            crate::render::debug_draw_rect(&mut annotated, d.x0 as i32, d.y0 as i32, d.x1 as i32, d.y1 as i32);
        }
        annotated.save("../scripts/out/dbg-pass1.png").unwrap();

        let (cw, chh) = img.dimensions();
        let (cx, cy, w, h) = crate::common::board_crop_bound(cw, chh, &d1).unwrap();
        println!("crop: ({cx},{cy}) {w}x{h}");
        let crop = image::imageops::crop_imm(&img, cx as u32, cy as u32, w as u32, h as u32).to_image();
        crop.save("../scripts/out/dbg-crop.png").unwrap();

        let d2 = crate::yolo::predict(crop).unwrap();
        for d in &d2 {
            let cxm = (d.x0 + d.x1) / 2.0;
            let cym = (d.y0 + d.y1) / 2.0;
            println!("pass2: {} @ center ({cxm:.0},{cym:.0}) conf={:.2}", d.label, d.confidence);
        }
    }
}
