// 棋盘表示与规则工具, 移植自参考项目 chessboard 的 chess.rs
pub const BOARD_MAP: [[&str; 9]; 10] = [
    ["a9", "b9", "c9", "d9", "e9", "f9", "g9", "h9", "i9"],
    ["a8", "b8", "c8", "d8", "e8", "f8", "g8", "h8", "i8"],
    ["a7", "b7", "c7", "d7", "e7", "f7", "g7", "h7", "i7"],
    ["a6", "b6", "c6", "d6", "e6", "f6", "g6", "h6", "i6"],
    ["a5", "b5", "c5", "d5", "e5", "f5", "g5", "h5", "i5"],
    ["a4", "b4", "c4", "d4", "e4", "f4", "g4", "h4", "i4"],
    ["a3", "b3", "c3", "d3", "e3", "f3", "g3", "h3", "i3"],
    ["a2", "b2", "c2", "d2", "e2", "f2", "g2", "h2", "i2"],
    ["a1", "b1", "c1", "d1", "e1", "f1", "g1", "h1", "i1"],
    ["a0", "b0", "c0", "d0", "e0", "f0", "g0", "h0", "i0"],
];

pub type Board = [[char; 9]; 10];

#[derive(Debug, serde::Serialize, Clone, serde::Deserialize)]
pub struct Position {
    pub piece: char,
    pub pos: String,
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Camp {
    #[default]
    None,
    Red,
    Black,
}

impl Camp {
    pub fn to_char(&self) -> char {
        match self {
            Camp::None => '0',
            Camp::Red => 'w',
            Camp::Black => 'b',
        }
    }

    pub fn from_char(c: char) -> Self {
        match c {
            'w' => Camp::Red,
            'b' => Camp::Black,
            _ => Camp::None,
        }
    }

    pub fn from_piece(p: char) -> Self {
        if p > 'Z' {
            Self::Black
        } else {
            Self::Red
        }
    }

    #[allow(dead_code)]
    pub fn is_black(&self) -> bool {
        Camp::Black.eq(self)
    }

    pub fn opposite(&self) -> Self {
        match self {
            Camp::Red => Camp::Black,
            Camp::Black => Camp::Red,
            Camp::None => Camp::None,
        }
    }
}

const BLACK_VERTICALS: [char; 9] = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];
const RED_VERTICALS: [char; 9] = ['九', '八', '七', '六', '五', '四', '三', '二', '一'];

#[allow(dead_code)]
pub const RED_STARTPOS: [[char; 9]; 10] = [
    ['r', 'n', 'b', 'a', 'k', 'a', 'b', 'n', 'r'],
    [' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '],
    [' ', 'c', ' ', ' ', ' ', ' ', ' ', 'c', ' '],
    ['p', ' ', 'p', ' ', 'p', ' ', 'p', ' ', 'p'],
    [' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '],
    [' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '],
    ['P', ' ', 'P', ' ', 'P', ' ', 'P', ' ', 'P'],
    [' ', 'C', ' ', ' ', ' ', ' ', ' ', 'C', ' '],
    [' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '],
    ['R', 'N', 'B', 'A', 'K', 'A', 'B', 'N', 'R'],
];

pub fn get_verticals(p: char) -> [char; 9] {
    if p > 'Z' {
        BLACK_VERTICALS
    } else {
        RED_VERTICALS
    }
}

pub struct Move {
    from_x: usize,
    from_y: usize,
    to_x: usize,
    to_y: usize,
}

impl Move {
    pub fn new(iccs: &str) -> Self {
        let mut cs = iccs.chars();
        let from_x = cs.next().unwrap() as usize - 97;
        let from_y = 9 - cs.next().unwrap().to_digit(10).unwrap() as usize;
        let to_x = cs.next().unwrap() as usize - 97;
        let to_y = 9 - cs.next().unwrap().to_digit(10).unwrap() as usize;
        Self { from_x, from_y, to_x, to_y }
    }
}

pub fn board_move(board: Board, iccs: &str) -> Board {
    let mv = Move::new(iccs);
    let mut new_board = board;
    let p = new_board[mv.from_y][mv.from_x];
    new_board[mv.to_y][mv.to_x] = p;
    new_board[mv.from_y][mv.from_x] = ' ';
    new_board
}

// 该步棋是否吃子, 返回被吃的棋子
pub fn captured_piece(board: Board, iccs: &str) -> Option<char> {
    let mv = Move::new(iccs);
    let target = board[mv.to_y][mv.to_x];
    (target != ' ').then_some(target)
}

// 棋盘转换FEN逻辑
pub fn board_fen(camp: &Camp, board: Board) -> String {
    let mut fen = String::new();
    for row in &board {
        let mut empty = 0;
        for &piece in row {
            if piece == ' ' {
                empty += 1;
            } else {
                if empty > 0 {
                    fen.push_str(&empty.to_string());
                    empty = 0;
                }
                fen.push(piece);
            }
        }
        if empty > 0 {
            fen.push_str(&empty.to_string());
        }
        fen.push('/');
    }
    fen.pop();
    fen.push(' ');
    fen.push(camp.to_char());
    fen
}

#[allow(dead_code)]
pub fn fen_to_board(mut fen: &str) -> Board {
    if fen.contains(' ') {
        fen = fen.split_once(' ').unwrap().0;
    }
    let mut board = [[' '; 9]; 10];
    let mut rank = 0;
    let mut file = 0;
    for c in fen.chars() {
        match c {
            '1'..='9' => {
                file += c.to_digit(10).unwrap() as usize;
            }
            '/' => {
                rank += 1;
                file = 0;
            }
            _ => {
                board[rank][file] = c;
                file += 1;
            }
        }
    }
    board
}

// 检测棋盘是否合法(布局层面)
pub fn board_check(board: Board) -> bool {
    let mut cnt = std::collections::HashMap::new();
    for (y, row) in board.iter().enumerate() {
        for (x, &col) in row.iter().enumerate() {
            match col {
                'k' => {
                    *cnt.entry('k').or_insert(0usize) += 1;
                    if y > 2 || !(3..=5).contains(&x) {
                        tracing::warn!("黑方'将'不在合法位置内");
                        return false;
                    }
                }
                'a' => {
                    *cnt.entry('a').or_insert(0usize) += 1;
                    if !(x == 3 && (y == 0 || y == 2))
                        && !(x == 4 && y == 1)
                        && !(x == 5 && (y == 0 || y == 2))
                    {
                        tracing::warn!("黑方'士'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                'b' => {
                    *cnt.entry('b').or_insert(0usize) += 1;
                    if !(y == 0 && (x == 2 || x == 6))
                        && !(y == 2 && (x == 0 || x == 4 || x == 8))
                        && !(y == 4 && (x == 2 || x == 6))
                    {
                        tracing::warn!("黑方'象'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                'p' => {
                    *cnt.entry('p').or_insert(0usize) += 1;
                    if y < 3 {
                        tracing::warn!("黑方'兵'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                    if y < 5 && x % 2 == 1 {
                        tracing::warn!("黑方'兵'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                'K' => {
                    *cnt.entry('K').or_insert(0usize) += 1;
                    if y < 7 || !(3..=5).contains(&x) {
                        tracing::warn!("红方'将'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                'A' => {
                    *cnt.entry('A').or_insert(0usize) += 1;
                    if !(x == 3 && (y == 7 || y == 9))
                        && !(x == 4 && y == 8)
                        && !(x == 5 && (y == 7 || y == 9))
                    {
                        tracing::warn!("红方'仕'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                'B' => {
                    *cnt.entry('B').or_insert(0usize) += 1;
                    if !(y == 9 && (x == 2 || x == 6))
                        && !(y == 7 && (x == 0 || x == 4 || x == 8))
                        && !(y == 5 && (x == 2 || x == 6))
                    {
                        tracing::warn!("红方'相'不在合法位置内, ({}行{}列)", y, x);
                        return false;
                    }
                }
                ' ' => {}
                piece => {
                    *cnt.entry(piece).or_insert(0usize) += 1;
                }
            }
        }
    }

    if cnt.get(&'k').copied().unwrap_or(0) != 1 || cnt.get(&'K').copied().unwrap_or(0) != 1 {
        tracing::warn!("黑方或红方'将'超出合法数量");
        return false;
    }
    for &p in &['a', 'A', 'b', 'B', 'c', 'C', 'r', 'R', 'n', 'N'] {
        if cnt.get(&p).copied().unwrap_or(0) > 2 {
            tracing::warn!("棋子 {} 超出合法数量", p);
            return false;
        }
    }
    for &p in &['p', 'P'] {
        if cnt.get(&p).copied().unwrap_or(0) > 5 {
            tracing::warn!("棋子 {} 超出合法数量", p);
            return false;
        }
    }
    true
}

pub const fn get_piece_name(piece: char) -> char {
    match piece {
        'K' => '帅',
        'k' => '将',
        'A' => '仕',
        'a' => '士',
        'B' => '相',
        'b' => '象',
        'N' => '马',
        'n' => '马',
        'R' => '车',
        'r' => '车',
        'C' => '炮',
        'c' => '炮',
        'P' => '兵',
        'p' => '卒',
        _ => ' ',
    }
}

#[allow(dead_code)]
pub fn startpos(board: Board) -> bool {
    board == RED_STARTPOS
}

// 校正棋盘方向: 统一为红方在下(row 9 为红方底线)
pub fn board_fix(camp: &Camp, board: &mut Board) {
    if Camp::Black.eq(camp) {
        board.reverse();
        for i in board {
            i.reverse();
        }
    }
}

// 棋盘数组转换为坐标模式
pub fn board_map(board: Board) -> Vec<Position> {
    let mut position = vec![];
    for row in 0..10 {
        for col in 0..9 {
            position.push(Position { piece: board[row][col], pos: BOARD_MAP[row][col].to_string() });
        }
    }
    position
}

pub fn board_from_positions(positions: &[Position]) -> Result<Board, String> {
    let mut board = [[' '; 9]; 10];
    for p in positions {
        let mut cs = p.pos.chars();
        let x = cs.next().ok_or("坐标缺少列")? as usize - 97;
        let y = 9 - cs.next().ok_or("坐标缺少行")?.to_digit(10).ok_or("坐标行非法")? as usize;
        if x > 8 || y > 9 {
            return Err(format!("坐标越界: {}", p.pos));
        }
        if p.piece == ' ' {
            continue;
        }
        if board[y][x] != ' ' {
            return Err(format!("坐标 {} 处棋子重复", p.pos));
        }
        board[y][x] = p.piece;
    }
    Ok(board)
}

fn overlap_piece_y(board: Board, x: usize, y: usize, piece: char) -> Vec<usize> {
    let mut other_ys = Vec::new();
    for (i, value) in board.iter().enumerate() {
        if i != y && value[x] == piece {
            other_ys.push(i);
        }
    }
    other_ys
}

fn overlap_piece_xy(board: Board, from_x: usize, piece: char) -> Option<std::collections::HashMap<usize, Vec<usize>>> {
    let mut other_xys = std::collections::HashMap::new();
    for x in 0..9 {
        if x != from_x {
            // y 传入 10 作为哨兵值, 意为查找该纵线上全部同型棋子
            let other_ys = overlap_piece_y(board, x, 10, piece);
            if other_ys.len() > 1 {
                other_xys.insert(x, other_ys);
                return Some(other_xys);
            }
        }
    }
    None
}

// 棋子坐标移动转中文模式(纵线记法), 移植自 chessboard
pub fn board_move_chinese(board: Board, iccs: &str) -> String {
    let mut chinese = String::new();
    let mv = Move::new(iccs);
    let piece = board[mv.from_y][mv.from_x];
    let verticals = get_verticals(piece);
    match piece {
        'K' => {
            chinese.push(get_piece_name(piece));
            chinese.push(verticals[mv.from_x]);
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Equal => {
                    chinese.push('平');
                    chinese.push(verticals[mv.to_x]);
                }
                std::cmp::Ordering::Less => {
                    chinese.push('退');
                    chinese.push(verticals[8]);
                }
                std::cmp::Ordering::Greater => {
                    chinese.push('进');
                    chinese.push(verticals[8]);
                }
            }
        }
        'k' => {
            chinese.push(get_piece_name(piece));
            chinese.push(verticals[mv.from_x]);
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Less => {
                    chinese.push('进');
                    chinese.push(verticals[0]);
                }
                std::cmp::Ordering::Greater => {
                    chinese.push('退');
                    chinese.push(verticals[0]);
                }
                std::cmp::Ordering::Equal => {
                    chinese.push('平');
                    chinese.push(verticals[mv.to_x]);
                }
            }
        }
        'A' | 'B' => {
            chinese.push(get_piece_name(piece));
            chinese.push(verticals[mv.from_x]);
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Less => chinese.push('退'),
                _ => chinese.push('进'),
            }
            chinese.push(verticals[mv.to_x]);
        }
        'a' | 'b' => {
            chinese.push(get_piece_name(piece));
            chinese.push(verticals[mv.from_x]);
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Less => chinese.push('进'),
                _ => chinese.push('退'),
            }
            chinese.push(verticals[mv.to_x]);
        }
        'R' | 'C' => {
            let other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if mv.from_y > other_ys[0] {
                chinese.push('后');
                chinese.push(get_piece_name(piece));
            } else {
                chinese.push('前');
                chinese.push(get_piece_name(piece));
            }
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Less => {
                    let step = 9 - mv.to_y + mv.from_y;
                    chinese.push('退');
                    chinese.push(verticals[step]);
                }
                std::cmp::Ordering::Greater => {
                    let step = 9 - mv.from_y + mv.to_y;
                    chinese.push('进');
                    chinese.push(verticals[step]);
                }
                std::cmp::Ordering::Equal => {
                    chinese.push('平');
                    chinese.push(verticals[mv.to_x]);
                }
            }
        }
        'r' | 'c' => {
            let other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if mv.from_y > other_ys[0] {
                chinese.push('后');
                chinese.push(get_piece_name(piece));
            } else {
                chinese.push('前');
                chinese.push(get_piece_name(piece));
            }
            match mv.from_y.cmp(&mv.to_y) {
                std::cmp::Ordering::Less => {
                    let step = mv.to_y - mv.from_y - 1;
                    chinese.push('进');
                    chinese.push(verticals[step]);
                }
                std::cmp::Ordering::Greater => {
                    let step = mv.from_y - mv.to_y - 1;
                    chinese.push('退');
                    chinese.push(verticals[step]);
                }
                std::cmp::Ordering::Equal => {
                    chinese.push('平');
                    chinese.push(verticals[mv.to_x]);
                }
            }
        }
        'N' => {
            let other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if mv.from_y > other_ys[0] {
                chinese.push('后');
                chinese.push(get_piece_name(piece));
            } else {
                chinese.push('前');
                chinese.push(get_piece_name(piece));
            }
            if mv.from_y < mv.to_y {
                chinese.push('退');
            } else {
                chinese.push('进');
            }
            chinese.push(verticals[mv.to_x]);
        }
        'n' => {
            let other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if mv.from_y > other_ys[0] {
                chinese.push('前');
                chinese.push(get_piece_name(piece));
            } else {
                chinese.push('后');
                chinese.push(get_piece_name(piece));
            }
            if mv.from_y < mv.to_y {
                chinese.push('进');
            } else {
                chinese.push('退');
            }
            chinese.push(verticals[mv.to_x]);
        }
        'P' => {
            let mut other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if let Some(other_xys) = overlap_piece_xy(board, mv.from_x, piece) {
                for y in &mut other_ys {
                    *y = (mv.from_x + 10) * 100 - *y;
                }
                for (x, ys) in &other_xys {
                    for y in ys {
                        other_ys.push((x + 10) * 100 - y);
                    }
                }
                let value = (mv.from_x + 10) * 100 - mv.from_y;
                other_ys.push(value);
                other_ys.sort_by(|a, b| b.cmp(a));
                let seq = other_ys.iter().position(|&v| v == value).map(|i| i + 1).unwrap();
                chinese.push(verticals[9 - seq]);
            } else if other_ys.len() > 1 {
                let mut num = 1;
                for y in other_ys {
                    if mv.from_y < y {
                        break;
                    }
                    num += 1;
                }
                chinese.push_str(num.to_string().as_str());
            } else if mv.from_y > other_ys[0] {
                chinese.push('后');
            } else {
                chinese.push('前');
            }
            chinese.push(get_piece_name(piece));
            if mv.from_y == mv.to_y {
                chinese.push('平');
                chinese.push(verticals[mv.to_x]);
            } else {
                chinese.push('进');
                chinese.push(verticals[8]);
            }
        }
        'p' => {
            let mut other_ys = overlap_piece_y(board, mv.from_x, mv.from_y, piece);
            if other_ys.is_empty() {
                chinese.push(get_piece_name(piece));
                chinese.push(verticals[mv.from_x]);
            } else if let Some(other_xys) = overlap_piece_xy(board, mv.from_x, piece) {
                for y in &mut other_ys {
                    *y += mv.from_x * 100;
                }
                for (x, ys) in &other_xys {
                    for y in ys {
                        other_ys.push(x * 100 + y);
                    }
                }
                let value = mv.from_x * 100 + mv.from_y;
                other_ys.push(value);
                other_ys.sort_by(|a, b| b.cmp(a));
                let seq = other_ys.iter().position(|&v| v == value).unwrap();
                chinese.push(verticals[seq]);
            } else if other_ys.len() > 1 {
                let mut num = 0;
                for y in other_ys {
                    if mv.from_y > y {
                        break;
                    }
                    num += 1;
                }
                chinese.push_str(num.to_string().as_str());
            } else if mv.from_y > other_ys[0] {
                chinese.push('前');
            } else {
                chinese.push('后');
            }
            chinese.push(get_piece_name(piece));
            if mv.from_y == mv.to_y {
                chinese.push('平');
                chinese.push(verticals[mv.to_x]);
            } else {
                chinese.push('进');
                chinese.push(verticals[0]);
            }
        }
        _ => {}
    }
    chinese
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_fen_roundtrip() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w";
        let board = fen_to_board(fen);
        assert_eq!(board_fen(&Camp::Red, board), fen);
        assert!(startpos(board));
        assert!(board_check(board));
    }

    #[test]
    fn test_move_chinese() {
        // 经典中炮开局: 炮二平五
        let board = fen_to_board("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w");
        assert_eq!(board_move_chinese(board, "h2e2"), "炮二平五");
        // 马二进三
        assert_eq!(board_move_chinese(board, "h0g2"), "马二进三");
        // 黑方应招: 马8进7 (h9g7)
        assert_eq!(board_move_chinese(board_move(board, "h2e2"), "h9g7"), "马8进7");
    }
}
