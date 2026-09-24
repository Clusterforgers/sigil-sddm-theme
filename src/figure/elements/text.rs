use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::figure::symbols::Symbol;
use crate::geometric::shapes::pt;
use crate::geometric::{slot, Track};
use crate::text::Orient;

use serde::Deserialize;

/// Clear space between a word and the first flanking symbol, in canvas units along the track.
const FLANK_GAP: f32 = 3.0;

/// A ring of words laid along a track, one per evenly spaced slot.
///
/// ```json5
/// { type: "text", along: { circle: 262 }, size: 15, rotate: 25.71,
///   items: ["SAAIEMES", "BTZKASE", ""] }
/// ```
///
/// Items run clockwise from `rotate`. An item may be:
/// * `"word"` — centred on the slot, baseline on the track;
/// * `""` — an empty slot;
/// * `["outer", "inner"]` — lines stacked `line_gap` apart, centred on the track;
/// * `"{cross}"` — a symbol instead of text, standing on the baseline.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub items: Vec<Item>,
    /// The track the baselines follow.
    pub along: Track,
    pub size: f32,
    /// Degrees clockwise of the first slot.
    #[serde(default)]
    pub rotate: f32,
    /// Lift the baseline this far off the track, square to it.
    #[serde(default)]
    pub offset: f32,
    /// Distance between stacked lines; defaults to `size`.
    pub line_gap: Option<f32>,
    /// Symbols set either side of every word.
    pub flank: Option<Flank>,
    #[serde(default)]
    pub orient: Orient,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Item {
    Line(String),
    Stack(Vec<String>),
}

/// `flank: { count: 3, size: 10 }` — a short run of symbols hugging each side of a word.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flank {
    pub count: usize,
    pub size: f32,
    #[serde(default = "default_flank_symbol")]
    pub symbol: Symbol,
}

fn default_flank_symbol() -> Symbol {
    Symbol::Cross
}

impl Text {
    /// Each line of item `k` with its baseline radius: a stack is centred on the track,
    /// first line outermost.
    fn lines<'a>(&'a self, item: &'a Item, r: f32) -> Vec<(&'a str, f32)> {
        match item {
            Item::Line(s) => vec![(s.as_str(), r)],
            Item::Stack(lines) => {
                let gap = self.line_gap.unwrap_or(self.size);
                let mid = (lines.len() as f32 - 1.0) * 0.5;
                lines.iter().enumerate().map(|(j, s)| (s.as_str(), r + gap * (mid - j as f32))).collect()
            }
        }
    }

    /// A run of `flank` symbols either side of `word`, centred on `a`.
    fn draw_flank(&self, pen: &mut Pen, track: &Track, a: f32, word: &str) {
        let Some(f) = &self.flank else { return };
        let r = track.radius_at(a);
        let half = pen.font().width(word, self.size) * 0.5 / r;
        for side in [-1.0f32, 1.0] {
            for i in 0..f.count {
                let along = FLANK_GAP + f.size * (1.02 * i as f32 + 0.5);
                let b = a + side * (half + along / r);
                f.symbol.draw(pen, pt(pen.center(), track.radius_at(b), b), f.size, -b);
            }
        }
    }
}

impl Draw for Text {
    fn draw(&self, pen: &mut Pen) {
        let track = self.along.offset(self.offset);
        let n = self.items.len();
        for (k, item) in self.items.iter().enumerate() {
            let a = slot(k, n, self.rotate);
            for (s, r) in self.lines(item, track.radius_at(a)) {
                match Symbol::from_token(s) {
                    // A symbol stands on the baseline as tall as a capital, so it reads as
                    // one more letter in the run rather than an ornament dropped on it.
                    Some(Ok(sym)) => {
                        let size = pen.font().cap_height(self.size);
                        sym.draw(pen, pt(pen.center(), r + size * 0.5, a), size, -a);
                    }
                    _ => pen.text(a, r, self.size, s, self.orient),
                }
            }
            if let Item::Line(word) = item {
                if !word.is_empty() && Symbol::from_token(word).is_none() {
                    self.draw_flank(pen, &track, a, word);
                }
            }
        }
    }

    fn check(&self) -> Result<(), String> {
        positive("size", self.size)?;
        self.along.check().map_err(|e| format!("along: {e}"))?;
        if self.items.is_empty() {
            return Err("`items` is empty".into());
        }
        for item in &self.items {
            let lines = match item {
                Item::Line(s) => std::slice::from_ref(s),
                Item::Stack(v) => v.as_slice(),
            };
            for s in lines {
                if let Some(Err(e)) = Symbol::from_token(s) {
                    return Err(format!("item {s:?}: {e}"));
                }
            }
        }
        if let Some(f) = &self.flank {
            positive("flank.size", f.size)?;
        }
        Ok(())
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stack_is_centred_on_the_track_first_line_outermost() {
        let t: Text = json5::from_str(r#"{ items: [], along: { circle: 100 }, size: 12, line_gap: 13.5 }"#).unwrap();
        let item = Item::Stack(vec!["a".into(), "b".into()]);
        let lines = t.lines(&item, 363.75);
        assert_eq!(lines, [("a", 370.5), ("b", 357.0)]);
    }
}
