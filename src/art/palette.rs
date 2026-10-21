//! The Kerfuffle ink set.
//!
//! A small, fixed set of spot colours keeps every book looking like part of one series, the
//! way a riso studio only owns a handful of drums. Cast files refer to these by name.

use std::fmt;

/// An sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
	#[must_use]
	pub const fn rgb(self) -> (u8, u8, u8) {
		((self.0 >> 16) as u8, (self.0 >> 8) as u8, self.0 as u8)
	}

	/// Linear blend towards `other`.
	#[must_use]
	pub fn mix(self, other: Color, t: f32) -> Color {
		let t = t.clamp(0.0, 1.0);
		let (r1, g1, b1) = self.rgb();
		let (r2, g2, b2) = other.rgb();
		let blend =
			|a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u32;

		Color((blend(r1, r2) << 16) | (blend(g1, g2) << 8) | blend(b1, b2))
	}

	#[must_use]
	pub fn darken(self, amount: f32) -> Color {
		self.mix(INK, amount)
	}

	#[must_use]
	pub fn lighten(self, amount: f32) -> Color {
		self.mix(Color(0xFF_FF_FF), amount)
	}

	/// Parses a palette name (`"tangerine"`) or a hex code (`"#ff8a3d"`).
	pub fn parse(value: &str) -> Option<Color> {
		let value = value.trim();

		if let Some(hex) = value.strip_prefix('#') {
			if hex.len() != 6 {
				return None;
			}

			return u32::from_str_radix(hex, 16).ok().map(Color);
		}

		NAMED
			.iter()
			.find(|(name, _)| name.eq_ignore_ascii_case(value))
			.map(|(_, color)| *color)
	}
}

impl fmt::Display for Color {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "#{:06x}", self.0)
	}
}

/// Warm uncoated paper; the page colour behind everything.
pub const PAPER: Color = Color(0xFF_F4_E2);
/// Line and text ink: a deep aubergine rather than flat black.
pub const INK: Color = Color(0x2A_22_38);
pub const WHITE: Color = Color(0xFF_FF_FF);
pub const TANGERINE: Color = Color(0xFF_8A_3D);
pub const SUNFLOWER: Color = Color(0xFF_C9_3C);
pub const MUSTARD: Color = Color(0xE3_A8_2B);
pub const BUBBLEGUM: Color = Color(0xFF_6F_AE);
pub const FLAMINGO: Color = Color(0xFF_A8_C0);
pub const BLUSH: Color = Color(0xFF_7F_96);
pub const TOMATO: Color = Color(0xF2_54_3D);
pub const TEAL: Color = Color(0x1F_B5_AC);
pub const MINT: Color = Color(0x9B_F2_D5);
pub const ICE: Color = Color(0xD8_FB_F2);
pub const SKY: Color = Color(0x9E_D8_F0);
pub const CORNFLOWER: Color = Color(0x5B_7F_E0);
pub const GRAPE: Color = Color(0x7E_5B_D6);
pub const LILAC: Color = Color(0xC9_A8_F0);
pub const LEAF: Color = Color(0x5D_BB_63);
pub const MEADOW: Color = Color(0x9C_D6_6B);
pub const MOSS: Color = Color(0x9D_A8_3A);
pub const BOG: Color = Color(0x6B_7A_2A);
pub const COCOA: Color = Color(0x8A_5A_44);
pub const TOFFEE: Color = Color(0xC9_86_5A);
pub const SAND: Color = Color(0xF2_D9_A6);
pub const PLUM: Color = Color(0x5B_2E_5E);
pub const SLATE: Color = Color(0x5E_6A_84);
pub const NIGHT: Color = Color(0x2E_2F_6B);
pub const CREAM: Color = Color(0xFF_EC_C8);
/// The inside of an open mouth.
pub const MAW: Color = Color(0x6B_1E_3C);
/// Teeth that have not met a brush in weeks.
pub const GROT: Color = Color(0xC9_C2_5A);

/// Every palette colour a cast or story file may name.
pub const NAMED: &[(&str, Color)] = &[
	("paper", PAPER),
	("ink", INK),
	("white", WHITE),
	("tangerine", TANGERINE),
	("sunflower", SUNFLOWER),
	("mustard", MUSTARD),
	("bubblegum", BUBBLEGUM),
	("flamingo", FLAMINGO),
	("blush", BLUSH),
	("tomato", TOMATO),
	("teal", TEAL),
	("mint", MINT),
	("ice", ICE),
	("sky", SKY),
	("cornflower", CORNFLOWER),
	("grape", GRAPE),
	("lilac", LILAC),
	("leaf", LEAF),
	("meadow", MEADOW),
	("moss", MOSS),
	("bog", BOG),
	("cocoa", COCOA),
	("toffee", TOFFEE),
	("sand", SAND),
	("plum", PLUM),
	("slate", SLATE),
	("night", NIGHT),
	("cream", CREAM),
];

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_names_and_hex() {
		assert_eq!(Color::parse("Tangerine"), Some(TANGERINE));
		assert_eq!(Color::parse("#2a2238"), Some(INK));
		assert_eq!(Color::parse("#12"), None);
		assert_eq!(Color::parse("chartreuse"), None);
	}

	#[test]
	fn mix_hits_endpoints() {
		assert_eq!(TEAL.mix(INK, 0.0), TEAL);
		assert_eq!(TEAL.mix(INK, 1.0), INK);
		assert_eq!(WHITE.to_string(), "#ffffff");
	}
}
