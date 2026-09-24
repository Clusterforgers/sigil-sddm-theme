use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::figure::symbols::Symbol;
use crate::geometric::shapes::{lens_fit, pt};
use crate::geometric::{clockwise, slot, Track};

use serde::Deserialize;

/// A ring of evenly spaced symbols, each upright to the centre.
///
/// Where they sit is one of:
///
/// ```json5
/// { type: "symbols", symbol: "cross-potent", count: 7, size: 24, r: 236.9 }       // on a circle
/// { type: "symbols", symbol: "cross-potent", count: 7, size: 13,                  // along a track
///   along: { polygon: { sides: 7, r: 153.5, rotate: 25.71 } }, offset: 15 }
/// { type: "symbols", symbol: "cross-potent", count: 7, size: 17, rotate: 6.2,     // fitted in a lens
///   fit: { roof: 388, floor: { polygon: { sides: 7, r: 388.5 } } } }
/// ```
///
/// With `run: { count: 6, step: 5.4 }`, each of the `count` places gets a short run of
/// symbols `step` degrees apart, centred on it — the rows of crosses round a corner.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbols {
    pub symbol: Symbol,
    pub count: usize,
    pub r: Option<f32>,
    pub along: Option<Track>,
    pub fit: Option<Fit>,
    /// With `along`: lift the symbols' centres this far off the track, square to it.
    #[serde(default)]
    pub offset: f32,
    /// Height; with `fit`, the largest a symbol may be.
    pub size: f32,
    /// Degrees clockwise of the first.
    #[serde(default)]
    pub rotate: f32,
    pub run: Option<Run>,
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub count: usize,
    /// Degrees between neighbours in the run.
    pub step: f32,
}

impl Symbols {
    /// Internal angles of every symbol: `count` places, each a run centred on its place.
    fn angles(&self) -> impl Iterator<Item = f32> + '_ {
        let run = self.run.unwrap_or(Run { count: 1, step: 0.0 });
        (0..self.count).flat_map(move |i| {
            let middle = slot(i, self.count, self.rotate);
            (0..run.count).map(move |j| {
                let from_middle = (j as f32 - (run.count as f32 - 1.0) * 0.5) * run.step;
                middle + clockwise(from_middle)
            })
        })
    }
}

impl Draw for Symbols {
    fn draw(&self, pen: &mut Pen) {
        let c = pen.center();
        let track = self.along.map(|t| t.offset(self.offset));
        for a in self.angles() {
            let (at, size) = match (&self.fit, self.r, &track) {
                (Some(f), _, _) => {
                    let (dx, dy, size) = lens_fit(&f.floor, f.roof, self.size, a);
                    ((c[0] + dx, c[1] + dy), size)
                }
                (None, Some(r), _) => (pt(c, r, a), self.size),
                (None, None, Some(t)) => (pt(c, t.radius_at(a), a), self.size),
                (None, None, None) => return,
            };
            self.symbol.draw(pen, at, size, -a);
        }
    }

    fn check(&self) -> Result<(), String> {
        positive("size", self.size)?;
        positive("count", self.count as f32)?;
        if let Some(run) = self.run {
            positive("run.count", run.count as f32)?;
        }
        match (&self.fit, self.r, &self.along) {
            (None, Some(r), None) => positive("r", r),
            (None, None, Some(t)) => t.check().map_err(|e| format!("along: {e}")),
            (Some(f), None, None) => f.floor.check().map_err(|e| format!("fit.floor: {e}")),
            _ => Err("give exactly one of `r`, `along` and `fit`".into()),
        }
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_is_centred_on_its_place() {
        let s: Symbols = json5::from_str(
            r#"{ symbol: "cross", count: 2, r: 10, size: 1, rotate: 30, run: { count: 3, step: 4 } }"#,
        )
        .unwrap();
        let degrees: Vec<f32> = s.angles().map(|a| (-a.to_degrees()).rem_euclid(360.0)).collect();
        let want = [26.0, 30.0, 34.0, 206.0, 210.0, 214.0];
        for (got, want) in degrees.iter().zip(want) {
            assert!((got - want).abs() < 1e-3, "{degrees:?}");
        }
    }
}
