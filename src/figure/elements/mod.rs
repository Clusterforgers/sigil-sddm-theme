//! Everything a layer can contain.
//!
//! To add a kind of element:
//! 1. Write `my_thing.rs`: a struct deriving `Deserialize` with `deny_unknown_fields`,
//!    holding its own fields plus `color` and `note` like the others, and an
//!    `impl Draw for MyThing`.
//! 2. Add `mod my_thing;` and one line to the `registry!` below.
//!
//! That is all: the figure file can now say `{ type: "my-thing", … }`.

mod circle;
mod label;
mod polygon;
mod spokes;
mod star;
mod symbol;
mod symbols;
mod text;

use super::pen::Pen;
use super::spec::Rgb;

use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

/// What every element does.
pub trait Draw {
    /// Issue this element's marks.
    fn draw(&self, pen: &mut Pen);

    /// Reject values that parse but make no sense, with a message saying why.
    fn check(&self) -> Result<(), String> {
        Ok(())
    }

    /// Its own colour, if it overrides the figure's `ink`.
    fn color(&self) -> Option<Rgb>;
}

/// Declares `Element` — one variant per kind, named in the file by `type` — and the one
/// place its variants are dispatched.
///
/// Deserialized by hand rather than with `#[serde(tag = "type")]`: serde's tagged enums
/// buffer the whole element first, which throws away the parser's positions, so every
/// error inside an element would point at the start of the list. Reading `type` and then
/// handing the rest of the live map to the variant keeps them exact — at the cost of
/// `type` having to come first, which every example already does.
macro_rules! registry {
    ($($variant:ident($ty:ty) = $name:literal,)*) => {
        #[derive(Debug)]
        pub enum Element {
            $( $variant($ty), )*
        }

        const KINDS: &[&str] = &[$($name),*];

        impl Element {
            /// The `type` it was written with.
            pub fn kind(&self) -> &'static str {
                match self { $( Element::$variant(_) => $name, )* }
            }

            fn inner(&self) -> &dyn Draw {
                match self { $( Element::$variant(e) => e, )* }
            }

            fn from_map<'de, A: MapAccess<'de>>(kind: &str, rest: A) -> Result<Self, A::Error> {
                let rest = MapAccessDeserializer::new(rest);
                match kind {
                    $( $name => <$ty>::deserialize(rest).map(Element::$variant), )*
                    other => Err(de::Error::unknown_variant(other, KINDS)),
                }
            }
        }
    };
}

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(ElementVisitor)
    }
}

struct ElementVisitor;

impl<'de> Visitor<'de> for ElementVisitor {
    type Value = Element;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an element such as {{ type: \"circle\", r: 100 }}")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Element, A::Error> {
        if map.next_key::<String>()?.as_deref() != Some("type") {
            return Err(de::Error::custom(format!(
                "an element must start with its `type`, one of: {}",
                KINDS.join(", ")
            )));
        }
        let kind: String = map.next_value()?;
        Element::from_map(&kind, map)
    }
}

registry! {
    Circle(circle::Circle) = "circle",
    Polygon(polygon::Polygon) = "polygon",
    Star(star::Star) = "star",
    Spokes(spokes::Spokes) = "spokes",
    Text(text::Text) = "text",
    Label(label::Label) = "label",
    Symbols(symbols::Symbols) = "symbols",
    Symbol(symbol::SymbolMark) = "symbol",
}

impl Element {
    pub fn draw(&self, pen: &mut Pen) {
        self.inner().draw(pen)
    }

    pub fn check(&self) -> Result<(), String> {
        self.inner().check()
    }

    pub fn color(&self) -> Option<Rgb> {
        self.inner().color()
    }
}

/// `Err(message)` unless `value > 0`.
fn positive(name: &str, value: f32) -> Result<(), String> {
    if value > 0.0 {
        Ok(())
    } else {
        Err(format!("`{name}` must be greater than 0, got {value}"))
    }
}
