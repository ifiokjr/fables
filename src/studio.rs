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

use crate::art::dials::Fill;
use crate::art::figure::Mood;
use crate::art::light::Preset;
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
use crate::art::settings;
use crate::art::settings::Setting;
use crate::cast::Kind;
use crate::cast::Member;
use crate::press::sheets;
use crate::publish;
use crate::raster;
use crate::raster::BODY_FAMILY;
use crate::stage::ShotSize;
use crate::stage::Travel;
use crate::story::Story;
use crate::story::Transition;
use crate::video::frames::Format;
use crate::video::voice;
use crate::video::voice::Delivery;
use crate::video::voice::Utterance;
use crate::video::voice::Voice;
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
	/// The other worlds under the root, whose characters a script's `guests` can invite.
	pub visitors: Vec<VisitorWorld>,
}

/// A world beside this one, whose characters a book can invite as guests.
#[derive(Clone, Debug, Serialize)]
pub struct VisitorWorld {
	/// The world's folder; a script's `guests` names its characters `folder/id`.
	pub folder: String,
	pub name: String,
	pub style: String,
	/// Who could be invited, by cast id.
	pub cast: Vec<MemberReport>,
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

/// A voice to cast: which provider speaks and as whom (see `voice_providers`).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct VoiceChoice {
	/// `say`, `command`, `elevenlabs`, `openai`, `cartesia` or `hume`.
	pub provider: String,
	/// A macOS voice for `say` (see `voice_list`), or a built-in voice for `openai`.
	pub name: Option<String>,
	/// The provider's voice id, for `elevenlabs`, `cartesia` and `hume`.
	pub id: Option<String>,
	/// The provider's model, if not its default.
	pub model: Option<String>,
	/// How the character talks, as acting direction (`openai`, `hume`), e.g. "A cheeky
	/// six-year-old, loud and bouncy".
	pub style: Option<String>,
	/// Words per minute, for `say`.
	pub rate: Option<u32>,
}

impl VoiceChoice {
	fn voice(&self) -> Voice {
		let plain = Voice::default();
		Voice {
			provider: self.provider.clone(),
			name: self.name.clone().unwrap_or(plain.name),
			rate: self.rate.or(plain.rate),
			command: None,
			id: self.id.clone(),
			model: self.model.clone(),
			style: self.style.clone(),
		}
	}

	/// The `[id.voice]` table for a cast file.
	fn table(&self, id: &str) -> String {
		let mut lines = vec![
			format!("[{id}.voice]"),
			format!("provider = {}", quoted(&self.provider)),
		];
		let mut text = |key: &str, value: Option<&String>| {
			if let Some(value) = value {
				lines.push(format!("{key} = {}", quoted(value)));
			}
		};
		text("name", self.name.as_ref());
		text("id", self.id.as_ref());
		text("model", self.model.as_ref());
		text("style", self.style.as_ref());

		if let Some(rate) = self.rate {
			lines.push(format!("rate = {rate}"));
		}

		lines.join("\n") + "\n"
	}
}

/// A voice provider and whether it can speak right now.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderReport {
	pub provider: &'static str,
	/// Whether it runs on this machine without a network.
	pub offline: bool,
	/// Whether it can speak now: the program is here, or its key is in the environment.
	pub ready: bool,
	/// What it needs.
	pub needs: &'static str,
}

/// A line spoken in a voice.
#[derive(Clone, Debug)]
pub struct Audition {
	pub label: String,
	/// 16-bit mono WAV.
	pub wav: Vec<u8>,
	pub seconds: f32,
}

/// Which book outputs to make.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Output {
	/// Every page as a PNG, and a contact sheet.
	Proofs,
	/// The fixed-layout ePub and its store cover.
	Ebook,
	/// KDP paperback PDFs and setup numbers.
	Print,
}

/// A video shape.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VideoShape {
	/// 16:9 for `YouTube`.
	Landscape,
	/// 9:16 for Shorts, `TikTok` and Reels.
	#[default]
	Portrait,
}

/// Which moments of a video to look at.
#[derive(Clone, Debug)]
pub enum Frames {
	/// These seconds, from the start of the video.
	Times(Vec<f32>),
	/// Every n seconds, from the start.
	Every(f32),
}

/// The most frames one tile can hold, so a tool's answer stays something a model can look at.
pub const MAX_FRAMES: usize = 12;

impl Frames {
	/// The moments to render, within a video of `duration` seconds.
	pub fn times(&self, duration: f32) -> Result<Vec<f32>> {
		let times = match self {
			Frames::Times(times) => times.clone(),
			Frames::Every(step) => {
				ensure!(
					*step > 0.0,
					"sample every n seconds: n must be more than zero"
				);

				(0..(duration / step).ceil() as usize)
					.map(|index| index as f32 * step)
					.collect::<Vec<_>>()
			}
		};

		ensure!(
			!times.is_empty(),
			"name moments to look at (`times`), or an interval (`every`)"
		);
		ensure!(
			times.len() <= MAX_FRAMES,
			"{} frames is too many to look at; name at most {MAX_FRAMES} moments, or sample every \
			 {:.0}s",
			times.len(),
			(duration / MAX_FRAMES as f32).ceil()
		);

		for &time in &times {
			ensure!(
				time >= 0.0 && time < duration,
				"{time:.1}s is outside the video, which runs {duration:.0}s"
			);
		}

		Ok(times)
	}
}

/// What a render wrote, relative to the studio's root.
#[derive(Clone, Debug, Serialize)]
pub struct Rendered {
	pub files: Vec<String>,
}

/// A world's setting in brief.
#[derive(Clone, Debug, Serialize)]
pub struct SettingSummary {
	pub name: String,
	pub about: String,
	/// `world` for the world's own, `built-in` for fables' own.
	pub from: &'static str,
}

/// One part of a setting, as SVG markup.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct SettingPart {
	/// The part's id: `far-…`, `near-…` or `front-…`.
	pub id: String,
	/// One SVG element with that id, e.g. `<path id="near-hedge" d="…" fill="leaf"
	/// data-wilt="moss"/>`.
	pub markup: String,
}

/// The weather to show a setting in.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
pub struct SettingWeather {
	#[serde(default)]
	pub night: bool,
	/// 0 to 1.
	#[serde(default)]
	pub wilt: f32,
}

/// A light preset, and a sentence on what it feels like.
#[derive(Clone, Debug, Serialize)]
pub struct LightLine {
	pub name: String,
	pub about: &'static str,
}

/// A piece of camera vocabulary, and a sentence on what it says.
#[derive(Clone, Debug, Serialize)]
pub struct ShotLine {
	pub name: String,
	pub about: &'static str,
}

/// The camera vocabulary: shot sizes, moves, how the camera arrives, and page transitions.
#[derive(Clone, Debug, Serialize)]
pub struct ShotsReport {
	pub sizes: Vec<ShotLine>,
	pub moves: Vec<ShotLine>,
	pub arrivals: Vec<ShotLine>,
	pub transitions: Vec<ShotLine>,
}

/// A sentence on each light preset, in [`Preset::ALL`] order.
const LIGHTS: [&str; 10] = [
	"Plain open daylight: the honest default, with shadows where they fall.",
	"Cool air and a warm horizon: beginnings and fresh starts.",
	"Honeyed light from a low sun: the day at its best, or a last chance.",
	"Violet and settled: the day ending, journeys nearly done.",
	"Dark and closed: the world asleep, or something to find in the dark.",
	"Silver and calm: a still world lit by the moon.",
	"Flat grey light with no shadows: waiting weather.",
	"The darkest grade: the low point, the worst page.",
	"Ordinary light at home: nothing special happening, yet.",
	"One small warm pool of light: safe, secret, precious.",
];

/// A sentence on each shot size, in [`ShotSize::ALL`] order.
const SHOT_SIZES: [&str; 5] = [
	"The whole setting, the star small in it: open a sequence.",
	"Head to toe: movement, entrances and the body's comedy.",
	"The waist up: conversation, two people trading lines.",
	"Head and shoulders: a decision, a feeling held back.",
	"The face alone: the turn, the gasp, what only the reader notices.",
];

/// A sentence on each camera move, in [`Travel::MOVES`] order.
const SHOT_MOVES: [&str; 7] = [
	"Glide in, tightening: attention arriving.",
	"Glide out, widening: room to breathe after a peak.",
	"Glide left, revealing what was beside us.",
	"Glide right, revealing what was beside us.",
	"Rise: how big, how far, how high.",
	"Descend: to the ground, to what fell.",
	"Follow the subject as they move.",
];

/// A sentence on each way a camera arrives at a framing.
const SHOT_ARRIVALS: [&str; 2] = [
	"Arrive on the cut: the same moment, a new angle.",
	"Glide to the framing: the older, gentler camera.",
];

/// A sentence on each page transition, in [`Transition::ALL`] order.
const SHOT_TRANSITIONS: [&str; 6] = [
	"The paper page turn: the book's default hello.",
	"No effect at all: the same moment continues.",
	"A little time passes between the pages.",
	"Through the paper colour: a chapter's end, or a big change.",
	"A circle closes and opens: old-fashioned attention on one face.",
	"The new page slides in: the story chases forward.",
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

		let visitors = world
			.neighbours()?
			.iter()
			.map(|other| {
				Ok(VisitorWorld {
					folder: other
						.dir
						.file_name()
						.and_then(|name| name.to_str())
						.unwrap_or_default()
						.to_owned(),
					name: other.name.clone(),
					style: other.style.clone(),
					cast: other
						.cast()?
						.members
						.values()
						.map(|member| member_report(other, member))
						.collect(),
				})
			})
			.collect::<Result<Vec<_>>>()?;

		Ok(WorldReport {
			summary: summary(&world),
			narrator: world.narrator.as_ref().map(|voice| voice.name.clone()),
			cast: cast
				.members
				.values()
				.map(|member| member_report(&world, member))
				.collect(),
			stories,
			visitors,
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

	/// The voice providers, and which can speak right now.
	pub fn voice_providers() -> Vec<ProviderReport> {
		let key =
			|variable: &str| std::env::var_os(variable).is_some_and(|value| !value.is_empty());
		let report = |provider, offline, ready, needs| {
			ProviderReport {
				provider,
				offline,
				ready,
				needs,
			}
		};

		vec![
			report(
				"say",
				true,
				cfg!(target_os = "macos"),
				"macOS; `voice_list` shows its voices",
			),
			report(
				"command",
				true,
				true,
				"any local TTS program (Piper, mlx-audio), set in the cast file",
			),
			report(
				"elevenlabs",
				false,
				key("ELEVENLABS_API_KEY"),
				"ELEVENLABS_API_KEY and a voice id",
			),
			report("openai", false, key("OPENAI_API_KEY"), "OPENAI_API_KEY"),
			report(
				"cartesia",
				false,
				key("CARTESIA_API_KEY"),
				"CARTESIA_API_KEY and a voice id",
			),
			report(
				"hume",
				false,
				key("HUME_API_KEY"),
				"HUME_API_KEY and a voice id",
			),
		]
	}

	/// The macOS voices `say` can speak with, by name and language.
	pub fn say_voices() -> Result<Vec<(String, String)>> {
		let output = std::process::Command::new("say")
			.args(["-v", "?"])
			.output()
			.context("could not run `say`; it is only on macOS")?;
		ensure!(output.status.success(), "`say -v ?` failed");

		Ok(String::from_utf8_lossy(&output.stdout)
			.lines()
			.filter_map(|line| {
				let (name, rest) = line.split_once('#')?.0.trim_end().rsplit_once(' ')?;
				Some((name.trim().to_owned(), rest.trim().to_owned()))
			})
			.collect())
	}

	/// Gives a cast member their own voice, replacing any they had.
	pub fn cast_voice(&self, folder: &str, id: &str, choice: &VoiceChoice) -> Result<MemberReport> {
		let world = self.world(folder)?;
		check_slug("character", id)?;
		choice.voice().validate()?;
		ensure!(
			world.cast()?.member(id).is_some(),
			"`{id}` is not in {}'s cast",
			world.name
		);
		let cast_file = world.cast_file();
		let before = std::fs::read_to_string(&cast_file)?;
		let mut cast = without_table(&before, &format!("[{id}.voice]"));

		if !cast.ends_with("\n\n") {
			cast.push('\n');
		}

		cast.push_str(&choice.table(id));
		std::fs::write(&cast_file, &cast)?;

		match world.cast() {
			Ok(cast) => {
				let member = cast.member(id).context("the character left the cast")?;
				Ok(member_report(&world, member))
			}
			Err(error) => {
				std::fs::write(&cast_file, before)?;
				Err(error.context(format!("`{id}`'s voice was not changed")))
			}
		}
	}

	/// Speaks a line in a cast member's voice (or the narrator's), with a mood's delivery.
	/// Takes are cached under the root's `out/cache/voice`, so a line is only paid for once.
	pub fn audition(
		&self,
		folder: &str,
		who: &str,
		line: &str,
		mood: Option<&str>,
	) -> Result<Audition> {
		audition(
			&self.world(folder)?,
			who,
			line,
			mood,
			&self.root.join("out/cache/voice"),
		)
	}

	/// A world's book, loaded with its cast and furnished by its world.
	fn book(&self, folder: &str, story: &str) -> Result<(World, Story, crate::cast::Cast)> {
		let world = self.world(folder)?;
		check_slug("story", story)?;
		let path = world.stories().join(story).join("story.md");
		ensure!(
			path.is_file(),
			"{} has no story `{story}`; world_describe lists its stories",
			world.name
		);
		let mut script = Story::load(&path)?;
		world.furnish(&mut script)?;
		let cast = world.cast_for(&script)?;
		Ok((world, script, cast))
	}

	/// Writes a book's script (a whole `story.md`) and checks it against the cast. A script
	/// that doesn't parse or rehearse is not kept; the error says what to fix, by line.
	pub fn write_story(
		&self,
		folder: &str,
		story: &str,
		script: &str,
	) -> Result<publish::CheckReport> {
		let world = self.world(folder)?;
		check_slug("story", story)?;
		crate::story::parse(script)?;
		let dir = world.stories().join(story);
		let path = dir.join("story.md");
		let before = std::fs::read_to_string(&path).ok();
		std::fs::create_dir_all(&dir)?;
		std::fs::write(&path, script)?;

		match self.check_story(folder, story) {
			Ok(report) => Ok(report),
			Err(error) => {
				match before {
					Some(text) => std::fs::write(&path, text)?,
					None => std::fs::remove_dir_all(&dir)?,
				}

				Err(error.context(format!("`{story}` was not written")))
			}
		}
	}

	/// A book's script.
	pub fn read_story(&self, folder: &str, story: &str) -> Result<String> {
		let world = self.world(folder)?;
		check_slug("story", story)?;
		let path = world.stories().join(story).join("story.md");
		std::fs::read_to_string(&path).with_context(|| format!("could not read {}", path.display()))
	}

	/// Rehearses a book against its cast and lays it out, without rendering.
	pub fn check_story(&self, folder: &str, story: &str) -> Result<publish::CheckReport> {
		let (_, script, cast) = self.book(folder, story)?;
		publish::check(&script, &cast)
	}

	/// Critiques a book the way a director would: findings with line numbers and suggestions,
	/// never a refusal. Timings are estimated offline, so no voice is recorded.
	pub fn review_story(&self, folder: &str, story: &str) -> Result<Vec<crate::review::Finding>> {
		let (_, script, cast) = self.book(folder, story)?;
		publish::review(&script, &cast, &self.root.join("out"))
	}

	/// Makes a book's outputs under the root's `out/`. Returns what was written, and the proofs'
	/// contact sheet when proofs were made.
	pub fn render_book(
		&self,
		folder: &str,
		story: &str,
		outputs: &[Output],
	) -> Result<(Rendered, Option<Image>)> {
		let (_, script, cast) = self.book(folder, story)?;
		let out = self.root.join("out");
		let outputs = if outputs.is_empty() {
			&[Output::Proofs][..]
		} else {
			outputs
		};
		let mut files = Vec::new();
		let mut contact = None;

		for output in outputs {
			match output {
				Output::Proofs => {
					let dir = publish::proofs(&script, &cast, &out)?;
					let sheet = dir.join("contact-sheet.png");
					let pixmap = Pixmap::load_png(&sheet)
						.with_context(|| format!("could not read {}", sheet.display()))?;
					contact = Some(Image {
						label: format!("{}: every page", script.meta.title),
						png: raster::png(&fit_width(&pixmap, SHEET_PX))?,
					});
					files.push(dir);
				}
				Output::Ebook => {
					let ebook = publish::ebook(&script, &cast, &out)?;
					files.extend([ebook.epub, ebook.cover]);
				}
				Output::Print => {
					let print = publish::print(&script, &cast, &out)?;
					files.extend([print.interior, print.cover, print.setup]);
				}
			}
		}

		Ok((self.relative(&files), contact))
	}

	/// One page of a book as an image: page 1 is the front cover, then every page in order.
	pub fn render_page(&self, folder: &str, story: &str, page: usize) -> Result<Image> {
		let (_, script, cast) = self.book(folder, story)?;
		let mut pages = vec![crate::book::front_cover(&script, &cast)?];
		pages.extend(crate::book::assemble(&script, &cast)?);
		pages.push(crate::book::back_cover(&script, &cast)?);
		let count = pages.len();
		let chosen = page
			.checked_sub(1)
			.and_then(|index| pages.get(index))
			.with_context(|| format!("there is no page {page}; the book has {count} pages"))?;
		let mut pixmap = raster::render(&crate::book::page_svg(
			chosen,
			crate::book::Side::Screen,
			SHEET_PX,
			SHEET_PX,
			true,
		))?;
		raster::finish(&mut pixmap, &chosen.style, 0.035, 0);

		Ok(Image {
			label: format!(
				"{}: page {page} of {count} ({})",
				script.meta.title, chosen.label
			),
			png: raster::png(&pixmap)?,
		})
	}

	/// Narrates and animates a book into a video under the root's `out/`. A preview is half size
	/// at 15 frames a second, for checking timing quickly.
	pub fn render_video(
		&self,
		folder: &str,
		story: &str,
		shape: VideoShape,
		preview: bool,
	) -> Result<Rendered> {
		let (_, script, cast) = self.book(folder, story)?;
		let format = match shape {
			VideoShape::Landscape => Format::Landscape,
			VideoShape::Portrait => Format::Portrait,
		};
		let files = publish::video(&script, &cast, &self.root.join("out"), &[format], preview)?;
		Ok(self.relative(&files))
	}

	/// The storyboard as images: the video's camera plan drawn as a comic strip, one labelled
	/// panel per setup, in the shape's framing. Timings are estimated offline, so no voice is
	/// recorded and no video is encoded.
	pub fn storyboard_images(
		&self,
		folder: &str,
		story: &str,
		shape: VideoShape,
	) -> Result<Vec<Image>> {
		let (_, script, cast) = self.book(folder, story)?;
		let timeline = publish::estimated_timeline(&script, &cast, &self.root.join("out"))?;
		let boards = crate::press::storyboard::boards(
			&script,
			&timeline,
			&cast,
			shape == VideoShape::Portrait,
		)?;

		boards
			.into_iter()
			.enumerate()
			.map(|(page, board)| {
				Ok(Image {
					label: format!("{}: storyboard, page {}", script.meta.title, page + 1),
					png: raster::png(&board)?,
				})
			})
			.collect()
	}

	/// Frames from a story's video at the sampled moments, tiled into one labelled image and
	/// drawn straight from the frame renderer, so no video is encoded and no voice recorded
	/// (timings are estimated offline). At most 12 frames.
	pub fn video_frames(
		&self,
		folder: &str,
		story: &str,
		sample: &Frames,
		shape: VideoShape,
	) -> Result<Image> {
		let (_, script, cast) = self.book(folder, story)?;
		let timeline = publish::estimated_timeline(&script, &cast, &self.root.join("out"))?;
		let times = sample.times(timeline.duration)?;

		let format = match shape {
			VideoShape::Landscape => Format::Landscape,
			VideoShape::Portrait => Format::Portrait,
		};
		let (full_w, full_h) = format.size(false);
		let renderer =
			crate::video::frames::Renderer::new(&timeline, &script, format, full_w / 2, full_h / 2);

		let frames: Vec<(String, Pixmap)> = times
			.iter()
			.map(|&time| {
				let label = match timeline.shot_at(time).kind {
					crate::video::timeline::ShotKind::Title => "title card".to_owned(),
					crate::video::timeline::ShotKind::Page(page) => {
						format!("{time:.1}s · page {}", page + 1)
					}
					crate::video::timeline::ShotKind::End => format!("{time:.1}s · the end"),
				};

				Ok((label, renderer.render(time, 0)?))
			})
			.collect::<Result<_>>()?;

		Ok(Image {
			label: format!(
				"{}: {} frames of the {} video ({:.0}s, estimated timings)",
				script.meta.title,
				frames.len(),
				if shape == VideoShape::Portrait {
					"portrait"
				} else {
					"landscape"
				},
				timeline.duration
			),
			png: raster::png(&tile_frames(&frames)?)?,
		})
	}

	/// Paths as the studio's root sees them.
	fn relative(&self, files: &[PathBuf]) -> Rendered {
		Rendered {
			files: files
				.iter()
				.map(|file| {
					file.strip_prefix(&self.root)
						.unwrap_or(file)
						.display()
						.to_string()
				})
				.collect(),
		}
	}

	/// The settings a world's books can use.
	pub fn settings(&self, folder: &str) -> Result<Vec<SettingSummary>> {
		let world = self.world(folder)?;
		let own = world.dir.join("settings");

		Ok(world
			.settings()?
			.settings()
			.map(|setting| {
				SettingSummary {
					name: setting.name().to_owned(),
					about: setting.about().to_owned(),
					from: if own.join(format!("{}.svg", setting.name())).is_file() {
						"world"
					} else {
						"built-in"
					},
				}
			})
			.collect())
	}

	/// Starts a world's own setting: a copy of a setting it can already use (to repaint it), or
	/// a sky over the ground.
	pub fn create_setting(
		&self,
		folder: &str,
		name: &str,
		about: &str,
		from: Option<&str>,
	) -> Result<SettingSummary> {
		let world = self.world(folder)?;
		check_slug("setting", name)?;
		let file = world.dir.join("settings").join(format!("{name}.svg"));
		ensure!(
			!file.exists(),
			"{} already has its own `{name}`",
			world.name
		);
		let source = match from {
			Some(base) => {
				let own = world.dir.join("settings").join(format!("{base}.svg"));

				if own.is_file() {
					std::fs::read_to_string(&own)?
				} else {
					settings::built_in_source(base)
						.with_context(|| format!("there is no setting `{base}` to start from"))?
						.to_owned()
				}
			}
			None => {
				settings::SettingFile {
					ground: 2060.0,
					title: about.to_owned(),
					ambience: Vec::new(),
					indoors: false,
					parts: vec![
					(
						"far-sky".into(),
						r#"<path id="far-sky" d="M-160 -160 L2560 -160 L2560 1660 L-160 1660 Z" fill="sky" data-night="night" data-line="none"/>"#.into(),
					),
					(
						"near-ground".into(),
						r#"<path id="near-ground" d="M-160 1600 L2560 1600 L2560 2560 L-160 2560 Z" fill="meadow" data-wilt="moss" data-line="none"/>"#.into(),
					),
				],
				}
				.write()
			}
		};
		let mut setting_file = settings::SettingFile::read(&source)?;
		about.clone_into(&mut setting_file.title);
		let text = setting_file.write();
		Setting::parse(name, &text)?;
		std::fs::create_dir_all(world.dir.join("settings"))?;
		std::fs::write(&file, text)?;

		Ok(SettingSummary {
			name: name.to_owned(),
			about: about.to_owned(),
			from: "world",
		})
	}

	/// A setting's parts as markup.
	pub fn setting_parts(&self, folder: &str, name: &str) -> Result<Vec<SettingPart>> {
		let world = self.world(folder)?;
		check_slug("setting", name)?;
		let own = world.dir.join("settings").join(format!("{name}.svg"));
		let source = if own.is_file() {
			std::fs::read_to_string(&own)?
		} else {
			settings::built_in_source(name)
				.with_context(|| format!("there is no setting `{name}`"))?
				.to_owned()
		};
		Ok(settings::SettingFile::read(&source)?
			.parts
			.into_iter()
			.map(|(id, markup)| SettingPart { id, markup })
			.collect())
	}

	/// Writes or replaces parts of a world's own setting (new parts go on top of their layer's
	/// last part) and removes others. A file that wouldn't read is not written.
	pub fn set_setting_parts(
		&self,
		folder: &str,
		name: &str,
		parts: &[SettingPart],
		remove: &[String],
	) -> Result<SettingSummary> {
		let world = self.world(folder)?;
		check_slug("setting", name)?;
		let file = world.dir.join("settings").join(format!("{name}.svg"));
		ensure!(
			file.is_file(),
			"{} has no setting `{name}` of its own; setting_create it first (from a built-in to \
			 repaint one)",
			world.name
		);
		let mut edited = settings::SettingFile::read(&std::fs::read_to_string(&file)?)?;
		let current = &mut edited.parts;

		for id in remove {
			let before = current.len();
			current.retain(|(part, _)| part != id);
			ensure!(current.len() < before, "`{name}` has no part `{id}`");
		}

		for part in parts {
			ensure!(
				part.markup.contains(&format!("id=\"{}\"", part.id)),
				"`{}`'s markup must carry id=\"{}\"",
				part.id,
				part.id
			);

			if let Some(slot) = current.iter_mut().find(|(id, _)| *id == part.id) {
				slot.1.clone_from(&part.markup);
			} else {
				let layer = part.id.split('-').next().unwrap_or_default();
				let at = current
					.iter()
					.rposition(|(id, _)| id.split('-').next() == Some(layer))
					.map_or(current.len(), |index| index + 1);
				current.insert(at, (part.id.clone(), part.markup.clone()));
			}
		}

		let text = edited.write();
		let setting = Setting::parse(name, &text).context("nothing was written")?;
		std::fs::write(&file, text)?;

		Ok(SettingSummary {
			name: name.to_owned(),
			about: setting.about().to_owned(),
			from: "world",
		})
	}

	/// A setting as an image, in some weather.
	pub fn render_setting(
		&self,
		folder: &str,
		name: &str,
		weather: SettingWeather,
	) -> Result<Image> {
		let world = self.world(folder)?;
		let setting = world.settings()?.find(name)?;
		let mut svg = crate::art::Svg::new();
		let weather = settings::Weather {
			night: weather.night,
			wilt: weather.wilt.clamp(0.0, 1.0),
			..settings::Weather::default()
		};
		settings::backdrop(&mut svg, &setting, &weather);
		settings::foreground(&mut svg, &setting, &weather);
		let stage = crate::art::STAGE;
		let pixmap = raster::render(&svg.finish((0.0, 0.0, stage, stage), SHEET_PX, SHEET_PX))?;

		Ok(Image {
			label: format!("{name}: {}", setting.about()),
			png: raster::png(&pixmap)?,
		})
	}

	/// The light presets, with a sentence on each.
	pub fn lights() -> Vec<LightLine> {
		Preset::ALL
			.iter()
			.zip(LIGHTS)
			.map(|(preset, about)| {
				LightLine {
					name: preset.name().to_owned(),
					about,
				}
			})
			.collect()
	}

	/// The camera vocabulary, with a sentence on each.
	pub fn shots() -> ShotsReport {
		let line = |name: &str, about: &'static str| {
			ShotLine {
				name: name.to_owned(),
				about,
			}
		};

		ShotsReport {
			sizes: ShotSize::ALL
				.iter()
				.zip(SHOT_SIZES)
				.map(|(size, about)| line(size, about))
				.collect(),
			moves: Travel::MOVES
				.iter()
				.zip(SHOT_MOVES)
				.map(|(r#move, about)| line(r#move, about))
				.collect(),
			arrivals: [Travel::Cut, Travel::Glide]
				.iter()
				.zip(SHOT_ARRIVALS)
				.map(|(travel, about)| line(travel.name(), about))
				.collect(),
			transitions: Transition::ALL
				.iter()
				.zip(SHOT_TRANSITIONS)
				.map(|(transition, about)| line(transition, about))
				.collect(),
		}
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
		let mut model = sheets::model_sheet(&format!("The {name} style"), &look, px)?;

		if pack.shade.fill == Fill::Screentone {
			raster::screentone(&mut model, pack.shade.tone);
		}

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

			let mut pixmap = fit_width(&pixmap, px);

			if puppet.style.shade.fill == Fill::Screentone {
				raster::screentone(&mut pixmap, puppet.style.shade.tone);
			}

			Ok(Image {
				label: format!("{}: {label} sheet", member.name),
				png: raster::png(&pixmap)?,
			})
		})
		.collect()
}

/// Speaks a line in a world's cast member's voice (`narrator` for the narrator's), with a mood's
/// delivery, caching takes in `cache`.
pub fn audition(
	world: &World,
	who: &str,
	line: &str,
	mood: Option<&str>,
	cache: &Path,
) -> Result<Audition> {
	let cast = world.cast()?;
	let voice = if who == "narrator" {
		world.narrator.clone().unwrap_or_default()
	} else {
		let member = cast
			.member(who)
			.with_context(|| format!("`{who}` is not in {}'s cast", world.name))?;
		member
			.voice
			.clone()
			.with_context(|| format!("`{who}` has no voice yet; cast one with voice_cast"))?
	};
	let mood = mood
		.map(|name| {
			Mood::ALL
				.iter()
				.find(|(key, _)| *key == name)
				.map(|(_, mood)| *mood)
				.with_context(|| format!("unknown mood `{name}`"))
		})
		.transpose()?;
	let speech = voice::speak(
		&Utterance {
			text: line,
			voice: &voice,
			delivery: Delivery { mood, shout: false },
		},
		cache,
	)?;
	let speaker = voice.id.as_deref().unwrap_or(&voice.name);

	Ok(Audition {
		label: format!("{who} ({} {speaker}): {line}", voice.provider),
		wav: wav_bytes(&speech.samples)?,
		seconds: speech.duration(),
	})
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

/// A cast file with one table (its header line and every line up to the next header) taken out.
fn without_table(source: &str, header: &str) -> String {
	let mut kept = Vec::new();
	let mut skipping = false;

	for line in source.lines() {
		let trimmed = line.trim();

		if trimmed.starts_with('[') {
			skipping = trimmed == header;
		}

		if !skipping {
			kept.push(line);
		}
	}

	let mut text = kept.join("\n");
	text.push('\n');
	text
}

/// Mono samples as a 16-bit WAV file.
fn wav_bytes(samples: &[f32]) -> Result<Vec<u8>> {
	let spec = hound::WavSpec {
		channels: 1,
		sample_rate: voice::SAMPLE_RATE,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};
	let mut bytes = std::io::Cursor::new(Vec::new());
	let mut writer = hound::WavWriter::new(&mut bytes, spec)?;

	for sample in samples {
		writer.write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)?;
	}

	writer.finalize()?;
	Ok(bytes.into_inner())
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

/// Frames tiled onto one board, four across, each with its moment written underneath.
fn tile_frames(frames: &[(String, Pixmap)]) -> Result<Pixmap> {
	const COLUMNS: u32 = 4;
	const GAP: u32 = 16;
	const LABEL: u32 = 46;

	let sample = frames
		.first()
		.map_or((1, 1), |(_, frame)| (frame.width(), frame.height()));
	let cell_w = sample.0 + GAP;
	let cell_h = sample.1 + LABEL + GAP;
	let columns = COLUMNS.min(frames.len() as u32).max(1);
	let rows = (frames.len() as u32).div_ceil(columns);
	let mut board = Pixmap::new(GAP + columns * cell_w, GAP + rows * cell_h)
		.context("too many frames to tile")?;
	board.fill(resvg::tiny_skia::Color::from_rgba8(0x2a, 0x22, 0x38, 0xff));

	for (slot, (label, frame)) in frames.iter().enumerate() {
		let column = slot as u32 % columns;
		let row = slot as u32 / columns;
		let x = (GAP + column * cell_w) as i32;
		let y = (GAP + row * cell_h) as i32;

		let stamp = format!(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{LABEL}">
				<text x="6" y="30" font-family="{BODY_FAMILY}" font-size="26" fill="#f4ede1">{}</text>
			</svg>"##,
			sample.0,
			crate::art::escape(label),
		);

		board.draw_pixmap(
			x,
			y,
			frame.as_ref(),
			&PixmapPaint::default(),
			Transform::default(),
			None,
		);
		board.draw_pixmap(
			x,
			y + sample.1 as i32,
			raster::render(&stamp)?.as_ref(),
			&PixmapPaint::default(),
			Transform::default(),
			None,
		);
	}

	Ok(board)
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
		assert!(report.cast.is_empty(), "no cast yet");
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
		assert!(
			studio.describe_world("nibbleton").unwrap().cast.is_empty(),
			"no cast yet"
		);
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
	fn a_voice_is_cast_and_recast() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		let choice = |name: &str| {
			VoiceChoice {
				provider: "say".into(),
				name: Some(name.into()),
				id: None,
				model: None,
				style: None,
				rate: Some(170),
			}
		};
		studio
			.cast_voice("nibbleton", "nell", &choice("Samantha"))
			.unwrap();
		studio
			.cast_voice("nibbleton", "nell", &choice("Moira"))
			.unwrap();

		let world = studio.world("nibbleton").unwrap();
		let cast = world.cast().unwrap();
		let voice = cast.member("nell").unwrap().voice.clone().unwrap();
		assert_eq!(voice.name, "Moira");
		let file = std::fs::read_to_string(world.cast_file()).unwrap();
		assert_eq!(file.matches("[nell.voice]").count(), 1, "{file}");

		let bad = VoiceChoice {
			provider: "cartesia".into(),
			..choice("Moira")
		};
		assert!(
			studio.cast_voice("nibbleton", "nell", &bad).is_err(),
			"a library voice needs an id"
		);
	}

	#[test]
	fn every_provider_says_what_it_needs() {
		let providers = Studio::voice_providers();
		assert_eq!(providers.len(), voice::PROVIDERS.len());
		assert!(
			providers
				.iter()
				.any(|provider| provider.provider == "say" && provider.offline)
		);
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn the_narrator_auditions_offline() {
		let (_dir, studio) = studio();
		let audition = studio
			.audition("nibbleton", "narrator", "Once upon a time.", Some("happy"))
			.unwrap();
		assert!(audition.wav.starts_with(b"RIFF"));
		assert!(audition.seconds > 0.3);
	}

	#[test]
	fn every_light_and_shot_has_a_sentence() {
		let lights = Studio::lights();
		assert_eq!(lights.len(), Preset::ALL.len());

		for preset in Preset::ALL {
			assert!(
				lights.iter().any(|light| light.name == preset.name()),
				"{} has no sentence",
				preset.name()
			);
		}

		let shots = Studio::shots();
		assert_eq!(shots.sizes.len(), ShotSize::ALL.len());
		assert_eq!(shots.moves.len(), Travel::MOVES.len());
		assert_eq!(shots.transitions.len(), Transition::ALL.len());

		for (name, list) in [
			("wide", &shots.sizes),
			("extreme-close", &shots.sizes),
			("push", &shots.moves),
			("track", &shots.moves),
			("cut", &shots.arrivals),
			("ease", &shots.arrivals),
			("iris", &shots.transitions),
		] {
			assert!(
				list.iter().any(|line| line.name == name),
				"{name} has no sentence"
			);
		}
	}

	#[test]
	fn a_book_can_be_seen_without_rendering_a_video() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		studio
			.write_story(
				"nibbleton",
				"nell-nibbles",
				"+++\ntitle = \"Nell Nibbles\"\nauthor = \"A\"\nstar = \"nell\"\n+++\n# \
				 park\n@nell x=0.5 happy wave talk\nNell waved hello.\n",
			)
			.unwrap();

		let boards = studio
			.storyboard_images("nibbleton", "nell-nibbles", VideoShape::Landscape)
			.unwrap();
		assert_eq!(boards.len(), 1, "one board page");
		assert!(boards[0].png.starts_with(b"\x89PNG"));
		assert!(boards[0].label.contains("storyboard"));

		let frames = studio
			.video_frames(
				"nibbleton",
				"nell-nibbles",
				&Frames::Times(vec![0.0, 2.0]),
				VideoShape::Portrait,
			)
			.unwrap();
		assert!(frames.png.starts_with(b"\x89PNG"));
		assert!(frames.label.contains("2 frames"));

		// Sampling past the end, or too finely, says what to do instead.
		let error = format!(
			"{:#}",
			studio
				.video_frames(
					"nibbleton",
					"nell-nibbles",
					&Frames::Times(vec![999.0]),
					VideoShape::Portrait,
				)
				.unwrap_err()
		);
		assert!(error.contains("outside the video"), "{error}");

		let error = format!(
			"{:#}",
			studio
				.video_frames(
					"nibbleton",
					"nell-nibbles",
					&Frames::Every(0.1),
					VideoShape::Portrait,
				)
				.unwrap_err()
		);
		assert!(error.contains("too many"), "{error}");
	}

	#[test]
	fn a_story_is_written_checked_and_rendered() {
		let (dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		let script = "+++\ntitle = \"Nell Nibbles\"\nauthor = \"A\"\nstar = \"nell\"\n+++\n# \
		              park\n@nell x=0.5 happy wave\nNell waved.\n";
		let report = studio
			.write_story("nibbleton", "nell-nibbles", script)
			.unwrap();
		assert_eq!(report.script_pages, 1);
		assert_eq!(
			studio.read_story("nibbleton", "nell-nibbles").unwrap(),
			script
		);

		let page = studio.render_page("nibbleton", "nell-nibbles", 4).unwrap();
		assert!(page.png.starts_with(b"\x89PNG"));
		assert!(
			studio
				.render_page("nibbleton", "nell-nibbles", 999)
				.is_err()
		);

		let (rendered, contact) = studio
			.render_book("nibbleton", "nell-nibbles", &[Output::Proofs])
			.unwrap();
		assert!(contact.is_some());
		assert!(
			rendered.files[0].starts_with("out/"),
			"{:?}",
			rendered.files
		);
		assert!(dir.path().join(&rendered.files[0]).is_dir());
	}

	#[test]
	fn a_script_that_does_not_rehearse_is_not_kept() {
		let (_dir, studio) = studio();
		let bad = "+++\ntitle = \"Who\"\nauthor = \"A\"\n+++\n# park\n@nobody wave\nHello.\n";
		let error = format!(
			"{:#}",
			studio.write_story("nibbleton", "who", bad).unwrap_err()
		);
		assert!(error.contains("nobody"), "{error}");
		assert!(
			studio.read_story("nibbleton", "who").is_err(),
			"nothing is left behind"
		);
	}

	#[test]
	fn a_setting_is_started_repainted_and_rendered() {
		let (_dir, studio) = studio();
		let blank = studio
			.create_setting("nibbleton", "harbour", "The harbour wall", None)
			.unwrap();
		assert_eq!(blank.from, "world");
		studio
			.set_setting_parts(
				"nibbleton",
				"harbour",
				&[SettingPart {
					id: "near-wall".into(),
					markup: r#"<path id="near-wall" d="M0 1500 L2400 1500 L2400 1700 L0 1700 Z" fill="slate"/>"#.into(),
				}],
				&[],
			)
			.unwrap();
		let parts = studio.setting_parts("nibbleton", "harbour").unwrap();
		assert_eq!(
			parts
				.iter()
				.map(|part| part.id.as_str())
				.collect::<Vec<_>>(),
			["far-sky", "near-ground", "near-wall"]
		);
		let image = studio
			.render_setting(
				"nibbleton",
				"harbour",
				SettingWeather {
					night: true,
					wilt: 0.0,
				},
			)
			.unwrap();
		assert!(image.png.starts_with(b"\x89PNG"));

		// A repaint starts from a built-in; a broken part is refused.
		studio
			.create_setting("nibbleton", "park", "Our park", Some("park"))
			.unwrap();
		let broken = studio.set_setting_parts(
			"nibbleton",
			"park",
			&[SettingPart {
				id: "near-bench".into(),
				markup: r#"<path id="near-bench" d="M0 0 L1 1" fill="not-a-colour"/>"#.into(),
			}],
			&[],
		);
		assert!(broken.is_err());
		assert!(
			studio
				.settings("nibbleton")
				.unwrap()
				.iter()
				.any(|setting| setting.name == "park" && setting.from == "world")
		);
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
