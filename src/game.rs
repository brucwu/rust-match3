//! Pure match-3 game logic (no browser deps).
//!
//! Cell encoding (u8): low nibble = color (0..kinds, 15 = colorless),
//! high nibble = special kind (see `SP_*`). `EMPTY` = 255.
pub const W: usize = 8;
pub const H: usize = 8;
pub const N: usize = W * H;
pub const KINDS: u8 = 5;
pub const EMPTY: u8 = 255;
/// Probability (percent) that a refilled gem copies the color of the gem below it.
pub const REFILL_BIAS_PCT: u64 = 22;

pub const SP_NONE: u8 = 0;
pub const SP_LINE_H: u8 = 1; // clears its row
pub const SP_LINE_V: u8 = 2; // clears its column
pub const SP_AREA: u8 = 3; // clears 3x3
pub const SP_COLOR: u8 = 4; // color bomb (colorless)
pub const COLORLESS: u8 = 15;

pub fn color(c: u8) -> u8 { c & 0x0F }
pub fn kind(c: u8) -> u8 { if c == EMPTY { SP_NONE } else { c >> 4 } }
pub fn make(color: u8, kind: u8) -> u8 { (kind << 4) | color }
pub fn idx(r: usize, c: usize) -> usize { r * W + c }

#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self { Rng(seed.max(1)) }
    pub fn next(&mut self) -> u64 { let mut x = self.0; x ^= x << 13; x ^= x >> 7; x ^= x << 17; self.0 = x; x }
    pub fn below(&mut self, n: u64) -> u64 { self.next() % n }
}

/// Tunable rules (lets tests/sim compare "classic" vs "combo-friendly").
#[derive(Clone, Copy, Debug)]
pub struct Rules { pub kinds: u8, pub specials: bool, pub refill_bias_pct: u64 }
impl Rules {
    pub const DEFAULT: Rules = Rules { kinds: KINDS, specials: true, refill_bias_pct: REFILL_BIAS_PCT };
    pub const CLASSIC: Rules = Rules { kinds: 6, specials: false, refill_bias_pct: 0 };
}

/// One cascade step, for the UI to animate.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub cleared: Vec<usize>,
    pub points: u32,
    pub combo: u32,
    /// Specials that fired this step: (cell, kind).
    pub effects: Vec<(usize, u8)>,
    /// Specials created this step: (cell, encoded value).
    pub spawned: Vec<(usize, u8)>,
    pub board_after: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub enum SwapResult { NotAdjacent, NoMatch, Ok { steps: usize, points: u32, reshuffled: bool } }

pub struct Game {
    pub board: Vec<u8>,
    pub score: u32,
    pub moves_left: u32,
    pub rng: Rng,
    pub rules: Rules,
    pub last_steps: Vec<Step>,
}

/// A straight run of 3+ same-colored cells.
#[derive(Debug, Clone)]
pub struct Run { pub cells: Vec<usize>, pub horizontal: bool }

pub fn find_runs(b: &[u8], kinds: u8) -> Vec<Run> {
    let ok = |v: u8| v != EMPTY && color(v) < kinds;
    let mut out = vec![];
    for r in 0..H {
        let mut c = 0;
        while c < W {
            let g = b[idx(r, c)]; let mut e = c + 1;
            while e < W && ok(g) && color(b[idx(r, e)]) == color(g) && ok(b[idx(r, e)]) { e += 1; }
            if ok(g) && e - c >= 3 { out.push(Run { cells: (c..e).map(|k| idx(r, k)).collect(), horizontal: true }); }
            c = e;
        }
    }
    for c in 0..W {
        let mut r = 0;
        while r < H {
            let g = b[idx(r, c)]; let mut e = r + 1;
            while e < H && ok(g) && color(b[idx(e, c)]) == color(g) && ok(b[idx(e, c)]) { e += 1; }
            if ok(g) && e - r >= 3 { out.push(Run { cells: (r..e).map(|k| idx(k, c)).collect(), horizontal: false }); }
            r = e;
        }
    }
    out
}

/// Sorted unique indices of all matched cells.
pub fn find_matches_k(b: &[u8], kinds: u8) -> Vec<usize> {
    let mut hit = vec![false; N];
    for run in find_runs(b, kinds) { for i in run.cells { hit[i] = true; } }
    (0..N).filter(|&i| hit[i]).collect()
}
pub fn find_matches(b: &[u8]) -> Vec<usize> { find_matches_k(b, 15) }

pub fn adjacent(a: usize, b: usize) -> bool {
    let (ar, ac, br, bc) = (a / W, a % W, b / W, b % W);
    (ar == br && ac.abs_diff(bc) == 1) || (ac == bc && ar.abs_diff(br) == 1)
}

/// All valid swaps (color-bomb swaps are always valid).
pub fn all_moves(b: &[u8]) -> Vec<(usize, usize)> {
    let mut t = b.to_vec(); let mut out = vec![];
    for i in 0..N {
        for j in [i + 1, i + W] {
            if j >= N || !adjacent(i, j) { continue; }
            if kind(b[i]) == SP_COLOR || kind(b[j]) == SP_COLOR { out.push((i, j)); continue; }
            t.swap(i, j);
            let ok = !find_runs(&t, 15).is_empty();
            t.swap(i, j);
            if ok { out.push((i, j)); }
        }
    }
    out
}
pub fn find_move(b: &[u8]) -> Option<(usize, usize)> { all_moves(b).into_iter().next() }
pub fn has_valid_move(b: &[u8]) -> bool { find_move(b).is_some() }

impl Game {
    pub fn new(seed: u64, moves: u32) -> Self { Self::with_rules(seed, moves, Rules::DEFAULT) }
    pub fn with_rules(seed: u64, moves: u32, rules: Rules) -> Self {
        let mut g = Game { board: vec![EMPTY; N], score: 0, moves_left: moves, rng: Rng::new(seed), rules, last_steps: vec![] };
        g.fill_fresh();
        g
    }
    pub fn from_board(board: Vec<u8>, seed: u64, moves: u32) -> Self {
        Game { board, score: 0, moves_left: moves, rng: Rng::new(seed), rules: Rules::DEFAULT, last_steps: vec![] }
    }
    fn gem(&mut self) -> u8 { self.rng.below(self.rules.kinds as u64) as u8 }

    /// Fill with no existing matches and at least one valid move.
    pub fn fill_fresh(&mut self) {
        loop {
            for r in 0..H { for c in 0..W {
                loop {
                    let g = self.gem();
                    let h = c >= 2 && self.board[idx(r, c-1)] == g && self.board[idx(r, c-2)] == g;
                    let v = r >= 2 && self.board[idx(r-1, c)] == g && self.board[idx(r-2, c)] == g;
                    if !h && !v { self.board[idx(r, c)] = g; break; }
                }
            }}
            if find_matches(&self.board).is_empty() && has_valid_move(&self.board) { return; }
        }
    }

    pub fn is_over(&self) -> bool { self.moves_left == 0 }

    pub fn try_swap(&mut self, a: usize, b: usize) -> SwapResult {
        self.last_steps.clear();
        if self.is_over() || a >= N || b >= N || !adjacent(a, b) { return SwapResult::NotAdjacent; }
        let (va, vb) = (self.board[a], self.board[b]);
        let mut combo = 0; let mut total = 0;
        if kind(va) == SP_COLOR || kind(vb) == SP_COLOR {
            // Color bomb: always valid; clears every gem of the partner's color.
            self.moves_left -= 1;
            let mut set = vec![false; N]; let mut fired = vec![false; N];
            let mut effects = vec![];
            for &(bomb, other) in &[(a, b), (b, a)] {
                if kind(self.board[bomb]) != SP_COLOR || fired[bomb] { continue; }
                fired[bomb] = true; set[bomb] = true; effects.push((bomb, SP_COLOR));
                if kind(self.board[other]) == SP_COLOR { set.iter_mut().for_each(|s| *s = true); }
                else { let col = color(self.board[other]); for i in 0..N { if color(self.board[i]) == col { set[i] = true; } } }
            }
            combo = 1;
            total += self.resolve(set, fired, effects, vec![], combo);
        } else {
            self.board.swap(a, b);
            if find_runs(&self.board, 15).is_empty() { self.board.swap(a, b); return SwapResult::NoMatch; }
            self.moves_left -= 1;
        }
        let swapped = [a, b];
        loop {
            let runs = find_runs(&self.board, 15);
            if runs.is_empty() { break; }
            combo += 1;
            let mut set = vec![false; N];
            for r in &runs { for &i in &r.cells { set[i] = true; } }
            let spawns = if self.rules.specials { self.plan_spawns(&runs, if combo == 1 { &swapped[..] } else { &[] }) } else { vec![] };
            total += self.resolve(set, vec![false; N], vec![], spawns, combo);
        }
        self.score += total;
        let mut reshuffled = false;
        if !has_valid_move(&self.board) { self.reshuffle(); reshuffled = true; }
        SwapResult::Ok { steps: combo as usize, points: total, reshuffled }
    }

    /// Decide which specials the matched runs create: (cell, encoded).
    fn plan_spawns(&self, runs: &[Run], prefer: &[usize]) -> Vec<(usize, u8)> {
        let mut used = vec![false; runs.len()];
        let mut out: Vec<(usize, u8)> = vec![];
        let pick = |cells: &[usize]| -> usize { *cells.iter().find(|c| prefer.contains(c)).unwrap_or(&cells[cells.len() / 2]) };
        let taken = |out: &Vec<(usize, u8)>, p: usize| out.iter().any(|&(q, _)| q == p);
        for (k, r) in runs.iter().enumerate() {
            if r.cells.len() >= 5 { let p = pick(&r.cells); if !taken(&out, p) { out.push((p, make(COLORLESS, SP_COLOR))); } used[k] = true; }
        }
        for i in 0..runs.len() { for j in 0..runs.len() {
            if used[i] || used[j] || !runs[i].horizontal || runs[j].horizontal { continue; }
            if let Some(&x) = runs[i].cells.iter().find(|c| runs[j].cells.contains(c)) {
                if !taken(&out, x) { out.push((x, make(color(self.board[x]), SP_AREA))); }
                used[i] = true; used[j] = true;
            }
        }}
        for (k, r) in runs.iter().enumerate() {
            if !used[k] && r.cells.len() == 4 {
                let p = pick(&r.cells);
                if !taken(&out, p) { out.push((p, make(color(self.board[p]), if r.horizontal { SP_LINE_H } else { SP_LINE_V }))); }
            }
        }
        out
    }

    /// Expand `set` by firing any specials inside it (chains), clear, place spawns,
    /// apply gravity/refill, record the step. Returns points.
    fn resolve(&mut self, mut set: Vec<bool>, mut fired: Vec<bool>, mut effects: Vec<(usize, u8)>, spawns: Vec<(usize, u8)>, combo: u32) -> u32 {
        let spawn_cells: Vec<usize> = spawns.iter().map(|s| s.0).collect();
        let mut queue: Vec<usize> = (0..N).filter(|&i| set[i]).collect();
        while let Some(i) = queue.pop() {
            let v = self.board[i]; let k = kind(v);
            if k == SP_NONE || fired[i] || spawn_cells.contains(&i) && k == SP_NONE { continue; }
            fired[i] = true; effects.push((i, k));
            let (r, c) = (i / W, i % W);
            let targets: Vec<usize> = match k {
                SP_LINE_H => (0..W).map(|x| idx(r, x)).collect(),
                SP_LINE_V => (0..H).map(|y| idx(y, c)).collect(),
                SP_AREA => { let mut t = vec![]; for y in r.saturating_sub(1)..=(r + 1).min(H - 1) { for x in c.saturating_sub(1)..=(c + 1).min(W - 1) { t.push(idx(y, x)); } } t }
                SP_COLOR => { // caught in a blast: clear the most common color
                    let mut cnt = [0usize; 16]; for &b in &self.board { if b != EMPTY && color(b) < 15 { cnt[color(b) as usize] += 1; } }
                    let col = (0..15).max_by_key(|&c| cnt[c]).unwrap() as u8;
                    (0..N).filter(|&j| color(self.board[j]) == col).collect()
                }
                _ => vec![],
            };
            for t in targets { if !set[t] { set[t] = true; queue.push(t); } }
        }
        for &(p, v) in &spawns { set[p] = false; self.board[p] = v; }
        let cleared: Vec<usize> = (0..N).filter(|&i| set[i]).collect();
        for &i in &cleared { self.board[i] = EMPTY; }
        let pts = cleared.len() as u32 * 10 * combo;
        self.gravity_refill();
        self.last_steps.push(Step { cleared, points: pts, combo, effects, spawned: spawns, board_after: self.board.clone() });
        pts
    }

    pub fn gravity_refill(&mut self) {
        for c in 0..W {
            let mut write = H as isize - 1;
            for r in (0..H).rev() {
                let v = self.board[idx(r, c)];
                if v != EMPTY { self.board[idx(write as usize, c)] = v; write -= 1; }
            }
            while write >= 0 {
                let w = write as usize;
                let below = if w + 1 < H { color(self.board[idx(w + 1, c)]) } else { COLORLESS };
                let g = if below < self.rules.kinds && self.rng.below(100) < self.rules.refill_bias_pct { below } else { self.gem() };
                self.board[idx(w, c)] = g; write -= 1;
            }
        }
    }

    pub fn reshuffle(&mut self) {
        for _ in 0..200 {
            for i in (1..N).rev() { let j = self.rng.below(i as u64 + 1) as usize; self.board.swap(i, j); }
            if find_matches(&self.board).is_empty() && has_valid_move(&self.board) { return; }
        }
        self.fill_fresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 5-color board with no matches and no valid moves.
    fn no_move_board() -> Vec<u8> { (0..N).map(|i| (((i / W) * 2 + (i % W) * 3 / 2 + (i % W) % 2) % 5) as u8).collect() }

    #[test] fn fixture_is_dead() { let b = no_move_board(); assert!(find_matches(&b).is_empty(), "{:?}", b); assert!(!has_valid_move(&b)); }
    #[test] fn fresh_board_valid_five_types() {
        let mut seen = [false; 8];
        for s in 1..50 { let g = Game::new(s, 30);
            assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
            for &x in &g.board { assert!(x < KINDS); seen[x as usize] = true; } }
        assert_eq!(seen.iter().filter(|&&x| x).count(), 5);
    }
    #[test] fn adjacency() { assert!(adjacent(0,1)); assert!(adjacent(0,8)); assert!(!adjacent(7,8)); assert!(!adjacent(0,9)); }
    #[test] fn invalid_swap_reverts() {
        let mut g = Game::from_board(no_move_board(), 7, 30); let before = g.board.clone();
        assert_eq!(g.try_swap(0, 1), SwapResult::NoMatch); assert_eq!(g.board, before); assert_eq!(g.moves_left, 30);
        assert_eq!(g.try_swap(0, 9), SwapResult::NotAdjacent);
    }
    #[test] fn valid_swap_scores_and_settles() {
        for s in 1..30 { let mut g = Game::new(s, 30); let (a,b) = find_move(&g.board).unwrap();
            assert!(matches!(g.try_swap(a,b), SwapResult::Ok{..})); assert_eq!(g.moves_left, 29);
            assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
            assert!(g.board.iter().all(|&x| x != EMPTY)); }
    }
    #[test] fn gravity_drops_down() {
        let mut g = Game::from_board(no_move_board(), 3, 30);
        let top = g.board[idx(6,2)]; g.board[idx(7,2)] = EMPTY;
        g.gravity_refill(); assert_eq!(g.board[idx(7,2)], top); assert!(g.board.iter().all(|&x| x != EMPTY));
    }

    /// Put `col` at cells, then swap a<->b.
    fn setup(cells: &[(usize, usize)], col: u8) -> Game {
        let mut b = no_move_board();
        for &(r, c) in cells { b[idx(r, c)] = col; }
        Game::from_board(b, 11, 30)
    }

    #[test] fn four_match_creates_line_bomb() {
        // row 7: X X _ X, X at (6,2); swap (6,2)<->(7,2) -> horizontal 4 (cols 0..=3)
        let x = 4; let mut g = setup(&[(7,0),(7,1),(7,3),(6,2)], x);
        g.board[idx(7,4)] = (x + 1) % 5; g.board[idx(7,2)] = (x + 2) % 5;
        assert!(matches!(g.try_swap(idx(6,2), idx(7,2)), SwapResult::Ok{..}));
        let s = &g.last_steps[0];
        assert!(s.spawned.contains(&(idx(7,2), make(x, SP_LINE_H))), "{:?}", s.spawned);
        assert_eq!(s.cleared.len(), 3);
    }
    #[test] fn five_match_creates_color_bomb() {
        let x = 0; let mut g = setup(&[(7,0),(7,1),(7,3),(7,4),(6,2)], x);
        g.board[idx(7,2)] = 3; g.board[idx(7,5)] = 2;
        assert!(matches!(g.try_swap(idx(6,2), idx(7,2)), SwapResult::Ok{..}));
        assert!(g.last_steps[0].spawned.contains(&(idx(7,2), make(COLORLESS, SP_COLOR))), "{:?}", g.last_steps[0].spawned);
    }
    #[test] fn l_shape_creates_area_bomb() {
        // horizontal (7,0..2) + vertical (5..7,2) sharing (7,2); trigger by swap at (7,3)->(7,2)? build directly
        let x = 1; let mut b = no_move_board();
        for &(r,c) in &[(7,0),(7,1),(7,2),(6,2),(5,2)] { b[idx(r,c)] = x; }
        let g = Game::from_board(b.clone(), 1, 30);
        let runs = find_runs(&b, 15);
        let sp = g.plan_spawns(&runs, &[]);
        assert!(sp.contains(&(idx(7,2), make(x, SP_AREA))), "{:?}", sp);
    }
    #[test] fn line_bomb_clears_row_and_col() {
        let mut g = Game::from_board(no_move_board(), 1, 30);
        g.board[idx(3,3)] = make(color(g.board[idx(3,3)]), SP_LINE_H);
        let mut set = vec![false; N]; set[idx(3,3)] = true;
        g.resolve(set, vec![false; N], vec![], vec![], 1);
        let s = &g.last_steps[0]; assert_eq!(s.cleared, (0..W).map(|c| idx(3,c)).collect::<Vec<_>>());
        let mut g = Game::from_board(no_move_board(), 1, 30);
        g.board[idx(3,3)] = make(color(g.board[idx(3,3)]), SP_LINE_V);
        let mut set = vec![false; N]; set[idx(3,3)] = true;
        g.resolve(set, vec![false; N], vec![], vec![], 1);
        assert_eq!(g.last_steps[0].cleared, (0..H).map(|r| idx(r,3)).collect::<Vec<_>>());
    }
    #[test] fn color_bomb_swap_clears_color() {
        let mut b = no_move_board(); b[idx(4,4)] = make(COLORLESS, SP_COLOR);
        let target = color(b[idx(4,5)]); let count = b.iter().filter(|&&v| v == target).count();
        let mut g = Game::from_board(b, 2, 30);
        assert!(has_valid_move(&g.board));
        assert!(matches!(g.try_swap(idx(4,4), idx(4,5)), SwapResult::Ok{..}));
        let s = &g.last_steps[0];
        assert_eq!(s.cleared.len(), count + 1); assert_eq!(s.effects[0], (idx(4,4), SP_COLOR));
        assert_eq!(g.moves_left, 29);
    }
    #[test] fn specials_chain() {
        // a row bomb whose row contains a column bomb -> row + column cleared
        let mut g = Game::from_board(no_move_board(), 1, 30);
        g.board[idx(2,1)] = make(color(g.board[idx(2,1)]), SP_LINE_H);
        g.board[idx(2,6)] = make(color(g.board[idx(2,6)]), SP_LINE_V);
        let mut set = vec![false; N]; set[idx(2,1)] = true;
        g.resolve(set, vec![false; N], vec![], vec![], 1);
        let s = &g.last_steps[0];
        assert_eq!(s.effects.len(), 2); assert_eq!(s.cleared.len(), W + H - 1);
        assert_eq!(s.points, (W + H - 1) as u32 * 10);
    }
    #[test] fn reshuffle_restores_moves() {
        let mut g = Game::from_board(no_move_board(), 5, 30); g.reshuffle();
        assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
    }
    #[test] fn game_over_blocks_moves() {
        let mut g = Game::new(9, 0); let (a,b) = find_move(&g.board).unwrap();
        assert_eq!(g.try_swap(a,b), SwapResult::NotAdjacent);
    }

    fn simulate(rules: Rules, moves: usize) -> (f64, f64, f64) {
        let mut rng = Rng::new(12345); let mut done = 0; let (mut depth, mut combos, mut pts) = (0usize, 0usize, 0u64);
        while done < moves {
            let mut g = Game::with_rules(rng.next(), 30, rules);
            while !g.is_over() && done < moves {
                let ms = all_moves(&g.board); let (a, b) = ms[rng.below(ms.len() as u64) as usize];
                if let SwapResult::Ok { steps, points, .. } = g.try_swap(a, b) { depth += steps; if steps >= 2 { combos += 1; } pts += points as u64; done += 1; }
            }
        }
        (depth as f64 / moves as f64, combos as f64 / moves as f64 * 100.0, pts as f64 / moves as f64)
    }
    #[test] fn combo_rate_stats() {
        let before = simulate(Rules::CLASSIC, 2000);
        let after = simulate(Rules::DEFAULT, 2000);
        println!("STATS classic(6 types, no specials, no bias): avg cascade depth {:.2}, combo rate {:.1}%, avg pts/move {:.0}", before.0, before.1, before.2);
        println!("STATS new(5 types, specials, bias {}%): avg cascade depth {:.2}, combo rate {:.1}%, avg pts/move {:.0}", REFILL_BIAS_PCT, after.0, after.1, after.2);
        assert!(after.1 > before.1);
    }
}
