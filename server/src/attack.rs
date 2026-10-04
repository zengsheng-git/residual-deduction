// 攻击检测, 移植自参考项目 chessboard 的 attack.rs
use crate::chess::Camp;
use crate::chess::Board;

fn same_camp(camp: &Camp, piece: char) -> bool {
    match camp {
        Camp::Red => piece.is_ascii_uppercase(),
        Camp::Black => piece.is_ascii_lowercase(),
        Camp::None => false,
    }
}

// 棋子是否在本方九宫内
fn in_palace(piece: char, x: usize, y: usize) -> bool {
    if !(3..=5).contains(&x) {
        return false;
    }
    if piece.is_ascii_uppercase() {
        (7..=9).contains(&y)
    } else {
        (0..=2).contains(&y)
    }
}

// 同一横/竖线上两点之间的棋子数(不含端点)
fn count_between(board: &Board, x: usize, y: usize, tx: usize, ty: usize) -> usize {
    let mut count = 0;
    if y == ty {
        for xx in x.min(tx) + 1..x.max(tx) {
            if board[y][xx] != ' ' {
                count += 1;
            }
        }
    } else {
        for yy in y.min(ty) + 1..y.max(ty) {
            if board[yy][x] != ' ' {
                count += 1;
            }
        }
    }
    count
}

// (x,y) 处棋子是否攻击 (tx,ty)。攻击语义: 炮需恰好一个炮架, 车需通路无阻
pub fn piece_attacks(board: &Board, x: usize, y: usize, tx: usize, ty: usize) -> bool {
    if x == tx && y == ty {
        return false;
    }
    let piece = board[y][x];
    if piece == ' ' {
        return false;
    }
    let dx = tx as i32 - x as i32;
    let dy = ty as i32 - y as i32;
    match piece {
        'R' | 'r' => (x == tx || y == ty) && count_between(board, x, y, tx, ty) == 0,
        'C' | 'c' => (x == tx || y == ty) && count_between(board, x, y, tx, ty) == 1,
        'N' | 'n' => {
            let (adx, ady) = (dx.abs(), dy.abs());
            if !((adx == 1 && ady == 2) || (adx == 2 && ady == 1)) {
                return false;
            }
            // 马腿在直线方向上相邻的格子
            let (lx, ly) = if adx == 2 { (x as i32 + dx / 2, y as i32) } else { (x as i32, y as i32 + dy / 2) };
            board[ly as usize][lx as usize] == ' '
        }
        'P' | 'p' => {
            let forward: i32 = if piece.is_ascii_uppercase() { -1 } else { 1 };
            if dx == 0 && dy == forward {
                return true;
            }
            let crossed = if piece.is_ascii_uppercase() { y <= 4 } else { y >= 5 };
            crossed && dy == 0 && dx.abs() == 1
        }
        'K' | 'k' => {
            // 白脸将: 双将同线且中间无子
            let target = board[ty][tx];
            let facing = (target == 'K' || target == 'k') && x == tx && count_between(board, x, y, tx, ty) == 0;
            facing || (dx.abs() + dy.abs() == 1 && in_palace(piece, tx, ty))
        }
        'A' | 'a' => dx.abs() == 1 && dy.abs() == 1 && in_palace(piece, tx, ty),
        'B' | 'b' => {
            if dx.abs() != 2 || dy.abs() != 2 {
                return false;
            }
            // 象不能过河
            if piece.is_ascii_uppercase() && ty < 5 {
                return false;
            }
            if !piece.is_ascii_uppercase() && ty > 4 {
                return false;
            }
            let (ex, ey) = (x as i32 + dx / 2, y as i32 + dy / 2);
            board[ey as usize][ex as usize] == ' '
        }
        _ => false,
    }
}

// (tx,ty) 是否被 camp 方任一棋子攻击
pub fn is_attacked(board: &Board, camp: &Camp, tx: usize, ty: usize) -> bool {
    for y in 0..10 {
        for x in 0..9 {
            let piece = board[y][x];
            if piece != ' ' && same_camp(camp, piece) && piece_attacks(board, x, y, tx, ty) {
                return true;
            }
        }
    }
    false
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

// camp 方的将是否被将军
pub fn in_check(board: &Board, camp: &Camp) -> bool {
    let king = if camp.eq(&Camp::Red) { 'K' } else { 'k' };
    let Some((kx, ky)) = find_king(board, king) else { return false };
    is_attacked(board, &camp.opposite(), kx, ky)
}

// camp 方(刚走完棋的一方)这步棋是否形成将军
pub fn gives_check(board_after: &Board, mover: &Camp) -> bool {
    in_check(board_after, &mover.opposite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::fen_to_board;

    #[test]
    fn test_check_detection() {
        // 黑将在 e9, 黑卒 e7 作炮架, 红炮 e5 架起将军; 红帅 e0 有卒阻挡不成白脸将
        let board = fen_to_board("4k4/9/4p4/9/4C4/9/9/9/9/4K4 w");
        assert!(in_check(&board, &Camp::Black));
        assert!(!in_check(&board, &Camp::Red));
        // 炮平开后不再将军
        let moved = crate::chess::board_move(board, "e5d5");
        assert!(!in_check(&moved, &Camp::Black));
    }

    #[test]
    fn test_white_facing_king() {
        // 双将同线无隔子: 白脸将, 双方互将
        let board = fen_to_board("4k4/9/9/9/9/9/9/9/9/4K4 w");
        assert!(in_check(&board, &Camp::Black));
        assert!(in_check(&board, &Camp::Red));
    }
}
