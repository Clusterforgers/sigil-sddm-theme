use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::geometric::shapes::pt;
use crate::geometric::{slot, Track};

use serde::Deserialize;
use tiny_skia::PathBuilder;

/// `{ type: "spokes", count: 40, from: { circle: 353.6 }, to: { circle: 384 } }` — evenly
/// spaced radial lines between two tracks, e.g. the dividers of a ring of cells.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spokes {
    pub count: usize,
    pub from: Track,
    pub to: Track,
    /// Degrees clockwise of the first spoke.
    #[serde(default)]
    pub rotate: f32,
    pub width: Option<f32>,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

impl Draw for Spokes {
    fn draw(&self, pen: &mut Pen) {
        let c = pen.center();
        let mut pb = PathBuilder::new();
        for i in 0..self.count {
            let a = slot(i, self.count, self.rotate);
            let (x0, y0) = pt(c, self.from.radius_at(a), a);
            let (x1, y1) = pt(c, self.to.radius_at(a), a);
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
        }
        pen.line(pb.finish(), self.width);
    }

    fn check(&self) -> Result<(), String> {
        positive("count", self.count as f32)?;
        self.from.check().map_err(|e| format!("from: {e}"))?;
        self.to.check().map_err(|e| format!("to: {e}"))
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}
