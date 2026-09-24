use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::geometric::shapes::{closed_path, polygon};

use serde::Deserialize;

/// `{ type: "polygon", sides: 7, r: 318, rotate: 0 }` — a regular polygon outline.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Polygon {
    pub sides: u32,
    /// Circumradius: centre to vertex.
    pub r: f32,
    /// Degrees clockwise of the vertex nearest North; 0 puts one at 12 o'clock.
    #[serde(default)]
    pub rotate: f32,
    pub width: Option<f32>,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

impl Draw for Polygon {
    fn draw(&self, pen: &mut Pen) {
        let pts = polygon(pen.center(), self.sides as usize, self.r, self.rotate);
        pen.line(closed_path(&pts), self.width);
    }

    fn check(&self) -> Result<(), String> {
        if self.sides < 3 {
            return Err(format!("a polygon needs at least 3 `sides`, got {}", self.sides));
        }
        positive("r", self.r)
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}
