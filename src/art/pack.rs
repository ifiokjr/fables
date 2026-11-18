//! Style packs: a way of drawing characters, kept as data instead of constants.
//!
//! A style pack is a folder under `styles/`:
//!
//! ```text
//! styles/cartoon/
//!   style.toml      the finish (line weights, light, cel shadow), the face cylinder, the kit's
//!                   shared inks, the bodies characters are built on (kid, grown-up), and the
//!                   page skin that suits the look
//!   kit.svg         parts every character in the style starts with: hands, eyes, mouths, props
//!   template.svg    a plain kid's own parts, which new kids are drawn from
//!   template-grown-up.svg
//!                   a plain grown-up's own parts, cut for the grown-up body
//! ```
//!
//! An authored puppet ([`puppet`](super::puppet)) draws its part file over its style's kit and
//! reads every number from its style, so a new look is a new folder rather than new code. A
//! character with no part file is drawn from the template for their body, so a new world needs
//! no drawing at all to render. The built-in packs are compiled into fables, so a world anywhere
//! on disk can name one in its `world.toml`.
//!
//! These are not the page skins in [`style`](super::style) (riso, tv, bold…), which print the
//! page; a style pack names the skin that suits it in `style.toml`.

use std::collections::BTreeMap;
use std::path::PathBuf;
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
use super::parts::Feature;
use super::parts::Finding;
use super::parts::Level;
use super::parts::Parts;
use super::parts::is_variant_name;
use super::puppet;
use super::puppet::Authored;
use super::puppet::Physique;
use super::puppet::Proportions;

/// The style a world draws in unless its `world.toml` names another.
pub const DEFAULT: &str = "cartoon";

/// The packs compiled into fables: name, `style.toml`, `kit.svg`, `template.svg` and
/// `template-grown-up.svg`.
const BUILT_IN: [(&str, &str, &str, &str, &str); 11] = [
	(
		"cartoon",
		include_str!("../../styles/cartoon/style.toml"),
		include_str!("../../styles/cartoon/kit.svg"),
		include_str!("../../styles/cartoon/template.svg"),
		include_str!("../../styles/cartoon/template-grown-up.svg"),
	),
	(
		"anime",
		include_str!("../../styles/anime/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
		include_str!("../../styles/anime/template-grown-up.svg"),
	),
	// Every manga style is anime anatomy, so they all share the anime kit and template.
	(
		"manga",
		include_str!("../../styles/manga/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
		include_str!("../../styles/anime/template-grown-up.svg"),
	),
	(
		"manga-colour",
		include_str!("../../styles/manga-colour/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
		include_str!("../../styles/anime/template-grown-up.svg"),
	),
	(
		"manga-shojo",
		include_str!("../../styles/manga-shojo/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
		include_str!("../../styles/anime/template-grown-up.svg"),
	),
	(
		"manga-shonen",
		include_str!("../../styles/manga-shonen/style.toml"),
		include_str!("../../styles/anime/kit.svg"),
		include_str!("../../styles/anime/template.svg"),
		include_str!("../../styles/anime/template-grown-up.svg"),
	),
	(
		"rubber-hose",
		include_str!("../../styles/rubber-hose/style.toml"),
		include_str!("../../styles/rubber-hose/kit.svg"),
		include_str!("../../styles/rubber-hose/template.svg"),
		include_str!("../../styles/rubber-hose/template-grown-up.svg"),
	),
	// The same characters in a 1930s print, so it shares the rubber-hose kit and template.
	(
		"rubber-hose-1930s",
		include_str!("../../styles/rubber-hose-1930s/style.toml"),
		include_str!("../../styles/rubber-hose/kit.svg"),
		include_str!("../../styles/rubber-hose/template.svg"),
		include_str!("../../styles/rubber-hose/template-grown-up.svg"),
	),
	// Clear line draws its simple characters with the cartoon kit's hands, mouths and props.
	(
		"clear-line",
		include_str!("../../styles/clear-line/style.toml"),
		include_str!("../../styles/cartoon/kit.svg"),
		include_str!("../../styles/clear-line/template.svg"),
		include_str!("../../styles/clear-line/template-grown-up.svg"),
	),
	(
		"mid-century",
		include_str!("../../styles/mid-century/style.toml"),
		include_str!("../../styles/mid-century/kit.svg"),
		include_str!("../../styles/mid-century/template.svg"),
		include_str!("../../styles/mid-century/template-grown-up.svg"),
	),
	(
		"chibi",
		include_str!("../../styles/chibi/style.toml"),
		include_str!("../../styles/chibi/kit.svg"),
		include_str!("../../styles/chibi/template.svg"),
		include_str!("../../styles/chibi/template-grown-up.svg"),
	),
];

/// A loaded style pack.
#[derive(Debug)]
pub struct StylePack {
	pub name: String,
	/// One sentence on the look, for people and for agents choosing a style.
	pub description: String,
	/// The page skin that suits the look (riso, clear, tv…): the skin a book drawn in this
	/// style prints in unless the script or `--style` names another.
	pub skin: String,
	pub finish: Finish,
	pub face: Face,
	pub features: Features,
	pub eyes: Eyes,
	pub line: Line,
	pub shade: Shade,
	pub motion: Motion,
	pub inks: KitInks,
	bodies: BTreeMap<String, Proportions>,
	/// The names a cast asks for the templates' own hair and the kit's own brows and nose by.
	own: Own,
	/// Parts every character in the style starts with.
	pub kit: Parts,
	/// A plain kid's own parts, drawn over the kit.
	pub template: Parts,
	/// A plain grown-up's own parts: a torso, sleeves, legwear and whatever else the style
	/// draws differently on a grown-up, cut for the grown-up body.
	grown_up: Parts,
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

/// What the templates and kit draw of their own, by the names a cast asks for them with. Their
/// other ways of drawing the hair, brows and nose are variants beside them (see [`Feature`]).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Own {
	kid_hair: String,
	grown_up_hair: String,
	brows: String,
	nose: String,
	/// Whether a cast can leave the nose out with `nose = "none"`. A style whose faces are
	/// built round the nose says no.
	#[serde(default = "yes")]
	noseless: bool,
}

fn yes() -> bool {
	true
}

/// The ways of drawing a figure's features a character asks for, by name. `None` keeps their
/// template's own (or their part file's).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picks {
	pub hair: Option<String>,
	pub brows: Option<String>,
	pub nose: Option<String>,
}

impl Picks {
	/// The name picked for a feature, if any.
	pub fn get(&self, feature: Feature) -> Option<&str> {
		match feature {
			Feature::Hair => self.hair.as_deref(),
			Feature::Brows => self.brows.as_deref(),
			Feature::Nose => self.nose.as_deref(),
		}
	}
}

/// The nose a character can ask for in any style, which leaves the nose out.
pub const NO_NOSE: &str = "none";

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Entry {
	name: String,
	description: String,
	/// The page skin the look prints in (see [`style`](super::style)).
	skin: String,
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
	own: Own,
}

/// The name of the body a build is drawn on, as `style.toml` and the cast file spell it.
pub fn body_name(build: Build) -> &'static str {
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

		let Some((_, style, kit, template, grown_up)) =
			BUILT_IN.iter().find(|(key, ..)| *key == name)
		else {
			bail!(
				"unknown drawing style `{name}`; use one of: {}",
				Self::names().collect::<Vec<_>>().join(", ")
			);
		};
		let pack = Arc::new(
			Self::parse(style, kit, template, grown_up)
				.with_context(|| format!("in styles/{name}"))?,
		);
		loaded.insert(name.to_owned(), Arc::clone(&pack));

		Ok(pack)
	}

	/// The default pack, [`DEFAULT`].
	pub fn default_pack() -> Arc<StylePack> {
		Self::named(DEFAULT)
			.unwrap_or_else(|error| panic!("the built-in {DEFAULT} style: {error:#}"))
	}

	/// Reads a pack from its `style.toml`, `kit.svg`, `template.svg` and
	/// `template-grown-up.svg`.
	pub fn parse(style: &str, kit: &str, template: &str, grown_up: &str) -> Result<StylePack> {
		let entry: Entry = toml::from_str(style).context("style.toml is not valid")?;

		for build in [Build::Kid, Build::Grownup] {
			let name = body_name(build);
			ensure!(
				entry.bodies.contains_key(name),
				"style.toml has no [bodies.{name}]"
			);
		}

		let skins: Vec<&str> = super::style::StyleName::ALL
			.iter()
			.map(|(name, _)| *name)
			.collect();
		ensure!(
			skins.contains(&entry.skin.as_str()),
			"style.toml's skin `{}` is not a page skin; use one of: {}",
			entry.skin,
			skins.join(", ")
		);

		let pack = StylePack {
			name: entry.name,
			description: entry.description,
			skin: entry.skin,
			finish: entry.finish,
			face: entry.face,
			features: entry.features,
			eyes: entry.eyes,
			line: entry.line,
			shade: entry.shade,
			motion: entry.motion,
			inks: entry.inks,
			bodies: entry.bodies,
			own: entry.own,
			kit: Parts::parse(kit).context("in kit.svg")?,
			template: Parts::parse(template).context("in template.svg")?,
			grown_up: Parts::parse(grown_up).context("in template-grown-up.svg")?,
		};

		for build in [Build::Kid, Build::Grownup] {
			for feature in Feature::ALL {
				let own = pack.own(build, feature);
				ensure!(
					is_variant_name(own) && own != NO_NOSE,
					"style.toml's [own] {} `{own}` must be lowercase words joined by hyphens, \
					 and not `{NO_NOSE}`",
					feature.name()
				);
				ensure!(
					!pack.variants(build, feature).contains(&own),
					"style.toml's [own] {} `{own}` is also drawn as a variant; name one of them \
					 differently",
					feature.name()
				);
			}
		}

		Ok(pack)
	}

	/// The body a build is drawn on (parsing made sure every build has one).
	pub fn body(&self, build: Build) -> &Proportions {
		&self.bodies[body_name(build)]
	}

	/// The named bodies, such as `kid` and `grown-up`.
	pub fn bodies(&self) -> impl Iterator<Item = (&str, &Proportions)> {
		self.bodies.iter().map(|(name, body)| (name.as_str(), body))
	}

	/// A plain character's own parts, drawn for a build's body: what `fables puppet template`
	/// starts a character from.
	pub fn template_for(&self, build: Build) -> Parts {
		self.drawn_for(build).clone()
	}

	fn drawn_for(&self, build: Build) -> &Parts {
		match build {
			Build::Kid => &self.template,
			Build::Grownup => &self.grown_up,
		}
	}

	/// The template character on a build's body, over the kit: a whole figure, ready to redraw.
	pub fn template_figure(&self, build: Build) -> Parts {
		self.template_for(build).over(self.kit.clone())
	}

	/// A template-drawn character's own parts, as a part file started from them should draw
	/// them: the template for a build's body with the `hair` they have (its own when `None`).
	pub fn template_with_hair(&self, build: Build, hair: Option<&str>) -> Result<Parts> {
		let mut parts = self.template_for(build);
		let picks = Picks {
			hair: hair.map(str::to_owned),
			..Picks::default()
		};
		self.pick(&mut parts, build, &picks)?;
		Ok(parts)
	}

	/// The template character on a build's body over the kit, drawn with the hair, brows and
	/// nose a character `picks` (see [`StylePack::pick`]).
	pub fn figure(&self, build: Build, picks: &Picks) -> Result<Parts> {
		let mut figure = self.template_figure(build);
		self.pick(&mut figure, build, picks)?;
		Ok(figure)
	}

	/// The name a cast asks for a feature as the template on a build's body (for the hair) or
	/// the kit (for the brows and nose) draws it of its own.
	pub fn own(&self, build: Build, feature: Feature) -> &str {
		match (feature, build) {
			(Feature::Hair, Build::Kid) => &self.own.kid_hair,
			(Feature::Hair, Build::Grownup) => &self.own.grown_up_hair,
			(Feature::Brows, _) => &self.own.brows,
			(Feature::Nose, _) => &self.own.nose,
		}
	}

	/// The variants of a feature the template on a build's body and the kit draw.
	fn variants(&self, build: Build, feature: Feature) -> Vec<&str> {
		let mut variants = self.drawn_for(build).variants(feature);
		variants.extend(self.kit.variants(feature));
		variants.sort_unstable();
		variants.dedup();
		variants
	}

	/// Every way the style draws a feature on a build's body, by the name a cast picks it with,
	/// in alphabetical order: its own, its variants, and `none` for a nose it lets a face go
	/// without.
	pub fn offers(&self, build: Build, feature: Feature) -> Vec<&str> {
		let mut offers = self.variants(build, feature);
		offers.push(self.own(build, feature));

		if feature == Feature::Nose && self.own.noseless {
			offers.push(NO_NOSE);
		}

		offers.sort_unstable();
		offers
	}

	/// Draws the features a character `picks` on a build's body into `parts`, a figure over
	/// this style's kit: a pick of the style's own (or no pick) leaves the feature as it is, a
	/// nose of `none` leaves the nose out, and any other pick swaps in that variant. A pick the
	/// style doesn't draw is an error naming the ones it does.
	pub fn pick(&self, parts: &mut Parts, build: Build, picks: &Picks) -> Result<()> {
		for feature in Feature::ALL {
			let Some(pick) = picks.get(feature) else {
				continue;
			};

			if pick == self.own(build, feature) {
				continue;
			}

			if feature == Feature::Nose && pick == NO_NOSE && self.own.noseless {
				parts.leave_out(feature);
				continue;
			}

			if !parts.variants(feature).contains(&pick) {
				let on = match feature {
					Feature::Hair => format!(" on a {}", body_name(build)),
					Feature::Brows | Feature::Nose => String::new(),
				};
				bail!(
					"the {} style draws no `{pick}` {}{on}; use one of: {}",
					self.name,
					feature.name(),
					self.offers(build, feature).join(", ")
				);
			}

			parts.pick(feature, pick)?;
		}

		Ok(())
	}

	/// Where a build's template is drawn in the style's folder: `template.svg` for a kid,
	/// `template-grown-up.svg` for a grown-up.
	pub fn template_file(&self, build: Build) -> PathBuf {
		let file = match build {
			Build::Kid => "template.svg",
			Build::Grownup => "template-grown-up.svg",
		};
		PathBuf::from(format!("styles/{}/{file}", self.name))
	}

	/// How a part file's title names the template it was started from: `the anime template`,
	/// or `the anime grown-up template`.
	pub fn template_origin(&self, build: Build) -> String {
		match build {
			Build::Kid => format!("the {} template", self.name),
			Build::Grownup => format!("the {} grown-up template", self.name),
		}
	}

	/// Every way the template on a build's body can be drawn that the check poses: as it is,
	/// with each other way of drawing each feature, and built slim and stout. Each comes with
	/// how a finding names it.
	fn variety(&self, build: Build) -> Vec<(String, Picks, Physique)> {
		let mut variety = vec![(String::new(), Picks::default(), Physique::Plain)];

		for feature in Feature::ALL {
			for offer in self.offers(build, feature) {
				if offer == self.own(build, feature) {
					continue;
				}

				let pick = Some(offer.to_owned());
				let picks = match feature {
					Feature::Hair => Picks { hair: pick, ..Picks::default() },
					Feature::Brows => Picks { brows: pick, ..Picks::default() },
					Feature::Nose => Picks { nose: pick, ..Picks::default() },
				};
				variety.push((
					format!(" ({} {offer})", feature.name()),
					picks,
					Physique::Plain,
				));
			}
		}

		for physique in [Physique::Slim, Physique::Stout] {
			variety.push((
				format!(" (build {})", physique.name()),
				Picks::default(),
				physique,
			));
		}

		variety
	}

	/// Checks the pack: the template over the kit must draw a whole figure on every body, every
	/// body must stand up (feet below the hips, the neck above the shoulders, positive bones),
	/// and the template's parts must join up on every body (see [`puppet::joins`]). So must
	/// every hair, brows and nose the style offers, and every build.
	pub fn check(self: &Arc<Self>) -> Vec<Finding> {
		let mut findings = Vec::new();

		for build in [Build::Kid, Build::Grownup] {
			let name = body_name(build);
			let file = PathBuf::from(format!("styles/{}/{name} template", self.name));

			for (drawn, picks, physique) in self.variety(build) {
				let figure = match self.figure(build, &picks) {
					Ok(figure) => figure,
					Err(error) => {
						findings.push(Finding {
							level: Level::Error,
							part: format!("{name} template{drawn}"),
							message: format!("{error:#}"),
						});
						continue;
					}
				};
				let mut found = figure.check();

				// A template that can't draw a whole figure (reported above) can't be posed.
				if let Ok(template) = Authored::from_parts(&file, figure, self) {
					found.extend(puppet::joins(&template.built(build, physique), build));
				}

				findings.extend(found.into_iter().map(|finding| {
					Finding {
						part: format!("{name} template{drawn}: {}", finding.part),
						..finding
					}
				}));
			}
		}

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
		StylePack::parse(&source, "<svg/>", "<svg/>", "<svg/>")
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
		let pack = Arc::new(
			with_kid_dial(
				"leg-profile = [40.0, 36.0, 0.0, 24.0, 30.0, 22.0, 16.0]\nknee-splay = 1.5",
			)
			.unwrap(),
		);
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
		let error = StylePack::parse(&source[..cut], "<svg/>", "<svg/>", "<svg/>").unwrap_err();
		assert!(
			format!("{error:#}").contains("bodies.grown-up"),
			"{error:#}"
		);
	}
}
