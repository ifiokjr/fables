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
use super::figure::Build;
use super::parts::Finding;
use super::parts::Level;
use super::parts::Parts;
use super::puppet::Proportions;

/// The style a world draws in unless its `world.toml` names another.
pub const DEFAULT: &str = "cartoon";

/// The packs compiled into fables: name, `style.toml`, `kit.svg` and `template.svg`.
const BUILT_IN: [(&str, &str, &str, &str); 1] = [(
	"cartoon",
	include_str!("../../styles/cartoon/style.toml"),
	include_str!("../../styles/cartoon/kit.svg"),
	include_str!("../../styles/cartoon/template.svg"),
)];

/// A loaded style pack.
#[derive(Debug)]
pub struct StylePack {
	pub name: String,
	/// One sentence on the look, for people and for agents choosing a style.
	pub description: String,
	pub finish: Finish,
	pub face: Face,
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
