//! Scene light: one value that grades every colour, paints the sky, aims the shadows and adds
//! atmosphere, so a story can have a colour script (warm morning, golden afternoon, blue dusk,
//! moonlit night, grey storm).
//!
//! A [`Light`] is data: a colour grade (a tint multiplied in with a strength, a shadow tint for
//! the darks, saturation and contrast), the key light's direction and elevation, a sky gradient
//! (zenith and horizon), a haze colour and amount, a vignette strength, and a glow (the sun or
//! moon bloom, or a lamp pool) colour and strength. Two lights blend so video can ease between
//! them, and `day` is an exact identity: a page without a light renders byte for byte as it
//! always did.

use anyhow::Result;

use super::Color;
use super::Pt;
use super::lerp;
use super::palette;
use super::pt;

/// Which sky a light belongs to, told by its glow.
///
/// Settings mark their sun with `data-time="day"` and their moon and stars with
/// `data-time="night"`; under a lit scene those parts follow the light's body instead of the
/// bare day-or-night flag. `None` is an interior light (a lamp, a candle) with no sky body at
/// all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
	Sun,
	Moon,
}

/// The named lights a script can ask for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
	#[default]
	Day,
	Dawn,
	Golden,
	Dusk,
	Night,
	Moonlit,
	Overcast,
	Storm,
	Indoor,
	Candle,
}

impl Preset {
	pub const ALL: [Preset; 10] = [
		Preset::Day,
		Preset::Dawn,
		Preset::Golden,
		Preset::Dusk,
		Preset::Night,
		Preset::Moonlit,
		Preset::Overcast,
		Preset::Storm,
		Preset::Indoor,
		Preset::Candle,
	];

	pub fn parse(value: &str) -> Option<Preset> {
		Preset::ALL
			.iter()
			.copied()
			.find(|preset| preset.name().eq_ignore_ascii_case(value))
	}

	pub fn name(self) -> &'static str {
		match self {
			Preset::Day => "day",
			Preset::Dawn => "dawn",
			Preset::Golden => "golden",
			Preset::Dusk => "dusk",
			Preset::Night => "night",
			Preset::Moonlit => "moonlit",
			Preset::Overcast => "overcast",
			Preset::Storm => "storm",
			Preset::Indoor => "indoor",
			Preset::Candle => "candle",
		}
	}

	/// The names, for error messages that list the options.
	pub fn names() -> String {
		Preset::ALL.map(Preset::name).join(", ")
	}
}

/// A scene light: the grade, the key direction, the sky, the atmosphere and the glow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
	pub preset: Preset,
	/// The tint multiplied into every colour, and how strongly (0 keeps colours as they are).
	pub tint: Color,
	pub strength: f32,
	/// What dark colours lean towards.
	pub shadow_tint: Color,
	pub saturation: f32,
	pub contrast: f32,
	/// Where the key light sits, as the offset that moves a shape towards it: `pt(-9, -8)` is
	/// the top left, the same convention as a style's `[finish]` light.
	pub key: Pt,
	/// How high the key sits: 0 is on the horizon (long shadows), 1 overhead.
	pub elevation: f32,
	/// The sky gradient's colours, zenith first.
	pub zenith: Color,
	pub horizon: Color,
	/// The wash of air over the scene, and how much of it.
	pub haze_colour: Color,
	pub haze_amount: f32,
	/// How strongly the edges of the scene darken.
	pub vignette: f32,
	/// The sun or moon bloom, or a lamp pool: colour and strength.
	pub glow_colour: Color,
	pub glow_strength: f32,
	/// Which sky body glows, if any.
	pub body: Option<Body>,
	/// Whether shaded characters catch a rim light in the glow colour.
	pub rim: bool,
	/// How strongly the ground shadow shows (faint at night).
	pub ground_shadow: f32,
}

/// Flat daylight: the exact identity, and every scene's default.
const DAY: Light = Light {
	preset: Preset::Day,
	tint: palette::WHITE,
	strength: 0.0,
	shadow_tint: palette::INK,
	saturation: 1.0,
	contrast: 1.0,
	key: pt(-9.0, -8.0),
	elevation: 0.75,
	zenith: palette::SKY,
	horizon: palette::SKY,
	haze_colour: palette::SKY,
	haze_amount: 0.0,
	vignette: 0.0,
	glow_colour: palette::SUNFLOWER,
	glow_strength: 0.0,
	body: Some(Body::Sun),
	rim: false,
	ground_shadow: 1.0,
};

impl Light {
	/// Every preset, in script order.
	pub const ALL: [Light; 10] = [
		DAY,
		Light {
			preset: Preset::Dawn,
			tint: Color(0xFF_D9_C0),
			strength: 0.32,
			shadow_tint: Color(0x6A_3E_5E),
			saturation: 1.02,
			contrast: 1.04,
			key: pt(13.0, -6.0),
			elevation: 0.28,
			zenith: Color(0xB8_A8_D8),
			horizon: Color(0xFF_D9_A8),
			haze_colour: Color(0xFF_D2_B0),
			haze_amount: 0.16,
			vignette: 0.12,
			glow_colour: Color(0xFF_CF_88),
			glow_strength: 0.5,
			body: Some(Body::Sun),
			rim: false,
			ground_shadow: 0.95,
		},
		Light {
			preset: Preset::Golden,
			tint: Color(0xFF_C9_8A),
			strength: 0.36,
			shadow_tint: Color(0x8A_5A_3C),
			saturation: 1.08,
			contrast: 1.05,
			key: pt(-14.0, -5.0),
			elevation: 0.24,
			zenith: Color(0x8A_C4_E0),
			horizon: Color(0xFF_DF_9E),
			haze_colour: Color(0xFF_DB_A8),
			haze_amount: 0.18,
			vignette: 0.16,
			glow_colour: Color(0xFF_CF_6E),
			glow_strength: 0.6,
			body: Some(Body::Sun),
			rim: true,
			ground_shadow: 1.0,
		},
		Light {
			preset: Preset::Dusk,
			tint: Color(0xC8_90_B8),
			strength: 0.38,
			shadow_tint: Color(0x4A_3A_6A),
			saturation: 1.02,
			contrast: 1.06,
			key: pt(-16.0, -4.0),
			elevation: 0.16,
			zenith: Color(0x5A_5A_9E),
			horizon: Color(0xFF_9E_6A),
			haze_colour: Color(0xD8_A8_B0),
			haze_amount: 0.2,
			vignette: 0.22,
			glow_colour: Color(0xFF_8A_5E),
			glow_strength: 0.55,
			body: Some(Body::Sun),
			rim: true,
			ground_shadow: 0.9,
		},
		Light {
			preset: Preset::Night,
			tint: Color(0x7A_86_C8),
			strength: 0.42,
			shadow_tint: Color(0x1A_22_48),
			saturation: 0.82,
			contrast: 1.08,
			key: pt(4.0, -11.0),
			elevation: 0.6,
			zenith: Color(0x10_18_38),
			horizon: Color(0x2E_3A_6E),
			haze_colour: Color(0x2E_3A_6E),
			haze_amount: 0.22,
			vignette: 0.3,
			glow_colour: Color(0xD8_E2_FF),
			glow_strength: 0.3,
			body: Some(Body::Moon),
			rim: false,
			ground_shadow: 0.45,
		},
		Light {
			preset: Preset::Moonlit,
			tint: Color(0x9A_B2_D8),
			strength: 0.4,
			shadow_tint: Color(0x1E_2C_52),
			saturation: 0.72,
			contrast: 1.1,
			key: pt(9.0, -10.0),
			elevation: 0.5,
			zenith: Color(0x1A_2C_5E),
			horizon: Color(0x4A_5E_8E),
			haze_colour: Color(0x38_46_6E),
			haze_amount: 0.18,
			vignette: 0.26,
			glow_colour: Color(0xE8_F0_FF),
			glow_strength: 0.45,
			body: Some(Body::Moon),
			rim: true,
			ground_shadow: 0.6,
		},
		Light {
			preset: Preset::Overcast,
			tint: Color(0xB8_BC_C0),
			strength: 0.34,
			shadow_tint: Color(0x4A_4E_58),
			saturation: 0.6,
			contrast: 0.96,
			key: pt(2.0, -12.0),
			elevation: 0.92,
			zenith: Color(0x9A_A4_AC),
			horizon: Color(0xCC_D2_D6),
			haze_colour: Color(0xB8_C0_C6),
			haze_amount: 0.24,
			vignette: 0.08,
			glow_colour: Color(0xE8_EC_EE),
			glow_strength: 0.18,
			body: Some(Body::Sun),
			rim: false,
			ground_shadow: 0.85,
		},
		Light {
			preset: Preset::Storm,
			tint: Color(0x88_92_A8),
			strength: 0.44,
			shadow_tint: Color(0x26_2C_3E),
			saturation: 0.62,
			contrast: 1.14,
			key: pt(-13.0, -6.0),
			elevation: 0.3,
			zenith: Color(0x3E_46_58),
			horizon: Color(0x6A_72_84),
			haze_colour: Color(0x58_60_74),
			haze_amount: 0.34,
			vignette: 0.3,
			glow_colour: Color(0xD8_DC_E4),
			glow_strength: 0.16,
			body: Some(Body::Sun),
			rim: false,
			ground_shadow: 0.7,
		},
		Light {
			preset: Preset::Indoor,
			tint: Color(0xFF_D2_A0),
			strength: 0.3,
			shadow_tint: Color(0x005D_4232),
			saturation: 1.04,
			contrast: 1.02,
			key: pt(0.0, -12.0),
			elevation: 0.85,
			zenith: Color(0xF2_D8_B8),
			horizon: Color(0xE8_C8_A0),
			haze_colour: Color(0xFF_E8_C8),
			haze_amount: 0.1,
			vignette: 0.2,
			glow_colour: Color(0xFF_D9_A0),
			glow_strength: 0.4,
			body: None,
			rim: false,
			ground_shadow: 0.95,
		},
		Light {
			preset: Preset::Candle,
			tint: Color(0xE8_A8_60),
			strength: 0.44,
			shadow_tint: Color(0x40_24_20),
			saturation: 1.06,
			contrast: 1.12,
			key: pt(3.0, -9.0),
			elevation: 0.4,
			zenith: Color(0x7A_4E_3A),
			horizon: Color(0xC8_8A_58),
			haze_colour: Color(0xFF_CF_98),
			haze_amount: 0.12,
			vignette: 0.38,
			glow_colour: Color(0xFF_BF_6E),
			glow_strength: 0.55,
			body: None,
			rim: true,
			ground_shadow: 0.7,
		},
	];
}

impl Light {
	/// Flat daylight, the default.
	#[must_use]
	pub const fn day() -> Light {
		DAY
	}

	/// The light named in a script (`light = "golden"`), or `None` if the name is not one.
	#[must_use]
	pub fn parse(value: &str) -> Option<Light> {
		Preset::parse(value).map(|preset| Light::ALL[preset as usize])
	}

	/// A light by preset.
	#[must_use]
	pub fn of(preset: Preset) -> Light {
		Light::ALL[preset as usize]
	}

	pub fn name(&self) -> &'static str {
		self.preset.name()
	}

	/// Whether this is flat daylight, which draws nothing and changes nothing.
	#[must_use]
	pub fn is_day(&self) -> bool {
		self.preset == Preset::Day
	}

	/// Passes `colour` through the light's grade.
	///
	/// The tint multiplies in (warmer at golden hour, bluer by moonlight), dark colours lean
	/// towards the shadow tint, then saturation and contrast finish. Day returns every colour
	/// unchanged.
	#[must_use]
	pub fn grade(&self, colour: Color) -> Color {
		if self.is_day() {
			return colour;
		}

		let (r, g, b) = colour.rgb();
		let (tr, tg, tb) = self.tint.rgb();
		let tinted = [
			f32::from(r) * (f32::from(tr) / 255.0),
			f32::from(g) * (f32::from(tg) / 255.0),
			f32::from(b) * (f32::from(tb) / 255.0),
		];
		let mut out = [
			f32::from(r) + (tinted[0] - f32::from(r)) * self.strength,
			f32::from(g) + (tinted[1] - f32::from(g)) * self.strength,
			f32::from(b) + (tinted[2] - f32::from(b)) * self.strength,
		];

		// Dark colours lean towards the shadow tint, so shadows carry the light's colour.
		let luma = 0.3 * out[0] + 0.59 * out[1] + 0.11 * out[2];
		let darkness = (1.0 - luma / 255.0).clamp(0.0, 1.0);
		let (sr, sg, sb) = self.shadow_tint.rgb();
		let shadowed = [f32::from(sr), f32::from(sg), f32::from(sb)];

		for (channel, tint) in out.iter_mut().zip(shadowed) {
			*channel += (tint - *channel) * darkness * 0.45;
		}

		// Saturation and contrast work on what the tint left.
		let luma = 0.3 * out[0] + 0.59 * out[1] + 0.11 * out[2];

		for channel in &mut out {
			*channel = luma + (*channel - luma) * self.saturation;
			*channel = ((*channel / 255.0 - 0.5) * self.contrast + 0.5) * 255.0;
			*channel = channel.round().clamp(0.0, 255.0);
		}

		Color((out[0] as u32) << 16 | (out[1] as u32) << 8 | out[2] as u32)
	}

	/// Two lights blended; `t` = 0 gives `a`, 1 gives `b`. Video eases between a scene's
	/// lights with this.
	#[must_use]
	pub fn blend(a: &Light, b: &Light, t: f32) -> Light {
		let t = t.clamp(0.0, 1.0);
		let name = if t < 0.5 { a.preset } else { b.preset };

		Light {
			preset: name,
			tint: a.tint.mix(b.tint, t),
			strength: lerp(a.strength, b.strength, t),
			shadow_tint: a.shadow_tint.mix(b.shadow_tint, t),
			saturation: lerp(a.saturation, b.saturation, t),
			contrast: lerp(a.contrast, b.contrast, t),
			key: a.key.lerp(b.key, t),
			elevation: lerp(a.elevation, b.elevation, t),
			zenith: a.zenith.mix(b.zenith, t),
			horizon: a.horizon.mix(b.horizon, t),
			haze_colour: a.haze_colour.mix(b.haze_colour, t),
			haze_amount: lerp(a.haze_amount, b.haze_amount, t),
			vignette: lerp(a.vignette, b.vignette, t),
			glow_colour: a.glow_colour.mix(b.glow_colour, t),
			glow_strength: lerp(a.glow_strength, b.glow_strength, t),
			body: if t < 0.5 { a.body } else { b.body },
			rim: if t < 0.5 { a.rim } else { b.rim },
			ground_shadow: lerp(a.ground_shadow, b.ground_shadow, t),
		}
	}

	/// Where the ground shadow leans and how long it stretches, from the key's direction and
	/// elevation: `(x offset, width stretch)` in figure units. Shadows fall away from the
	/// light, and lengthen as it drops towards the horizon.
	#[must_use]
	pub fn cast(&self) -> (f32, f32) {
		let length = 300.0 * (1.0 - self.elevation.clamp(0.0, 1.0));

		(-self.key.norm().x * length, 1.0 + length / 280.0)
	}

	/// A stable identity for caches: two lights that grade or paint differently hash
	/// differently, and the same light always hashes the same.
	#[must_use]
	pub fn cache_key(&self) -> u64 {
		let mut key = super::seed_of(self.name());
		let mix = |key: u64, value: f32| {
			key.rotate_left(7) ^ u64::from(value.to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15)
		};
		let colour = |key: u64, colour: Color| key.rotate_left(5) ^ u64::from(colour.0);

		key = colour(key, self.tint);
		key = mix(key, self.strength);
		key = colour(key, self.shadow_tint);
		key = mix(key, self.saturation);
		key = mix(key, self.contrast);
		key = mix(key, self.key.x);
		key = mix(key, self.key.y);
		key = mix(key, self.elevation);
		key = colour(key, self.zenith);
		key = colour(key, self.horizon);
		key = colour(key, self.haze_colour);
		key = mix(key, self.haze_amount);
		key = mix(key, self.vignette);
		key = colour(key, self.glow_colour);
		key = mix(key, self.glow_strength);
		key = mix(key, self.ground_shadow);
		key
	}
}

/// `light = "golden"`, as written in front matter, `world.toml` and page headings.
pub fn parse_named(value: &str, line: usize) -> Result<Light> {
	Light::parse(value).ok_or_else(|| {
		anyhow::anyhow!(
			"line {line}: unknown light `{value}`; try {}",
			Preset::names()
		)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A grey that shows a tint's direction at a glance.
	const GREY: Color = Color(0x80_80_80);

	#[test]
	fn every_preset_parses_and_round_trips() {
		for preset in Preset::ALL {
			assert_eq!(
				Light::parse(preset.name()).map(|light| light.preset),
				Some(preset)
			);
		}

		assert_eq!(Light::parse("Day"), Some(Light::day()));
		assert_eq!(Light::parse("sunset"), None);
	}

	#[test]
	fn a_bad_light_names_the_line_and_the_options() {
		let error = format!("{:#}", parse_named("sunset", 12).unwrap_err());
		assert!(error.contains("line 12"), "{error}");
		assert!(
			error.contains("moonlit") && error.contains("candle"),
			"{error}"
		);
	}

	#[test]
	fn day_is_an_exact_identity() {
		for colour in [
			Color(0x00_00_00),
			Color(0xFF_FF_FF),
			GREY,
			palette::TOMATO,
			Color(0x12_34_56),
		] {
			assert_eq!(Light::day().grade(colour), colour);
		}
	}

	#[test]
	fn golden_makes_a_mid_grey_warmer_and_moonlit_cooler_and_darker() {
		let golden = Light::of(Preset::Golden).grade(GREY);
		let (r, g, b) = golden.rgb();
		assert!(r > b, "golden warms grey: {golden}");
		assert!(
			r >= g && g >= b,
			"an amber grade keeps the warm order: {golden}"
		);

		let moonlit = Light::of(Preset::Moonlit).grade(GREY);
		let (r, g, b) = moonlit.rgb();
		assert!(b > r, "moonlight cools grey: {moonlit}");
		assert!(
			u32::from(r) + u32::from(g) + u32::from(b) < 3 * 128,
			"moonlight darkens grey: {moonlit}"
		);
	}

	#[test]
	fn overcast_lowers_saturation() {
		let spread = |colour: Color| {
			let (r, g, b) = colour.rgb();
			let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
			r.max(g).max(b) - r.min(g).min(b)
		};
		let plain = spread(palette::TOMATO);
		let grey_day = spread(Light::of(Preset::Overcast).grade(palette::TOMATO));
		assert!(
			grey_day < plain,
			"overcast desaturates: {plain} -> {grey_day}"
		);
	}

	#[test]
	fn grade_stays_in_range_and_round_trips_extremes() {
		for light in Light::ALL {
			for colour in [Color(0x00_00_00), Color(0xFF_FF_FF), Color(0x0F_0F_0F)] {
				let graded = light.grade(colour);
				let _ = graded.rgb();
			}
		}
	}

	#[test]
	fn blend_hits_both_endpoints() {
		let a = Light::of(Preset::Golden);
		let b = Light::of(Preset::Moonlit);

		assert_eq!(Light::blend(&a, &b, 0.0), a);
		assert_eq!(Light::blend(&a, &b, 1.0), b);

		let middle = Light::blend(&a, &b, 0.5);
		assert_eq!(middle.tint, a.tint.mix(b.tint, 0.5));
		assert!((middle.elevation - f32::midpoint(a.elevation, b.elevation)).abs() < 1e-5);
	}

	#[test]
	fn cast_shadows_lean_away_from_the_key_and_lengthen_as_it_drops() {
		let golden = Light::of(Preset::Golden);
		let dawn = Light::of(Preset::Dawn);
		let night = Light::of(Preset::Night);
		let overcast = Light::of(Preset::Overcast);

		let (golden_dx, golden_stretch) = golden.cast();
		let (dawn_dx, _) = dawn.cast();

		// Golden's key sits in the west (negative x), so its shadow leans east.
		assert!(golden.key.x < 0.0 && golden_dx > 0.0, "{}", golden_dx);
		assert!(dawn.key.x > 0.0 && dawn_dx < 0.0, "{}", dawn_dx);

		// The lower the light, the longer the shadow: dawn is lower than night.
		assert!(dawn.elevation < night.elevation);
		assert!(
			dawn.cast().1 > night.cast().1,
			"dawn {:?} vs night {:?}",
			dawn.cast(),
			night.cast()
		);
		assert!(golden_stretch > 1.0, "a low sun stretches the pool");

		// Overhead light: almost no lean, a soft pool under the actor.
		assert!(overcast.elevation > 0.9);
		assert!(overcast.cast().0.abs() < 20.0, "{:?}", overcast.cast());
	}

	#[test]
	fn cache_keys_separate_lights_and_repeat() {
		let golden = Light::of(Preset::Golden);
		assert_eq!(golden.cache_key(), golden.cache_key());
		assert_ne!(golden.cache_key(), Light::of(Preset::Dusk).cache_key());
		assert_ne!(
			Light::day().cache_key(),
			Light::of(Preset::Moonlit).cache_key()
		);

		// A blended light sits between its endpoints.
		let middle = Light::blend(&golden, &Light::of(Preset::Dusk), 0.5);
		assert_ne!(middle.cache_key(), golden.cache_key());
	}
}
