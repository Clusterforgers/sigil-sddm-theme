//! The order the figure draws itself in when it builds up: every mark gets a moment, on a
//! timeline from 0 (nothing yet) to 1 (all there).
//!
//! Lines are traced along their own paths, layer by layer from the core outward, as if
//! drawn with a pen. Letters and symbols follow, appearing one by one in a sweep clockwise
//! from North and drifting outward, starting while the outer lines are still being drawn.

use super::pen::{Mark, Style};

/// When a mark appears.
#[derive(Debug, Clone, Copy)]
pub enum Reveal {
    /// Drawn along its path, from `start` to `start + span`.
    Along { start: f32, span: f32 },
    /// Appears whole at `start`.
    At(f32),
}

/// Share of the timeline the lines are drawn over; the letters fill the rest.
const LINES_END: f32 = 0.62;
/// Where the letters begin: before the lines finish, so the two overlap.
const LETTERS_START: f32 = 0.38;

/// The moment of every mark of every layer. `extents` are the layers' ink radii, used to
/// work from the core outward whatever order the layers are stacked in.
pub fn choreograph(layers: &[Vec<Mark>], extents: &[[f32; 2]], center: [f32; 2], disc: f32) -> Vec<Vec<Reveal>> {
    // Rank the layers by how far out they start, so the core is drawn first.
    let mut order: Vec<usize> = (0..layers.len()).collect();
    order.sort_by(|&a, &b| extents[a][0].total_cmp(&extents[b][0]));
    let rank = |i: usize| order.iter().position(|&o| o == i).unwrap_or(0) as f32 / layers.len().max(1) as f32;

    layers
        .iter()
        .enumerate()
        .map(|(i, marks)| {
            // Each layer starts a little after the one inside it; its lines are staggered
            // so they do not all set off together.
            let layer_start = rank(i) * LINES_END * 0.55;
            let mut strokes = 0;
            marks
                .iter()
                .map(|m| match m.style {
                    Style::Stroke(_) => {
                        let start = layer_start + strokes as f32 * 0.03;
                        strokes += 1;
                        // Longer lines take longer, within reason.
                        let span = (length(&m.path) / 2600.0).clamp(0.08, 0.26);
                        Reveal::Along { start: start.min(LINES_END - span), span }
                    }
                    Style::Fill => {
                        let b = m.path.bounds();
                        let (x, y) = ((b.left() + b.right()) * 0.5 - center[0], (b.top() + b.bottom()) * 0.5 - center[1]);
                        let turn = x.atan2(-y).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
                        let out = (x.hypot(y) / disc.max(1.0)).min(1.0);
                        Reveal::At(LETTERS_START + (1.0 - LETTERS_START - 0.04) * (0.75 * turn + 0.25 * out))
                    }
                })
                .collect()
        })
        .collect()
}

fn length(path: &tiny_skia::Path) -> f32 {
    super::render::pieces(path).iter().map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum()
}
