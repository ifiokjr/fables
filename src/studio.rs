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

use std::fmt::Write as _;
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
use crate::art::figure::Build;
use crate::art::figure::Mood;
use crate::art::light::Preset;
use crate::art::pack::Face;
use crate::art::pack::Finish;
use crate::art::pack::Picks;
use crate::art::pack::StylePack;
use crate::art::pack::body_name;
use crate::art::parts;
use crate::art::parts::FIGURE_PARTS;
use crate::art::parts::Feature;
use crate::art::parts::Home;
use crate::art::parts::Level;
use crate::art::parts::Need;
use crate::art::parts::Parts;
use crate::art::parts::Views;
use crate::art::props;
use crate::art::props::PropFile;
use crate::art::puppet::Physique;
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
	/// The author credited in its books, unless a book names its own.
	pub author: Option<String>,
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
	/// How a person is drawn: `puppet` (their part file), `template` (their world's drawing
	/// style's template) or `parametric` (the older puppet). `None` for animals and things.
	pub drawn: Option<&'static str>,
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
	/// The author credited in the world's books, unless a book names its own.
	pub author: Option<String>,
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
	fn name(self) -> &'static str {
		match self {
			BuildName::Kid => "kid",
			BuildName::GrownUp => "grown-up",
		}
	}

	/// The body this names.
	pub fn build(self) -> Build {
		match self {
			BuildName::Kid => Build::Kid,
			BuildName::GrownUp => Build::Grownup,
		}
	}
}

/// How heavily built a character is on their body.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PhysiqueName {
	Slim,
	Plain,
	Stout,
}

impl PhysiqueName {
	/// The build this names.
	pub fn physique(self) -> Physique {
		match self {
			PhysiqueName::Slim => Physique::Slim,
			PhysiqueName::Plain => Physique::Plain,
			PhysiqueName::Stout => Physique::Stout,
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

/// A book to start from a story flow: its title, its star, and optionally the second
/// character and the flow (or the world's recipe.md when neither names one).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct StoryBrief {
	pub title: String,
	/// The cast id of the star.
	pub star: String,
	/// The cast id of the second character, for a flow that stages two or a crowd.
	pub friend: Option<String>,
	/// The flow, from `flow_list`. Without one, the world's `recipe.md`, or three-tries.
	pub flow: Option<String>,
	/// The author on the title page; the world's author when left out.
	pub author: Option<String>,
}

/// A new character: who they are and their colours. Their drawing starts as the style's
/// template, to redraw part by part.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CharacterBrief {
	/// The cast id scripts use after `@`: lowercase letters, digits and hyphens.
	pub id: String,
	/// Their full name, e.g. "Nell Nibbles".
	pub name: String,
	/// `kid` or `grown-up`: the body they are drawn on.
	#[serde(default)]
	pub kind: BuildName,
	/// How heavily built they are. Left out, one is picked from their id.
	pub build: Option<PhysiqueName>,
	/// A hairdo their style draws on their body (see `style_describe`). Left out, one is picked
	/// from their id.
	pub hair: Option<String>,
	/// Brows their style draws (see `style_describe`); left out, the style's own.
	pub brows: Option<String>,
	/// A nose their style draws, or `none` where it lets a face go without (see
	/// `style_describe`); left out, the style's own.
	pub nose: Option<String>,
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
	/// Also write `puppets/<id>.svg` from the style's template for their body (`build`) and set
	/// `puppet = …`, so every part can be redrawn. Without it the character is drawn straight
	/// from that template.
	#[serde(default)]
	pub draw: bool,
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
	/// The page skin the look prints in (riso, clear…).
	pub skin: String,
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
	/// The template a character on this body is drawn from, e.g.
	/// `styles/anime/template-grown-up.svg`.
	pub template: String,
	/// How many of their own parts that template draws (the rest come from the kit).
	pub template_parts: usize,
	/// Where the neck sits, in figure units (the soles at y = 0, up is negative).
	pub neck_y: f32,
	pub shoulder_y: f32,
	pub pelvis_y: f32,
	/// The hairdos a character on this body can pick with `hair`, the template's own first.
	pub hair: Vec<String>,
	/// The brows they can pick with `brows`, the kit's own first.
	pub brows: Vec<String>,
	/// The noses they can pick with `nose`, the kit's own first.
	pub noses: Vec<String>,
	/// The builds they can pick with `build`, plain (the body as drawn) first.
	pub builds: Vec<&'static str>,
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

/// What an animatic wrote: the MP4 and the shot list it holds.
#[derive(Clone, Debug, Serialize)]
pub struct AnimaticReport {
	pub file: String,
	pub seconds: f32,
	pub shots: usize,
	/// Lines left silent and timed by their estimate: their voice could not record here, or
	/// `silent` was asked for.
	pub silent_lines: usize,
	/// The shot list, ending with a note when any line is silent.
	pub shot_list: String,
}

/// What pinning a book's shots into its script wrote.
#[derive(Clone, Debug, Serialize)]
pub struct PinReport {
	/// The script after pinning (or unpinning).
	pub script: String,
	/// The 1-based pages pinned, in script order.
	pub pages: Vec<usize>,
	/// How many `@camera` directions were written.
	pub pinned: usize,
	/// How many pinned directions were removed (`unpin`).
	pub unpinned: usize,
	/// Pages left alone, and where the portrait plan differs.
	pub notes: Vec<String>,
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

/// A world's prop in brief.
#[derive(Clone, Debug, Serialize)]
pub struct PropSummary {
	pub name: String,
	pub about: String,
	/// The point that sits in the fist, in the prop's own units.
	pub grip: [f32; 2],
	/// `one` or `two`.
	pub hands: String,
	/// How somebody standing at ease carries it (`data-carry`): `hand`, `up`, `front`,
	/// `string` or `head`.
	pub carry: &'static str,
	/// `world` for the world's own, `built-in` for fables' own.
	pub from: &'static str,
}

impl PropSummary {
	fn of(prop: &props::Prop, from: &'static str) -> PropSummary {
		PropSummary {
			name: prop.name.clone(),
			about: prop.about.clone(),
			grip: [prop.grip.x, prop.grip.y],
			hands: prop.hands.name().to_owned(),
			carry: prop.carry.name(),
			from,
		}
	}
}

/// One part of a prop, as SVG markup.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct PropPart {
	/// The part's id, its name.
	pub id: String,
	/// One SVG element with that id, e.g. `<path id="body" d="…" fill="teal"
	/// data-shade="1"/>`.
	pub markup: String,
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
		let dir = world::under(&self.worlds_dir(), "world", folder)?;
		ensure!(
			dir.join(world::FILE).is_file(),
			"there is no world `{folder}`; `world_list` shows the worlds"
		);
		World::load(&dir)
	}

	/// Starts a world: a `world.toml`, a commented cast example and empty `puppets/` and
	/// `stories/`.
	pub fn create_world(&self, brief: &WorldBrief) -> Result<WorldSummary> {
		let folder = crate::story::slugify(&brief.name);
		ensure!(
			!folder.is_empty(),
			"a world needs a name with letters in it"
		);
		let style = brief.style.as_deref().unwrap_or(crate::art::pack::DEFAULT);
		StylePack::named(style)?;
		let dir = world::under(&self.worlds_dir(), "world", &folder)?;
		ensure!(!dir.exists(), "a world already lives in worlds/{folder}");

		let mut file = format!(
			"name = {}\nstyle = {}\n",
			quoted(&brief.name),
			quoted(style)
		);

		if let Some(about) = &brief.about {
			file = format!("{file}about = {}\n", quoted(about));
		}

		if let Some(author) = &brief.author {
			file = format!("{file}author = {}\n", quoted(author));
		}

		for sub in ["puppets", "stories"] {
			std::fs::create_dir_all(dir.join(sub))
				.with_context(|| format!("could not create worlds/{folder}/{sub}"))?;
		}

		std::fs::write(dir.join(world::FILE), file)?;
		std::fs::write(dir.join("cast.toml"), cast_example(&brief.name, style))?;

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

	/// Adds a character to a world's cast. With `draw`, their part file is written from the
	/// style's template and `puppet = …` set; without it they are drawn from the template.
	pub fn create_character(&self, folder: &str, brief: &CharacterBrief) -> Result<MemberReport> {
		create_character(&self.world(folder)?, brief)
	}

	/// A character's parts as path data: their own parts, or the named parts from wherever the
	/// character gets them (their file, their style's template or the kit).
	pub fn character_parts(
		&self,
		folder: &str,
		id: &str,
		names: &[String],
	) -> Result<Vec<PartReport>> {
		character_parts(&self.world(folder)?, id, names)
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
		set_parts(&self.world(folder)?, id, parts, remove)
	}

	/// Starts a character's own part file (from the style's template, or another cast member's
	/// drawings) and points their cast entry at it, so either order works.
	pub fn template_puppet(
		&self,
		folder: &str,
		id: &str,
		from: Option<&str>,
	) -> Result<MemberReport> {
		template_puppet(&self.world(folder)?, id, from)
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
		let path = world::under(&world.stories(), "story", Path::new(story).join("story.md"))?;
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
	/// that doesn't parse or rehearse is not kept; the error says what to fix, by line. A story
	/// belongs to the world it is written into, so its front matter may not name a `world`.
	pub fn write_story(
		&self,
		folder: &str,
		story: &str,
		script: &str,
	) -> Result<publish::CheckReport> {
		let world = self.world(folder)?;
		check_slug("story", story)?;
		let parsed = crate::story::parse(script)?;
		ensure!(
			parsed.meta.world.is_none(),
			"a story written over MCP belongs to the world it is written into; remove `world =` \
			 from its front matter (use `--world` or a relative `world =` at the command line)"
		);
		let dir = world::under(&world.stories(), "story", story)?;
		let path = dir.join("story.md");
		let before = std::fs::read_to_string(&path).ok();
		let created = !dir.exists();
		std::fs::create_dir_all(&dir)?;
		std::fs::write(&path, script)?;

		match self.check_story(folder, story) {
			Ok(report) => Ok(report),
			Err(error) => {
				// Only what this call made is rolled back: the folder if it created it, else
				// just the script, so a folder's other files are never lost.
				match before {
					Some(text) => std::fs::write(&path, text)?,
					None if created => std::fs::remove_dir_all(&dir)?,
					None => std::fs::remove_file(&path)?,
				}

				Err(error.context(format!("`{story}` was not written")))
			}
		}
	}

	/// A book's script.
	pub fn read_story(&self, folder: &str, story: &str) -> Result<String> {
		let world = self.world(folder)?;
		check_slug("story", story)?;
		let path = world::under(&world.stories(), "story", Path::new(story).join("story.md"))?;
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

	/// The story flows a world's books can start from: its own first, then the built-ins.
	pub fn flows(&self, folder: &str) -> Result<Vec<crate::flow::Report>> {
		let world = self.world(folder)?;

		Ok(crate::flow::list(Some(&world))?
			.iter()
			.map(crate::flow::Flow::report)
			.collect())
	}

	/// One story flow by name, with its beats.
	pub fn describe_flow(&self, folder: &str, name: &str) -> Result<crate::flow::Report> {
		let world = self.world(folder)?;

		Ok(crate::flow::find(Some(&world), name)?.report())
	}

	/// The script a new book starts from: a flow's scaffold, or the world's own recipe. The
	/// book is not written; the agent fills the bracketed lines in and calls `story_write`.
	pub fn start_story(&self, folder: &str, brief: &StoryBrief) -> Result<String> {
		let world = self.world(folder)?;

		Ok(crate::flow::start(
			&world,
			&brief.title,
			&brief.star,
			brief.friend.as_deref(),
			brief.flow.as_deref(),
			brief.author.as_deref(),
		)?
		.script)
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

	/// The storyboard as images and its shot list: the video's camera plan drawn as a comic
	/// strip, one labelled panel per shot, in the shape's framing. Timings are estimated
	/// offline, so no voice is recorded and no video is encoded.
	pub fn storyboard(
		&self,
		folder: &str,
		story: &str,
		shape: VideoShape,
	) -> Result<(String, Vec<Image>)> {
		let (_, script, cast) = self.book(folder, story)?;
		let out = self.root.join("out");
		let timeline = publish::estimated_timeline(&script, &cast, &out)?;
		let portrait = shape == VideoShape::Portrait;
		let rows = crate::press::storyboard::plan(&script, &timeline, &cast, portrait);
		let text = crate::press::storyboard::shot_list(&script, &timeline, &rows, portrait);
		let boards = crate::press::storyboard::boards(&script, &timeline, &cast, portrait)?;
		let images = boards
			.into_iter()
			.enumerate()
			.map(|(page, board)| {
				Ok(Image {
					label: format!("{}: storyboard, page {}", script.meta.title, page + 1),
					png: raster::png(&board)?,
				})
			})
			.collect::<Result<Vec<_>>>()?;

		Ok((text, images))
	}

	/// The storyboard's images alone.
	pub fn storyboard_images(
		&self,
		folder: &str,
		story: &str,
		shape: VideoShape,
	) -> Result<Vec<Image>> {
		self.storyboard(folder, story, shape)
			.map(|(_, images)| images)
	}

	/// Pins the director's shots into a book's script as editable `@camera` directions, one
	/// before the line or direction that starts each shot, or removes the ones pinned before.
	/// Returns the new script.
	pub fn pin_story(
		&self,
		folder: &str,
		story: &str,
		pages: Option<&[usize]>,
		unpin: bool,
		shape: VideoShape,
	) -> Result<PinReport> {
		let (world, script, cast) = self.book(folder, story)?;
		let path = world::under(&world.stories(), "story", Path::new(story).join("story.md"))?;
		let source = std::fs::read_to_string(&path)
			.with_context(|| format!("could not read {}", path.display()))?;

		if unpin {
			let (restored, removed) = crate::press::storyboard::unpin(&source);
			std::fs::write(&path, &restored)
				.with_context(|| format!("could not write {}", path.display()))?;

			return Ok(PinReport {
				script: restored,
				pages: Vec::new(),
				pinned: 0,
				unpinned: removed,
				notes: Vec::new(),
			});
		}

		let timeline = publish::estimated_timeline(&script, &cast, &self.root.join("out"))?;
		let pinned = crate::press::storyboard::pin(
			&source,
			&script,
			&timeline,
			pages,
			shape == VideoShape::Portrait,
		)?;
		std::fs::write(&path, &pinned.script)
			.with_context(|| format!("could not write {}", path.display()))?;

		Ok(PinReport {
			script: pinned.script,
			pages: pinned.pages,
			pinned: pinned.count,
			unpinned: 0,
			notes: pinned.notes,
		})
	}

	/// Cuts an animatic: each storyboard shot held for its real duration over the soundtrack,
	/// at half size. Narrates any missing line once, then reuses the voice cache.
	pub fn render_animatic(
		&self,
		folder: &str,
		story: &str,
		shape: VideoShape,
		silent: bool,
	) -> Result<AnimaticReport> {
		let (_, script, cast) = self.book(folder, story)?;
		let format = match shape {
			VideoShape::Landscape => Format::Landscape,
			VideoShape::Portrait => Format::Portrait,
		};
		// The video's own take cache and folder, so a line narrated for one is reused by the
		// other; the book's slug names a folder under `out/` and nowhere else.
		let out = self.root.join("out");
		world::under(&out, "book", &script.slug)?;
		let narration = if silent {
			publish::Narration::Silent
		} else {
			publish::Narration::Voiced
		};
		let cut = publish::animatic(&script, &cast, &out, format, narration)?;
		let (path, timeline) = (cut.path, cut.timeline);
		let portrait = shape == VideoShape::Portrait;
		let rows = crate::press::storyboard::plan(&script, &timeline, &cast, portrait);
		let mut shot_list =
			crate::press::storyboard::shot_list(&script, &timeline, &rows, portrait);

		if let Some(note) = &cut.note {
			let _ = write!(shot_list, "\n**Voice.** {note}.\n");
		}

		Ok(AnimaticReport {
			file: path
				.strip_prefix(&self.root)
				.unwrap_or(&path)
				.display()
				.to_string(),
			seconds: timeline.duration,
			shots: rows.len(),
			silent_lines: cut.silent,
			shot_list,
		})
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

		world
			.settings()?
			.settings()
			.map(|setting| {
				// A name read from the folder's file names: prove the file it names stays in
				// the world before asking whether it is there.
				let file = world::under(&own, "setting", format!("{}.svg", setting.name()))?;

				Ok(SettingSummary {
					name: setting.name().to_owned(),
					about: setting.about().to_owned(),
					from: if file.is_file() { "world" } else { "built-in" },
				})
			})
			.collect()
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
		create_setting(&self.world(folder)?, name, about, from)
	}

	/// A setting's parts as markup.
	pub fn setting_parts(&self, folder: &str, name: &str) -> Result<Vec<SettingPart>> {
		let world = self.world(folder)?;
		check_slug("setting", name)?;
		let own = world::under(
			&world.dir.join("settings"),
			"setting",
			format!("{name}.svg"),
		)?;
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
		let file = world::under(
			&world.dir.join("settings"),
			"setting",
			format!("{name}.svg"),
		)?;
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

	/// The props a world's books can use.
	pub fn props(&self, folder: &str) -> Result<Vec<PropSummary>> {
		list_props(&self.world(folder)?)
	}

	/// Starts a world's own prop: a copy of a prop it can use (to redraw it), or a plain shape
	/// with its grip at the fist.
	pub fn create_prop(
		&self,
		folder: &str,
		name: &str,
		about: &str,
		from: Option<&str>,
	) -> Result<PropSummary> {
		create_prop(&self.world(folder)?, name, about, from)
	}

	/// A prop's parts as markup.
	pub fn prop_parts(&self, folder: &str, name: &str) -> Result<Vec<PropPart>> {
		let world = self.world(folder)?;
		Ok(PropFile::read(&prop_source(&world, name)?)?
			.parts
			.into_iter()
			.map(|(id, markup)| PropPart { id, markup })
			.collect())
	}

	/// Writes or replaces parts of a world's own prop (new parts go on top) and removes others.
	/// A file that wouldn't read is not written.
	pub fn set_prop_parts(
		&self,
		folder: &str,
		name: &str,
		parts: &[PropPart],
		remove: &[String],
	) -> Result<PropSummary> {
		let world = self.world(folder)?;
		let file = own_prop(&world, name)?;
		ensure!(
			file.is_file(),
			"{} has no prop `{name}` of its own; prop_create it first (from a built-in to redraw \
			 one)",
			world.name
		);
		let mut edited = PropFile::read(&std::fs::read_to_string(&file)?)?;
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
				current.push((part.id.clone(), part.markup.clone()));
			}
		}

		let text = edited.write();
		let prop =
			props::Prop::parse(name, &text, &world.style()?).context("nothing was written")?;
		std::fs::write(&file, text)?;

		Ok(PropSummary::of(&prop, "world"))
	}

	/// A prop as an image: alone, fitted to the sheet, with the name and grip.
	pub fn render_prop(&self, folder: &str, name: &str) -> Result<Image> {
		let world = self.world(folder)?;
		check_slug("prop", name)?;
		let prop = world.props()?.find(name)?;
		let stage = crate::art::STAGE;
		let mut svg = crate::art::Svg::new();
		svg.fill(
			&crate::art::Path::rect(-stage * 0.25, -stage * 0.25, stage * 1.5, stage * 1.5),
			crate::art::palette::PAPER,
		);
		let (lo, hi) = prop.bounds();
		let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
		let fit = (stage * 0.7 / w).min(stage * 0.7 / h);
		let centre = crate::art::pt(f32::midpoint(lo.x, hi.x), f32::midpoint(lo.y, hi.y));
		prop.draw(
			&mut svg,
			crate::art::pt(stage * 0.5, stage * 0.5) - centre * fit,
			fit,
		);
		let pixmap = raster::render(&svg.finish(
			(-stage * 0.25, -stage * 0.25, stage * 1.5, stage * 1.5),
			SHEET_PX,
			SHEET_PX,
		))?;

		Ok(Image {
			label: format!(
				"{name}: {} (grip {:.0}, {:.0})",
				prop.about, prop.grip.x, prop.grip.y
			),
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
			skin: pack.skin.clone(),
			finish: pack.finish,
			face: pack.face,
			bodies: [Build::Kid, Build::Grownup]
				.into_iter()
				.map(|build| {
					let body = pack.body(build);
					let offers = |feature| own_first(&pack, build, feature);
					BodyReport {
						name: body_name(build).to_owned(),
						template: pack.template_file(build).display().to_string(),
						template_parts: pack.template_for(build).iter().count(),
						neck_y: body.neck.y,
						shoulder_y: body.shoulder_y,
						pelvis_y: body.limbs.pelvis_y,
						hair: offers(Feature::Hair),
						brows: offers(Feature::Brows),
						noses: offers(Feature::Nose),
						builds: [Physique::Plain, Physique::Slim, Physique::Stout]
							.map(Physique::name)
							.to_vec(),
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

	/// A style's template kid on a model sheet, its kit and template parts, and its variants:
	/// every hair, brows, nose and build it offers, body by body.
	pub fn style_sheets(name: &str, px: u32) -> Result<Vec<Image>> {
		let pack = StylePack::named(name)?;
		let look = crate::cast::template_look(&pack, Build::Kid)?;
		let mut model = sheets::model_sheet(&format!("The {name} style"), &look, px)?;
		let screentone = |sheet: &mut Pixmap| {
			if pack.shade.fill == Fill::Screentone {
				raster::screentone(sheet, pack.shade.tone);
			}
		};
		screentone(&mut model);

		let parts = sheets::parts_sheet(
			&format!("The {name} style: kit and template"),
			&pack.template_figure(Build::Kid),
		)?;
		let mut variants = sheets::variants_sheet(
			&format!("The {name} style: variants"),
			&variant_lineups(&pack)?,
			px,
		)?;
		screentone(&mut variants);

		Ok(vec![
			Image {
				label: format!("{name}: the template on a model sheet"),
				png: raster::png(&model)?,
			},
			Image {
				label: format!("{name}: kit and template parts"),
				png: raster::png(&fit_width(&parts, px))?,
			},
			Image {
				label: format!("{name}: every hair, brows, nose and build"),
				png: raster::png(&variants)?,
			},
		])
	}
}

/// Adds a character to a world's cast. With `brief.draw`, their part file is written from the
/// style's template for their body and `puppet = …` set; without it they are drawn from that
/// template in their own colours.
pub fn create_character(world: &World, brief: &CharacterBrief) -> Result<MemberReport> {
	check_slug("character", &brief.id)?;
	let style = world.style()?;
	let cast_file = world.cast_file();
	let before = std::fs::read_to_string(&cast_file).unwrap_or_default();
	ensure!(
		world.cast()?.member(&brief.id).is_none(),
		"`{}` is already in the cast",
		brief.id
	);
	let puppet = world::under(
		&world.dir,
		"puppet",
		Path::new("puppets").join(format!("{}.svg", brief.id)),
	)?;

	if brief.draw {
		ensure!(!puppet.exists(), "{} already exists", puppet.display());
	}

	// The variants they're given are written out, so they keep them even if their style later
	// draws more to pick from. Those the brief leaves out are the ones the cast uses least.
	let build = brief.kind.build();
	let (fresh, fresh_physique) =
		crate::cast::fresh_picks(&brief.id, &style, build, &cast_words(&before));
	let picks = Picks {
		hair: brief.hair.clone().or(fresh.hair),
		brows: brief.brows.clone().or(fresh.brows),
		nose: brief.nose.clone().or(fresh.nose),
	};
	let physique = brief.build.map_or(fresh_physique, PhysiqueName::physique);
	// The template drawn for their body with their variants: a grown-up's parts are cut for a
	// grown-up. A variant the style doesn't draw stops here, before anything is written.
	style.figure(build, &picks)?;
	let template = if brief.draw {
		Some(style.template_with(build, &picks)?)
	} else {
		None
	};
	let mut cast = before.clone();

	if !cast.is_empty() && !cast.ends_with("\n\n") {
		cast.push('\n');
	}

	// A part file draws their hair, brows and nose, so only a template-drawn character's cast
	// names them.
	let named = if template.is_none() {
		picks
	} else {
		Picks::default()
	};
	cast.push_str(&cast_entry(brief, &named, physique));
	std::fs::write(&cast_file, &cast)
		.with_context(|| format!("could not write {}", cast_file.display()))?;

	if let Some(template) = &template {
		let drawings: Vec<(&str, &crate::art::shape::Shape)> = template.iter().collect();
		let title = format!(
			"{}: own parts (started from {}; redraw each one)",
			brief.name,
			style.template_origin(build)
		);
		std::fs::create_dir_all(world.dir.join("puppets"))
			.with_context(|| format!("could not create {}", world.dir.join("puppets").display()))?;

		if let Err(error) = std::fs::write(&puppet, parts::write(&title, &drawings)) {
			// Put the cast back, so a failed write never leaves a half-made character.
			std::fs::write(&cast_file, before)?;
			return Err(error).with_context(|| format!("could not write {}", puppet.display()));
		}
	}

	// A brief the cast file can't read (an unknown colour or extra) is undone, so the world
	// still loads.
	match world.cast() {
		Ok(cast) => {
			let member = cast
				.member(&brief.id)
				.context("the new character is missing from the cast")?;
			let report = member_report(world, member);

			if brief.draw && report.puppet.is_none() {
				std::fs::write(&cast_file, before)?;
				let _ = std::fs::remove_file(&puppet);
				bail!(
					"`{}`'s `puppet` line was not kept; nothing was added",
					brief.id
				);
			}

			Ok(report)
		}
		Err(error) => {
			std::fs::write(&cast_file, before)?;

			if brief.draw {
				let _ = std::fs::remove_file(&puppet);
			}

			Err(error.context(format!("`{}` was not added", brief.id)))
		}
	}
}

/// Starts a world's own setting: a copy of a setting it can already use (to repaint it), or a
/// sky over the ground.
pub fn create_setting(
	world: &World,
	name: &str,
	about: &str,
	from: Option<&str>,
) -> Result<SettingSummary> {
	check_slug("setting", name)?;
	let file = world::under(
		&world.dir.join("settings"),
		"setting",
		format!("{name}.svg"),
	)?;
	ensure!(
		!file.exists(),
		"{} already has its own `{name}`",
		world.name
	);
	let source = match from {
		Some(base) => {
			check_slug("setting", base)?;
			let own = world::under(
				&world.dir.join("settings"),
				"setting",
				format!("{base}.svg"),
			)?;

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
	std::fs::create_dir_all(world.dir.join("settings"))
		.with_context(|| format!("could not create {}", world.dir.join("settings").display()))?;
	std::fs::write(&file, text)?;

	Ok(SettingSummary {
		name: name.to_owned(),
		about: about.to_owned(),
		from: "world",
	})
}

/// The props a world's books can use: the built-in ones and its own.
pub fn list_props(world: &World) -> Result<Vec<PropSummary>> {
	world
		.props()?
		.props()
		.map(|prop| {
			// A name read from the folder's file names: prove the file it names stays in the
			// world before asking whether it is there.
			let from = if own_prop(world, &prop.name)?.is_file() {
				"world"
			} else {
				"built-in"
			};
			Ok(PropSummary::of(prop, from))
		})
		.collect()
}

/// Starts a world's own prop: a copy of a prop it can use (to redraw it), or a plain shape with
/// its grip at the fist.
pub fn create_prop(
	world: &World,
	name: &str,
	about: &str,
	from: Option<&str>,
) -> Result<PropSummary> {
	let file = own_prop(world, name)?;
	ensure!(
		!file.exists(),
		"{} already has its own `{name}`",
		world.name
	);
	let source = match from {
		Some(base) => prop_source(world, base)?,
		None => {
			PropFile {
				about: about.to_owned(),
				parts: vec![
					(
						"body".into(),
						r#"<path id="body" d="M-30 -60 C-10 -66 10 -66 30 -60 C36 -40 36 -20 30 0 C10 6 -10 6 -30 0 C-36 -20 -36 -40 -30 -60 Z" fill="teal" data-shade="1"/>"#
							.into(),
					),
				],
				..PropFile::default()
			}
			.write()
		}
	};
	let mut prop_file = PropFile::read(&source)?;
	about.clone_into(&mut prop_file.about);
	let text = prop_file.write();
	let prop = props::Prop::parse(name, &text, &world.style()?)?;
	std::fs::create_dir_all(world.dir.join("props"))
		.with_context(|| format!("could not create {}", world.dir.join("props").display()))?;
	std::fs::write(&file, text)?;

	Ok(PropSummary::of(&prop, "world"))
}

/// Where a world's own prop file is, or would be: always inside its `props/`.
fn own_prop(world: &World, name: &str) -> Result<PathBuf> {
	check_slug("prop", name)?;
	world::under(&world.dir.join("props"), "prop", format!("{name}.svg"))
}

/// A prop's file text: the world's own, else the built-in one of that name.
fn prop_source(world: &World, name: &str) -> Result<String> {
	let own = own_prop(world, name)?;

	if own.is_file() {
		return Ok(std::fs::read_to_string(&own)?);
	}

	Ok(props::built_in_source(name)
		.with_context(|| format!("there is no prop `{name}`"))?
		.to_owned())
}

/// A character's own parts, and where a redraw of them is written.
struct OwnParts {
	parts: Parts,
	/// The style's kit, which the parts are drawn over.
	kit: Parts,
	/// The part file: the character's own, or where theirs will be written when they are still
	/// the style's template.
	file: PathBuf,
	/// The sheet title, so rewriting keeps the file's own.
	title: String,
	/// Whether the parts are still the style's untouched template.
	template: bool,
	/// The style the parts are drawn in, and the body they are drawn for, so a redraw can be
	/// posed and its joins measured.
	style: std::sync::Arc<StylePack>,
	build: Build,
}

/// A character's own parts, or why they have none to redraw.
fn own_parts(world: &World, id: &str) -> Result<OwnParts> {
	check_slug("character", id)?;
	let cast = world.cast()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}'s cast", world.name))?;
	let Kind::Figure(look) = &member.kind else {
		bail!("`{id}` is not a person, so has no parts");
	};
	let Some(puppet) = &look.puppet else {
		bail!(
			"`{id}` is drawn by the parametric puppet, so has no parts to redraw; give them a \
			 drawing with `fables puppet template {id}`"
		)
	};
	let file = if puppet.template {
		world::under(
			&world.dir,
			"puppet",
			Path::new("puppets").join(format!("{id}.svg")),
		)?
	} else {
		puppet.file.clone()
	};
	// A character still drawn from the template has the template for their body as their own.
	let parts = if puppet.template {
		puppet.style.template_with(look.build, &puppet.picks)?
	} else {
		Parts::load(&puppet.file)?
	};
	let title = if puppet.template {
		format!(
			"{}: own parts (started from {}; redraw each one)",
			member.name,
			puppet.style.template_origin(look.build)
		)
	} else {
		first_title(&puppet.file).unwrap_or_else(|| format!("{id}: own parts"))
	};

	Ok(OwnParts {
		parts,
		kit: puppet.style.kit.clone(),
		file,
		title,
		template: puppet.template,
		style: puppet.style.clone(),
		build: look.build,
	})
}

/// A character's parts as path data: their own parts, or the named parts from wherever the
/// character gets them (their file, their style's template or the kit).
pub fn character_parts(world: &World, id: &str, names: &[String]) -> Result<Vec<PartReport>> {
	let own = own_parts(world, id)?;
	let from = if own.template { "template" } else { "own" };
	let report = |name: &str, shape: &crate::art::shape::Shape, from| {
		PartReport {
			name: name.to_owned(),
			d: shape.path().as_str().to_owned(),
			from,
		}
	};

	if names.is_empty() {
		return Ok(own
			.parts
			.iter()
			.map(|(name, shape)| report(name, shape, from))
			.collect());
	}

	names
		.iter()
		.map(|name| {
			if let Some(shape) = own.parts.get(name) {
				Ok(report(name, shape, from))
			} else if let Some(shape) = own.kit.get(name) {
				Ok(report(name, shape, "kit"))
			} else {
				bail!("`{id}` has no part `{name}`")
			}
		})
		.collect()
}

/// Writes or replaces parts in a character's file and removes others, then checks the figure.
/// Changes that leave errors are not written; the findings say why. A character drawn from
/// their style's template is given a file of their own, seeded with the template, and their
/// cast entry is pointed at it.
pub fn set_parts(
	world: &World,
	id: &str,
	parts: &[PartData],
	remove: &[String],
) -> Result<PartsUpdate> {
	let own = own_parts(world, id)?;
	let mut drawings = own.parts;
	let mut problems = Vec::new();

	for part in parts {
		match parts::read_part(&part.name, &part.d) {
			Ok(shape) => drawings.insert(&part.name, shape),
			Err(error) => problems.push(format!("{error:#}")),
		}
	}

	for name in remove {
		if drawings.remove(name).is_none() {
			problems.push(format!("`{name}` is not one of `{id}`'s own parts"));
		}
	}

	ensure!(
		problems.is_empty(),
		"nothing was written:\n  {}",
		problems.join("\n  ")
	);

	let figure = drawings.clone().over(own.kit.clone());
	let mut findings = figure.check();
	let broken =
		|findings: &[parts::Finding]| findings.iter().any(|finding| finding.level == Level::Error);

	// A whole figure must also join up on the character's own body, posed: a redraw that
	// leaves a gap at the neck, a shoulder or a hip is refused like any other broken part.
	if !broken(&findings) {
		let posed = crate::art::puppet::Authored::from_parts(&own.file, figure, &own.style)?;
		findings.extend(crate::art::puppet::joins(&posed, own.build));
	}

	let written = !broken(&findings);

	if written {
		if own.template {
			ensure!(
				!own.file.exists(),
				"{} already exists, but the cast doesn't name it; add `puppet = \
				 \"puppets/{id}.svg\"` to [{id}] in the cast file, or move the file aside",
				own.file.display()
			);
		}

		std::fs::create_dir_all(own.file.parent().unwrap_or(Path::new("")))
			.with_context(|| format!("could not create {}", own.file.display()))?;
		let drawings: Vec<(&str, &crate::art::shape::Shape)> = drawings.iter().collect();
		std::fs::write(&own.file, parts::write(&own.title, &drawings))
			.with_context(|| format!("could not write {}", own.file.display()))?;

		if own.template
			&& let Err(error) = point_cast_at(world, id, &format!("puppets/{id}.svg"))
		{
			let _ = std::fs::remove_file(&own.file);
			return Err(error);
		}
	}

	Ok(PartsUpdate {
		written,
		own_parts: drawings.iter().count(),
		findings: findings.iter().map(ToString::to_string).collect(),
	})
}

/// Starts a character's own part file (from the style's template for their body, or another
/// cast member's drawings) and points their cast entry at it. Either order works: add the cast
/// entry first and run this, or write the file first and name it in the entry.
pub fn template_puppet(world: &World, id: &str, from: Option<&str>) -> Result<MemberReport> {
	check_slug("character", id)?;
	let cast = world.cast()?;
	let style = world.style()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}'s cast", world.name))?;
	let Kind::Figure(look) = &member.kind else {
		bail!("`{id}` is not a person, so has no puppet");
	};

	if let Some(puppet) = &look.puppet
		&& !puppet.template
	{
		bail!(
			"`{id}` already has a part file ({}); edit it, or move it aside to start again",
			puppet.file.display()
		);
	}

	let target = world::under(
		&world.dir,
		"puppet",
		Path::new("puppets").join(format!("{id}.svg")),
	)?;
	ensure!(
		!target.exists(),
		"{} already exists; edit it, or move it aside to start again",
		target.display()
	);

	let (parts, origin) = if let Some(from) = from {
		let source = own_parts(world, from)?;
		(source.parts, format!("{from}'s drawings"))
	} else {
		// The template drawn for their body, with the hair, brows and nose they have: a
		// grown-up's parts are cut for a grown-up.
		let picks = look
			.puppet
			.as_ref()
			.map(|puppet| puppet.picks.clone())
			.unwrap_or_default();
		(
			style.template_with(look.build, &picks)?,
			style.template_origin(look.build),
		)
	};
	let title = format!(
		"{}: own parts (started from {origin}; redraw each one)",
		member.name
	);
	let drawings: Vec<(&str, &crate::art::shape::Shape)> = parts.iter().collect();
	std::fs::create_dir_all(world.dir.join("puppets"))
		.with_context(|| format!("could not create {}", world.dir.join("puppets").display()))?;
	std::fs::write(&target, parts::write(&title, &drawings))
		.with_context(|| format!("could not write {}", target.display()))?;

	if let Err(error) = point_cast_at(world, id, &format!("puppets/{id}.svg")) {
		let _ = std::fs::remove_file(&target);
		return Err(error);
	}

	let cast = world.cast()?;
	let member = cast.member(id).context("the character left the cast")?;
	Ok(member_report(world, member))
}

/// Points a cast entry at `file`, relative to the cast file, and puts it back if the cast stops
/// loading.
fn point_cast_at(world: &World, id: &str, file: &str) -> Result<()> {
	let cast_file = world.cast_file();
	let before = std::fs::read_to_string(&cast_file)?;
	let after = with_puppet(&before, id, file)
		.with_context(|| format!("`{id}` has no table in {}", cast_file.display()))?;
	std::fs::write(&cast_file, &after)
		.with_context(|| format!("could not write {}", cast_file.display()))?;

	match world.cast() {
		Ok(_) => Ok(()),
		Err(error) => {
			std::fs::write(&cast_file, before)?;
			Err(error.context(format!("`{id}`'s puppet line was not kept")))
		}
	}
}

/// A cast file with `puppet = "…"` set in `id`'s table, added after its other keys when it has
/// none. `None` when the file has no such table.
fn with_puppet(source: &str, id: &str, file: &str) -> Option<String> {
	let header = format!("[{id}]");
	let setting = format!("puppet = {}", quoted(file));
	let mut lines = Vec::new();
	let mut inside = false;
	let mut done = false;

	for line in source.lines() {
		let trimmed = line.trim();

		if trimmed.starts_with('[') {
			if inside && !done {
				insert_after_last_line(&mut lines, &setting);
				done = true;
			}

			inside = trimmed == header;
		}

		if inside
			&& !done && trimmed
			.strip_prefix("puppet")
			.is_some_and(|rest| rest.trim_start().starts_with('='))
		{
			lines.push(setting.clone());
			done = true;
			continue;
		}

		lines.push(line.to_owned());
	}

	if inside && !done {
		insert_after_last_line(&mut lines, &setting);
		done = true;
	}

	done.then(|| lines.join("\n") + "\n")
}

/// Adds a line after the last non-blank one, so a blank line still separates TOML tables.
fn insert_after_last_line(lines: &mut Vec<String>, line: &str) {
	let at = lines
		.iter()
		.rposition(|line| !line.trim().is_empty())
		.map_or(lines.len(), |index| index + 1);
	lines.insert(at, line.to_owned());
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
		author: world.author.clone(),
	}
}

fn member_report(world: &World, member: &Member) -> MemberReport {
	let (puppet, drawn) = match &member.kind {
		Kind::Figure(look) => {
			match &look.puppet {
				Some(puppet) if puppet.template => (None, Some("template")),
				Some(puppet) => {
					(
						Some(
							puppet
								.file
								.strip_prefix(&world.dir)
								.unwrap_or(&puppet.file)
								.display()
								.to_string(),
						),
						Some("puppet"),
					)
				}
				None => (None, Some("parametric")),
			}
		}
		_ => (None, None),
	};

	MemberReport {
		id: member.id.clone(),
		name: member.name.clone(),
		kind: member.kind.name().to_owned(),
		habit: member.habit.clone(),
		about: member.about.clone(),
		puppet,
		drawn,
	}
}

/// The variants a cast file's members use, as `(kind, field, word)`: what a new character's
/// variants are picked to differ from. A cast that doesn't read gives none.
fn cast_words(cast: &str) -> Vec<(String, String, String)> {
	let Ok(table) = cast.parse::<toml::Table>() else {
		return Vec::new();
	};

	table
		.values()
		.filter_map(toml::Value::as_table)
		.flat_map(|entry| {
			let kind = entry
				.get("kind")
				.and_then(toml::Value::as_str)
				.unwrap_or("kid")
				.to_owned();

			["hair", "build", "brows", "nose"]
				.into_iter()
				.filter_map(move |field| {
					let word = entry.get(field)?.as_str()?;
					Some((kind.clone(), field.to_owned(), word.to_owned()))
				})
		})
		.collect()
}

/// A cast table for a new character, with a `puppet` line only when one was written, the
/// variants they are drawn from the template with (`picks`), and how heavily built they are.
fn cast_entry(brief: &CharacterBrief, picks: &Picks, physique: Physique) -> String {
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
	let puppet = format!("puppets/{}.svg", brief.id);
	let mut keys: Vec<(&str, &str)> = vec![("kind", brief.kind.name())];

	if brief.draw {
		keys.push(("puppet", &puppet));
	}

	if let Some(hair) = &picks.hair {
		keys.push(("hair", hair));
	}

	keys.push(("build", physique.name()));

	for (key, value) in [("brows", &picks.brows), ("nose", &picks.nose)] {
		if let Some(value) = value {
			keys.push((key, value));
		}
	}

	for (key, value) in [
		("skin", &colours.skin),
		("hair-colour", &colours.hair),
		("top", &colours.top),
		("trim", &colours.trim),
		("bottom", &colours.bottom),
		("shoes", &colours.shoes),
		("eyes", &colours.eyes),
	] {
		keys.push((key, value));
	}

	for (key, value) in keys {
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

/// The commented example a new world's cast starts with: no members, and one written out to
/// copy.
fn cast_example(name: &str, style: &str) -> String {
	format!(
		"# The folk of {name}. Every table is one character.\n#\n# A person has a `kind` of kid \
		 or grown-up, and colours from the palette (or #rrggbb).\n# Without a `puppet` line they \
		 are drawn from the {style} style's template in their own\n# colours. `fables character \
		 new <id> --name \"…\"` writes this table for you.\n#\n# [pip]\n# name = \"Pip \
		 Pocket\"\n# kind = \"kid\"\n# skin = \"tangerine\"\n# hair-colour = \"plum\"\n# top = \
		 \"teal\"\n# trim = \"cream\"\n# bottom = \"night\"\n# shoes = \"ink\"\n# eyes = \
		 \"cocoa\"\n# pattern = \"plain\"\n"
	)
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
/// The ways a style draws a feature on a build's body, its own first and then the rest in
/// alphabetical order.
fn own_first(pack: &StylePack, build: Build, feature: Feature) -> Vec<String> {
	let own = pack.own(build, feature);

	std::iter::once(own)
		.chain(
			pack.offers(build, feature)
				.into_iter()
				.filter(|offer| *offer != own),
		)
		.map(str::to_owned)
		.collect()
}

/// A variants sheet's lineups for a style, body by body: every hair on one head, every brows
/// and nose on one face, and every build on one body, each otherwise the plain template.
fn variant_lineups(pack: &std::sync::Arc<StylePack>) -> Result<Vec<sheets::Lineup>> {
	let mut lineups = Vec::new();

	for build in [Build::Kid, Build::Grownup] {
		let body = body_name(build);
		let look =
			|picks: Picks, physique| crate::cast::template_look_with(pack, build, &picks, physique);
		let heads = |feature: Feature| -> Result<Vec<(String, crate::art::figure::Look)>> {
			own_first(pack, build, feature)
				.into_iter()
				.map(|offer| {
					let pick = Some(offer.clone());
					let picks = match feature {
						Feature::Hair => {
							Picks {
								hair: pick,
								..Picks::default()
							}
						}
						Feature::Brows => {
							Picks {
								brows: pick,
								..Picks::default()
							}
						}
						Feature::Nose => {
							Picks {
								nose: pick,
								..Picks::default()
							}
						}
					};
					Ok((offer, look(picks, Physique::Plain)?))
				})
				.collect()
		};
		let mut faces = heads(Feature::Brows)?
			.into_iter()
			.map(|(name, look)| (format!("brows: {name}"), look))
			.collect::<Vec<_>>();
		faces.extend(
			heads(Feature::Nose)?
				.into_iter()
				.map(|(name, look)| (format!("nose: {name}"), look)),
		);

		lineups.push(sheets::Lineup {
			title: format!("{body}: hair"),
			crop: sheets::Crop::Head,
			looks: heads(Feature::Hair)?,
		});
		lineups.push(sheets::Lineup {
			title: format!("{body}: brows and noses"),
			crop: sheets::Crop::Head,
			looks: faces,
		});
		lineups.push(sheets::Lineup {
			title: format!("{body}: builds"),
			crop: sheets::Crop::Body,
			looks: Physique::ALL
				.into_iter()
				.map(|physique| {
					Ok((
						physique.name().to_owned(),
						look(Picks::default(), physique)?,
					))
				})
				.collect::<Result<_>>()?,
		});
	}

	Ok(lineups)
}

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
			kind: BuildName::Kid,
			build: None,
			hair: None,
			brows: None,
			nose: None,
			born: None,
			about: Some("Bites her nails.".into()),
			habit: Some("Nibbles her nails, so her fingers are tiny stubs.".into()),
			colours: colours(),
			pattern: Some("stripes".into()),
			extras: vec!["freckles".into()],
			draw: false,
		}
	}

	#[test]
	fn a_new_character_gets_variants_of_their_own_written_into_the_cast() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					hair: Some("bun".into()),
					build: Some(PhysiqueName::Stout),
					..brief("ned")
				},
			)
			.unwrap();
		let world = studio.world("nibbleton").unwrap();
		let cast = std::fs::read_to_string(world.cast_file()).unwrap();
		let table: toml::Table = cast.parse().unwrap();
		let word = |id: &str, field: &str| -> String {
			table[id][field]
				.as_str()
				.unwrap_or_else(|| panic!("{id} has no {field}: {cast}"))
				.to_owned()
		};

		assert_eq!(word("ned", "hair"), "bun");
		assert_ne!(
			word("nell", "hair"),
			"bun",
			"Nell came first, but Ned asked"
		);
		assert_eq!(word("ned", "build"), "stout");
		assert_ne!(
			word("nell", "brows"),
			word("ned", "brows"),
			"Ned gets the brows the cast uses least"
		);
		assert_ne!(word("nell", "nose"), word("ned", "nose"));
		assert_eq!(cast_words(&cast).len(), 8, "four words each");

		// The cast reads back as written, so the picks never change afterwards.
		let ned = world.cast().unwrap();
		let Kind::Figure(look) = &ned.member("ned").unwrap().kind else {
			panic!("ned is a person");
		};
		let puppet = look.puppet.as_ref().unwrap();
		assert_eq!(puppet.physique, Physique::Stout);
		assert_eq!(puppet.picks.hair.as_deref(), Some("bun"));
	}

	#[test]
	fn a_variant_the_style_does_not_draw_adds_nobody() {
		let (_dir, studio) = studio();
		let error = studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					hair: Some("mohawk".into()),
					..brief("nell")
				},
			)
			.unwrap_err();
		assert!(format!("{error:#}").contains("pigtails"), "{error:#}");

		let error = studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					nose: Some("none".into()),
					draw: true,
					..brief("nell")
				},
			)
			.unwrap_err();
		assert!(format!("{error:#}").contains("part file"), "{error:#}");

		let world = studio.world("nibbleton").unwrap();
		assert!(world.cast().unwrap().member("nell").is_none());
		assert!(!world.dir.join("puppets/nell.svg").exists());
	}

	#[test]
	fn a_drawn_character_s_file_draws_their_variants() {
		let (_dir, studio) = studio();
		studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					hair: Some("bob".into()),
					brows: Some("thin".into()),
					draw: true,
					..brief("nell")
				},
			)
			.unwrap();
		let world = studio.world("nibbleton").unwrap();
		let file = Parts::load(&world.dir.join("puppets/nell.svg")).unwrap();
		let style = world.style().unwrap();
		let bob = style
			.figure(
				Build::Kid,
				&Picks {
					hair: Some("bob".into()),
					brows: Some("thin".into()),
					..Picks::default()
				},
			)
			.unwrap();

		assert_eq!(file.get("hair.front"), bob.get("hair.front"));
		assert_eq!(file.get("brow"), bob.get("brow"));
		assert!(
			file.get("tuft.front").is_none(),
			"the bob replaces the tuft"
		);
		let cast = std::fs::read_to_string(world.cast_file()).unwrap();
		assert!(!cast.contains("hair = "), "the file draws the hair: {cast}");
		assert!(
			cast.contains("build = "),
			"the build is still the cast's: {cast}"
		);
	}

	fn studio() -> (tempfile::TempDir, Studio) {
		let dir = tempfile::tempdir().unwrap();
		let studio = Studio::new(dir.path());
		studio
			.create_world(&WorldBrief {
				name: "Nibbleton".into(),
				about: Some("A town of small habits.".into()),
				style: None,
				author: Some("A. Writer".into()),
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
					author: None,
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
		assert_eq!(member.puppet, None, "no part file is written by default");
		assert_eq!(member.drawn, Some("template"));
		assert_eq!(member.kind, "kid");

		let images = studio
			.character_sheets("nibbleton", "nell", &[SheetKind::Model])
			.unwrap();
		assert_eq!(images.len(), 1);
		assert!(images[0].png.starts_with(b"\x89PNG"));

		let drawn = studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					id: "gus".into(),
					draw: true,
					..brief("gus")
				},
			)
			.unwrap();
		assert_eq!(drawn.puppet.as_deref(), Some("puppets/gus.svg"));
		assert_eq!(drawn.drawn, Some("puppet"));
	}

	#[test]
	fn a_drawn_character_and_a_template_character_both_draw_the_same_figure() {
		let (_dir, studio) = studio();
		let plain = studio.create_character("nibbleton", &brief("pip")).unwrap();
		let drawn = studio
			.create_character(
				"nibbleton",
				&CharacterBrief {
					id: "mum".into(),
					draw: true,
					..brief("mum")
				},
			)
			.unwrap();
		assert_eq!(plain.drawn, Some("template"));
		assert_eq!(drawn.drawn, Some("puppet"));

		let world = studio.world("nibbleton").unwrap();
		let cast = world.cast().unwrap();
		let head = |id: &str| {
			let Kind::Figure(look) = cast.resolve(id).unwrap() else {
				panic!("{id} is a person");
			};
			let puppet = look.puppet.as_ref().expect("both are drawn from parts");
			(
				puppet.template,
				puppet
					.parts
					.get("head.front")
					.map(crate::art::shape::Shape::exact_data),
			)
		};
		assert!(head("pip").0);
		assert!(!head("mum").0);
		assert_eq!(
			head("pip").1,
			head("mum").1,
			"the template and the untouched file draw the same parts"
		);
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
		let own = studio.character_parts("nibbleton", "nell", &[]).unwrap();
		assert!(
			own.iter().all(|part| part.from == "template"),
			"a character with no file reads their style's template"
		);

		// A bigger ear of her own: writing it gives her a file, and points the cast at it.
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
		let world = studio.world("nibbleton").unwrap();
		let cast = world.cast().unwrap();
		let Kind::Figure(look) = cast.resolve("nell").unwrap() else {
			panic!("nell is a person");
		};
		assert!(
			look.puppet.is_some_and(|puppet| !puppet.template),
			"the change was written to a file of her own"
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
	fn a_grown_up_starts_from_the_grown_up_template_and_a_redraw_that_opens_a_join_is_refused() {
		let dir = tempfile::tempdir().unwrap();
		let studio = Studio::new(dir.path());
		studio
			.create_world(&WorldBrief {
				name: "Hoop Lane".into(),
				about: None,
				style: Some("rubber-hose".into()),
				author: None,
			})
			.unwrap();
		let grown_up = |id: &str, draw| {
			CharacterBrief {
				kind: BuildName::GrownUp,
				extras: Vec::new(),
				draw,
				// The style's own, so Pop's file is the template's parts and no more.
				hair: Some("short".into()),
				brows: Some("arched".into()),
				nose: Some("round".into()),
				..brief(id)
			}
		};
		studio
			.create_character("hoop-lane", &grown_up("gran", false))
			.unwrap();
		studio
			.create_character("hoop-lane", &grown_up("pop", true))
			.unwrap();

		// Both start from the grown-up's torso, not the kid's: Gran reads it as her own parts,
		// and Pop's file was written with it (as `character_create` with `draw` does).
		let style = StylePack::named("rubber-hose").unwrap();
		let torso = |parts: &Parts| parts.get("torso.front").unwrap().path().as_str().to_owned();
		let grown_up_torso = torso(&style.template_for(Build::Grownup));
		assert_ne!(grown_up_torso, torso(&style.template));
		let gran = studio
			.character_parts("hoop-lane", "gran", &["torso.front".into()])
			.unwrap();
		assert_eq!(
			(gran[0].from, gran[0].d.as_str()),
			("template", grown_up_torso.as_str())
		);
		let world = studio.world("hoop-lane").unwrap();
		let pop = Parts::load(&world.dir.join("puppets/pop.svg")).unwrap();
		assert!(
			pop.get("tuft.front").is_none(),
			"a grown-up has no kid's tuft"
		);
		assert_eq!(
			pop.iter().count(),
			style.template_for(Build::Grownup).iter().count()
		);

		// The kid's torso on a grown-up leaves the head and arms in the air: the whole figure
		// still draws, but the redraw is refused with the gaps, and nothing is written.
		let kid_torso = |view: &str| {
			PartData {
				name: format!("torso.{view}"),
				d: style
					.template
					.get(&format!("torso.{view}"))
					.unwrap()
					.path()
					.as_str()
					.to_owned(),
			}
		};
		let update = studio
			.set_parts(
				"hoop-lane",
				"gran",
				&[kid_torso("front"), kid_torso("3q")],
				&[],
			)
			.unwrap();
		assert!(!update.written);
		assert!(
			update
				.findings
				.iter()
				.any(|finding| finding.contains("the head and its neck are clear of the torso")),
			"{:#?}",
			update.findings
		);
		assert!(!world.dir.join("puppets/gran.svg").exists());

		// The same redraw is refused for a grown-up with a file of their own, which keeps it.
		let before = std::fs::read_to_string(world.dir.join("puppets/pop.svg")).unwrap();
		let update = studio
			.set_parts(
				"hoop-lane",
				"pop",
				&[kid_torso("front"), kid_torso("3q")],
				&[],
			)
			.unwrap();
		assert!(!update.written, "{:#?}", update.findings);
		assert_eq!(
			std::fs::read_to_string(world.dir.join("puppets/pop.svg")).unwrap(),
			before
		);
	}

	#[test]
	fn the_circular_order_works_from_either_end() {
		let (dir, studio) = studio();

		// A cast entry first: without a `puppet` line the world loads and draws the template,
		// then `template_puppet` writes the file and adds the line.
		std::fs::write(
			dir.path().join("worlds/nibbleton/cast.toml"),
			"[pip]\nname = \"Pip\"\nskin = \"teal\"\n",
		)
		.unwrap();
		let report = studio.template_puppet("nibbleton", "pip", None).unwrap();
		assert_eq!(report.puppet.as_deref(), Some("puppets/pip.svg"));
		let world = studio.world("nibbleton").unwrap();
		let file = std::fs::read_to_string(world.cast_file()).unwrap();
		assert!(file.contains("puppet = \"puppets/pip.svg\""), "{file}");

		// A file first: write the parts beside the cast, name them in the entry, and load.
		let root = tempfile::tempdir().unwrap();
		let other = Studio::new(root.path());
		other
			.create_world(&WorldBrief {
				name: "Nibbleton".into(),
				about: None,
				style: Some("chibi".into()),
				author: None,
			})
			.unwrap();
		let world = other.world("nibbleton").unwrap();
		let style = world.style().unwrap();
		let drawings: Vec<(&str, &crate::art::shape::Shape)> = style.template.iter().collect();
		std::fs::create_dir_all(world.dir.join("puppets")).unwrap();
		std::fs::write(
			world.dir.join("puppets/rosa.svg"),
			parts::write("Rosa: own parts", &drawings),
		)
		.unwrap();
		std::fs::write(
			world.cast_file(),
			"[rosa]\nname = \"Rosa\"\npuppet = \"puppets/rosa.svg\"\nskin = \"teal\"\n",
		)
		.unwrap();
		other
			.character_sheets("nibbleton", "rosa", &[SheetKind::Model])
			.unwrap();

		// `--draw` does both at once.
		let drawn = other
			.create_character(
				"nibbleton",
				&CharacterBrief {
					id: "gus".into(),
					draw: true,
					..brief("gus")
				},
			)
			.unwrap();
		assert_eq!(drawn.puppet.as_deref(), Some("puppets/gus.svg"));
		assert_eq!(drawn.drawn, Some("puppet"));
	}

	#[test]
	fn the_parametric_puppet_has_no_parts_to_redraw_but_still_draws() {
		let (dir, studio) = studio();
		std::fs::write(
			dir.path().join("worlds/nibbleton/cast.toml"),
			"[old]\nname = \"Old\"\npuppet = \"parametric\"\nskin = \"teal\"\ntop = \"tomato\"\n",
		)
		.unwrap();
		let world = studio.world("nibbleton").unwrap();
		let cast = world.cast().unwrap();
		let Kind::Figure(look) = cast.resolve("old").unwrap() else {
			panic!("old is a person");
		};
		assert!(look.puppet.is_none(), "the old parametric puppet");

		for error in [
			studio.character_parts("nibbleton", "old", &[]).unwrap_err(),
			studio
				.set_parts(
					"nibbleton",
					"old",
					&[PartData {
						name: "ear".into(),
						d: "M0 -20 C-16 -32 -36 -20 -34 2 Z".into(),
					}],
					&[],
				)
				.unwrap_err(),
		] {
			assert!(format!("{error:#}").contains("parametric"), "{error:#}");
		}

		// It still draws: a page with the old puppet renders.
		studio
			.write_story(
				"nibbleton",
				"old-story",
				"+++\ntitle = \"Old\"\nauthor = \"A\"\nstar = \"old\"\n+++\n# park\n@old x=0.5 \
				 grin\nHello.\n",
			)
			.unwrap();
		studio.render_page("nibbleton", "old-story", 4).unwrap();
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
	fn a_prop_is_started_redrawn_and_rendered() {
		let (_dir, studio) = studio();
		let blank = studio
			.create_prop("nibbleton", "kite-spool", "A spool of kite string", None)
			.unwrap();
		assert_eq!(blank.from, "world");
		assert_eq!(blank.hands, "one");
		assert_eq!(
			studio.prop_parts("nibbleton", "kite-spool").unwrap().len(),
			1
		);

		studio
			.set_prop_parts(
				"nibbleton",
				"kite-spool",
				&[PropPart {
					id: "winding".into(),
					markup: r#"<path id="winding" d="M-20 -50 L20 -50 L20 -30 L-20 -30 Z" fill="cream" data-line="detail"/>"#.into(),
				}],
				&[],
			)
			.unwrap();
		let parts = studio.prop_parts("nibbleton", "kite-spool").unwrap();
		assert_eq!(
			parts
				.iter()
				.map(|part| part.id.as_str())
				.collect::<Vec<_>>(),
			["body", "winding"]
		);
		let image = studio.render_prop("nibbleton", "kite-spool").unwrap();
		assert!(image.png.starts_with(b"\x89PNG"));
		assert!(image.label.starts_with("kite-spool:"));

		// A redraw starts from a built-in; a broken part is refused and the file is kept.
		studio
			.create_prop("nibbleton", "kite", "Our kite", Some("kite"))
			.unwrap();
		let before = std::fs::read_to_string(
			Path::new(&studio.root).join("worlds/nibbleton/props/kite.svg"),
		)
		.unwrap();
		let error = studio
			.set_prop_parts(
				"nibbleton",
				"kite",
				&[PropPart {
					id: "sail".into(),
					markup: r#"<path id="sail" d="M0 0 C1 1 2 2 3 3 Z" fill="chartreuse"/>"#.into(),
				}],
				&[],
			)
			.unwrap_err();
		assert!(format!("{error:#}").contains("chartreuse"), "{error:#}");
		assert_eq!(
			std::fs::read_to_string(
				Path::new(&studio.root).join("worlds/nibbleton/props/kite.svg")
			)
			.unwrap(),
			before,
			"nothing was written"
		);
	}

	#[test]
	fn prop_tools_refuse_names_that_are_not_slugs() {
		let (_dir, studio) = studio();

		for bad in ["../escape", "Kite", "kite spool", ""] {
			let error = format!(
				"{:#}",
				studio.create_prop("nibbleton", bad, "x", None).unwrap_err()
			);
			assert!(error.contains("not a prop name"), "{bad}: {error}");

			let error = format!("{:#}", studio.prop_parts("nibbleton", bad).unwrap_err());
			assert!(error.contains("not a prop name"), "{bad}: {error}");
		}
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
		let (dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		studio
			.create_setting("nibbleton", "harbour", "The harbour wall", None)
			.unwrap();

		for name in ["../etc", "/etc/passwd", "..", "a/b", "", "Nell"] {
			assert!(studio.world(name).is_err(), "world {name:?}");
			assert!(
				studio.character_parts("nibbleton", name, &[]).is_err(),
				"character {name:?}"
			);
			assert!(
				studio.setting_parts("nibbleton", name).is_err(),
				"setting {name:?}"
			);
			assert!(
				studio.read_story("nibbleton", name).is_err(),
				"story {name:?}"
			);
			assert!(
				studio
					.write_story(
						"nibbleton",
						name,
						"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park\nHi.\n"
					)
					.is_err(),
				"written story {name:?}"
			);
		}

		assert_eq!(studio.worlds().unwrap().len(), 1, "no world was created");
		assert!(
			!dir.path().join("etc").exists() && !dir.path().join("worlds/etc").exists(),
			"nothing was created outside the worlds folder"
		);
	}

	#[test]
	fn a_setting_cannot_start_from_a_path_outside_the_world() {
		let (dir, studio) = studio();
		let outside = tempfile::tempdir().unwrap();
		let decoy = outside.path().join("decoy.svg");
		let parts = "<svg xmlns=\"http://www.w3.org/2000/svg\" data-ground=\"2000\"><path \
		             id=\"near-x\" d=\"M0 0 L100 100 Z\" fill=\"moss\"/></svg>";
		std::fs::write(&decoy, parts).unwrap();
		// A relative `..` needs the settings folder to exist to walk through.
		studio
			.create_setting("nibbleton", "harbour", "The harbour wall", None)
			.unwrap();

		for from in [
			// The tool appends `.svg`, so a decoy is named without it, as the report's
			// reproduction does.
			decoy.with_extension("").display().to_string(),
			decoy.display().to_string(),
			"../../outside/decoy".into(),
			"../harbour".into(),
			"..".into(),
			"/etc/passwd".into(),
		] {
			let error = format!(
				"{:#}",
				studio
					.create_setting("nibbleton", "stolen", "Probe", Some(&from))
					.unwrap_err()
			);

			assert!(error.contains("setting name"), "{from}: {error}");
		}

		assert_eq!(std::fs::read_to_string(&decoy).unwrap(), parts, "untouched");
		assert!(
			!dir.path()
				.join("worlds/nibbleton/settings/stolen.svg")
				.exists(),
			"nothing was copied into the world"
		);
	}

	#[test]
	fn a_failed_write_leaves_a_folder_it_did_not_create_alone() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		let world = studio.world("nibbleton").unwrap();
		let folder = world.stories().join("draft");
		std::fs::create_dir_all(&folder).unwrap();
		std::fs::write(folder.join("notes.txt"), "keep me").unwrap();

		let bad = "+++\ntitle = \"Who\"\nauthor = \"A\"\n+++\n# park\n@nobody wave\nHello.\n";
		let error = format!(
			"{:#}",
			studio.write_story("nibbleton", "draft", bad).unwrap_err()
		);

		assert!(error.contains("nobody"), "{error}");
		assert_eq!(
			std::fs::read_to_string(folder.join("notes.txt")).unwrap(),
			"keep me",
			"the folder's other files survive a refused write"
		);
		assert!(
			!folder.join("story.md").exists(),
			"the new script is rolled back"
		);
	}

	#[test]
	fn a_story_written_over_the_studio_belongs_to_its_world() {
		let (_dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		let script = "+++\ntitle = \"Nell\"\nauthor = \"A\"\nstar = \"nell\"\nworld = \
		              \"../elsewhere\"\n+++\n# park\n@nell happy\nHi.\n";
		let error = format!(
			"{:#}",
			studio
				.write_story("nibbleton", "nell-nibbles", script)
				.unwrap_err()
		);

		assert!(
			error.contains("written into") && error.contains("world"),
			"{error}"
		);
		assert!(
			studio.read_story("nibbleton", "nell-nibbles").is_err(),
			"nothing is left behind"
		);
	}

	#[test]
	fn every_style_describes_its_parts() {
		for (name, _) in Studio::styles().unwrap() {
			let report = Studio::describe_style(&name).unwrap();
			assert!(report.parts.iter().any(|part| part.name == "head"));
			assert!(!report.skin.is_empty(), "{name} names its page skin");
		}
	}

	#[test]
	fn a_world_credits_its_author_and_names_its_drawing_and_skin() {
		let (dir, studio) = studio();
		studio
			.create_character("nibbleton", &brief("nell"))
			.unwrap();
		std::fs::create_dir_all(dir.path().join("worlds/nibbleton/stories/one")).unwrap();
		std::fs::write(
			dir.path().join("worlds/nibbleton/stories/one/story.md"),
			"+++\ntitle = \"One\"\n+++\n# park\n@nell\nHello.\n",
		)
		.unwrap();
		let report = studio.check_story("nibbleton", "one").unwrap();
		assert_eq!(report.drawing, "cartoon");
		assert_eq!(report.skin, "riso");
		assert_eq!(report.title, "One");
		assert!(
			report.notes.is_empty(),
			"nothing is ignored by a template character with no shape fields"
		);

		let world = studio.world("nibbleton").unwrap();
		let mut story = Story::load(&world.stories().join("one/story.md")).unwrap();
		world.furnish(&mut story).unwrap();
		assert_eq!(story.meta.author, "A. Writer");
		assert_eq!(story.drawing_style, "cartoon");
		assert_eq!(story.skin.as_deref(), Some("riso"));
	}
}
