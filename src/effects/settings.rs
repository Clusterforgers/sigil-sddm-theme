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
        assert!(json5::from_str::<Settings>("{ drops: { every: 3 } }").is_err(), "drops are Enter's now");
    }
}
