//! The shared `layers.json` format.

use crate::geom::Layer;
use image::RgbImage;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub source: String,
    pub center: [f32; 2],
    pub background: String,
    pub out_size: [u32; 2],
    pub frames: u32,
    pub fps: u32,
    pub colors: usize,
    pub layers: Vec<Layer>,
}

pub fn parse_hex(s: &str) -> [u8; 3] {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

pub fn load(path: &str) -> (Config, RgbImage) {
    let txt =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let cfg: Config =
        serde_json::from_str(&txt).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"));
    let src = image::open(&cfg.source)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", cfg.source))
        .to_rgb8();
    (cfg, src)
}

/// Write new `turns`, `frames` and `fps` values into a layers.json *text* in place,
/// leaving geometry, comments and formatting untouched.
///
/// `turns` is indexed by the layer's position in the file, not by render order.
pub fn rewrite_turns(text: &str, turns: &[i32], frames: u32, fps: u32) -> String {
    let mut out = String::new();
    let mut li = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        let new = if trimmed.starts_with("\"frames\":") {
            format!("{indent}\"frames\": {frames},")
        } else if trimmed.starts_with("\"fps\":") {
            format!("{indent}\"fps\": {fps},")
        } else if let (true, Some(at)) = (li < turns.len(), line.rfind("\"turns\":")) {
            // Keep whatever followed the number (`}`, `},`) so the file stays valid.
            let tail = &line[at..];
            let rest = tail.find('}').map(|i| &tail[i..]).unwrap_or("");
            let s = format!("{}\"turns\": {:>2} {}", &line[..at], turns[li], rest);
            li += 1;
            s
        } else {
            line.to_string()
        };

        out.push_str(&new);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
  "source": "images/1.png",
  "frames": 150,
  "fps": 25,
  "layers": [
    { "name": "a", "outer": { "circle": 410 }, "inner": { "circle": 389 }, "turns":  0 },
    { "name": "b", "outer": { "circle": 389 }, "inner": { "circle": 353 }, "turns":  1 },
    { "name": "c", "outer": { "circle": 353 }, "inner": { "circle": 0   }, "turns": -3 }
  ]
}
"#;

    #[test]
    fn rewrite_updates_turns_frames_and_fps() {
        let out = rewrite_turns(SAMPLE, &[0, -2, 5], 240, 20);
        assert!(out.contains("\"frames\": 240,"));
        assert!(out.contains("\"fps\": 20,"));
        assert!(out.contains("\"turns\":  0 }"));
        assert!(out.contains("\"turns\": -2 }"));
        assert!(out.contains("\"turns\":  5 }"));
        // geometry must survive untouched
        assert!(out.contains("\"outer\": { \"circle\": 389 }"));
        assert!(out.contains("\"source\": \"images/1.png\""));
    }

    #[test]
    fn rewrite_output_is_still_valid_json() {
        let out = rewrite_turns(SAMPLE, &[7, -7, 0], 300, 30);
        let v: serde_json::Value = serde_json::from_str(&out).expect("rewrite produced invalid JSON");
        let layers = v["layers"].as_array().unwrap();
        assert_eq!(layers[0]["turns"], 7);
        assert_eq!(layers[1]["turns"], -7);
        assert_eq!(layers[2]["turns"], 0);
        assert_eq!(v["frames"], 300);
        assert_eq!(v["fps"], 30);
    }

    #[test]
    fn rewrite_preserves_trailing_comma_style() {
        let one = "    { \"name\": \"a\", \"turns\":  1 },\n";
        let out = rewrite_turns(one, &[-4], 1, 1);
        assert_eq!(out, "    { \"name\": \"a\", \"turns\": -4 },\n");
    }
}
