//! Scene light: one value that grades every colour, paints the sky, aims the shadows and adds
//! atmosphere, so a story can have a colour script (warm morning, golden afternoon, blue dusk,
//! moonlit night, grey storm).
//!
//! A [`Light`] is data: a colour grade, the key light's direction and elevation, a sky gradient
//! (zenith and horizon), a haze colour and amount, a vignette strength, and a glow (the sun or
//! moon bloom, or a lamp pool) colour and strength. Two lights blend so video can ease between
//! them, and `day` is an exact identity: a page without a light renders byte for byte as it
//! always did.
//!
//! The grade works the way a colour script does. The light changes the relationships between
//! colours (their temperature, value range and saturation) while every colour stays
//! recognisably itself. It works in `OKLab`, where a hue is an angle and chroma a distance from
//! grey, so the rules are measured, not guessed:
//!
//! - the key light's colour adds to every colour, most to the light ones, and a hue turns
//!   towards it by at most [`TURN`] degrees, keeping at least [`KEEP`] of its chroma, so a
//!   blue never crosses grey into orange at sunset; a colour opposite the light (blue under a
//!   golden sun, yellow under the moon) has no near way round and only greys;
//! - the light sets value (`exposure`, `contrast`) and saturation; a dark light scales values
//!   down, which keeps every step between them;
//! - ink (lines and near-black fills) takes a fraction of the tint, so lines stay ink;
//! - warm local colours (skin, the blush and shading made from it, brown hair) keep a floor
//!   of warmth and, on a figure, of lightness, so no face goes grey or green;
//! - figures take a share of the grade (`figure`) and the setting takes all of it plus a
//!   little desaturation and value compression that grows with depth (`ground`), so the
//!   characters sit forward of the backdrop. [`Plane`] says which a colour belongs to;
//! - a pool of light painted into a setting (a lamp, a lit window's glow) is a light, not a
//!   surface: it takes the light's colour but keeps its brightness, so lamps glow at night.

use anyhow::Result;

use super::Color;
use super::Pt;
use super::lerp;
use super::palette;
use super::palette::Oklab;
use super::pt;

/// The most a colour's hue turns towards the light, in degrees.
pub const TURN: f32 = 20.0;
/// The least share of its chroma a colour keeps when the light's own colour opposes it,
/// before the light's saturation: the light can grey a colour, never push it through grey.
pub const KEEP: f32 = 0.7;
/// The most a warm local colour's hue turns, in degrees.
pub const SKIN_TURN: f32 = 10.0;
/// The least share of its chroma a warm local colour keeps, after saturation.
pub const SKIN_KEEP: f32 = 0.8;
/// The least share of its lightness a warm local colour keeps on a figure.
pub const SKIN_VALUE: f32 = 0.7;
/// The most a multiplied shadow tone turns towards the light's shadow colour, in degrees.
pub const SHADE_TURN: f32 = 30.0;
/// The most a rim light turns a part's own colour towards the glow, in degrees.
pub const RIM_TURN: f32 = 35.0;
/// The chroma at which a colour takes no more colour from the light, only a turn of hue.
const VIVID: f32 = 0.16;

/// Which sky a light belongs to, told by its glow.
///
/// Settings mark their sun with `data-time="day"` and their moon and stars with
/// `data-time="night"`; under a lit scene those parts follow the light's body instead of the
/// bare day-or-night flag. `None` means no sky body shows: an interior light (a lamp, a
/// candle), or a sky covered over by cloud (`overcast`, `storm`), which hides the sun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
	Sun,
	Moon,
}

/// What a colour belongs to, which decides how much of the light it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plane {
	/// A character or prop on stage: the gentler grade that keeps it forward of the setting.
	Figure,
	/// The setting, at a depth from 0 (beside the actors) to 1 (the far hills).
	Ground(f32),
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
	/// The key light's colour, whose hue and chroma add to every colour, and how strongly
	/// (0 keeps colours as they are).
	pub tint: Color,
	pub strength: f32,
	/// The colour of shadow: what dark colours and cel shadows lean towards.
	pub shadow_tint: Color,
	pub saturation: f32,
	pub contrast: f32,
	/// How bright the light is: every lightness is scaled by it (1 keeps values).
	pub exposure: f32,
	/// The share of the grade a figure takes (1 grades it like the setting).
	pub figure: f32,
	/// The setting's extra desaturation and value compression at full depth.
	pub ground: f32,
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
	exposure: 1.0,
	figure: 1.0,
	ground: 0.0,
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
			tint: Color(0xFF_C8_B0),
			strength: 0.5,
			shadow_tint: Color(0x6A_3E_6E),
			saturation: 1.0,
			contrast: 1.0,
			exposure: 0.96,
			figure: 0.8,
			ground: 0.22,
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
			tint: Color(0xFF_C0_70),
			strength: 0.42,
			shadow_tint: Color(0x5E_4A_7E),
			saturation: 1.06,
			contrast: 1.06,
			exposure: 0.95,
			figure: 0.8,
			ground: 0.2,
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
			tint: Color(0xD8_90_C0),
			strength: 0.55,
			shadow_tint: Color(0x4A_3A_7A),
			saturation: 0.95,
			contrast: 1.04,
			exposure: 0.82,
			figure: 0.8,
			ground: 0.25,
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
			tint: Color(0x7A_86_D8),
			strength: 0.55,
			shadow_tint: Color(0x1A_22_58),
			saturation: 0.55,
			contrast: 1.0,
			exposure: 0.5,
			figure: 0.8,
			ground: 0.35,
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
			tint: Color(0x9A_B2_E8),
			strength: 0.55,
			shadow_tint: Color(0x1E_2C_62),
			saturation: 0.48,
			contrast: 1.02,
			exposure: 0.66,
			figure: 0.8,
			ground: 0.35,
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
			tint: Color(0xB8_BE_C8),
			strength: 0.3,
			shadow_tint: Color(0x4A_50_60),
			saturation: 0.7,
			contrast: 0.95,
			exposure: 0.92,
			figure: 0.75,
			ground: 0.3,
			key: pt(2.0, -12.0),
			elevation: 0.92,
			zenith: Color(0x9A_A4_AC),
			horizon: Color(0xCC_D2_D6),
			haze_colour: Color(0xB8_C0_C6),
			haze_amount: 0.24,
			vignette: 0.08,
			glow_colour: Color(0xE8_EC_EE),
			glow_strength: 0.18,
			body: None,
			rim: false,
			ground_shadow: 0.85,
		},
		Light {
			preset: Preset::Storm,
			tint: Color(0x88_96_B8),
			strength: 0.45,
			shadow_tint: Color(0x26_2C_48),
			saturation: 0.6,
			contrast: 1.05,
			exposure: 0.7,
			figure: 0.8,
			ground: 0.35,
			key: pt(-13.0, -6.0),
			elevation: 0.3,
			zenith: Color(0x3E_46_58),
			horizon: Color(0x6A_72_84),
			haze_colour: Color(0x58_60_74),
			haze_amount: 0.34,
			vignette: 0.3,
			glow_colour: Color(0xD8_DC_E4),
			glow_strength: 0.16,
			body: None,
			rim: false,
			ground_shadow: 0.7,
		},
		Light {
			preset: Preset::Indoor,
			tint: Color(0xFF_D2_A0),
			strength: 0.4,
			shadow_tint: Color(0x5D_42_42),
			saturation: 1.0,
			contrast: 1.0,
			exposure: 0.95,
			figure: 0.85,
			ground: 0.15,
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
			tint: Color(0xF0_A8_60),
			strength: 0.35,
			shadow_tint: Color(0x40_24_30),
			saturation: 0.95,
			contrast: 1.05,
			exposure: 0.75,
			figure: 0.8,
			ground: 0.25,
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

	/// How much of the grade a plane takes, and how far the setting recedes past it.
	fn reach(&self, plane: Plane) -> (f32, f32) {
		match plane {
			Plane::Figure => (self.figure, 0.0),
			Plane::Ground(depth) => (1.0, self.ground * (0.4 + 0.6 * depth.clamp(0.0, 1.0))),
		}
	}

	/// Passes `colour`, painted on `plane`, through the light's grade.
	///
	/// The key light's colour adds in, dark colours lean towards the shadow colour, the hue
	/// is held within [`TURN`] of where it started, then exposure, contrast and saturation
	/// finish, and warm local colours keep their floors. Ink takes a fraction of the tint, a
	/// figure takes its share of all of it, and the setting recedes with depth. Day returns
	/// every colour unchanged.
	#[must_use]
	pub fn grade(&self, colour: Color, plane: Plane) -> Color {
		if self.is_day() {
			return colour;
		}

		let (share, recede) = self.reach(plane);
		let lab = colour.oklab();
		let tinted = self.tinted(lab, share);

		// The light sets value and saturation; the setting compresses towards the air.
		let exposure = lerp(1.0, self.exposure, share);
		let contrast = lerp(1.0, self.contrast, share);
		let pivot = 0.5 * exposure;
		let mut l = pivot + (lab.l * exposure - pivot) * contrast;
		l += (self.haze_colour.oklab().l - l) * 0.5 * recede;
		let saturation = lerp(1.0, self.saturation, share) * (1.0 - recede);
		let mut graded = Oklab {
			l,
			a: tinted.a * saturation,
			b: tinted.b * saturation,
		};

		// On a figure, skin and the warm colours made from it keep their warmth and their
		// lightness, whatever the light.
		let warm = if plane == Plane::Figure {
			warmth(lab)
		} else {
			0.0
		};

		if warm > 0.0 {
			let mut floored = hold(lab, graded, SKIN_TURN, SKIN_KEEP);
			floored.l = floored.l.max(lab.l * SKIN_VALUE);
			graded = mix(graded, floored, warm);
		}

		Color::from_oklab(beneath_white(graded))
	}

	/// The light's colour on `lab`, at `share` of its strength: the key adds in, dark colours
	/// lean towards the colour of shadow, and the hue is held within [`TURN`] of where it
	/// started (less the more the light opposes it). Lightness is left alone.
	fn tinted(&self, lab: Oklab, share: f32) -> Oklab {
		// Ink barely takes the light's colour; light colours take the most of it.
		let inked = 1.0 - 0.85 * ink(lab.l);
		let carry = self.strength * share * inked * (lab.l / 0.8).min(1.0);
		let key = self.tint.oklab();
		let mut a = lab.a + key.a * carry;
		let mut b = lab.b + key.b * carry;

		// Dark colours lean towards the colour of shadow.
		let shadow = self.shadow_tint.oklab();
		let dark = (1.0 - lab.l / 0.6).clamp(0.0, 1.0);
		let lean = 0.35 * dark * dark * share * inked;
		a += (shadow.a - a) * lean;
		b += (shadow.b - b) * lean;

		// The light's colour tints pale and greyed colours but cannot make a vivid one more
		// vivid: what it may add to the chroma shrinks to nothing at [`VIVID`]. Saturation
		// is the light's own dial.
		let chroma = lab.chroma();
		let gain =
			(key.chroma() * carry + shadow.chroma() * lean) * (1.0 - chroma / VIVID).max(0.0);
		let pushed = a.hypot(b);

		if pushed > chroma + gain {
			let scale = (chroma + gain) / pushed;
			a *= scale;
			b *= scale;
		}

		hold(
			lab,
			Oklab { l: lab.l, a, b },
			facing(lab.hue(), key.hue(), TURN),
			KEEP,
		)
	}

	/// A pool of light painted into the setting (a lamp's glow, the haze by a lit window)
	/// under this light. It is a light itself, not a surface the scene light falls on, so it
	/// keeps its brightness and saturation and takes only half the light's colour, held like
	/// any other; a lamp still glows warm at night. Day returns the colour unchanged.
	#[must_use]
	pub fn glow(&self, colour: Color) -> Color {
		if self.is_day() {
			return colour;
		}

		Color::from_oklab(beneath_white(self.tinted(colour.oklab(), 0.5)))
	}

	/// A multiplied tone (a cel shadow) under this light. What it darkens has already been
	/// graded, so the tone keeps its own value and only turns towards the colour of shadow, by
	/// at most [`SHADE_TURN`], at the light's saturation: under a warm sun the shadows go
	/// cool instead of darkening twice into mud.
	#[must_use]
	pub fn shade(&self, colour: Color, plane: Plane) -> Color {
		if self.is_day() {
			return colour;
		}

		let (share, _) = self.reach(plane);
		let lab = colour.oklab();
		let turned = turn(lab, self.shadow_tint.oklab().hue(), SHADE_TURN * share);
		let saturation = lerp(1.0, self.saturation, share);

		Color::from_oklab(Oklab {
			l: lab.l,
			a: turned.a * saturation,
			b: turned.b * saturation,
		})
	}

	/// The rim the glow paints along a figure's lit edge, over a part whose local colour is
	/// `fill`: that colour much lighter and turned towards the glow by at most [`RIM_TURN`], so
	/// a warm sun edges blue hair in a pale, warmer blue instead of washing it green-grey.
	/// A neutral part (a white shirt) takes the glow's own colour.
	#[must_use]
	pub fn rim_on(&self, fill: Color) -> Color {
		let lab = fill.oklab();
		let glow = self.glow_colour.oklab();
		let lit = Oklab {
			l: lab.l + (1.0 - lab.l) * 0.6,
			a: lerp(lab.a, glow.a, 0.6),
			b: lerp(lab.b, glow.b, 0.6),
		};

		Color::from_oklab(hold(lab, lit, RIM_TURN, KEEP))
	}

	/// Two lights blended; `t` = 0 gives `a`, 1 gives `b`, exactly (interpolating would miss a
	/// dial by a rounding step, and a held light would then grade unlike itself). Video eases
	/// between a scene's lights with this.
	#[must_use]
	pub fn blend(a: &Light, b: &Light, t: f32) -> Light {
		let t = t.clamp(0.0, 1.0);

		if t <= 0.0 {
			return *a;
		}

		if t >= 1.0 {
			return *b;
		}

		let name = if t < 0.5 { a.preset } else { b.preset };

		Light {
			preset: name,
			tint: a.tint.mix(b.tint, t),
			strength: lerp(a.strength, b.strength, t),
			shadow_tint: a.shadow_tint.mix(b.shadow_tint, t),
			saturation: lerp(a.saturation, b.saturation, t),
			contrast: lerp(a.contrast, b.contrast, t),
			exposure: lerp(a.exposure, b.exposure, t),
			figure: lerp(a.figure, b.figure, t),
			ground: lerp(a.ground, b.ground, t),
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
		key = mix(key, self.exposure);
		key = mix(key, self.figure);
		key = mix(key, self.ground);
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

/// How much a colour of lightness `l` is ink: 1 for lines and near-black fills, 0 from mid
/// values up.
fn ink(l: f32) -> f32 {
	1.0 - smoothstep(0.2, 0.45, l)
}

/// How much a colour is a warm local colour that must keep its life: skin from pale to deep,
/// the blush and shading made from it, reds, browns. Hues from pink to amber, with some
/// colour in them, lighter than ink.
fn warmth(lab: Oklab) -> f32 {
	let from_centre = (lab.hue() - 50.0 + 180.0).rem_euclid(360.0) - 180.0;

	(1.0 - smoothstep(35.0, 60.0, from_centre.abs()))
		* smoothstep(0.015, 0.04, lab.chroma())
		* smoothstep(0.22, 0.35, lab.l)
}

/// How far a colour of `hue` may turn towards a light of hue `light`: all of `limit` up to
/// 120° apart, falling to nothing at the opposite hue. Opposite a light there is no near way
/// round the wheel (blue under a golden sun would go violet one way and green the other), so
/// the light greys it instead of turning it.
fn facing(hue: f32, light: f32, limit: f32) -> f32 {
	let apart = ((light - hue + 180.0).rem_euclid(360.0) - 180.0).abs();

	limit * (1.0 - smoothstep(120.0, 180.0, apart))
}

/// `graded` with its hue held within `limit` degrees of `original` and its chroma at least
/// `keep` of the original's. Chroma a light took away is given back along the colour's own
/// hue, so a colour greyed by an opposing light comes back as itself, not as its neighbour.
/// Neutral originals have no hue to keep and pass through.
fn hold(original: Oklab, graded: Oklab, limit: f32, keep: f32) -> Oklab {
	let chroma = original.chroma();
	let hued = smoothstep(0.008, 0.03, chroma);

	if hued <= 0.0 {
		return graded;
	}

	let own = Oklab::lch(0.0, 1.0, original.hue());
	let short = (chroma * keep - graded.chroma()).max(0.0);
	let restored = Oklab {
		l: graded.l,
		a: graded.a + own.a * short,
		b: graded.b + own.b * short,
	};
	let turned = (restored.hue() - original.hue() + 180.0).rem_euclid(360.0) - 180.0;
	let held = Oklab::lch(
		graded.l,
		restored.chroma().max(chroma * keep),
		original.hue() + turned.clamp(-limit, limit),
	);

	mix(graded, held, hued)
}

/// A tinted colour near white, lowered just enough for its tint to show: sRGB has no room
/// for colour at white itself, so a white shirt in golden light is a warm cream a step below
/// it, the way a coloured light is dimmer than a white one. Lowered by a tenth at most.
fn beneath_white(colour: Oklab) -> Oklab {
	const DROP: f32 = 0.1;

	if colour.l < 0.75 || colour.fits() {
		return colour;
	}

	let floor = Oklab {
		l: colour.l - DROP,
		..colour
	};

	// A colour too vivid to fit a tenth lower keeps its lightness and gives up chroma.
	if !floor.fits() {
		return colour;
	}

	// The brightest lightness that holds the tint.
	let (mut low, mut high) = (floor.l, colour.l);

	for _ in 0..12 {
		let mid = f32::midpoint(low, high);

		if (Oklab { l: mid, ..colour }).fits() {
			low = mid;
		} else {
			high = mid;
		}
	}

	Oklab { l: low, ..colour }
}

/// `colour` with its hue turned towards `hue` by at most `limit` degrees.
fn turn(colour: Oklab, hue: f32, limit: f32) -> Oklab {
	let towards = (hue - colour.hue() + 180.0).rem_euclid(360.0) - 180.0;

	Oklab::lch(
		colour.l,
		colour.chroma(),
		colour.hue() + towards.clamp(-limit, limit),
	)
}

fn mix(a: Oklab, b: Oklab, t: f32) -> Oklab {
	Oklab {
		l: lerp(a.l, b.l, t),
		a: lerp(a.a, b.a, t),
		b: lerp(a.b, b.b, t),
	}
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
	let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
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
	/// Hana's navy hair.
	const NAVY: Color = Color(0x2B_2F_5E);
	const RED: Color = palette::TOMATO;
	const GREEN: Color = palette::LEAF;
	const PALE_SKIN: Color = Color(0xF8_DC_C8);
	const DEEP_SKIN: Color = Color(0x7A_4A_2E);
	/// The line colour authored puppets draw with.
	const PUPPET_LINE: Color = Color(0x2B_1D_2E);
	/// The setting just behind the actors.
	const NEAR: Plane = Plane::Ground(0.0);

	/// How far a hue turned, in degrees.
	fn turned(from: Color, to: Color) -> f32 {
		let (from, to) = (from.oklab(), to.oklab());
		((to.hue() - from.hue() + 180.0).rem_euclid(360.0) - 180.0).abs()
	}

	/// How far an eight-bit colour's hue can sit from where the grade put it from rounding
	/// alone, in degrees: half a step in a channel moves a colour about 0.0025 in `OKLab`,
	/// which turns a pale colour's hue further than a vivid one's.
	fn rounding(colour: Color) -> f32 {
		(0.0025 / colour.oklab().chroma().max(0.0025))
			.atan()
			.to_degrees()
	}

	fn lights() -> impl Iterator<Item = Light> {
		Light::ALL.into_iter().filter(|light| !light.is_day())
	}

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
			for plane in [Plane::Figure, NEAR, Plane::Ground(1.0)] {
				assert_eq!(Light::day().grade(colour, plane), colour);
				assert_eq!(Light::day().shade(colour, plane), colour);
			}

			assert_eq!(Light::day().glow(colour), colour);
		}
	}

	#[test]
	fn a_colour_opposite_the_light_greys_instead_of_turning() {
		const SEA: Color = Color(0x3A_64_C8);

		// Blue sits opposite a golden or candle sun, and yellow opposite the moon: neither has
		// a near way round the wheel, so the turn shrinks to a few degrees.
		for (preset, colour) in [
			(Preset::Golden, SEA),
			(Preset::Golden, NAVY),
			(Preset::Candle, SEA),
			(Preset::Night, palette::SUNFLOWER),
			(Preset::Moonlit, palette::SUNFLOWER),
		] {
			let light = Light::of(preset);
			let bound = facing(colour.oklab().hue(), light.tint.oklab().hue(), TURN);
			assert!(
				bound < TURN / 2.0,
				"{colour} sits nearly opposite {}: bound {bound:.1}",
				light.name()
			);

			for plane in [Plane::Figure, NEAR, Plane::Ground(1.0)] {
				let graded = light.grade(colour, plane);
				let turn = turned(colour, graded);
				assert!(
					turn <= bound + rounding(graded),
					"{colour} under {} turned {turn:.1}° (bound {bound:.1}°) to {graded}",
					light.name()
				);
			}
		}

		// A hue a quarter turn from the light still takes all of it.
		let green = palette::LEAF.oklab().hue();
		let golden = Light::of(Preset::Golden).tint.oklab().hue();
		assert!((facing(green, golden, TURN) - TURN).abs() < f32::EPSILON);
	}

	#[test]
	fn a_lamp_keeps_glowing_at_night() {
		for preset in [Preset::Dusk, Preset::Night, Preset::Moonlit] {
			let light = Light::of(preset);
			let lamp = palette::SUNFLOWER.oklab();
			let glow = light.glow(palette::SUNFLOWER).oklab();
			let lit = light.grade(palette::SUNFLOWER, NEAR).oklab();

			assert!(
				glow.l >= lamp.l - 0.1,
				"a lamp keeps its brightness under {}: {:.2} from {:.2}",
				light.name(),
				glow.l,
				lamp.l
			);
			assert!(
				glow.chroma() >= lamp.chroma() * KEEP,
				"a lamp keeps its colour under {}",
				light.name()
			);
			assert!(
				turned(palette::SUNFLOWER, light.glow(palette::SUNFLOWER)) <= TURN + 0.5,
				"a lamp's hue is held under {}",
				light.name()
			);
			assert!(
				glow.l > lit.l,
				"a lamp is brighter than a yellow wall under {}",
				light.name()
			);
		}
	}

	#[test]
	fn golden_makes_a_mid_grey_warmer_and_moonlit_cooler_and_darker() {
		let golden = Light::of(Preset::Golden).grade(GREY, NEAR);
		let (r, g, b) = golden.rgb();
		assert!(r > b, "golden warms grey: {golden}");
		assert!(
			r >= g && g >= b,
			"an amber grade keeps the warm order: {golden}"
		);

		let moonlit = Light::of(Preset::Moonlit).grade(GREY, NEAR);
		let (r, g, b) = moonlit.rgb();
		assert!(b > r, "moonlight cools grey: {moonlit}");
		assert!(
			u32::from(r) + u32::from(g) + u32::from(b) < 3 * 128,
			"moonlight darkens grey: {moonlit}"
		);
	}

	/// Under every light and on every plane, a coloured local colour turns towards the light
	/// by no more than [`TURN`], and on a figure a warm local colour (skin, a red) by no more
	/// than [`SKIN_TURN`], give or take the hue's rounding to eight-bit channels.
	#[test]
	fn a_hue_turns_towards_the_light_by_a_bounded_amount() {
		for light in lights() {
			for plane in [Plane::Figure, NEAR, Plane::Ground(1.0)] {
				for (colour, limit) in [
					(NAVY, TURN),
					(RED, SKIN_TURN),
					(GREEN, TURN),
					(PALE_SKIN, SKIN_TURN),
					(DEEP_SKIN, SKIN_TURN),
				] {
					let graded = light.grade(colour, plane);
					let limit = if plane == Plane::Figure { limit } else { TURN };
					assert!(
						turned(colour, graded) <= limit + rounding(graded),
						"{} turned {colour} to {graded} on {plane:?}: {} degrees",
						light.name(),
						turned(colour, graded)
					);
				}
			}
		}
	}

	#[test]
	fn blue_stays_blue_and_red_stays_red_under_every_light() {
		for light in lights() {
			for plane in [Plane::Figure, NEAR, Plane::Ground(1.0)] {
				let (r, _, b) = light.grade(NAVY, plane).rgb();
				assert!(
					b > r,
					"{} greys the navy past blue on {plane:?}",
					light.name()
				);

				let (r, _, b) = light.grade(RED, plane).rgb();
				assert!(
					r > b,
					"{} pushes the red past red on {plane:?}",
					light.name()
				);
			}
		}
	}

	#[test]
	fn ink_stays_ink_under_every_light() {
		for light in lights() {
			for plane in [Plane::Figure, NEAR] {
				for line in [palette::INK, PUPPET_LINE] {
					let graded = light.grade(line, plane).oklab();
					assert!(
						graded.chroma() < 0.06,
						"{} tints the ink {line} to {graded:?}",
						light.name()
					);
					assert!(graded.l < 0.3, "{} lifts the ink {line}", light.name());
				}
			}
		}
	}

	/// Warmth is chroma along the colour's own hue: a face that loses it goes grey, one that
	/// turns away from it goes green or blue. The floors are [`SKIN_KEEP`] of the chroma (less
	/// the cosine of [`SKIN_TURN`]) and [`SKIN_VALUE`] of the lightness, give or take rounding to
	/// eight-bit channels (0.003 in chroma, 0.01 in lightness).
	#[test]
	fn skin_keeps_its_warmth_and_light_under_every_light() {
		for light in lights() {
			for skin in [PALE_SKIN, DEEP_SKIN, Color(0xD9_9A_6C)] {
				let before = skin.oklab();
				let graded = light.grade(skin, Plane::Figure);
				let after = graded.oklab();
				let warmth = after.chroma() * (after.hue() - before.hue()).to_radians().cos();
				assert!(
					warmth >= before.chroma() * SKIN_KEEP * SKIN_TURN.to_radians().cos() - 0.003,
					"{} drains {skin} to {graded}: warmth {warmth} of {}",
					light.name(),
					before.chroma()
				);
				assert!(
					after.l >= before.l * SKIN_VALUE - 0.01,
					"{} darkens {skin} to {graded}",
					light.name()
				);
				let (r, g, b) = graded.rgb();
				assert!(r > g && g > b, "{} turns {skin} to {graded}", light.name());
			}
		}
	}

	/// A coloured local colour keeps more of its chroma on a figure than in the setting, near or
	/// far, and the setting loses more with depth. (A pale colour under a strongly coloured
	/// light is left out: it takes on the light's own colour, and the setting takes more of
	/// that. The red is left out of the depth step: it sits on the edge of what sRGB can show,
	/// so its chroma there is the gamut's, not the grade's.)
	#[test]
	fn a_figure_keeps_more_colour_than_the_setting_behind_it() {
		for light in lights() {
			for colour in [NAVY, RED, GREEN, DEEP_SKIN] {
				let figure = light.grade(colour, Plane::Figure).oklab().chroma();
				let near = light.grade(colour, NEAR).oklab().chroma();
				let far = light.grade(colour, Plane::Ground(1.0)).oklab().chroma();
				assert!(
					figure > near && figure > far && (colour == RED || near > far),
					"{} {colour}: figure {figure}, near {near}, far {far}",
					light.name()
				);
			}
		}
	}

	/// At night the setting carries the darkness, and a face stays readable: the lit skin
	/// keeps a mid value well clear of the ink its eyes and mouth are drawn in.
	#[test]
	fn a_face_reads_at_night() {
		for preset in [Preset::Night, Preset::Moonlit] {
			let light = Light::of(preset);
			let face = light.grade(PALE_SKIN, Plane::Figure).oklab().l;
			let ink = light.grade(palette::INK, Plane::Figure).oklab().l;
			let lawn = light.grade(GREEN, NEAR).oklab().l;
			assert!(face > 0.6, "{} face {face}", light.name());
			assert!(
				face - ink > 0.4,
				"{} face {face} on ink {ink}",
				light.name()
			);
			assert!(
				lawn < GREEN.oklab().l * 0.7,
				"{} lawn {lawn} carries the dark",
				light.name()
			);
		}
	}

	#[test]
	fn a_cel_shadow_keeps_its_value_and_leans_to_the_colour_of_shadow() {
		let lilac = Color(0xC4_B2_DC);

		for light in lights() {
			let shade = light.shade(lilac, Plane::Figure);
			assert!(
				(shade.oklab().l - lilac.oklab().l).abs() < 0.01,
				"{} darkens the shadow tone twice: {shade}",
				light.name()
			);
			assert!(turned(lilac, shade) <= SHADE_TURN + 1.0, "{shade}");
		}
	}

	#[test]
	fn the_rim_is_the_part_s_own_colour_lit() {
		for light in lights().filter(|light| light.rim) {
			let rim = light.rim_on(NAVY);
			let (r, _, b) = rim.rgb();
			assert!(b > r, "{} rims navy in {rim}, not a blue", light.name());
			assert!(rim.oklab().l > NAVY.oklab().l + 0.3, "the rim is light");
			assert!(turned(NAVY, rim) <= RIM_TURN + 1.0);
		}
	}

	#[test]
	fn cloud_covers_the_sun_under_overcast_and_storm() {
		assert_eq!(Light::of(Preset::Overcast).body, None);
		assert_eq!(Light::of(Preset::Storm).body, None);
		assert_eq!(Light::of(Preset::Golden).body, Some(Body::Sun));
		assert_eq!(Light::of(Preset::Moonlit).body, Some(Body::Moon));
	}

	#[test]
	fn overcast_lowers_saturation() {
		let spread = |colour: Color| {
			let (r, g, b) = colour.rgb();
			let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
			r.max(g).max(b) - r.min(g).min(b)
		};
		let plain = spread(palette::TOMATO);
		let grey_day = spread(Light::of(Preset::Overcast).grade(palette::TOMATO, NEAR));
		assert!(
			grey_day < plain,
			"overcast desaturates: {plain} -> {grey_day}"
		);
	}

	#[test]
	fn grade_stays_in_range_and_round_trips_extremes() {
		for light in Light::ALL {
			for colour in [Color(0x00_00_00), Color(0xFF_FF_FF), Color(0x0F_0F_0F)] {
				for plane in [Plane::Figure, NEAR, Plane::Ground(1.0)] {
					let graded = light.grade(colour, plane);
					assert!(graded.0 <= 0xFF_FF_FF, "{graded}");
				}
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
		assert!((middle.exposure - f32::midpoint(a.exposure, b.exposure)).abs() < 1e-5);
		assert!((middle.figure - f32::midpoint(a.figure, b.figure)).abs() < 1e-5);
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

		// The grade's new dials count.
		let lifted = Light {
			figure: golden.figure + 0.1,
			..golden
		};
		assert_ne!(lifted.cache_key(), golden.cache_key());
	}
}
