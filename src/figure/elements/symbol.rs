use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::figure::symbols::Symbol;

use serde::Deserialize;

/// `{ type: "symbol", symbol: "latin-cross", at: [0, 0], size: 26 }` — one symbol, upright,
/// centred `at` from the figure's centre.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolMark {
    pub symbol: Symbol,
    #[serde(default)]
    pub at: [f32; 2],
    pub size: f32,
    /// Degrees clockwise.
    #[serde(default)]
    pub rotate: f32,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

impl Draw for SymbolMark {
    fn draw(&self, pen: &mut Pen) {
        let [cx, cy] = pen.center();
        let at = (cx + self.at[0], cy + self.at[1]);
        self.symbol.draw(pen, at, self.size, self.rotate.to_radians());
    }

    fn check(&self) -> Result<(), String> {
        positive("size", self.size)
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}
