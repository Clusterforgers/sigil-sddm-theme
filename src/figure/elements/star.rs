use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::geometric::shapes::{closed_path, star, star_inside};

use serde::Deserialize;

/// `{ type: "star", points: 7, skip: 2, r: 314, ribbon: 28.2 }` — a star polygon: `points`
/// vertices on a circle of radius `r`, each joined to the one `skip` further round.
///
/// With `ribbon`, a second star is drawn that far inside the first, so the star reads as a
/// band with parallel sides rather than a line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Star {
    pub points: u32,
    pub skip: u32,
    pub r: f32,
    #[serde(default)]
    pub rotate: f32,
    pub ribbon: Option<f32>,
    pub width: Option<f32>,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

impl Draw for Star {
    fn draw(&self, pen: &mut Pen) {
        let (n, skip) = (self.points as usize, self.skip as usize);
        let inner = self.ribbon.map(|w| star_inside(self.r, skip, n, w));
        for r in std::iter::once(self.r).chain(inner) {
            let pts = star(pen.center(), n, r, self.rotate, skip);
            pen.line(closed_path(&pts), self.width);
        }
    }

    fn check(&self) -> Result<(), String> {
        positive("r", self.r)?;
        let (n, k) = (self.points, self.skip);
        if n < 5 || k < 2 || 2 * k >= n || gcd(n, k) != 1 {
            return Err(format!(
                "no single star has {n} points joined every {k}: `skip` must be at least 2, \
                 less than half of `points`, and share no factor with it"
            ));
        }
        if let Some(w) = self.ribbon {
            positive("ribbon", w)?;
            if star_inside(self.r, k as usize, n as usize, w) <= 0.0 {
                return Err(format!("a ribbon {w} wide does not fit inside a star of radius {}", self.r));
            }
        }
        Ok(())
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}
