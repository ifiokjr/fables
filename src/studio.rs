//! The studio: making worlds and characters as plain operations on folders, so a person at the
//! command line and an agent over MCP (see [`mcp`](crate::mcp)) do the same things the same way.
//!
//! Everything lives under one root folder, with each world in `worlds/<folder>/` (see
//! [`world`](crate::world)). Worlds, characters and parts are named by short slugs (`kerfuffle`,
//! `bogey`, `hair.front`), never by paths, so nothing outside the root is read or written.
//!
//! A character is drawn the way every Kerfuffle character was: start from the style's template,
//! redraw parts as SVG path data, look at the model sheet, fix, repeat. Writes that would leave
//! a puppet unable to draw a figure are refused with the part check's findings, so a world on
//! disk always loads.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use resvg::tiny_skia::FilterQuality;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::PixmapPaint;
use resvg::tiny_skia::Transform;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::art::pack::Face;
use crate::art::pack::Finish;
use crate::art::pack::StylePack;
use crate::art::parts;
use crate::art::parts::FIGURE_PARTS;
use crate::art::parts::Home;
use crate::art::parts::Level;
use crate::art::parts::Need;
use crate::art::parts::Parts;
use crate::art::parts::Views;
use crate::cast::Kind;
use crate::cast::Member;
use crate::press::sheets;
use crate::raster;
use crate::world;
use crate::world::World;

/// The folder under the root that holds the worlds.
pub const WORLDS: &str = "worlds";

/// A folder of worlds that agents and people work in.
#[derive(Clone, Debug)]
pub struct Studio {
	root: PathBuf,
}

/// A world in brief.
#[derive(Clone, Debug, Serialize)]
pub struct WorldSummary {
	/// The world's folder under `worlds/`, which every tool names it by.
	pub folder: String,
	pub name: String,
	pub style: String,
	pub about: Option<String>,
}

/// A world in full: who lives there and which books are set there.
#[derive(Clone, Debug, Serialize)]
pub struct WorldReport {
	#[serde(flatten)]
	pub summary: WorldSummary,
	/// The narrator's voice name, if the world has one.
	pub narrator: Option<String>,
	pub cast: Vec<MemberReport>,
	/// The books, by folder under `stories/`.
	pub stories: Vec<String>,
}

/// One cast member.
#[derive(Clone, Debug, Serialize)]
pub struct MemberReport {
	pub id: String,
	pub name: String,
	/// `kid`, `grown-up`, or an animal or thing (`dog`, `bird`, `worm`, …).
	pub kind: String,
	pub habit: Option<String>,
	pub about: Option<String>,
	/// The authored puppet's part file, relative to the world.
	pub puppet: Option<String>,
}

/// A new world.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct WorldBrief {
	/// The world's name, e.g. "Kerfuffle". Its folder is the name as a slug.
	pub name: String,
	/// A few sentences on the world: where it is, its rule, its tone.
	pub about: Option<String>,
	/// The drawing style (see `style_list`); the default is `cartoon`.
	pub style: Option<String>,
}

/// A body to build a character on.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildName {
	#[default]
	Kid,
	GrownUp,
}

impl BuildName {
	fn kind(self) -> &'static str {
		match self {
			BuildName::Kid => "kid",
			BuildName::GrownUp => "grown-up",
		}
	}
}

/// A character's colours: palette names (`tangerine`, `teal`…) or hex codes (`#ff8a3d`).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Colours {
	pub skin: String,
	pub hair: String,
	/// The top (shirt, jumper, dress).
	pub top: String,
	/// Trim on the top, and its pattern's colour.
	pub trim: String,
	/// Trousers, shorts or skirt.
	pub bottom: String,
	pub shoes: String,
	/// Iris colour.
	pub eyes: String,
}

/// A new character: who they are and their colours. Their drawing starts as the style's
/// template, to redraw part by part.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CharacterBrief {
	/// The cast id scripts use after `@`: lowercase letters, digits and hyphens.
	pub id: String,
	/// Their full name, e.g. "Nell Nibbles".
	pub name: String,
	#[serde(default)]
	pub build: BuildName,
	/// The name they were born with, before their habit gave them a new one.
	pub born: Option<String>,
	/// Who they are, in a sentence.
	pub about: Option<String>,
	/// The habit their tale is about, in a sentence.
	pub habit: Option<String>,
	pub colours: Colours,
	/// The pattern on the top, in the trim colour: plain, stripes, dots or star.
	pub pattern: Option<String>,
	/// Extras from the kit, e.g. glasses, freckles, lashes, bow.
	#[serde(default)]
	pub extras: Vec<String>,
}

/// One part as SVG path data.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct PartData {
	/// The part's name, e.g. `head.front`, `ear`, `hair-lock-1.3q`.
	pub name: String,
	/// SVG path data for one outline (`M`, `L`, `Q`, `C`, `Z`; absolute or relative), in the
	/// part's own space (see `style_describe`).
	pub d: String,
}

/// A part read back, and where it came from.
#[derive(Clone, Debug, Serialize)]
pub struct PartReport {
	pub name: String,
	pub d: String,
	/// `own` (the character's file) or `kit` (the style's shared parts).
	pub from: &'static str,
}

/// What a style offers a character designer.
#[derive(Clone, Debug, Serialize)]
pub struct StyleReport {
	pub name: String,
	pub description: String,
	pub finish: Finish,
	pub face: Face,
	pub bodies: Vec<BodyReport>,
	/// Every part a figure is drawn with, and the space it is drawn in.
	pub parts: Vec<PartSpecReport>,
	/// How to draw parts.
	pub rules: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BodyReport {
	pub name: String,
	/// Where the neck sits, in figure units (the soles at y = 0, up is negative).
	pub neck_y: f32,
	pub shoulder_y: f32,
	pub pelvis_y: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartSpecReport {
	pub name: String,
	/// `once` (drawn as `name`) or `turning` (drawn as `name.front` and `name.3q`).
	pub views: &'static str,
	/// `required`, `optional`, or `numbered` (any number of copies: `name-1`, `name-2`…).
	pub need: &'static str,
	/// `character` (each character draws their own) or `kit` (shared; a character may redraw it).
	pub home: &'static str,
	/// The coordinate space: where the origin is and which way is down.
	pub space: &'static str,
}

/// An image, as PNG.
#[derive(Clone, Debug)]
pub struct Image {
	pub label: String,
	pub png: Vec<u8>,
}

/// Sheets a character can be shown on.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SheetKind {
	/// Five beats (moods, poses, a talking mouth), each front-on and three-quarters.
	Model,
	/// Every part, each turning part front-on, half-turned and three-quarters.
	Parts,
	/// All fifteen moods.
	Moods,
}

/// How wide sheets are sent back, in pixels: enough to judge a drawing, small enough to look
/// at often.
pub const SHEET_PX: u32 = 1400;

/// The rules for drawing parts, for agents.
const RULES: [&str; 8] = [
	"Each part is one closed outline (or an open line for creases and strands), as SVG path data.",
	"Turning parts have two drawings, `name.front` and `name.3q`, with the same number of curve \
	 segments in the same order, so the turn can blend between them. Three-quarters faces the \
	 page right.",
	"Head units: the origin is between the eyes, the head is about 196 across, y grows down.",
	"Figure units: the soles are at y = 0 and up is negative; a kid is about 500 tall.",
	"Bone units: the joint is at the origin and the bone runs down +y.",
	"Hands: the wrist is at the origin, fingers down +y, the thumb on the -x side.",
	"Fills come from the cast colours; parts are outlines only. Keep shapes simple and round: a \
	 few smooth curves read better than many small ones.",
	"Draw a part as the character would look in the style: compare with the template's parts \
	 (`character_parts` with names) before redrawing.",
];

impl Studio {
	pub fn new(root: &Path) -> Studio {
		Studio {
			root: root.to_path_buf(),
		}
	}

	fn worlds_dir(&self) -> PathBuf {
		self.root.join(WORLDS)
	}

	/// Every world under the root.
	pub fn worlds(&self) -> Result<Vec<WorldSummary>> {
		let mut found = Vec::new();

		for entry in std::fs::read_dir(self.worlds_dir()).into_iter().flatten() {
			let path = entry?.path();

			if path.join(world::FILE).is_file() {
				found.push(summary(&World::load(&path)?));
			}
		}

		found.sort_by(|a, b| a.folder.cmp(&b.folder));
		Ok(found)
	}

	/// A world by its folder.
	pub fn world(&self, folder: &str) -> Result<World> {
		check_slug("world", folder)?;
		let dir = self.worlds_dir().join(folder);
		ensure!(
			dir.join(world::FILE).is_file(),
			"there is no world `{folder}`; `world_list` shows the worlds"
		);
		World::load(&dir)
	}

	/// Starts a world: a `world.toml`, an empty cast and empty `puppets/` and `stories/`.
	pub fn create_world(&self, brief: &WorldBrief) -> Result<WorldSummary> {
		let folder = crate::story::slugify(&brief.name);
		ensure!(
			!folder.is_empty(),
			"a world needs a name with letters in it"
		);
		let style = brief.style.as_deref().unwrap_or(crate::art::pack::DEFAULT);
		StylePack::named(style)?;
		let dir = self.worlds_dir().join(&folder);
		ensure!(!dir.exists(), "a world already lives in worlds/{folder}");

		let mut file = format!(
			"name = {}\nstyle = {}\n",
			quoted(&brief.name),
			quoted(style)
		);

		if let Some(about) = &brief.about {
			file = format!("{file}about = {}\n", quoted(about));
		}

		for sub in ["puppets", "stories"] {
			std::fs::create_dir_all(dir.join(sub))
				.with_context(|| format!("could not create worlds/{folder}/{sub}"))?;
		}

		std::fs::write(dir.join(world::FILE), file)?;
		std::fs::write(
			dir.join("cast.toml"),
			format!(
				"# The folk of {}. Every table is one character.\n",
				brief.name
			),
		)?;

		Ok(summary(&World::load(&dir)?))
	}

	/// A world in full.
	pub fn describe_world(&self, folder: &str) -> Result<WorldReport> {
		let world = self.world(folder)?;
		let cast = world.cast()?;
		let mut stories: Vec<String> = std::fs::read_dir(world.stories())
			.into_iter()
			.flatten()
			.filter_map(|entry| {
				let path = entry.ok()?.path();
				path.join("story.md")
					.is_file()
					.then(|| path.file_name()?.to_str().map(str::to_owned))
					.flatten()
			})
			.collect();
		stories.sort();

		Ok(WorldReport {
			summary: summary(&world),
			narrator: world.narrator.as_ref().map(|voice| voice.name.clone()),
			cast: cast
				.members
				.values()
				.map(|member| member_report(&world, member))
				.collect(),
			stories,
		})
	}

	/// Adds a character to a world's cast, with a part file started from the style's template.
	pub fn create_character(&self, folder: &str, brief: &CharacterBrief) -> Result<MemberReport> {
		let world = self.world(folder)?;
		check_slug("character", &brief.id)?;
		let style = world.style()?;
		let cast_file = world.cast_file();
		let before = std::fs::read_to_string(&cast_file).unwrap_or_default();
		ensure!(
			world.cast()?.member(&brief.id).is_none(),
			"`{}` is already in the cast",
			brief.id
		);
		let puppet = world.dir.join("puppets").join(format!("{}.svg", brief.id));
		ensure!(!puppet.exists(), "{} already exists", puppet.display());

		let drawings: Vec<(&str, &crate::art::shape::Shape)> = style.template.iter().collect();
		let title = format!(
			"{}: own parts (started from the {} template; redraw each one)",
			brief.name, style.name
		);
		std::fs::create_dir_all(world.dir.join("puppets"))?;
		std::fs::write(&puppet, parts::write(&title, &drawings))?;
		let mut cast = before.clone();

		if !cast.is_empty() && !cast.ends_with("\n\n") {
			cast.push('\n');
		}

		cast.push_str(&cast_entry(brief));
		std::fs::write(&cast_file, &cast)?;

		// A brief the cast file can't read (an unknown colour or extra) is undone, so the world
		// still loads.
		match world.cast() {
			Ok(cast) => {
				let member = cast
					.member(&brief.id)
					.context("the new character is missing from the cast")?;
				Ok(member_report(&world, member))
			}
			Err(error) => {
				std::fs::write(&cast_file, before)?;
				std::fs::remove_file(&puppet)?;
				Err(error.context(format!("`{}` was not added", brief.id)))
			}
		}
	}

	/// A character's parts as path data: their own parts, or the named parts from wherever the
	/// character gets them (their file or the kit).
	pub fn character_parts(
		&self,
		folder: &str,
		id: &str,
		names: &[String],
	) -> Result<Vec<PartReport>> {
		let world = self.world(folder)?;
		let file = puppet_file(&world, id)?;
		let own = Parts::load(&file)?;
		let kit = world.style()?.kit.clone();
		let report = |name: &str, shape: &crate::art::shape::Shape, from| {
			PartReport {
				name: name.to_owned(),
				d: shape.path().as_str().to_owned(),
				from,
			}
		};

		if names.is_empty() {
			return Ok(own
				.iter()
				.map(|(name, shape)| report(name, shape, "own"))
				.collect());
		}

		names
			.iter()
			.map(|name| {
				if let Some(shape) = own.get(name) {
					Ok(report(name, shape, "own"))
				} else if let Some(shape) = kit.get(name) {
					Ok(report(name, shape, "kit"))
				} else {
					bail!("`{id}` has no part `{name}`")
				}
			})
			.collect()
	}

	/// Writes or replaces parts in a character's file and removes others, then checks the
	/// figure. Changes that leave errors are not written; the findings say why.
	pub fn set_parts(
		&self,
		folder: &str,
		id: &str,
		parts: &[PartData],
		remove: &[String],
	) -> Result<PartsUpdate> {
		let world = self.world(folder)?;
		let file = puppet_file(&world, id)?;
		let style = world.style()?;
		let mut own = Parts::load(&file)?;
		let mut problems = Vec::new();

		for part in parts {
			match parts::read_part(&part.name, &part.d) {
				Ok(shape) => own.insert(&part.name, shape),
				Err(error) => problems.push(format!("{error:#}")),
			}
		}

		for name in remove {
			if own.remove(name).is_none() {
				problems.push(format!("`{name}` is not one of `{id}`'s own parts"));
			}
		}

		ensure!(
			problems.is_empty(),
			"nothing was written:\n  {}",
			problems.join("\n  ")
		);

		let findings = own.clone().over(style.kit.clone()).check();
		let written = !findings.iter().any(|finding| finding.level == Level::Error);

		if written {
			let title = first_title(&file).unwrap_or_else(|| format!("{id}: own parts"));
			let drawings: Vec<(&str, &crate::art::shape::Shape)> = own.iter().collect();
			std::fs::write(&file, parts::write(&title, &drawings))
				.with_context(|| format!("could not write {}", file.display()))?;
		}

		Ok(PartsUpdate {
			written,
			own_parts: own.iter().count(),
			findings: findings.iter().map(ToString::to_string).collect(),
		})
	}

	/// Renders a character's sheets, [`SHEET_PX`] wide.
	pub fn character_sheets(
		&self,
		folder: &str,
		id: &str,
		kinds: &[SheetKind],
	) -> Result<Vec<Image>> {
		sheets_for(&self.world(folder)?, id, kinds, SHEET_PX)
	}

	/// The built-in styles, with a sentence on each.
	pub fn styles() -> Result<Vec<(String, String)>> {
		StylePack::names()
			.map(|name| Ok((name.to_owned(), StylePack::named(name)?.description.clone())))
			.collect()
	}

	/// What a style offers: its finish, bodies, and every part with the space it's drawn in.
	pub fn describe_style(name: &str) -> Result<StyleReport> {
		let pack = StylePack::named(name)?;

		Ok(StyleReport {
			name: pack.name.clone(),
			description: pack.description.clone(),
			finish: pack.finish,
			face: pack.face,
			bodies: pack
				.bodies()
				.map(|(name, body)| {
					BodyReport {
						name: name.to_owned(),
						neck_y: body.neck.y,
						shoulder_y: body.shoulder_y,
						pelvis_y: body.limbs.pelvis_y,
					}
				})
				.collect(),
			parts: FIGURE_PARTS
				.iter()
				.map(|spec| {
					PartSpecReport {
						name: spec.name.to_owned(),
						views: match spec.views {
							Views::Once => "once",
							Views::Turning => "turning",
						},
						need: match spec.need {
							Need::Required => "required",
							Need::Optional => "optional",
							Need::Numbered => "numbered",
						},
						home: match spec.home {
							Home::Character => "character",
							Home::Kit => "kit",
						},
						space: spec.space.describe(),
					}
				})
				.collect(),
			rules: RULES.to_vec(),
		})
	}

	/// A style's template kid on a model sheet, and its kit and template parts.
	pub fn style_sheets(name: &str, px: u32) -> Result<Vec<Image>> {
		let pack = StylePack::named(name)?;
		let look = crate::cast::template_look(&pack, crate::art::figure::Build::Kid)?;
		let model = sheets::model_sheet(&format!("The {name} style"), &look, px)?;
		let parts = sheets::parts_sheet(
			&format!("The {name} style: kit and template"),
			&pack.template_figure(),
		)?;

		Ok(vec![
			Image {
				label: format!("{name}: the template on a model sheet"),
				png: raster::png(&model)?,
			},
			Image {
				label: format!("{name}: kit and template parts"),
				png: raster::png(&fit_width(&parts, px))?,
			},
		])
	}
}

/// A character's sheets (the model sheet if `kinds` is empty), no wider than `px`.
pub fn sheets_for(world: &World, id: &str, kinds: &[SheetKind], px: u32) -> Result<Vec<Image>> {
	let cast = world.cast()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}'s cast", world.name))?;
	let Kind::Figure(look) = &member.kind else {
		bail!("`{id}` is not a person, so has no sheets");
	};
	let puppet = look
		.puppet
		.as_ref()
		.with_context(|| format!("`{id}` has no authored puppet"))?;
	let kinds = if kinds.is_empty() {
		&[SheetKind::Model][..]
	} else {
		kinds
	};

	kinds
		.iter()
		.map(|kind| {
			let (label, pixmap) = match kind {
				SheetKind::Model => ("model", sheets::model_sheet(&member.name, look, px)?),
				SheetKind::Parts => {
					(
						"parts",
						sheets::parts_sheet(&format!("{}: parts", member.name), &puppet.parts)?,
					)
				}
				SheetKind::Moods => {
					(
						"moods",
						sheets::mood_sheet(member, crate::art::style::Style::default())?
							.context("only people have moods")?,
					)
				}
			};

			Ok(Image {
				label: format!("{}: {label} sheet", member.name),
				png: raster::png(&fit_width(&pixmap, px))?,
			})
		})
		.collect()
}

/// What [`Studio::set_parts`] did.
#[derive(Clone, Debug, Serialize)]
pub struct PartsUpdate {
	/// Whether the file was written (it isn't when the figure would have errors).
	pub written: bool,
	pub own_parts: usize,
	/// The part check's findings for the whole figure, errors first.
	pub findings: Vec<String>,
}

/// A character's part file, or why they have none.
fn puppet_file(world: &World, id: &str) -> Result<PathBuf> {
	check_slug("character", id)?;
	let cast = world.cast()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}'s cast", world.name))?;

	match &member.kind {
		Kind::Figure(look) => {
			look.puppet
				.as_ref()
				.map(|puppet| puppet.file.clone())
				.with_context(|| format!("`{id}` has no authored puppet"))
		}
		_ => bail!("`{id}` is not a person, so has no puppet"),
	}
}

fn summary(world: &World) -> WorldSummary {
	WorldSummary {
		folder: world
			.dir
			.file_name()
			.and_then(|name| name.to_str())
			.unwrap_or_default()
			.to_owned(),
		name: world.name.clone(),
		style: world.style.clone(),
		about: world.about.clone(),
	}
}

fn member_report(world: &World, member: &Member) -> MemberReport {
	let puppet = match &member.kind {
		Kind::Figure(look) => {
			look.puppet.as_ref().map(|puppet| {
				puppet
					.file
					.strip_prefix(&world.dir)
					.unwrap_or(&puppet.file)
					.display()
					.to_string()
			})
		}
		_ => None,
	};

	MemberReport {
		id: member.id.clone(),
		name: member.name.clone(),
		kind: member.kind.name().to_owned(),
		habit: member.habit.clone(),
		about: member.about.clone(),
		puppet,
	}
}

/// A cast table for a new character.
fn cast_entry(brief: &CharacterBrief) -> String {
	let mut lines = vec![
		format!("[{}]", brief.id),
		format!("name = {}", quoted(&brief.name)),
	];
	let mut text = |key: &str, value: Option<&String>| {
		if let Some(value) = value {
			lines.push(format!("{key} = {}", quoted(value)));
		}
	};
	text("born", brief.born.as_ref());
	text("about", brief.about.as_ref());
	text("habit", brief.habit.as_ref());
	let colours = &brief.colours;
	let kind = brief.build.kind().to_owned();
	let puppet = format!("puppets/{}.svg", brief.id);

	for (key, value) in [
		("kind", &kind),
		("puppet", &puppet),
		("skin", &colours.skin),
		("hair-colour", &colours.hair),
		("top", &colours.top),
		("trim", &colours.trim),
		("bottom", &colours.bottom),
		("shoes", &colours.shoes),
		("eyes", &colours.eyes),
	] {
		lines.push(format!("{key} = {}", quoted(value)));
	}

	if let Some(pattern) = &brief.pattern {
		lines.push(format!("pattern = {}", quoted(pattern)));
	}

	if !brief.extras.is_empty() {
		let extras: Vec<String> = brief.extras.iter().map(|extra| quoted(extra)).collect();
		lines.push(format!("extras = [{}]", extras.join(", ")));
	}

	lines.join("\n") + "\n"
}

/// A TOML string literal.
fn quoted(text: &str) -> String {
	toml::Value::String(text.to_owned()).to_string()
}

/// The sheet title a part file was written with, so rewriting keeps it.
fn first_title(file: &Path) -> Option<String> {
	let source = std::fs::read_to_string(file).ok()?;
	let document = roxmltree::Document::parse(&source).ok()?;
	document
		.descendants()
		.find(|node| node.has_tag_name("text"))
		.and_then(|node| node.text())
		.map(str::to_owned)
}

fn check_slug(what: &str, slug: &str) -> Result<()> {
	ensure!(
		slug.starts_with(|ch: char| ch.is_ascii_lowercase())
			&& slug
				.chars()
				.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
		"`{slug}` is not a {what} name: use lowercase letters, digits and hyphens"
	);
	Ok(())
}

/// A pixmap no wider than `px`, scaled down smoothly if it is.
fn fit_width(pixmap: &Pixmap, px: u32) -> Pixmap {
	if pixmap.width() <= px {
		return pixmap.clone();
	}

	let scale = px as f32 / pixmap.width() as f32;
	let height = ((pixmap.height() as f32 * scale).round() as u32).max(1);
	let mut out = Pixmap::new(px, height).unwrap_or_else(|| unreachable!("a non-empty size"));
	out.draw_pixmap(
		0,
		0,
		pixmap.as_ref(),
		&PixmapPaint {
			quality: FilterQuality::Bicubic,
			..PixmapPaint::default()
		},
		Transform::from_scale(scale, scale),
		None,
	);
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn colours() -> Colours {
		Colours {
			skin: "#f4c766".into(),
			hair: "#7a4a35".into(),
			top: "#ec7aa6".into(),
			trim: "#fff8ef".into(),
			bottom: "#2e9a90".into(),
			shoes: "#7a5cb8".into(),
			eyes: "#5d9e5a".into(),
		}
	}

	fn brief(id: &str) -> CharacterBrief {
		CharacterBrief {
			id: id.into(),
			name: "Nell Nibbles".into(),
			build: BuildName::Kid,
			born: None,
			about: Some("Bites her nails.".into()),
			habit: Some("Nibbles her nails, so her fingers are tiny stubs.".into()),
			colours: colours(),
			pattern: Some("stripes".into()),
			extras: vec!["freckles".into()],
		}
	}

	fn studio() -> (tempfile::TempDir, Studio) {
		let dir = tempfile::tempdir().unwrap();
		let studio = Studio::new(dir.path());
		studio
			.create_world(&WorldBrief {
				name: "Nibbleton".into(),
				about: Some("A town of small habits.".into()),
				style: None,
			})
			.unwrap();
		(dir, studio)
	}

	#[test]
	fn a_new_world_is_listed_and_described() {
		let (_dir, studio) = studio();
		let worlds = studio.worlds().unwrap();
		assert_eq!(worlds.len(), 1);
		assert_eq!(worlds[0].folder, "nibbleton");
		assert_eq!(worlds[0].style, "cartoon");

		let report = studio.describe_world("nibbleton").unwrap();
		assert!(report.cast.is_empty());
		assert_eq!(
			report.summary.about.as_deref(),
			Some("A town of small habits.")
		);
		assert!(
			studio
				.create_world(&WorldBrief {
					name: "Nibbleton".into(),
					about: None,
					style: None,
				})
				.is_err(),
			"a world is never overwritten"
		);
	}

	#[test]
	fn a_character_starts_from_the_template_and_draws_a_model_sheet() {
		let (_dir, studio) = studio();
		let member = studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		assert_eq!(member.puppet.as_deref(), Some("puppets/nell.svg"));
		assert_eq!(member.kind, "kid");

		let images = studio
			.character_sheets("nibbleton", "nell", &[SheetKind::Model])
			.unwrap();
		assert_eq!(images.len(), 1);
		assert!(images[0].png.starts_with(b"\x89PNG"));
	}

	#[test]
	fn a_brief_the_cast_cannot_read_is_undone() {
		let (_dir, studio) = studio();
		let mut bad = brief("nell");
		bad.extras = vec!["jetpack".into()];
		let error = format!(
			"{:#}",
			studio.create_character("nibbleton", &bad).unwrap_err()
		);
		assert!(error.contains("jetpack"), "{error}");
		assert!(studio.describe_world("nibbleton").unwrap().cast.is_empty());
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
	}

	#[test]
	fn parts_are_replaced_and_breaking_changes_are_refused() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		let ear = studio
			.character_parts("nibbleton", "nell", &["ear".into()])
			.unwrap();
		assert_eq!(ear[0].from, "kit");

		// A bigger ear of her own.
		let update = studio
			.set_parts(
				"nibbleton",
				"nell",
				&[PartData {
					name: "ear".into(),
					d: "M0 -20 C-16 -32 -36 -20 -34 2 C-32 22 -12 28 2 18 Z".into(),
				}],
				&[],
			)
			.unwrap();
		assert!(update.written, "{:#?}", update.findings);
		let own = studio.character_parts("nibbleton", "nell", &[]).unwrap();
		assert!(
			own.iter()
				.any(|part| part.name == "ear" && part.from == "own")
		);

		// Removing her head would leave a figure that can't be drawn.
		let update = studio
			.set_parts("nibbleton", "nell", &[], &["head.front".into()])
			.unwrap();
		assert!(!update.written);
		assert!(
			update
				.findings
				.iter()
				.any(|finding| finding.contains("head"))
		);
		studio
			.character_sheets("nibbleton", "nell", &[SheetKind::Model])
			.unwrap();
	}

	#[test]
	fn names_outside_the_studio_are_refused() {
		let (_dir, studio) = studio();
		assert!(studio.describe_world("../etc").is_err());
		assert!(studio.character_parts("nibbleton", "../x", &[]).is_err());
	}

	#[test]
	fn every_style_describes_its_parts() {
		for (name, _) in Studio::styles().unwrap() {
			let report = Studio::describe_style(&name).unwrap();
			assert!(report.parts.iter().any(|part| part.name == "head"));
		}
	}
}
