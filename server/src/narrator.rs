// 解说词模板引擎: 把引擎数据与局面特征转成中文解说
use crate::chess::get_piece_name;
use crate::chess::Board;
use crate::chess::Camp;

// 评分解读: 行棋方视角, 正=行棋方占优 (与参考项目 format.ts 一致)
pub fn eval_text(score: isize) -> String {
    let abs = score.abs();
    if score >= 29000 {
        return format!("{}步杀", 30000 - score);
    }
    if score <= -29000 {
        return format!("{}步被杀", 30000 + score);
    }
    if abs < 30 {
        return "均势".to_string();
    }
    let grade_pos = ["略优", "较优", "大优", "胜势"];
    let grade_neg = ["略差", "较差", "大差", "败势"];
    let idx = if abs < 150 { 0 } else if abs < 400 { 1 } else if abs < 800 { 2 } else { 3 };
    if score > 0 {
        format!("+{} {}", abs, grade_pos[idx])
    } else {
        format!("-{} {}", abs, grade_neg[idx])
    }
}

// 次优招法分差解读
pub fn gap_text(gap: isize) -> &'static str {
    if gap <= 30 {
        "与主招几乎等效"
    } else if gap < 200 {
        "略显吃亏"
    } else if gap < 600 {
        "明显吃亏"
    } else {
        "立刻崩盘"
    }
}

pub fn camp_name(camp: &Camp) -> &'static str {
    match camp {
        Camp::Red => "红",
        Camp::Black => "黑",
        Camp::None => "",
    }
}

// mate 分数 → 该方还需走的步数(引擎 round 即行棋方步数: mate 1 = 这一步就杀)
pub fn mate_moves(score: isize) -> Option<usize> {
    if score >= 29000 {
        Some((30000 - score) as usize)
    } else {
        None
    }
}

// 一方阵容描述: "车、马、炮、双兵、仕相全"
pub fn material_list(board: &Board, camp: &Camp) -> String {
    let red = matches!(camp, Camp::Red);
    let mut names: Vec<String> = Vec::new();
    let (r, n, b, a, c, p) = if red { ('R', 'N', 'B', 'A', 'C', 'P') } else { ('r', 'n', 'b', 'a', 'c', 'p') };
    for piece in [r, n, b, a, c, p] {
        let single = get_piece_name(piece);
        let n = board.iter().flatten().filter(|&&q| q == piece).count();
        let text = match n {
            0 => None,
            1 => Some(single.to_string()),
            2 => Some(format!("双{}", single)),
            k => Some(format!("{}{}", cn_num(k), single)),
        };
        if let Some(t) = text {
            names.push(t);
        }
    }
    if names.is_empty() {
        return "仅剩孤将".to_string();
    }
    names.join("、")
}

fn cn_num(k: usize) -> &'static str {
    match k {
        3 => "三",
        4 => "四",
        5 => "五",
        _ => "多",
    }
}

// 单步解说词
#[allow(clippy::too_many_arguments)]
pub fn move_comment(
    board_before: &Board,
    iccs: &str,
    chinese: &str,
    camp: &Camp,
    capture: Option<char>,
    check: bool,
    mate: bool,
    branches: &[(String, String)],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(chinese.to_string());
    if let Some(captured) = capture {
        parts.push(format!("吃掉{}{}", camp_name(&Camp::from_piece(captured)), get_piece_name(captured)));
    }
    if mate {
        // 有将军的终局是绝杀; 无子可动则是困毙
        if check {
            parts.push("绝杀, 无解!".to_string());
        } else {
            parts.push("困毙, 无解!".to_string());
        }
    } else if check {
        parts.push("将军!".to_string());
    }
    for (alt_chinese, note) in branches.iter().take(2) {
        if note == "与主招几乎等效" {
            parts.push(format!("如果改走{}, 效果与主招相当", alt_chinese));
        } else {
            parts.push(format!("如果改走{}, {}方有反制, {}", alt_chinese, camp_name(&camp.opposite()), note));
        }
    }
    let _ = board_before;
    let _ = iccs;
    parts.join(", ")
}

// 分支演示场景解说词: 假设防守方不走主线防着, 展示其备选防着与进攻方的反制
pub fn branch_demo_comment(defender: &Camp, main_chinese: &str, alt_chinese: &str, reply_chinese: Option<&str>, note: &str) -> String {
    let attacker = camp_name(&defender.opposite());
    match reply_chinese {
        Some(reply) => format!(
            "假如{}方不走{}, 改走{}, {}方立即应以{}, {}方{}。",
            camp_name(defender),
            main_chinese,
            alt_chinese,
            attacker,
            reply,
            camp_name(defender),
            note
        ),
        None => format!(
            "假如{}方不走{}, 改走{}, 效果与正着相当, {}方的进攻路线不变。",
            camp_name(defender),
            main_chinese,
            alt_chinese,
            attacker
        ),
    }
}

pub fn intro_comment(camp: &Camp, red_material: &str, black_material: &str, verdict: &str) -> String {
    let describe = |m: &str| {
        if m == "仅剩孤将" {
            m.to_string()
        } else {
            format!("有{}", m)
        }
    };
    format!(
        "欢迎来到残局推演。看当前局面, 红方{}, 黑方{}。{}方先行, {}。这盘棋为什么能赢? 我们一步一步推演。",
        describe(red_material),
        describe(black_material),
        camp_name(camp),
        verdict
    )
}

pub fn outro_comment(verdict: &str, key_moves: &[String]) -> String {
    let mut parts = Vec::new();
    if !key_moves.is_empty() {
        parts.push(format!("回顾整盘棋, 关键几手是{}", key_moves.join("、")));
    }
    parts.push(format!("残局取胜的思路, 就是利用先手不断压迫, 直到形成绝杀。最终{}。感谢观看, 我们下期再见", verdict));
    parts.join("。")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::fen_to_board;

    #[test]
    fn test_eval_text() {
        assert_eq!(eval_text(29998), "2步杀");
        assert_eq!(eval_text(10), "均势");
        assert_eq!(eval_text(200), "+200 较优");
        assert_eq!(eval_text(-900), "-900 败势");
    }

    #[test]
    fn test_material() {
        let board = fen_to_board("3k5/9/9/9/9/9/9/4C4/9/4K3R w");
        assert_eq!(material_list(&board, &Camp::Red), "车、炮");
        assert_eq!(material_list(&board, &Camp::Black), "仅剩孤将");
    }

    #[test]
    fn test_mate_moves() {
        assert_eq!(mate_moves(29999), Some(1));
        assert_eq!(mate_moves(29998), Some(2));
        assert_eq!(mate_moves(29994), Some(6));
        assert_eq!(mate_moves(500), None);
    }

    #[test]
    fn test_branch_demo_comment() {
        let black = Camp::Black;
        assert_eq!(
            branch_demo_comment(&black, "将4平5", "车4退2", Some("车五进一"), "明显吃亏"),
            "假如黑方不走将4平5, 改走车4退2, 红方立即应以车五进一, 黑方明显吃亏。"
        );
        assert_eq!(
            branch_demo_comment(&black, "将4平5", "将4退1", Some("车四进八"), "1步被杀"),
            "假如黑方不走将4平5, 改走将4退1, 红方立即应以车四进八, 黑方1步被杀。"
        );
        assert_eq!(
            branch_demo_comment(&black, "将4平5", "车4退2", None, "与主招几乎等效"),
            "假如黑方不走将4平5, 改走车4退2, 效果与正着相当, 红方的进攻路线不变。"
        );
    }
}
