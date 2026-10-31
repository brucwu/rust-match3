//! Pure match-3 game logic (no browser deps).
pub const W: usize = 8;
pub const H: usize = 8;
pub const KINDS: u8 = 6;
pub const EMPTY: u8 = 255;

/// Small xorshift RNG so logic stays deterministic & dependency-free.
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self { Rng(seed.max(1)) }
    pub fn next(&mut self) -> u64 { let mut x = self.0; x ^= x << 13; x ^= x >> 7; x ^= x << 17; self.0 = x; x }
    pub fn gem(&mut self) -> u8 { (self.next() % KINDS as u64) as u8 }
}

/// One cascade step, for the UI to animate.
#[derive(Debug, Clone, PartialEq)]
pub struct Step { pub cleared: Vec<usize>, pub points: u32, pub combo: u32, pub board_after: Vec<u8> }

#[derive(Debug, PartialEq)]
pub enum SwapResult { NotAdjacent, NoMatch, Ok { steps: usize, points: u32, reshuffled: bool } }

pub struct Game {
    pub board: Vec<u8>,
    pub score: u32,
    pub moves_left: u32,
    pub rng: Rng,
    pub last_steps: Vec<Step>,
}

pub fn idx(r: usize, c: usize) -> usize { r * W + c }

impl Game {
    pub fn new(seed: u64, moves: u32) -> Self {
        let mut g = Game { board: vec![EMPTY; W * H], score: 0, moves_left: moves, rng: Rng::new(seed), last_steps: vec![] };
        g.fill_fresh();
        g
    }

    pub fn from_board(board: Vec<u8>, seed: u64, moves: u32) -> Self {
        Game { board, score: 0, moves_left: moves, rng: Rng::new(seed), last_steps: vec![] }
    }

    /// Fill a board with no existing matches and at least one valid move.
    pub fn fill_fresh(&mut self) {
        loop {
            for r in 0..H { for c in 0..W {
                loop {
                    let g = self.rng.gem();
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
        if self.is_over() || a >= W*H || b >= W*H || !adjacent(a, b) { return SwapResult::NotAdjacent; }
        self.board.swap(a, b);
        if find_matches(&self.board).is_empty() { self.board.swap(a, b); return SwapResult::NoMatch; }
        self.moves_left -= 1;
        let mut combo = 0; let mut total = 0;
        loop {
            let m = find_matches(&self.board);
            if m.is_empty() { break; }
            combo += 1;
            let pts = m.len() as u32 * 10 * combo;
            total += pts;
            for &i in &m { self.board[i] = EMPTY; }
            self.gravity_refill();
            self.last_steps.push(Step { cleared: m, points: pts, combo, board_after: self.board.clone() });
        }
        self.score += total;
        let mut reshuffled = false;
        if !has_valid_move(&self.board) { self.reshuffle(); reshuffled = true; }
        SwapResult::Ok { steps: combo as usize, points: total, reshuffled }
    }

    pub fn gravity_refill(&mut self) {
        for c in 0..W {
            let mut write = H as isize - 1;
            for r in (0..H).rev() {
                let v = self.board[idx(r, c)];
                if v != EMPTY { self.board[idx(write as usize, c)] = v; write -= 1; }
            }
            while write >= 0 { self.board[idx(write as usize, c)] = self.rng.gem(); write -= 1; }
        }
    }

    /// Shuffle existing gems until no matches & a move exists; fallback to fresh fill.
    pub fn reshuffle(&mut self) {
        for _ in 0..200 {
            for i in (1..self.board.len()).rev() { let j = (self.rng.next() % (i as u64 + 1)) as usize; self.board.swap(i, j); }
            if find_matches(&self.board).is_empty() && has_valid_move(&self.board) { return; }
        }
        self.fill_fresh();
    }
}

pub fn adjacent(a: usize, b: usize) -> bool {
    let (ar, ac, br, bc) = (a / W, a % W, b / W, b % W);
    (ar == br && ac.abs_diff(bc) == 1) || (ac == bc && ar.abs_diff(br) == 1)
}

/// Sorted unique indices of all cells in horizontal/vertical runs of 3+.
pub fn find_matches(b: &[u8]) -> Vec<usize> {
    let mut hit = vec![false; W * H];
    for r in 0..H {
        let mut c = 0;
        while c < W {
            let g = b[idx(r, c)]; let mut e = c + 1;
            while e < W && b[idx(r, e)] == g { e += 1; }
            if g != EMPTY && e - c >= 3 { for k in c..e { hit[idx(r, k)] = true; } }
            c = e;
        }
    }
    for c in 0..W {
        let mut r = 0;
        while r < H {
            let g = b[idx(r, c)]; let mut e = r + 1;
            while e < H && b[idx(e, c)] == g { e += 1; }
            if g != EMPTY && e - r >= 3 { for k in r..e { hit[idx(k, c)] = true; } }
            r = e;
        }
    }
    (0..W * H).filter(|&i| hit[i]).collect()
}

/// Returns a hint (a, b) swap that creates a match, if any.
pub fn find_move(b: &[u8]) -> Option<(usize, usize)> {
    let mut t = b.to_vec();
    for i in 0..W * H {
        for j in [i + 1, i + W] {
            if j >= W * H || !adjacent(i, j) { continue; }
            t.swap(i, j);
            let ok = !find_matches(&t).is_empty();
            t.swap(i, j);
            if ok { return Some((i, j)); }
        }
    }
    None
}
pub fn has_valid_move(b: &[u8]) -> bool { find_move(b).is_some() }

#[cfg(test)]
mod tests {
    use super::*;
    // Checkerboard-ish board with no matches and no moves: pattern of 4 kinds by (r%2,c%2)? use diag pattern.
    fn no_move_board() -> Vec<u8> { (0..W*H).map(|i| (((i / W) + 2 * (i % W)) % 6) as u8).collect() }

    #[test] fn fresh_board_valid() {
        for s in 1..50 { let g = Game::new(s, 30);
            assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
            assert!(g.board.iter().all(|&x| x < KINDS)); }
    }
    #[test] fn detects_horizontal_and_vertical() {
        let mut b = no_move_board();
        b[idx(0,0)] = 0; b[idx(0,1)] = 0; b[idx(0,2)] = 0;
        assert!(find_matches(&b).contains(&idx(0,1)));
        let mut b = no_move_board();
        for r in 2..6 { b[idx(r,4)] = 5; }
        assert_eq!(find_matches(&b).len(), 4);
    }
    #[test] fn no_move_pattern() { let b = no_move_board(); assert!(find_matches(&b).is_empty()); assert!(!has_valid_move(&b)); }
    #[test] fn adjacency() { assert!(adjacent(0,1)); assert!(adjacent(0,8)); assert!(!adjacent(7,8)); assert!(!adjacent(0,9)); }
    #[test] fn invalid_swap_reverts() {
        let mut g = Game::new(7, 30); let before = g.board.clone();
        // find a non-matching adjacent swap
        let (a,b) = (0..W*H-1).filter(|&i| adjacent(i,i+1)).map(|i|(i,i+1)).find(|&(a,b)|{let mut t=before.clone();t.swap(a,b);find_matches(&t).is_empty()}).unwrap();
        assert_eq!(g.try_swap(a,b), SwapResult::NoMatch); assert_eq!(g.board, before); assert_eq!(g.moves_left, 30);
        assert_eq!(g.try_swap(0, 9), SwapResult::NotAdjacent);
    }
    #[test] fn valid_swap_scores_and_settles() {
        for s in 1..30 { let mut g = Game::new(s, 30); let (a,b) = find_move(&g.board).unwrap();
            match g.try_swap(a,b) { SwapResult::Ok{points, steps, ..} => { assert!(points >= 30); assert!(steps >= 1); } r => panic!("{r:?}") }
            assert_eq!(g.moves_left, 29); assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
            assert!(g.board.iter().all(|&x| x < KINDS)); }
    }
    #[test] fn gravity_drops_down() {
        let mut g = Game::from_board(no_move_board(), 3, 30);
        let top = g.board[idx(6,2)]; g.board[idx(7,2)] = EMPTY;
        g.gravity_refill(); assert_eq!(g.board[idx(7,2)], top); assert!(g.board.iter().all(|&x| x < KINDS));
    }
    #[test] fn combo_multiplier() {
        // Swap creates a match whose clearing causes a cascade. Build manually.
        let mut b = no_move_board();
        // column 0 bottom: rows 5,6,7 -> make rows 6,7 = kind 5 and row 4 = kind 5 above a gem at row5; plus horizontal at row 5
        // Simpler: verify points formula on a single step.
        b[idx(7,0)] = 4; b[idx(7,1)] = 4; b[idx(6,2)] = 4; // swap (6,2)<->(7,2)
        let mut g = Game::from_board(b, 11, 30);
        if let SwapResult::Ok{steps, points, ..} = g.try_swap(idx(6,2), idx(7,2)) {
            let expected: u32 = g.last_steps.iter().map(|s| s.cleared.len() as u32 * 10 * s.combo).sum();
            assert_eq!(points, expected); assert_eq!(steps, g.last_steps.len());
            assert_eq!(g.last_steps[0].combo, 1);
        } else { panic!() }
    }
    #[test] fn reshuffle_restores_moves() {
        let mut g = Game::from_board(no_move_board(), 5, 30); g.reshuffle();
        assert!(find_matches(&g.board).is_empty()); assert!(has_valid_move(&g.board));
    }
    #[test] fn game_over_blocks_moves() {
        let mut g = Game::new(9, 0); let (a,b) = find_move(&g.board).unwrap();
        assert!(g.is_over()); assert_eq!(g.try_swap(a,b), SwapResult::NotAdjacent);
    }
}
