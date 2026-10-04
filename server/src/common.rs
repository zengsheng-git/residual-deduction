// 识别结果 → 棋盘结构的映射, 移植自参考项目 chessboard 的 common.rs
use crate::chess::Board;
use crate::chess::Camp;
use crate::yolo;
use crate::yolo::Detection;

const MODEL_CELL_W: f32 = yolo::IMAGE_WIDTH as f32 / 9.0;
const MODEL_CELL_H: f32 = yolo::IMAGE_HEIGHT as f32 / 10.0;

// detections_to_board 识别结果转换为棋盘结构
// camp 含义: 图片底部一侧的阵营(用于方向校正), 棋盘统一校正为红方在下
pub fn detections_to_board(detections: &[Detection]) -> Result<(Camp, Board), String> {
    let mut camp = Camp::None;
    let mut board = [[' '; 9]; 10];

    match detections.iter().find(|&&x| x.label == '0') {
        Some(_) => {
            for det in detections.iter().filter(|d| d.label != '0') {
                // 中心点
                let cx = (det.x0 + det.x1) / 2.0;
                let cy = (det.y0 + det.y1) / 2.0;
                // 行列: x 轴分成 9 格, y 轴分成 10 格
                let col = (cx / MODEL_CELL_W).floor() as usize; // 0–8
                let row = (cy / MODEL_CELL_H).floor() as usize; // 0–9
                tracing::trace!("{} row={} col={}", det.label, row, col);

                // 边界处理
                if !(0..=8).contains(&col) || !(0..=9).contains(&row) {
                    continue;
                }

                // 构建board
                board[row][col] = det.label;

                // 判断阵营: 九宫区域出现的将/帅代表图片底部一侧的阵营
                if camp == Camp::None && (3..=5).contains(&col) && row >= 7 {
                    match det.label {
                        'k' => camp = Camp::Black,
                        'K' => camp = Camp::Red,
                        _ => {}
                    }
                }
            }
        }
        None => return Err("not board".to_string()),
    }
    Ok((camp, board))
}

// 在原始整图(压缩到 640x640 的模型坐标系)中定位棋盘框, 返回原图坐标系下的
// 棋盘+半格边距裁剪框 (x, y, w, h), 用于二次精识别
pub fn board_crop_bound(origin_width: u32, origin_height: u32, detections: &[Detection]) -> Result<(f32, f32, f32, f32), String> {
    let board_det = detections.iter().find(|d| d.label == '0').ok_or("未识别到棋盘")?;

    let scale_x = origin_width as f32 / yolo::IMAGE_WIDTH as f32;
    let scale_y = origin_height as f32 / yolo::IMAGE_HEIGHT as f32;

    let bx0 = (board_det.x0 * scale_x).max(0.0);
    let by0 = (board_det.y0 * scale_y).max(0.0);
    let bx1 = (board_det.x1 * scale_x).min(origin_width as f32);
    let by1 = (board_det.y1 * scale_y).min(origin_height as f32);

    let board_w = bx1 - bx0;
    let board_h = by1 - by0;
    let half_cell_x = board_w / 9.0 / 2.0;
    let half_cell_y = board_h / 10.0 / 2.0;

    let crop_x = (bx0 - half_cell_x).max(0.0);
    let crop_y = (by0 - half_cell_y).max(0.0);
    let x1p = (bx1 + half_cell_x).min(origin_width as f32);
    let y1p = (by1 + half_cell_y).min(origin_height as f32);

    Ok((crop_x, crop_y, x1p - crop_x, y1p - crop_y))
}
