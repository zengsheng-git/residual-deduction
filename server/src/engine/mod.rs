// Pikafish UCI 引擎封装, 移植自参考项目 chessboard 的 engine 模块(移除云库依赖)
pub mod command;

use std::fmt::Display;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;

#[derive(Debug, serde::Serialize, Default, Clone)]
pub struct QueryResult {
    pub depth: usize,              // 深度
    pub score: isize,              // 得分(行棋方视角)
    pub time: usize,               // 耗时
    pub pvs: Vec<String>,          // 最优线完整着法(iccs)
    pub alternatives: Vec<String>, // 次优候选首着(iccs)
    pub alt_scores: Vec<isize>,    // 次优候选与最优的分差
    pub alt_pvs: Vec<Vec<String>>, // 次优候选完整变化线(与 alternatives 平行, 第2着即对方应对)
    pub winrate: Option<usize>,    // 行棋方胜率(千分比)
    pub state: QueryState,
}

#[derive(Debug, serde::Serialize, Default, Clone, Copy, PartialEq)]
pub enum QueryState {
    #[default]
    NotResult,
    Success,
    InvalidBoard,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub struct EngineConfig {
    pub depth: usize,
    pub time: usize,
    pub threads: usize,
    pub hash: usize,
    pub multipv: usize,
    pub alt_score_gap: isize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { depth: 24, time: 3000, threads: 4, hash: 256, multipv: 3, alt_score_gap: 400 }
    }
}

// 引擎单行 info 解析结果
#[derive(Default)]
struct InfoLine {
    multipv: Option<usize>,
    pvs: Vec<String>,
    depth: usize,
    score: isize,
    time: usize,
    winrate: Option<usize>,
}

pub struct Engine {
    stdin: Box<dyn Write>,
    stdout: Box<dyn BufRead>,
    child: std::process::Child,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn from_child(mut child: Child, stdin: ChildStdin, libs: &Path) -> Self {
        let nnue = libs.join("pikafish.nnue");
        let stdout = Box::new(BufReader::new(child.stdout.take().unwrap()));
        let mut eng = Engine { stdin: Box::new(stdin), stdout, child };
        eng.setoption("EvalFile", nnue.display().to_string());
        eng.setoption("Sixty Move Rule", false);
        eng
    }

    fn write_command<A: Display>(&mut self, args: A) {
        writeln!(self.stdin, "{}", args).expect("write command error");
        self.stdin.flush().expect("write command flush error");
        tracing::debug!("{}", args);
    }

    pub fn set_threads(&mut self, num: usize) {
        self.setoption("Threads", num);
    }

    pub fn set_hash(&mut self, size: usize) {
        self.setoption("Hash", size);
    }

    pub fn setoption<T: Display>(&mut self, name: &str, value: T) {
        self.write_command(format!("setoption name {} value {}", name, value));
    }

    pub fn position(&mut self, fen: &str) {
        self.write_command(format!("position fen {}", fen));
    }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap_or(0);
        line.trim().to_string()
    }

    // 解析一行 info
    fn parse_info(&self, line: &str) -> InfoLine {
        let mut iter = line.split_whitespace();
        // 跳过 "info"
        iter.next();
        let mut info = InfoLine::default();
        loop {
            let Some(key) = iter.next() else { break };
            match key {
                "depth" => info.depth = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                "time" => info.time = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                "multipv" => info.multipv = iter.next().and_then(|v| v.parse().ok()),
                "wdl" => {
                    // wdl 为胜/和/负千分比, 取行棋方胜率
                    let win: usize = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    let _draw: usize = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    let _loss: usize = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    info.winrate = Some(win);
                }
                "score" => match iter.next().unwrap_or("") {
                    "cp" => info.score = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                    "mate" => {
                        let round: isize = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        info.score = if round > 0 { 30000 - round } else { -(30000 + round) };
                    }
                    _ => {}
                },
                "pv" => {
                    // pv 是 info 行最后一个字段，收集剩余所有着法
                    info.pvs.extend(iter.by_ref().map(|s| s.to_string()));
                    break;
                }
                _ => {}
            }
        }
        info
    }

    // 执行引擎搜索, 返回解析后的结果(含多候选)
    fn bestmove(&mut self, depth: usize, time: usize, multipv: usize, alt_score_gap: isize) -> QueryResult {
        self.setoption("MultiPV", multipv);
        self.write_command(format!("go depth {} movetime {}", depth, time));

        let mut result = QueryResult::default();

        // multipv编号 -> 该候选的完整pv序列与分数
        let mut pvs_by_id: std::collections::BTreeMap<usize, (Vec<String>, isize)> = std::collections::BTreeMap::new();
        let mut last_pvs: Vec<String> = Vec::new();

        loop {
            let line = self.read_line();
            if line.starts_with("bestmove") {
                // 引擎对已绝杀局面可能返回 bestmove (none)
                if line.contains("(none)") {
                    result.state = QueryState::InvalidBoard;
                }
                break;
            }
            if !line.starts_with("info") {
                continue;
            }
            let info = self.parse_info(&line);
            if info.pvs.is_empty() {
                continue;
            }
            last_pvs = info.pvs.clone();
            match info.multipv {
                Some(id) => {
                    pvs_by_id.insert(id, (info.pvs, info.score));
                    if id == 1 {
                        result.depth = info.depth;
                        result.score = info.score;
                        result.time = info.time;
                        result.winrate = info.winrate;
                        result.state = QueryState::Success;
                    }
                }
                None => {
                    result.depth = info.depth;
                    result.score = info.score;
                    result.time = info.time;
                    result.winrate = info.winrate;
                    result.state = QueryState::Success;
                }
            }
        }

        // 最优线
        if let Some((pv1, _)) = pvs_by_id.remove(&1) {
            result.pvs = pv1;
        } else if result.pvs.is_empty() {
            result.pvs = last_pvs;
        }
        // 次优候选首着（只保留分数接近最优的好招）
        for (pvs, sc) in pvs_by_id.values() {
            if let Some(first) = pvs.first() {
                if *sc >= result.score - alt_score_gap {
                    result.alternatives.push(first.clone());
                    result.alt_scores.push(result.score - sc);
                    result.alt_pvs.push(pvs.clone());
                }
            }
        }
        result
    }

    // 在指定局面搜索
    pub fn search(&mut self, fen: &str, params: &EngineConfig) -> Option<QueryResult> {
        self.position(fen);
        let result = self.bestmove(params.depth, params.time, params.multipv, params.alt_score_gap);
        match result.state {
            QueryState::InvalidBoard => None,
            _ => Some(result),
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.write_command("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path;

    fn engine_for_test() -> Engine {
        let libs = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../libs/pikafish");
        let (child, stdin) = command::new(&libs);
        Engine::from_child(child, stdin, &libs)
    }

    #[test]
    #[ignore] // 依赖本地引擎进程; cargo test -- --ignored 运行
    fn test_engine_mate() {
        let mut eng = engine_for_test();
        // 单车对孤将必胜: 车四进一路即将军推进
        let fen = "4k4/9/9/9/9/9/9/9/4R4/4K4 w";
        eng.set_threads(4);
        let cfg = EngineConfig::default();
        let result = eng.search(fen, &cfg).expect("搜索失败");
        println!("depth={} score={} pv={:?}", result.depth, result.score, result.pvs);
        assert!(!result.pvs.is_empty());
    }
}

// 追加测试: 连续两次搜索(剧本生成会这样调用)
#[cfg(test)]
mod consec_tests {
    use super::*;
    use std::path;

    #[test]
    #[ignore]
    fn test_consecutive_search() {
        let libs = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../libs/pikafish");
        let (child, stdin) = command::new(&libs);
        let mut eng = Engine::from_child(child, stdin, &libs);
        eng.set_threads(4);
        let fen = "3k5/9/9/9/9/9/9/9/4C4/4KR3 w";
        let cfg = EngineConfig::default();
        let r1 = eng.search(fen, &cfg);
        println!("search1: {:?}", r1.as_ref().map(|r| (r.state, r.score, r.pvs.clone())));
        let r2 = eng.search(fen, &cfg);
        println!("search2: {:?}", r2.as_ref().map(|r| (r.state, r.score, r.pvs.clone())));
        assert!(r1.is_some() && r2.is_some());
        assert!(!r2.unwrap().pvs.is_empty());
    }
}
