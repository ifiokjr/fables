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

	/// The colour in `OKLab`.
	#[must_use]
	pub fn oklab(self) -> Oklab {
		let (r, g, b) = self.rgb();
		let [r, g, b] = [r, g, b].map(|channel| to_linear(f32::from(channel) / 255.0));
		let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
		let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
		let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();

		Oklab {
			l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
			a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
			b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
		}
	}

	/// The sRGB colour nearest `lab`. A colour outside what sRGB can show keeps its lightness
	/// and hue and gives up chroma until it fits, so a graded colour never changes family by
	/// being clipped.
	#[must_use]
	pub fn from_oklab(lab: Oklab) -> Color {
		let l = lab.l.clamp(0.0, 1.0);
		let mut rgb = linear_rgb(Oklab { l, ..lab });

		if !in_gamut(rgb) {
			// Bisect the chroma scale: grey always fits.
			let (mut low, mut high) = (0.0_f32, 1.0_f32);

			for _ in 0..16 {
				let mid = f32::midpoint(low, high);

				if in_gamut(linear_rgb(Oklab {
					l,
					a: lab.a * mid,
					b: lab.b * mid,
				})) {
					low = mid;
				} else {
					high = mid;
				}
			}

			rgb = linear_rgb(Oklab {
				l,
				a: lab.a * low,
				b: lab.b * low,
			});
		}

		let [r, g, b] =
			rgb.map(|channel| (from_linear(channel.clamp(0.0, 1.0)) * 255.0).round() as u32);
		Color((r << 16) | (g << 8) | b)
	}
}

/// A colour in `OKLab`, a perceptual space: `l` is lightness (0 black to 1 white), `a` runs
/// from green to red and `b` from blue to yellow. A hue is an angle in the `a`–`b` plane and
/// chroma is the distance from grey, so a hue can be turned by a measured number of degrees
/// and equal steps look equal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
	pub l: f32,
	pub a: f32,
	pub b: f32,
}

impl Oklab {
	/// A colour from lightness, chroma and hue in degrees.
	#[must_use]
	pub fn lch(l: f32, chroma: f32, hue: f32) -> Oklab {
		let (sin, cos) = hue.to_radians().sin_cos();
		Oklab {
			l,
			a: chroma * cos,
			b: chroma * sin,
		}
	}

	/// How far from grey: 0 for a neutral, around 0.3 for the most vivid sRGB colours.
	#[must_use]
	pub fn chroma(self) -> f32 {
		self.a.hypot(self.b)
	}

	/// The hue angle in degrees, 0 to 360: reds sit near 30, yellows near 100, greens near
	/// 140, blues near 260.
	#[must_use]
	pub fn hue(self) -> f32 {
		self.b.atan2(self.a).to_degrees().rem_euclid(360.0)
	}

	/// Whether sRGB can show this colour as it is.
	#[must_use]
	pub fn fits(self) -> bool {
		(0.0..=1.0).contains(&self.l) && in_gamut(linear_rgb(self))
	}
}

fn in_gamut(rgb: [f32; 3]) -> bool {
	rgb.iter().all(|c| (-0.000_5..=1.000_5).contains(c))
}

fn to_linear(channel: f32) -> f32 {
	if channel <= 0.040_45 {
		channel / 12.92
	} else {
		((channel + 0.055) / 1.055).powf(2.4)
	}
}

fn from_linear(channel: f32) -> f32 {
	if channel <= 0.003_130_8 {
		channel * 12.92
	} else {
		1.055 * channel.powf(1.0 / 2.4) - 0.055
	}
}

/// `OKLab` to linear sRGB, unclamped.
fn linear_rgb(lab: Oklab) -> [f32; 3] {
	let l = (lab.l + 0.396_337_78 * lab.a + 0.215_803_76 * lab.b).powi(3);
	let m = (lab.l - 0.105_561_346 * lab.a - 0.063_854_17 * lab.b).powi(3);
	let s = (lab.l - 0.089_484_18 * lab.a - 1.291_485_5 * lab.b).powi(3);

	[
		4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
		-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
		-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
	]
}

/// Colours in data files are palette names or hex codes, as in [`Color::parse`].
impl<'de> serde::Deserialize<'de> for Color {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
		let value = String::deserialize(deserializer)?;
		Color::parse(&value).ok_or_else(|| {
			serde::de::Error::custom(format!(
				"`{value}` is not a colour; use a palette name or a hex code like #ff8a3d"
			))
		})
	}
}

impl serde::Serialize for Color {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.collect_str(self)
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
	fn oklab_round_trips_and_measures_the_known_points() {
		for colour in [
			Color(0x00_00_00),
			Color(0xFF_FF_FF),
			Color(0x80_80_80),
			INK,
			TOMATO,
			TEAL,
			CORNFLOWER,
			SUNFLOWER,
			Color(0xF8_DC_C8),
			Color(0x1E_2A_50),
		] {
			assert_eq!(Color::from_oklab(colour.oklab()), colour, "{colour}");
		}

		let white = WHITE.oklab();
		assert!(
			(white.l - 1.0).abs() < 1e-3 && white.chroma() < 1e-3,
			"{white:?}"
		);
		let black = Color(0).oklab();
		assert!(black.l.abs() < 1e-6, "{black:?}");

		// Hues land where they should: a red near 30, a blue near 260.
		let red = Color(0xFF_00_00).oklab();
		assert!((red.hue() - 29.2).abs() < 1.0, "{}", red.hue());
		let blue = Color(0x00_00_FF).oklab();
		assert!((blue.hue() - 264.1).abs() < 1.0, "{}", blue.hue());
	}

	#[test]
	fn an_out_of_gamut_colour_keeps_its_hue_and_lightness() {
		// Far more chroma than sRGB can show at this lightness and hue.
		let wanted = Oklab::lch(0.6, 0.4, 260.0);
		let shown = Color::from_oklab(wanted).oklab();
		assert!((shown.l - 0.6).abs() < 0.01, "{shown:?}");
		assert!((shown.hue() - 260.0).abs() < 2.0, "{}", shown.hue());
		assert!(shown.chroma() < 0.4 && shown.chroma() > 0.1, "{shown:?}");
	}

	#[test]
	fn mix_hits_endpoints() {
		assert_eq!(TEAL.mix(INK, 0.0), TEAL);
		assert_eq!(TEAL.mix(INK, 1.0), INK);
		assert_eq!(WHITE.to_string(), "#ffffff");
	}
}
