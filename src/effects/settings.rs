//! How often each ambient effect happens, as written in the figure file:
//!
//! ```json5
//! effects: {
//!   implosions: { every: [12, 22] },     // seconds between, drawn fresh each time
//!   glyphs:     { every: 2 },            // a single number means exactly that
//!   lightning:  { every: [0.08, 0.35], lulls: { chance: 0.22, last: [0.7, 1.6] } },
//!   surge:      { charge: 1.8, break: 1.4, stay_broken: true },
//! }
//! ```
//!
//! Everything is optional: whatever is left out keeps its default.

use crate::figure::Rgb;

use rand::rngs::SmallRng;
use rand::RngExt;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

/// A wait in seconds: exactly `min` if the two are equal, otherwise anywhere between.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seconds {
    pub min: f32,
    pub max: f32,
}

impl Seconds {
    const fn between(min: f32, max: f32) -> Self {
        Seconds { min, max }
    }

    /// One wait, drawn fresh.
    pub fn draw(self, rng: &mut SmallRng) -> f32 {
        if self.max > self.min { rng.random_range(self.min..self.max) } else { self.min }
    }

    fn check(self, what: &str) -> Result<(), String> {
        // A wait of nothing would fire every frame and flood the effect's pool.
        if self.min < 0.01 {
            return Err(format!("`{what}` must be at least 0.01 seconds, got {}", self.min));
        }
        if self.max < self.min {
            return Err(format!("`{what}` is [{}, {}]: the first must not be larger", self.min, self.max));
        }
        Ok(())
    }
}

/// When an effect happens on its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    pub every: Seconds,
    pub enabled: bool,
}

/// Now and then lightning pauses for longer, so the next burst stands out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lulls {
    /// Chance, 0 to 1, that the wait after a strike is a lull.
    pub chance: f32,
    pub last: Seconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(from = "SettingsIn")]
pub struct Settings {
    /// A ring closing in from the rim, landing, and throwing lightning out.
    pub implosions: Schedule,
    /// Lightning walking the figure.
    pub lightning: Schedule,
    pub lulls: Lulls,
    /// A cluster of letters lighting up and lifting off.
    pub glyphs: Schedule,
    /// What Enter sets off.
    pub surge: SurgeTiming,
    /// A band of light gliding round the gilt. Always on.
    pub shimmer: Shimmer,
    /// The ink wavering as if seen through heat, in canvas units. Always on; 0 is still.
    pub haze: f32,
    /// A wave of colour rolling out from the centre, recolouring the gold as it passes.
    pub colors: Schedule,
    pub palette: Palette,
    /// A layer burning away to embers and growing back.
    pub dissolve: Schedule,
    /// How long one dissolve takes, gone and back, in seconds.
    pub dissolve_time: f32,
    /// Which way the colour waves travel.
    pub color_direction: Direction,
    /// A run of letters flickering through other letters and settling back.
    pub scramble: Schedule,
    pub scramble_letters: u32,
    pub scramble_time: f32,
    /// The gold slowly swelling and dimming. Always on.
    pub breath: Breath,
    /// The figure drawing itself in.
    pub build: Build,
    /// Fading copies trailing a fast-spinning layer; 0 turns them off.
    pub echoes: f32,
    /// Glowing threads linking a few letters across the rings.
    pub constellations: Schedule,
    pub constellation_stars: u32,
    /// Ripples following the pointer, and letters near it glowing; 0 turns it off.
    pub hover: f32,
    /// Typing lights one more letter per character.
    pub typing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Build {
    /// Seconds from nothing to the whole figure.
    pub duration: f32,
    /// Draw it in when the viewer starts.
    pub on_start: bool,
    /// Draw it back in when it reforms after the surge, instead of fading it in.
    pub after_surge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// From the centre to the rim.
    Out,
    /// From the rim to the centre.
    In,
    /// Either, picked afresh for each wave.
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breath {
    /// How far the brightness swings either way, as a fraction; 0 turns it off.
    pub strength: f32,
    /// Seconds for one breath, in and out.
    pub period: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shimmer {
    /// How bright the band is; 0 turns it off.
    pub strength: f32,
    /// Revolutions per second.
    pub speed: f32,
}

/// Up to `Palette::MAX` colours, kept in a fixed array so `Settings` stays `Copy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    colors: [Rgb; Palette::MAX],
    len: usize,
}

impl Palette {
    pub const MAX: usize = 8;

    fn new(colors: &[Rgb]) -> Self {
        let mut p = Palette { colors: [Rgb([255; 3]); Palette::MAX], len: colors.len().min(Palette::MAX) };
        p.colors[..p.len].copy_from_slice(&colors[..p.len]);
        p
    }

    pub fn colors(&self) -> &[Rgb] {
        &self.colors[..self.len]
    }
}

/// The sequence Enter starts: the figure spins up and gathers light, explodes, and flies
/// apart. Durations in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurgeTiming {
    /// Spinning up and gathering, from the keypress to the explosion.
    pub charge: f32,
    /// The layers flying apart and fading, after the explosion.
    pub scatter: f32,
    /// How long it stays broken before coming back together.
    pub hold: f32,
    /// Coming back together.
    pub reform: f32,
    /// Never come back — for a login screen, where what follows is the session.
    pub stay_broken: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let on = |min, max| Schedule { every: Seconds::between(min, max), enabled: true };
        Settings {
            implosions: on(12.0, 22.0),
            // Mostly a quick rattle of strikes...
            lightning: on(0.08, 0.35),
            // ...with the occasional lull so the bursts stand out against something.
            lulls: Lulls { chance: 0.22, last: Seconds::between(0.7, 1.6) },
            // A group is already several glyphs, so leave a gap before the next one. Back to
            // back they overlap into a permanent wash: the pause is what makes each an event.
            glyphs: on(1.2, 2.8),
            surge: SurgeTiming { charge: 1.8, scatter: 1.4, hold: 1.2, reform: 1.6, stay_broken: false },
            shimmer: Shimmer { strength: 0.6, speed: 0.05 },
            haze: 0.8,
            colors: on(6.0, 11.0),
            // Ember, violet, ice and white: each reads against gold, and against the others.
            palette: Palette::new(&[
                Rgb([0xFF, 0x4A, 0x1C]),
                Rgb([0x9B, 0x5C, 0xFF]),
                Rgb([0x3F, 0xD0, 0xFF]),
                Rgb([0xFF, 0xF4, 0xE0]),
            ]),
            dissolve: on(8.0, 14.0),
            dissolve_time: 3.2,
            color_direction: Direction::Both,
            scramble: on(3.0, 6.0),
            scramble_letters: 5,
            scramble_time: 1.2,
            breath: Breath { strength: 0.22, period: 6.0 },
            build: Build { duration: 6.0, on_start: true, after_surge: true },
            echoes: 1.0,
            constellations: on(5.0, 9.0),
            constellation_stars: 5,
            hover: 0.8,
            typing: true,
        }
    }
}

impl Settings {
    /// Reject timings that parse but make no sense, naming the field.
    pub fn check(&self) -> Result<(), String> {
        for (name, s) in [
            ("implosions", self.implosions),
            ("lightning", self.lightning),
            ("glyphs", self.glyphs),
            ("colors", self.colors),
            ("dissolve", self.dissolve),
            ("scramble", self.scramble),
            ("constellations", self.constellations),
        ] {
            s.every.check(&format!("{name}.every"))?;
        }
        self.lulls.last.check("lightning.lulls.last")?;
        if !(0.0..=1.0).contains(&self.lulls.chance) {
            return Err(format!("`lightning.lulls.chance` must be between 0 and 1, got {}", self.lulls.chance));
        }
        let g = self.surge;
        for (name, secs) in [("charge", g.charge), ("break", g.scatter), ("hold", g.hold), ("reform", g.reform)] {
            if secs < 0.05 {
                return Err(format!("`surge.{name}` must be at least 0.05 seconds, got {secs}"));
            }
        }
        if self.shimmer.strength < 0.0 || self.haze < 0.0 {
            return Err("`shimmer.strength` and `haze.strength` must not be negative".into());
        }
        if self.palette.len == 0 {
            return Err("`colors.palette` needs at least one colour".into());
        }
        if self.dissolve_time < 0.5 {
            return Err(format!("`dissolve.duration` must be at least 0.5 seconds, got {}", self.dissolve_time));
        }
        if self.scramble_time < 0.2 {
            return Err(format!("`scramble.duration` must be at least 0.2 seconds, got {}", self.scramble_time));
        }
        if self.scramble_letters == 0 {
            return Err("`scramble.letters` must be at least 1".into());
        }
        if !(0.0..=1.0).contains(&self.breath.strength) || self.breath.period < 0.5 {
            return Err("`breath.strength` must be between 0 and 1, and `breath.period` at least 0.5 seconds".into());
        }
        if self.build.duration < 0.5 {
            return Err(format!("`build.duration` must be at least 0.5 seconds, got {}", self.build.duration));
        }
        if self.echoes < 0.0 || self.hover < 0.0 {
            return Err("`echoes.strength` and `hover.strength` must not be negative".into());
        }
        if !(2..=8).contains(&self.constellation_stars) {
            return Err(format!("`constellations.stars` must be between 2 and 8, got {}", self.constellation_stars));
        }
        Ok(())
    }
}

// What the file may say: every field optional, filled from the defaults above. Kept apart
// from `Settings` so the rest of the code never deals in `Option`s.

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SettingsIn {
    implosions: ScheduleIn,
    lightning: LightningIn,
    glyphs: ScheduleIn,
    surge: SurgeIn,
    shimmer: ShimmerIn,
    haze: HazeIn,
    colors: ColorsIn,
    dissolve: DissolveIn,
    scramble: ScrambleIn,
    breath: BreathIn,
    build: BuildIn,
    echoes: StrengthIn,
    constellations: ConstellationsIn,
    hover: StrengthIn,
    typing: EnabledIn,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StrengthIn {
    strength: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct EnabledIn {
    enabled: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ConstellationsIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
    stars: Option<u32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct BuildIn {
    duration: Option<f32>,
    on_start: Option<bool>,
    after_surge: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ScrambleIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
    letters: Option<u32>,
    duration: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct BreathIn {
    strength: Option<f32>,
    period: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ShimmerIn {
    strength: Option<f32>,
    speed: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct HazeIn {
    strength: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ColorsIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
    palette: Option<Vec<Rgb>>,
    direction: Option<Direction>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct DissolveIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
    duration: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SurgeIn {
    charge: Option<f32>,
    #[serde(rename = "break")]
    scatter: Option<f32>,
    hold: Option<f32>,
    reform: Option<f32>,
    stay_broken: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ScheduleIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LightningIn {
    every: Option<Seconds>,
    enabled: Option<bool>,
    lulls: LullsIn,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LullsIn {
    chance: Option<f32>,
    last: Option<Seconds>,
}

impl ScheduleIn {
    fn over(self, d: Schedule) -> Schedule {
        Schedule { every: self.every.unwrap_or(d.every), enabled: self.enabled.unwrap_or(d.enabled) }
    }
}

impl From<SettingsIn> for Settings {
    fn from(s: SettingsIn) -> Self {
        let d = Settings::default();
        let lightning = ScheduleIn { every: s.lightning.every, enabled: s.lightning.enabled };
        Settings {
            implosions: s.implosions.over(d.implosions),
            lightning: lightning.over(d.lightning),
            lulls: Lulls {
                chance: s.lightning.lulls.chance.unwrap_or(d.lulls.chance),
                last: s.lightning.lulls.last.unwrap_or(d.lulls.last),
            },
            glyphs: s.glyphs.over(d.glyphs),
            surge: SurgeTiming {
                charge: s.surge.charge.unwrap_or(d.surge.charge),
                scatter: s.surge.scatter.unwrap_or(d.surge.scatter),
                hold: s.surge.hold.unwrap_or(d.surge.hold),
                reform: s.surge.reform.unwrap_or(d.surge.reform),
                stay_broken: s.surge.stay_broken.unwrap_or(d.surge.stay_broken),
            },
            shimmer: Shimmer {
                strength: s.shimmer.strength.unwrap_or(d.shimmer.strength),
                speed: s.shimmer.speed.unwrap_or(d.shimmer.speed),
            },
            haze: s.haze.strength.unwrap_or(d.haze),
            colors: ScheduleIn { every: s.colors.every, enabled: s.colors.enabled }.over(d.colors),
            // An empty palette is kept empty, for `check` to reject, rather than quietly
            // falling back to the default.
            palette: s.colors.palette.map_or(d.palette, |p| Palette::new(&p)),
            dissolve: ScheduleIn { every: s.dissolve.every, enabled: s.dissolve.enabled }.over(d.dissolve),
            dissolve_time: s.dissolve.duration.unwrap_or(d.dissolve_time),
            color_direction: s.colors.direction.unwrap_or(d.color_direction),
            scramble: ScheduleIn { every: s.scramble.every, enabled: s.scramble.enabled }.over(d.scramble),
            scramble_letters: s.scramble.letters.unwrap_or(d.scramble_letters),
            scramble_time: s.scramble.duration.unwrap_or(d.scramble_time),
            breath: Breath {
                strength: s.breath.strength.unwrap_or(d.breath.strength),
                period: s.breath.period.unwrap_or(d.breath.period),
            },
            build: Build {
                duration: s.build.duration.unwrap_or(d.build.duration),
                on_start: s.build.on_start.unwrap_or(d.build.on_start),
                after_surge: s.build.after_surge.unwrap_or(d.build.after_surge),
            },
            echoes: s.echoes.strength.unwrap_or(d.echoes),
            constellations: ScheduleIn { every: s.constellations.every, enabled: s.constellations.enabled }
                .over(d.constellations),
            constellation_stars: s.constellations.stars.unwrap_or(d.constellation_stars),
            hover: s.hover.strength.unwrap_or(d.hover),
            typing: s.typing.enabled.unwrap_or(d.typing),
        }
    }
}

/// `3` or `[2.6, 4.8]`.
impl<'de> Deserialize<'de> for Seconds {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Seconds;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "seconds, as a number or as [min, max]")
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Seconds, E> {
                Ok(Seconds::between(v as f32, v as f32))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Seconds, E> {
                self.visit_f64(v as f64)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Seconds, E> {
                self.visit_f64(v as f64)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Seconds, A::Error> {
                let min: f32 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let max: f32 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                if seq.next_element::<f32>()?.is_some() {
                    return Err(de::Error::invalid_length(3, &self));
                }
                Ok(Seconds::between(min, max))
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Settings {
        json5::from_str(src).unwrap()
    }

    #[test]
    fn anything_left_out_keeps_its_default() {
        assert_eq!(parse("{}"), Settings::default());
        let s = parse("{ implosions: { every: 3 }, lightning: { lulls: { chance: 0 } }, glyphs: { enabled: false }, surge: { break: 2, stay_broken: true } }");
        assert_eq!(s.implosions.every, Seconds::between(3.0, 3.0));
        assert!(s.implosions.enabled);
        assert_eq!(s.surge.scatter, 2.0);
        assert!(s.surge.stay_broken);
        assert_eq!(s.surge.charge, Settings::default().surge.charge);
        assert_eq!(s.lulls.chance, 0.0);
        assert_eq!(s.lulls.last, Settings::default().lulls.last);
        assert!(!s.glyphs.enabled);
        assert_eq!(s.glyphs.every, Settings::default().glyphs.every);

        let t = parse(r##"{ colors: { palette: ["#FF0000"], every: 4 }, dissolve: { enabled: false }, shimmer: { speed: 0.1 } }"##);
        assert_eq!(t.palette.colors(), [Rgb([255, 0, 0])]);
        assert_eq!(t.colors.every, Seconds::between(4.0, 4.0));
        assert!(!t.dissolve.enabled);
        assert_eq!(t.shimmer, Shimmer { speed: 0.1, ..Settings::default().shimmer });
    }

    #[test]
    fn nonsense_is_rejected_by_name() {
        let e = parse("{ implosions: { every: [5, 2] } }").check().unwrap_err();
        assert!(e.contains("implosions.every"), "{e}");
        let e = parse("{ surge: { charge: 0 } }").check().unwrap_err();
        assert!(e.contains("surge.charge"), "{e}");
        let e = parse("{ lightning: { every: 0 } }").check().unwrap_err();
        assert!(e.contains("lightning.every"), "{e}");
        assert!(json5::from_str::<Settings>("{ glyphs: { evry: 3 } }").is_err());
        assert!(parse("{ colors: { palette: [] } }").check().unwrap_err().contains("palette"));
        assert!(parse("{ haze: { strength: -1 } }").check().is_err());
        assert!(json5::from_str::<Settings>("{ drops: { every: 3 } }").is_err(), "drops are Enter's now");
    }
}
