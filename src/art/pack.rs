//! Style packs: a way of drawing characters, kept as data instead of constants.
//!
//! A style pack is a folder under `styles/`:
//!
//! ```text
//! styles/cartoon/
//!   style.toml      the finish (line weights, light, cel shadow), the face cylinder, the kit's
//!                   shared inks and the bodies characters are built on (kid, grown-up)
//!   kit.svg         parts every character in the style starts with: hands, eyes, mouths, props
//!   template.svg    a plain character's own parts, which new characters are started from
//! ```
//!
//! An authored puppet ([`puppet`](super::puppet)) draws its part file over its style's kit and
//! reads every number from its style, so a new look is a new folder rather than new code. The
//! built-in packs are compiled into fables, so a world anywhere on disk can name one in its
//! `world.toml`.
//!
//! These are not the six skins in [`style`](super::style) (riso, tv, bold…), which restyle
//! the older parametric puppet and the settings.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;

use super::Color;
use super::Pt;
use super::dials::Line;
use super::dials::Motion;
use super::dials::Shade;
use super::figure::Build;
use super::parts::Finding;
use super::parts::Level;
use super::parts::Parts;
use super::puppet::Proportions;

/// The style a world draws in unless its `world.toml` names another.
pub const DEFAULT: &str = "cartoon";

/// The packs compiled into fables: name, `style.toml`, `kit.svg` and `template.svg`.
const BUILT_IN: [(&str, &str, &str, &str); 11] = [
	(
		"cartoon",
		include_str!("../../styles/cartoon/style.toml"),
		include_str!("../../styles/cartoon/kit.svg"),
		include_str!("../../styles/cartoon/template.svg"),
	),
	(
		"anime",
		include_str!("../../styles/anime/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
	),
	// Every manga style is anime anatomy, so they all share the anime kit and template.
	(
		"manga",
		include_str!("../../styles/manga/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
	),
	(
		"manga-colour",
		include_str!("../../styles/manga-colour/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
	),
	(
		"manga-shojo",
		include_str!("../../styles/manga-shojo/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
	),
	(
		"manga-shonen",
		include_str!("../../styles/manga-shonen/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
	),
	(
		"rubber-hose",
		include_str!("../../styles/rubber-hose/style.toml"),
		include_str!("../../styles/rubber-hose/kit.svg"),
		include_str!("../../styles/rubber-hose/template.svg"),
	),
	// The same characters in a 1930s print, so it shares the rubber-hose kit and template.
	(
		"rubber-hose-1930s",
		include_str!("../../styles/rubber-hose-1930s/style.toml"),
		include_str!("../../styles/rubber-hose/kit.svg"),
		include_str!("../../styles/rubber-hose/template.svg"),
	),
	// Clear line draws its simple characters with the cartoon kit's hands, mouths and props.
	(
		"clear-line",
		include_str!("../../styles/clear-line/style.toml"),
		include_str!("../../styles/cartoon/kit.svg"),
		include_str!("../../styles/clear-line/template.svg"),
	),
	(
		"mid-century",
		include_str!("../../styles/mid-century/style.toml"),
		include_str!("../../styles/mid-century/kit.svg"),
		include_str!("../../styles/mid-century/template.svg"),
	),
	(
		"chibi",
		include_str!("../../styles/chibi/style.toml"),
		include_str!("../../styles/chibi/kit.svg"),
		include_str!("../../styles/chibi/template.svg"),
	),
];

/// A loaded style pack.
#[derive(Debug)]
pub struct StylePack {
	pub name: String,
	/// One sentence on the look, for people and for agents choosing a style.
	pub description: String,
	pub finish: Finish,
	pub face: Face,
	pub features: Features,
	pub eyes: Eyes,
	pub line: Line,
	pub shade: Shade,
	pub motion: Motion,
	pub inks: KitInks,
	bodies: BTreeMap<String, Proportions>,
	/// Parts every character in the style starts with.
	pub kit: Parts,
	/// A plain character's own parts, drawn over the kit.
	pub template: Parts,
}

/// How ink goes on paper.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Finish {
	/// The silhouette's line weight, in figure units.
	pub line: f32,
	/// Interior lines: brows, creases, stripes.
	pub detail: f32,
	/// Where the light comes from, as the offset that moves a part towards it.
	pub light: Pt,
	/// The one tone of shadow multiplied over the fills.
	pub cel_shadow: Color,
	/// What limbs on the far side of the body are pushed back towards.
	pub far: Color,
}

/// The cylinder face features are wrapped round as the head turns.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Face {
	/// How far the three-quarter view turns the face, in radians.
	pub turn: f32,
	/// The cylinder's radius, in head units.
	pub radius: f32,
	/// How far the face slides sideways per unit of facing, so reaching hands find the nose.
	pub slide: f32,
}

/// Where the face's features sit, in head units (the origin between the eyes, y down).
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Features {
	/// The page-right eye's centre; the other eye mirrors it.
	pub eye: Pt,
	/// The page-right brow's centre, at rest.
	pub brow: Pt,
	pub brow_weight: f32,
	/// The page-right cheek's blush: centre and half size.
	pub blush: Pt,
	pub blush_size: Pt,
	/// Where the kit's nose is drawn, and how big.
	pub nose: Pt,
	pub nose_scale: f32,
	/// A glint on the nose.
	pub nose_shine: bool,
	/// The middle of the mouth, and how big the kit's mouth chart is drawn.
	pub mouth: Pt,
	pub mouth_scale: f32,
	/// The nose's colour, if not the skin's own (a rubber-hose button nose is black).
	#[serde(default)]
	pub nose_colour: Option<Color>,
	/// Gloves: hands are this colour whatever the skin, with the kit's `glove-cuff`.
	#[serde(default)]
	pub gloves: Option<Color>,
}

/// How an eye is built: a white, an iris (with an optional darker band, pupil and rim),
/// highlights, and lids whose lash line is the eye's heaviest stroke.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Eyes {
	/// The white's half width and height.
	pub size: Pt,
	/// The iris's half width and height.
	pub iris: Pt,
	/// How far the iris colour is darkened: 0 is the cast's eye colour, 1 near black.
	pub iris_dark: f32,
	/// How much of the iris, from the top, is shaded darker (0 for none).
	pub iris_band: f32,
	/// A dark pupil inside the iris: half width and height (0 for none).
	pub pupil: Pt,
	/// A dark rim round the iris: its line weight (0 for none).
	pub rim: f32,
	/// Two highlights: offset from the iris centre and radius, `[x, y, r]`.
	pub shines: [[f32; 3]; 2],
	/// How far the iris moves when the eyes look across and up or down.
	pub gaze: Pt,
	/// The upper lash line's weight, and the lower lid's line in a smile or squint.
	pub lash: f32,
	pub lower_lid: f32,
	/// A short lower lash line under the outer half of each eye (0 for none).
	pub lower_lash: f32,
	/// Whether every character wears the kit's lash parts, not only those with the `lashes`
	/// extra.
	pub lashes: bool,
	/// A wedge cut out of each pupil, the way early animated cartoons drew eyes.
	#[serde(default)]
	pub pie_cut: bool,
}

/// Inks the kit's parts share; a character's own colours come from the cast.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct KitInks {
	/// Cheeks on warm skin (cool skin is shaded plum instead).
	pub blush: Color,
	pub eye: Color,
	/// Inside an open mouth.
	pub maw: Color,
	pub tongue: Color,
	pub sock: Color,
	pub sole: Color,
	pub clean_teeth: Color,
	pub jam: Color,
	pub cream: Color,
	pub brush: Color,
	pub bristles: Color,
	pub paste: Color,
	pub kraft: Color,
	pub string: Color,
	pub lens: Color,
	pub bogey: Color,
	pub cap: Color,
	pub badge: Color,
	pub satchel: Color,
	pub apron: Color,
	pub mud: Color,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Entry {
	name: String,
	description: String,
	finish: Finish,
	face: Face,
	features: Features,
	eyes: Eyes,
	#[serde(default)]
	line: Line,
	#[serde(default)]
	shade: Shade,
	#[serde(default)]
	motion: Motion,
	inks: KitInks,
	bodies: BTreeMap<String, Proportions>,
}

/// The body a build is drawn on.
fn body_name(build: Build) -> &'static str {
	match build {
		Build::Kid => "kid",
		Build::Grownup => "grown-up",
	}
}

static LOADED: LazyLock<Mutex<BTreeMap<String, Arc<StylePack>>>> =
	LazyLock::new(|| Mutex::new(BTreeMap::new()));

impl StylePack {
	/// The names of the built-in packs.
	pub fn names() -> impl Iterator<Item = &'static str> {
		BUILT_IN.iter().map(|(name, ..)| *name)
	}

	/// A built-in pack by name, read once and shared.
	pub fn named(name: &str) -> Result<Arc<StylePack>> {
		let mut loaded = LOADED
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);

		if let Some(pack) = loaded.get(name) {
			return Ok(Arc::clone(pack));
		}

		let Some((_, style, kit, template)) = BUILT_IN.iter().find(|(key, ..)| *key == name) else {
			bail!(
				"unknown drawing style `{name}`; use one of: {}",
				Self::names().collect::<Vec<_>>().join(", ")
			);
		};
		let pack = Arc::new(
			Self::parse(style, kit, template).with_context(|| format!("in styles/{name}"))?,
		);
		loaded.insert(name.to_owned(), Arc::clone(&pack));

		Ok(pack)
	}

	/// The default pack, [`DEFAULT`].
	pub fn default_pack() -> Arc<StylePack> {
		Self::named(DEFAULT)
			.unwrap_or_else(|error| panic!("the built-in {DEFAULT} style: {error:#}"))
	}

	/// Reads a pack from its `style.toml`, `kit.svg` and `template.svg`.
	pub fn parse(style: &str, kit: &str, template: &str) -> Result<StylePack> {
		let entry: Entry = toml::from_str(style).context("style.toml is not valid")?;

		for build in [Build::Kid, Build::Grownup] {
			let name = body_name(build);
			ensure!(
				entry.bodies.contains_key(name),
				"style.toml has no [bodies.{name}]"
			);
		}

		Ok(StylePack {
			name: entry.name,
			description: entry.description,
			finish: entry.finish,
			face: entry.face,
			features: entry.features,
			eyes: entry.eyes,
			line: entry.line,
			shade: entry.shade,
			motion: entry.motion,
			inks: entry.inks,
			bodies: entry.bodies,
			kit: Parts::parse(kit).context("in kit.svg")?,
			template: Parts::parse(template).context("in template.svg")?,
		})
	}

	/// The body a build is drawn on (parsing made sure every build has one).
	pub fn body(&self, build: Build) -> &Proportions {
		&self.bodies[body_name(build)]
	}

	/// The named bodies, such as `kid` and `grown-up`.
	pub fn bodies(&self) -> impl Iterator<Item = (&str, &Proportions)> {
		self.bodies.iter().map(|(name, body)| (name.as_str(), body))
	}

	/// The template character over the kit: a whole figure, ready to redraw.
	pub fn template_figure(&self) -> Parts {
		self.template.clone().over(self.kit.clone())
	}

	/// Checks the pack: the template over the kit must draw a whole figure, and every body must
	/// stand up (feet below the hips, the neck above the shoulders, positive bones).
	pub fn check(&self) -> Vec<Finding> {
		let mut findings = self.template_figure().check();

		for (name, body) in self.bodies() {
			let limbs = body.limbs;
			let mut problem = |message: String| {
				findings.push(Finding {
					level: Level::Error,
					part: format!("bodies.{name}"),
					message,
				});
			};

			for (bone, length) in [
				("thigh", limbs.thigh),
				("shin", limbs.shin),
				("upper-arm", limbs.upper_arm),
				("forearm", limbs.forearm),
			] {
				if length <= 0.0 {
					problem(format!("the {bone} must be longer than zero"));
				}
			}

			if !(limbs.pelvis_y < limbs.ankle_y && limbs.ankle_y < 0.0) {
				problem("the pelvis must be above the ankles, and both above the soles".into());
			}

			if !(body.neck.y < body.shoulder_y && body.shoulder_y < limbs.pelvis_y) {
				problem("the neck must be above the shoulders, and both above the pelvis".into());
			}

			for message in body.problems() {
				problem(message);
			}
		}

		findings.sort_by_key(|finding| finding.level);
		findings
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_built_in_pack_loads_and_passes_its_check() {
		for name in StylePack::names() {
			let pack = StylePack::named(name).unwrap();
			let errors: Vec<String> = pack
				.check()
				.iter()
				.filter(|finding| finding.level == Level::Error)
				.map(ToString::to_string)
				.collect();
			assert!(errors.is_empty(), "{name}: {errors:#?}");
		}
	}

	#[test]
	fn a_grown_up_is_taller_than_a_kid_in_the_default_style() {
		let pack = StylePack::default_pack();
		assert!(pack.body(Build::Grownup).neck.y < pack.body(Build::Kid).neck.y * 2.0);
	}

	#[test]
	fn an_unknown_style_lists_the_known_ones() {
		let error = format!("{:#}", StylePack::named("vaporwave").unwrap_err());
		assert!(error.contains("cartoon"), "{error}");
	}

	/// The cartoon style with `dial` added to its kid's body.
	fn with_kid_dial(dial: &str) -> Result<StylePack> {
		let source = include_str!("../../styles/cartoon/style.toml");
		let at = source
			.find("[bodies.kid.limbs]")
			.expect("the cartoon style has a kid");
		let source = format!("{}{dial}\n\n{}", &source[..at], &source[at..]);
		StylePack::parse(&source, "<svg/>", "<svg/>")
	}

	#[test]
	fn construction_dials_are_read_and_bad_ones_are_reported() {
		let pack = with_kid_dial(
			"leg-profile = [40.0, 36.0, 28.0, 24.0, 30.0, 22.0, 16.0]\nfoot-scale = 1.2\nfloor = \
			 true",
		)
		.unwrap();
		let kid = pack.body(Build::Kid);
		assert_eq!(kid.leg_profile.map(|widths| widths[4]), Some(30.0));
		assert!((kid.foot_scale - 1.2).abs() < f32::EPSILON && kid.floor);

		// A profile needs all seven stations; the error says where and what it wants.
		let error = format!(
			"{:#}",
			with_kid_dial("arm-profile = [10.0, 9.0]").unwrap_err()
		);
		assert!(
			error.contains("arm-profile") && error.contains("length 7"),
			"{error}"
		);

		let error = format!("{:#}", with_kid_dial("knee-bend = 0.5").unwrap_err());
		assert!(error.contains("knee-bend"), "{error}");

		// Values that parse but can't draw are errors in the style's check.
		let pack = with_kid_dial(
			"leg-profile = [40.0, 36.0, 0.0, 24.0, 30.0, 22.0, 16.0]\nknee-splay = 1.5",
		)
		.unwrap();
		let errors: Vec<String> = pack
			.check()
			.iter()
			.filter(|finding| finding.level == Level::Error && finding.part == "bodies.kid")
			.map(|finding| finding.message.clone())
			.collect();
		assert!(errors.iter().any(|message| message.contains("leg-profile")));
		assert!(errors.iter().any(|message| message.contains("knee-splay")));
	}

	#[test]
	fn a_style_needs_a_body_for_every_build() {
		let source = include_str!("../../styles/cartoon/style.toml");
		let cut = source
			.find("# A grown-up")
			.expect("the cartoon style has a grown-up");
		let error = StylePack::parse(&source[..cut], "<svg/>", "<svg/>").unwrap_err();
		assert!(
			format!("{error:#}").contains("bodies.grown-up"),
			"{error:#}"
		);
	}
}
