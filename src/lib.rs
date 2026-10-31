//! wasm-bindgen bindings around the pure game logic.
pub mod game;
use game::{Game, SwapResult};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Match3 { g: Game }

#[wasm_bindgen]
impl Match3 {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: f64, moves: u32) -> Match3 { Match3 { g: Game::new(seed as u64, moves) } }
    pub fn width(&self) -> usize { game::W }
    pub fn height(&self) -> usize { game::H }
    pub fn board(&self) -> Vec<u8> { self.g.board.clone() }
    pub fn score(&self) -> u32 { self.g.score }
    pub fn moves_left(&self) -> u32 { self.g.moves_left }
    pub fn is_over(&self) -> bool { self.g.is_over() }
    /// 0 = not adjacent / over, 1 = no match (reverted), 2 = ok, 3 = ok + reshuffled
    pub fn swap(&mut self, a: usize, b: usize) -> u8 {
        match self.g.try_swap(a, b) { SwapResult::NotAdjacent => 0, SwapResult::NoMatch => 1,
            SwapResult::Ok { reshuffled, .. } => if reshuffled { 3 } else { 2 } }
    }
    pub fn step_count(&self) -> usize { self.g.last_steps.len() }
    pub fn step_cleared(&self, i: usize) -> Vec<u32> { self.g.last_steps[i].cleared.iter().map(|&x| x as u32).collect() }
    pub fn step_board(&self, i: usize) -> Vec<u8> { self.g.last_steps[i].board_after.clone() }
    pub fn step_combo(&self, i: usize) -> u32 { self.g.last_steps[i].combo }
    pub fn step_points(&self, i: usize) -> u32 { self.g.last_steps[i].points }
    /// Hint: returns [a, b] or empty.
    pub fn hint(&self) -> Vec<u32> { game::find_move(&self.g.board).map(|(a, b)| vec![a as u32, b as u32]).unwrap_or_default() }
}
