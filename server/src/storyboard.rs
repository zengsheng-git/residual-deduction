// 剧本生成: 引擎主线推演 + 关键节点分支分析 + 每步解说
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::attack;
use crate::chess;
use crate::chess::board_fen;
use crate::chess::board_move;
use crate::chess::board_move_chinese;
use crate::chess::Board;
use crate::chess::Camp;
use crate::engine::Engine;
use crate::engine::EngineConfig;
use crate::narrator;

const MAX_PLIES: usize = 40;

#[derive(Debug, Serialize, Clone)]
pub struct Branch {
    pub iccs: String,
    pub chinese: String,
    pub gap: isize,
    pub note: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct MoveScene {
    pub ply: usize,                    // 第几步(从1开始)
    pub camp: char,                    // 行棋方 'w'/'b'
    pub board_before: Vec<chess::Position>, // 走子前局面
    pub iccs: String,                  // 引擎坐标着法
    pub chinese: String,               // 中文纵线着法
    pub score: isize,                  // 走子前评分(行棋方视角)
    pub score_text: String,
    pub winrate: Option<usize>,        // 走子前行棋方胜率(千分比)
    pub capture: Option<char>,         // 被吃的棋子
    pub check: bool,                   // 是否将军
    pub mate: bool,                    // 是否绝杀
    pub comment: String,               // 解说词
    pub branches: Vec<Branch>,         // 该节点的分支分析
}

#[derive(Debug, Serialize)]
pub struct Script {
    pub title: String,
    pub camp: char,                  // 先行方
    pub verdict: String,             // 结论: "红方5步绝杀"
    pub intro_comment: String,
    pub intro_info: Vec<String>,
    pub outro_comment: String,
    pub outro_info: Vec<String>,
    pub scenes: Vec<MoveScene>,
}

fn node_is_branch_root(ply: usize, prev_check: bool, score: isize, budget_left: bool) -> bool {
    if !budget_left {
        return false;
    }
    if ply == 0 {
        return true;
    }
    if score >= 29000 {
        return true;
    }
    if prev_check {
        return true;
    }
    ply % 4 == 3
}

fn verdict_text(score: isize, camp: &Camp) -> String {
    let name = narrator::camp_name(camp);
    let opp = narrator::camp_name(&camp.opposite());
    if let Some(n) = narrator::mate_moves(score) {
        return format!("{}方{}步绝杀", name, n);
    }
    if score <= -29000 {
        let round = (30000 + score) as usize;
        return format!("{}方{}步被绝杀, 局面并不乐观", name, (round + 1) / 2);
    }
    let abs = score.abs();
    let (who, sign) = if score > 0 { (name, true) } else { (opp, false) };
    if abs < 100 {
        "局面接近均势, 胜负取决于后续争夺".to_string()
    } else if abs < 400 {
        format!("{}方略占上风", who)
    } else if abs < 1000 {
        format!("{}方优势明显", who)
    } else {
        let _ = sign;
        format!("{}方呈胜势", who)
    }
}

fn title_text(score: isize, camp: &Camp) -> String {
    if let Some(n) = narrator::mate_moves(score) {
        format!("残局挑战: {}方{}步绝杀", narrator::camp_name(camp), n)
    } else {
        format!("残局推演: {}方主动进攻", narrator::camp_name(camp))
    }
}

pub struct BuildOutput {
    pub script: Script,
}

// 构建剧本。engine 由调用方持有; stop 为取消标记; branch_budget 为分支分析节点上限(0=纯主线)。
#[allow(clippy::too_many_arguments)]
pub fn build(
    engine: &mut Engine,
    cfg: &EngineConfig,
    start_board: Board,
    start_camp: Camp,
    stop: &AtomicBool,
    branch_budget: u32,
) -> Result<BuildOutput, String> {
    let mut cur_board = start_board;
    let mut cur_camp = start_camp;
    let mut ply = 0usize;
    let mut scenes: Vec<MoveScene> = Vec::new();
    let mut branch_budget = branch_budget.min(6);
    let mut prev_check = false;

    // 根节点搜索(带多候选)
    if stop.load(Ordering::Relaxed) {
        return Err("已取消".to_string());
    }
    let root = engine
        .search(&board_fen(&cur_camp, cur_board), cfg)
        .ok_or_else(|| "根节点搜索失败, 该局面可能已经结束".to_string())?;
    let mut node_score: isize = root.score;
    // node_score 是 node_score_camp 一方的视角评分; 走子方视角需换算
    let mut node_score_camp: Camp = start_camp;
    let mut node_winrate: Option<usize> = root.winrate;
    let mut queue: Vec<String> = root.pvs.clone();
    let mut game_over = false;
    if queue.is_empty() {
        return Err("引擎未能给出着法, 该局面可能已经结束".to_string());
    }

    loop {
        if stop.load(Ordering::Relaxed) {
            return Err("已取消".to_string());
        }

        // 分支节点: 用 MultiPV 重搜当前局面, 获取备选着法并刷新后续主线
        let mut branches: Vec<Branch> = Vec::new();
        // 当前节点的评分换算到走子方视角
        let score_for_mover = if node_score_camp == cur_camp { node_score } else { -node_score };
        let want_branch = node_is_branch_root(ply, prev_check, score_for_mover, branch_budget > 0);
        if want_branch {
            match engine.search(&board_fen(&cur_camp, cur_board), cfg) {
                Some(r) => {
                    node_score = r.score;
                    node_score_camp = cur_camp;
                    node_winrate = r.winrate;
                    queue = r.pvs.clone();
                    let alts: Vec<(String, isize)> = r.alternatives.iter().cloned().zip(r.alt_scores.iter().cloned()).collect();
                    for (alt, gap) in alts.into_iter().take(2) {
                        let alt_chinese = board_move_chinese(cur_board, &alt);
                        branches.push(Branch { iccs: alt.clone(), chinese: alt_chinese, gap, note: narrator::gap_text(gap).to_string() });
                    }
                    if !branches.is_empty() {
                        branch_budget -= 1;
                    }
                }
                // bestmove (none): 走子方已被绝杀或困毙, 上一手即为终局
                None => {
                    game_over = true;
                    break;
                }
            }
            if queue.is_empty() {
                return Err("引擎未能给出着法".to_string());
            }
        } else if queue.is_empty() {
            // 主线走完仍未结束: 低成本续搜
            let mut cheap = *cfg;
            cheap.multipv = 1;
            match engine.search(&board_fen(&cur_camp, cur_board), &cheap) {
                Some(r) => {
                    node_score = r.score;
                    node_score_camp = cur_camp;
                    node_winrate = r.winrate;
                    queue = r.pvs;
                }
                None => {
                    game_over = true;
                    break;
                }
            }
            if queue.is_empty() {
                break;
            }
        }

        let best = queue.remove(0);
        let before = cur_board;
        let after = board_move(before, &best);
        let mover = cur_camp;
        let capture = chess::captured_piece(before, &best);
        let check = attack::gives_check(&after, &mover);
        let score_for_mover = if node_score_camp == mover { node_score } else { -node_score };
        let mate_move = score_for_mover >= 29999;
        let chinese = board_move_chinese(before, &best);

        let branch_pairs: Vec<(String, isize)> = branches.iter().map(|b| (b.chinese.clone(), b.gap)).collect();
        let comment = narrator::move_comment(&before, &best, &chinese, &mover, capture, check, mate_move, score_for_mover, &branch_pairs);

        scenes.push(MoveScene {
            ply: ply + 1,
            camp: mover.to_char(),
            board_before: chess::board_map(before).into_iter().filter(|p| p.piece != ' ').collect(),
            iccs: best.clone(),
            chinese,
            score: score_for_mover,
            score_text: narrator::eval_text(score_for_mover),
            winrate: node_winrate,
            capture,
            check,
            mate: mate_move,
            comment,
            branches,
        });

        cur_board = after;
        cur_camp = mover.opposite();
        prev_check = check;
        ply += 1;

        if mate_move || ply >= MAX_PLIES {
            break;
        }
    }

    // 困毙/绝杀终局: 引擎对无子可动的局面返回 (none)
    if game_over
        && let Some(last) = scenes.last_mut()
    {
        last.mate = true;
        if !last.comment.contains("绝杀") {
            last.comment.push_str(", 对方无子可动, 困毙绝杀!");
        }
    }

    if scenes.is_empty() {
        return Err("未能推演出有效着法".to_string());
    }

    let verdict = verdict_text(root.score, &start_camp);
    let title = title_text(root.score, &start_camp);
    let red_material = narrator::material_list(&start_board, &Camp::Red);
    let black_material = narrator::material_list(&start_board, &Camp::Black);
    let intro_comment = narrator::intro_comment(&start_camp, &red_material, &black_material, &verdict);
    let intro_info = vec![
        format!("红方阵容: {}", red_material),
        format!("黑方阵容: {}", black_material),
        format!("先行方: {}方", narrator::camp_name(&start_camp)),
        format!("引擎判断: {}", verdict),
    ];

    // 总结: 挑选关键着法(绝杀/吃车马炮/分支节点/将军)
    let mut key_moves: Vec<String> = Vec::new();
    for s in &scenes {
        let notable = s.mate
            || s.branches.len() > 1
            || matches!(s.capture, Some('r' | 'R' | 'n' | 'N' | 'c' | 'C'))
            || (s.check && s.ply % 2 == 1);
        if notable {
            key_moves.push(format!("第{}步{}", s.ply, s.chinese));
        }
        if key_moves.len() >= 3 {
            break;
        }
    }
    if key_moves.is_empty() {
        key_moves.push(format!("第{}步{}", scenes[0].ply, scenes[0].chinese));
    }
    let outro_comment = narrator::outro_comment(&verdict, &key_moves);
    let mate_scene = scenes.iter().rfind(|s| s.mate);
    let outro_info = vec![
        format!("共推演 {} 步", scenes.len()),
        match mate_scene {
            Some(s) => format!("终局: 第{}步{}绝杀", s.ply, s.chinese),
            None => format!("终局评分: {}", narrator::eval_text(node_score)),
        },
        format!("关键着法: {}", key_moves.join("、")),
    ];

    Ok(BuildOutput {
        script: Script {
            title,
            camp: start_camp.to_char(),
            verdict,
            intro_comment,
            intro_info,
            outro_comment,
            outro_info,
            scenes,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::fen_to_board;
    use crate::engine::command;
    use std::path;

    #[test]
    #[ignore] // 依赖本地引擎; cargo test -- --ignored 运行
    fn test_build_script_mate() {
        let libs = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../libs/pikafish");
        let (child, stdin) = command::new(&libs);
        let mut engine = Engine::from_child(child, stdin, &libs);
        engine.set_threads(4);
        // 车炮对双士的必胜残局
        let board = fen_to_board("3k5/9/9/9/9/9/9/9/4C4/4KR3 w");
        let out = build(&mut engine, &EngineConfig::default(), board, Camp::Red, &AtomicBool::new(false), 4).expect("构建失败");
        println!("title: {}", out.script.title);
        println!("verdict: {}", out.script.verdict);
        println!("intro: {}", out.script.intro_comment);
        for s in &out.script.scenes {
            println!("#{} {} -> {} | {}", s.ply, s.iccs, s.chinese, s.comment);
            for b in &s.branches {
                println!("    分支: {} (亏{})", b.chinese, b.gap);
            }
        }
        println!("outro: {}", out.script.outro_comment);
        assert!(!out.script.scenes.is_empty());
    }
}
