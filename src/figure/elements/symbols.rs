use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::figure::symbols::Symbol;
use crate::geometric::shapes::{lens_fit, pt};
use crate::geometric::{slot, Track};

use serde::Deserialize;

/// A ring of evenly spaced symbols, each upright to the centre.
///
/// ```json5
/// { type: "symbols", symbol: "cross", count: 7, r: 236.9, size: 17 }
/// ```
///
/// Instead of `r`, `fit` hangs each one from a circle and shrinks it to the space above a
/// track, for symbols tucked into the lens between a circle and a polygon inside it:
///
/// ```json5
/// { type: "symbols", symbol: "cross", count: 7, rotate: 6.2, size: 17,
///   fit: { roof: 352, floor: { polygon: { sides: 7, r: 352.5 } } } }
/// ```
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbols {
    pub symbol: Symbol,
    pub count: usize,
    pub r: Option<f32>,
    pub fit: Option<Fit>,
    /// Height; with `fit`, the largest a symbol may be.
    pub size: f32,
    /// Degrees clockwise of the first.
    #[serde(default)]
    pub rotate: f32,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub roof: f32,
    pub floor: Track,
}

impl Draw for Symbols {
    fn draw(&self, pen: &mut Pen) {
        let c = pen.center();
        for i in 0..self.count {
            let a = slot(i, self.count, self.rotate);
            let (at, size) = match (&self.fit, self.r) {
                (Some(f), _) => {
                    let (dx, dy, size) = lens_fit(&f.floor, f.roof, self.size, a);
                    ((c[0] + dx, c[1] + dy), size)
                }
                (None, Some(r)) => (pt(c, r, a), self.size),
                (None, None) => return,
            };
            self.symbol.draw(pen, at, size, -a);
        }
    }

    fn check(&self) -> Result<(), String> {
        positive("size", self.size)?;
        positive("count", self.count as f32)?;
        match (&self.fit, self.r) {
            (Some(_), Some(_)) | (None, None) => Err("give exactly one of `r` and `fit`".into()),
            (None, Some(r)) => positive("r", r),
            (Some(f), None) => f.floor.check().map_err(|e| format!("fit.floor: {e}")),
        }
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}
