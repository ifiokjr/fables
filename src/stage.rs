//! The stage: what is in a scene and how stage directions change it.
//!
//! A [`Scene`] is the complete state of one page at one moment. Applying the page's directions
//! in order gives the still illustration (the final state) and, snapshot by snapshot, the
//! keyframes the video interpolates between.

use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;

use crate::art::Color;
use crate::art::figure::Build;
use crate::art::figure::Frame;
use crate::art::figure::Look;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::figure::Prop;
use crate::art::figure::Teeth;
use crate::art::light::Light;
use crate::art::pack::StylePack;
use crate::art::rig::Gait;
use crate::art::rig::Stance;
use crate::art::settings::Library;
use crate::art::settings::Setting;
use crate::art::settings::Weather;
use crate::cast::Cast;
use crate::cast::Kind;
use crate::story::Beat;
use crate::story::Directive;
use crate::story::Page;
use crate::story::Story;
use crate::story::TextPlace;
use crate::story::Token;
use crate::story::Transition;

/// Where an actor walks in from or off to (video only; stills just show or hide them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
	Left,
	Right,
}

/// One actor's complete state.
#[derive(Clone, Debug)]
pub struct Actor {
	pub id: String,
	pub kind: Kind,
	/// Horizontal position as a fraction of the stage; `None` means "find me a spot".
	pub x: Option<f32>,
	/// Vertical position (fraction of stage) for things in the air; `None` stands on the ground.
	pub y: Option<f32>,
	pub size: f32,
	/// -1 faces left, 0 faces out of the page, 1 faces right; `None` faces the middle.
	pub facing: Option<f32>,
	pub z: i32,
	pub mood: Mood,
	pub pose: Pose,
	pub mouth: Option<f32>,
	pub teeth: Teeth,
	pub prop: Option<Prop>,
	pub knocked: f32,
	pub stink: f32,
	pub mint: f32,
	pub lively: f32,
	pub wilt: f32,
	pub tint: Option<Color>,
	pub hidden: bool,
	pub enter: Option<Side>,
	pub exit: Option<Side>,
	/// Speaks during the next narration paragraph.
	pub talking: bool,
	/// Has spoken at some point on this page (stills draw speakers mid-sentence).
	pub talked: bool,
	/// Incremented by `hop`; the video bounces whenever it changes.
	pub hops: u32,
	/// What the legs and spine are doing.
	pub stance: Stance,
	/// Walking or running (in place unless the actor is also moving).
	pub gait: Option<Gait>,
}

/// How big a person is drawn on the page. The pages are composed around each build's height
/// (text cards sit above the tallest grown-up's head), so every kid stands as tall as a cartoon
/// kid and every grown-up as tall as the parametric one, whatever their style's body: an
/// authored grown-up, or a slender anime teen, is drawn smaller to fit.
pub fn figure_size(look: &Look) -> f32 {
	match (look.build, &look.puppet) {
		(Build::Kid, None) => 2.25,
		(Build::Kid, Some(_)) => {
			let cartoon = Frame::authored(&StylePack::default_pack(), Build::Kid).height();
			2.25 * (cartoon / look.frame().height())
		}
		(Build::Grownup, Some(_)) => {
			1.85 * Frame::of(Build::Grownup).height() / look.frame().height()
		}
		(Build::Grownup, None) => 1.85,
	}
}

impl Actor {
	fn new(id: &str, kind: Kind) -> Actor {
		let size = match kind {
			Kind::Figure(ref look) => figure_size(look),
			Kind::Bird(_) => 2.0,
			Kind::Dog(_) => 1.8,
			Kind::Worm => 1.4,
			Kind::Toothbrush(_) => 1.5,
			Kind::Flower(..) | Kind::Tree(_) => 1.0,
		};

		Actor {
			id: id.to_owned(),
			kind,
			x: None,
			y: None,
			size,
			facing: None,
			z: 0,
			mood: Mood::Happy,
			pose: Pose::Idle,
			mouth: None,
			teeth: Teeth::Clean,
			prop: None,
			knocked: 0.0,
			stink: 0.0,
			mint: 0.0,
			lively: 0.0,
			wilt: 0.0,
			tint: None,
			hidden: false,
			enter: None,
			exit: None,
			talking: false,
			talked: false,
			hops: 0,
			stance: Stance::Stand,
			gait: None,
		}
	}

	/// The part of the actor that should survive from page to page.
	fn traits(&self) -> Traits {
		Traits {
			teeth: self.teeth,
			stink: self.stink,
			mint: self.mint,
		}
	}
}

/// Character traits that persist across pages until changed (grotty teeth stay grotty).
#[derive(Clone, Copy, Debug)]
pub struct Traits {
	pub teeth: Teeth,
	pub stink: f32,
	pub mint: f32,
}

/// The size of a shot: how much of its subject the camera frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotSize {
	/// The whole setting, as wide as the format allows.
	Wide,
	/// The subjects head to toe, with margin.
	Full,
	/// The subjects from the waist up.
	Medium,
	/// Head and shoulders.
	Close,
	/// The face alone.
	ExtremeClose,
}

impl ShotSize {
	pub const ALL: [&'static str; 5] = ["wide", "full", "medium", "close", "extreme-close"];

	pub fn parse(value: &str) -> Option<ShotSize> {
		match value {
			"wide" => Some(ShotSize::Wide),
			"full" => Some(ShotSize::Full),
			"medium" => Some(ShotSize::Medium),
			"close" => Some(ShotSize::Close),
			"extreme-close" => Some(ShotSize::ExtremeClose),
			_ => None,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			ShotSize::Wide => "wide",
			ShotSize::Full => "full",
			ShotSize::Medium => "medium",
			ShotSize::Close => "close",
			ShotSize::ExtremeClose => "extreme-close",
		}
	}
}

/// How the camera travels to a shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Travel {
	/// Arrive on the cut: film cuts between shots, it does not glide.
	Cut,
	/// Glide, the way the camera always moved.
	Glide,
	/// Glide in, tightening on the subject.
	Push,
	/// Glide out, widening away from the subject.
	Pull,
	/// Glide sideways, showing what is to the camera's left.
	PanLeft,
	/// Glide sideways, showing what is to the camera's right.
	PanRight,
	/// Glide up, showing what is above.
	TiltUp,
	/// Glide down, showing what is below.
	TiltDown,
	/// Follow the subject as it moves.
	Track,
}

impl Travel {
	pub const MOVES: [&'static str; 7] = [
		"push",
		"pull",
		"pan-left",
		"pan-right",
		"tilt-up",
		"tilt-down",
		"track",
	];

	pub fn parse_move(value: &str) -> Option<Travel> {
		match value {
			"push" => Some(Travel::Push),
			"pull" => Some(Travel::Pull),
			"pan-left" => Some(Travel::PanLeft),
			"pan-right" => Some(Travel::PanRight),
			"tilt-up" => Some(Travel::TiltUp),
			"tilt-down" => Some(Travel::TiltDown),
			"track" => Some(Travel::Track),
			_ => None,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Travel::Cut => "cut",
			Travel::Glide => "ease",
			Travel::Push => "push",
			Travel::Pull => "pull",
			Travel::PanLeft => "pan-left",
			Travel::PanRight => "pan-right",
			Travel::TiltUp => "tilt-up",
			Travel::TiltDown => "tilt-down",
			Travel::Track => "track",
		}
	}

	/// Whether the travel is a named move whose length `over` measures.
	pub fn is_move(self) -> bool {
		!matches!(self, Travel::Cut | Travel::Glide)
	}
}

/// Camera framing: a legacy zoom and look point, or a shot size framed on actors.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
	pub zoom: f32,
	pub look: (f32, f32),
	/// A shot size framed on actors; `None` keeps the zoom and look framing.
	pub shot: Option<ShotSize>,
	/// Who the shot frames: actor ids. Empty means everyone on stage, or the current speaker
	/// for a close shot.
	pub on: Vec<String>,
	/// How the camera travels here. `None` means the script did not say: a shot change cuts
	/// (film grammar) and a legacy `zoom=` glides (as it always has).
	pub travel: Option<Travel>,
	/// Seconds a named move takes; zero means the rest of the shot.
	pub over: f32,
	/// A decaying camera shake when this direction lands (0 = none).
	pub shake: f32,
}

impl Default for Camera {
	fn default() -> Self {
		Self {
			zoom: 1.0,
			look: (0.5, 0.5),
			shot: None,
			on: Vec::new(),
			travel: None,
			over: 0.0,
			shake: 0.0,
		}
	}
}

impl Camera {
	/// Whether this camera state names any of the shot vocabulary (`shot=`, `on=`, a move,
	/// `cut`, `ease` or `shake=`), as opposed to the legacy `zoom=` and `look=`.
	pub fn is_directed(&self) -> bool {
		self.shot.is_some() || self.travel.is_some() || self.shake > 0.0 || !self.on.is_empty()
	}
}

/// Everything on stage at one moment.
#[derive(Clone, Debug)]
pub struct Scene {
	pub setting: Setting,
	pub weather: Weather,
	pub actors: Vec<Actor>,
	pub camera: Camera,
	pub text: TextPlace,
}

impl Scene {
	/// An empty scene for a page, with its setting found in `settings` and the page heading's
	/// options applied.
	pub fn for_page(page: &Page, settings: &Library) -> Result<Scene> {
		let setting = settings
			.find(&page.setting)
			.map_err(|error| anyhow::anyhow!("line {}: {error}", page.line))?;
		let mut scene = Scene {
			setting,
			weather: Weather::default(),
			actors: Vec::new(),
			camera: Camera::default(),
			text: page.text_place(),
		};

		for token in &page.options {
			match token.key.as_str() {
				// Page-level choices, not scene state: the book and the timeline read them
				// from the heading.
				"text" | "video" | "book" | "no-wormsley" => {}
				"size" => {
					let value = token.value.as_deref().with_context(|| {
						format!("line {}: `size` needs a scale, e.g. `size=0.9`", page.line)
					})?;
					let scale = number(token, value, page.line)?;
					ensure!(
						scale > 0.0,
						"line {}: `size` must be more than zero, e.g. `size=0.9`",
						page.line
					);
				}
				"transition" => {
					transition(token.value.as_deref(), page.line)?;
				}
				_ => scene.apply_scene_token(token, page.line)?,
			}
		}

		Ok(scene)
	}

	/// Applies one stage direction.
	pub fn apply(
		&mut self,
		directive: &Directive,
		cast: &Cast,
		memory: &mut HashMap<String, Traits>,
	) -> Result<()> {
		let line = directive.line;

		if directive.actor == "scene" || directive.actor == "camera" {
			for token in &directive.tokens {
				self.apply_scene_token(token, line)?;
			}

			return Ok(());
		}

		let index = if let Some(index) = self
			.actors
			.iter()
			.position(|actor| actor.id == directive.actor)
		{
			index
		} else {
			let kind = cast
				.resolve(&directive.actor)
				.map_err(|error| anyhow::anyhow!("line {line}: {error}"))?;
			let mut actor = Actor::new(&directive.actor, kind);

			if let Some(traits) = memory.get(&directive.actor) {
				actor.teeth = traits.teeth;
				actor.stink = traits.stink;
				actor.mint = traits.mint;
			}

			self.actors.push(actor);
			self.actors.len() - 1
		};

		let actor = &mut self.actors[index];

		for token in &directive.tokens {
			apply_actor_token(actor, token, line)?;
		}

		memory.insert(actor.id.clone(), actor.traits());

		Ok(())
	}

	/// Clears per-paragraph speech once a paragraph has been read.
	pub fn end_paragraph(&mut self) {
		for actor in &mut self.actors {
			actor.talking = false;
		}
	}

	fn apply_scene_token(&mut self, token: &Token, line: usize) -> Result<()> {
		let value = token.value.as_deref();

		match (token.key.as_str(), value) {
			("night", None) => self.weather.night = true,
			("night", Some(v)) => self.weather.night = v == "true",
			("day", None) => self.weather.night = false,
			("wilt", Some(v)) => self.weather.wilt = number(token, v, line)?,
			("stink", Some(v)) => self.weather.stink = number(token, v, line)?,
			("mint", Some(v)) => self.weather.mint = number(token, v, line)?,
			("light", Some(v)) => {
				self.weather.light = Light::parse(v).ok_or_else(|| {
					anyhow::anyhow!(
						"line {line}: unknown light `{v}`; try {}",
						crate::art::light::Preset::names()
					)
				})?;
			}
			("tint", Some(v)) => self.weather.tint = colour(v, line)?,
			("speed", Some(v)) => self.weather.speed = number(token, v, line)?,
			("impact", Some(v)) => self.weather.impact = number(token, v, line)?,
			("zoom", Some(v)) => self.camera.zoom = number(token, v, line)?.max(0.5),
			("look", Some(v)) => self.camera.look = pair(token, v, line)?,
			("shot", Some(v)) => {
				self.camera.shot = Some(ShotSize::parse(v).ok_or_else(|| {
					anyhow::anyhow!(
						"line {line}: unknown shot `{v}`; try {}",
						ShotSize::ALL.join(", ")
					)
				})?);
				// A new shot frames its default subjects unless this direction names them.
				self.camera.on.clear();
			}
			("on", Some(v)) => {
				self.camera.on = v
					.split(',')
					.map(str::trim)
					.filter(|id| !id.is_empty())
					.map(str::to_owned)
					.collect();

				if self.camera.on.is_empty() {
					bail!("line {line}: `on=` names nobody; use actor ids like `on=stinky,mum`");
				}
			}
			("move", Some(v)) => {
				self.camera.travel = Some(Travel::parse_move(v).ok_or_else(|| {
					anyhow::anyhow!(
						"line {line}: unknown move `{v}`; try {}",
						Travel::MOVES.join(", ")
					)
				})?);
			}
			("cut", None) => self.camera.travel = Some(Travel::Cut),
			("ease", None) => self.camera.travel = Some(Travel::Glide),
			("over", Some(v)) => self.camera.over = number(token, v, line)?.max(0.0),
			("shake", Some(v)) => self.camera.shake = number(token, v, line)?.clamp(0.0, 1.0),
			_ => {
				bail!(
					"line {line}: `{token}` is not a scene option; use night, day, light=, wilt=, \
					 stink=, mint=, tint=, speed=, impact=, zoom=, look=x,y, shot=, on=, move=, \
					 over=, shake=, cut or ease"
				)
			}
		}

		Ok(())
	}

	/// The visible actors, with automatic positions filled in, in drawing order.
	pub fn placed(&self) -> Vec<(&Actor, f32)> {
		let unplaced: Vec<usize> = self
			.actors
			.iter()
			.enumerate()
			.filter(|(_, actor)| actor.x.is_none() && !actor.hidden)
			.map(|(index, _)| index)
			.collect();
		let mut placed: Vec<(&Actor, f32)> = self
			.actors
			.iter()
			.enumerate()
			.filter(|(_, actor)| !actor.hidden)
			.map(|(index, actor)| {
				let x = actor.x.unwrap_or_else(|| {
					let slot = unplaced.iter().position(|i| *i == index).unwrap_or(0);
					(slot as f32 + 1.0) / (unplaced.len() as f32 + 1.0)
				});
				(actor, x)
			})
			.collect();

		placed.sort_by_key(|(actor, _)| actor.z);
		placed
	}
}

fn number(token: &Token, value: &str, line: usize) -> Result<f32> {
	let parsed = value.parse::<f32>().map_err(|_| {
		anyhow::anyhow!(
			"line {line}: `{token}` needs a number, e.g. `{}=0.5`",
			token.key
		)
	})?;
	ensure!(
		parsed.is_finite(),
		"line {line}: `{token}` needs a finite number, e.g. `{}=0.5`",
		token.key
	);

	Ok(parsed)
}

/// The page transition named in a heading (`transition=wipe|cut|dissolve|fade|iris|push`).
pub fn transition(value: Option<&str>, line: usize) -> Result<Transition> {
	let value = value.with_context(|| {
		format!(
			"line {line}: `transition` needs a name; try {}",
			Transition::ALL.join(", ")
		)
	})?;

	Transition::parse(value).ok_or_else(|| {
		anyhow::anyhow!(
			"line {line}: unknown transition `{value}`; try {}",
			Transition::ALL.join(", ")
		)
	})
}

fn colour(value: &str, line: usize) -> Result<Color> {
	Color::parse(value)
		.ok_or_else(|| anyhow::anyhow!("line {line}: `{value}` is not a palette colour or #rrggbb"))
}

fn pair(token: &Token, value: &str, line: usize) -> Result<(f32, f32)> {
	let parsed = value.split_once(',').map(|(x, y)| (x.trim(), y.trim()));

	let (x, y) = parsed.with_context(|| {
		anyhow::anyhow!("line {line}: `{token}` should be two numbers like `0.5,0.6`")
	})?;

	Ok((number(token, x, line)?, number(token, y, line)?))
}

fn position(token: &Token, value: &str, line: usize) -> Result<f32> {
	let named = match value {
		"far-left" => Some(0.12),
		"left" => Some(0.28),
		"center" | "centre" | "middle" => Some(0.5),
		"right" => Some(0.72),
		"far-right" => Some(0.88),
		"sky" => Some(0.32),
		"high" => Some(0.2),
		"low" => Some(0.55),
		_ => None,
	};

	if let Some(x) = named {
		return Ok(x);
	}

	number(token, value, line).map_err(|_| {
		anyhow::anyhow!(
			"line {line}: position `{value}` should be a finite number (0 to 1) or far-left, \
			 left, center, right, far-right, sky, high, low"
		)
	})
}

fn side(value: &str, line: usize) -> Result<Side> {
	match value {
		"left" => Ok(Side::Left),
		"right" => Ok(Side::Right),
		other => bail!("line {line}: `{other}` should be left or right"),
	}
}

fn apply_actor_token(actor: &mut Actor, token: &Token, line: usize) -> Result<()> {
	let Some(value) = token.value.as_deref() else {
		return apply_actor_word(actor, &token.key, line);
	};

	match token.key.as_str() {
		"x" => actor.x = Some(position(token, value, line)?),
		"y" => actor.y = Some(position(token, value, line)?),
		"size" => actor.size = number(token, value, line)?,
		"z" => {
			actor.z = value
				.parse()
				.map_err(|_| anyhow::anyhow!("line {line}: `z` needs a whole number"))?;
		}
		"face" => {
			actor.facing = Some(match value {
				"left" => -1.0,
				"right" => 1.0,
				"front" | "out" => 0.0,
				other => bail!("line {line}: face `{other}` should be left, right or front"),
			});
		}
		"mood" => {
			actor.mood = Mood::parse(value).ok_or_else(|| {
				let names: Vec<&str> = Mood::ALL.iter().map(|(name, _)| *name).collect();
				anyhow::anyhow!(
					"line {line}: unknown mood `{value}`; try {}",
					names.join(", ")
				)
			})?;
		}
		"pose" => {
			actor.pose = Pose::parse(value).ok_or_else(|| {
				let names: Vec<&str> = Pose::ALL.iter().map(|(name, _)| *name).collect();
				anyhow::anyhow!(
					"line {line}: unknown pose `{value}`; try {}",
					names.join(", ")
				)
			})?;
		}
		"stance" => {
			actor.stance = Stance::parse(value).ok_or_else(|| {
				let names: Vec<&str> = Stance::ALL.iter().map(|(name, _)| *name).collect();
				anyhow::anyhow!(
					"line {line}: unknown stance `{value}`; try {}",
					names.join(", ")
				)
			})?;
		}
		"gait" => {
			actor.gait = match value {
				"walk" => Some(Gait::Walk),
				"run" => Some(Gait::Run),
				"none" | "still" => None,
				other => bail!("line {line}: gait `{other}` should be walk, run or none"),
			};
		}
		"teeth" => {
			actor.teeth = match value {
				"clean" => Teeth::Clean,
				"grotty" => Teeth::Grotty,
				"sparkly" => Teeth::Sparkly,
				other => bail!("line {line}: teeth `{other}` should be clean, grotty or sparkly"),
			};
		}
		"holds" => {
			actor.prop = match value {
				"toothbrush" => Some(Prop::Toothbrush),
				"parcel" => Some(Prop::Parcel),
				"jam" => Some(Prop::Jam),
				"nothing" | "none" => None,
				other => {
					bail!(
						"line {line}: holds `{other}` should be toothbrush, parcel, jam or nothing"
					)
				}
			};
		}
		"mouth" => actor.mouth = Some(number(token, value, line)?.clamp(0.0, 1.0)),
		"stink" => actor.stink = number(token, value, line)?.max(0.0),
		"mint" => actor.mint = number(token, value, line)?.max(0.0),
		"wilt" => actor.wilt = number(token, value, line)?.clamp(0.0, 1.0),
		"colour" | "color" => actor.tint = Some(colour(value, line)?),
		"enter" => {
			actor.enter = Some(side(value, line)?);
			actor.hidden = false;
		}
		"exit" => {
			actor.exit = Some(side(value, line)?);
			actor.hidden = true;
		}
		other => bail!("line {line}: `{other}=` is not something an actor understands"),
	}

	Ok(())
}

fn apply_actor_word(actor: &mut Actor, word: &str, line: usize) -> Result<()> {
	if let Some(stance) = Stance::parse(word) {
		actor.stance = stance;
		return Ok(());
	}

	if let Some(mood) = Mood::parse(word) {
		actor.mood = mood;
		return Ok(());
	}

	if let Some(pose) = Pose::parse(word) {
		actor.pose = pose;
		return Ok(());
	}

	match word {
		"talk" | "says" | "shout" => {
			actor.talking = true;
			actor.talked = true;
		}
		"quiet" => {
			actor.talking = false;
			actor.talked = false;
		}
		"faint" => {
			actor.knocked = 1.0;
			actor.lively = 0.0;
			actor.y = None;

			if actor.kind.is_figure() {
				actor.mood = Mood::Fainted;
			}
		}
		"wake" => {
			actor.knocked = 0.0;

			if actor.mood == Mood::Fainted {
				actor.mood = Mood::Happy;
			}
		}
		"fly" => {
			actor.lively = 1.0;
			actor.knocked = 0.0;
			actor.y = actor.y.or(Some(0.4));
		}
		"land" => {
			actor.lively = 0.0;
			actor.y = None;
		}
		"wag" | "excited" => actor.lively = 1.0,
		"calm" => actor.lively = 0.0,
		"hide" => actor.hidden = true,
		"show" => actor.hidden = false,
		"hop" => actor.hops += 1,
		"flip" => actor.facing = Some(-actor.facing.unwrap_or(1.0)),
		"walk" => actor.gait = Some(Gait::Walk),
		"run" => actor.gait = Some(Gait::Run),
		"still" => actor.gait = None,
		_ => {
			let moods: Vec<&str> = Mood::ALL.iter().map(|(name, _)| *name).collect();
			let poses: Vec<&str> = Pose::ALL.iter().map(|(name, _)| *name).collect();
			bail!(
				"line {line}: `{word}` is not a mood, pose, stance or action.\n  moods: {}\n  \
				 poses: {}\n  stances: stand, crouch, sit, kneel, jump, bow, tiptoe\n  actions: \
				 talk, faint, wake, fly, land, wag, calm, hide, show, hop, flip, walk, run, still",
				moods.join(", "),
				poses.join(", ")
			);
		}
	}

	Ok(())
}

/// The scene for one page at every step of reading it.
#[derive(Clone, Debug)]
pub struct Rehearsal {
	/// Index into `story.pages`.
	pub page: usize,
	/// The scene before any of the page's beats run.
	pub opening: Scene,
	/// The scene after each beat, in the same order as `page.beats`.
	pub after: Vec<Scene>,
}

impl Rehearsal {
	/// The state at the end of the page: what the book illustration shows.
	pub fn finale(&self) -> &Scene {
		self.after.last().unwrap_or(&self.opening)
	}
}

/// Runs every page's stage directions in order, carrying character traits between pages.
pub fn rehearse(story: &Story, cast: &Cast) -> Result<Vec<Rehearsal>> {
	let mut memory: HashMap<String, Traits> = HashMap::new();
	let mut out = Vec::with_capacity(story.pages.len());

	for (index, page) in story.pages.iter().enumerate() {
		let mut scene = Scene::for_page(page, &story.settings)?;

		// A page takes the book's (or world's) default light unless its heading names one.
		if page.option("light").is_none() {
			scene.weather.light = story.light;
		}

		hide_wormsley(&mut scene, page, cast, index);
		let opening = scene.clone();
		let mut after = Vec::with_capacity(page.beats.len());

		for beat in &page.beats {
			match beat {
				Beat::Direct(directive) => scene.apply(directive, cast, &mut memory)?,
				Beat::Say { .. } | Beat::Pause { .. } | Beat::Sfx { .. } => {}
			}

			after.push(scene.clone());

			if matches!(beat, Beat::Say { .. }) {
				scene.end_paragraph();
			}
		}

		out.push(Rehearsal {
			page: index,
			opening,
			after,
		});
	}

	Ok(out)
}

/// Spots near the ground where a small worm can hide without being in anyone's way.
const HIDING_SPOTS: [f32; 6] = [0.07, 0.93, 0.19, 0.81, 0.4, 0.6];

/// Every page hides Wormsley somewhere (the series' "spot him" game) unless the script places
/// him itself or the cast has no Wormsley.
fn hide_wormsley(scene: &mut Scene, page: &Page, cast: &Cast, index: usize) {
	let Some(member) = cast.members.get("wormsley") else {
		return;
	};

	let mentioned = page
		.beats
		.iter()
		.any(|beat| matches!(beat, Beat::Direct(d) if d.actor == "wormsley"));

	if mentioned || page.flag("no-wormsley") {
		return;
	}

	let taken: Vec<f32> = page
		.beats
		.iter()
		.filter_map(|beat| {
			match beat {
				Beat::Direct(d) => {
					d.tokens
						.iter()
						.find(|t| t.key == "x")
						.and_then(|t| position(t, t.value.as_deref()?, 0).ok())
				}
				_ => None,
			}
		})
		.collect();
	let start = index % HIDING_SPOTS.len();
	let spot = (0..HIDING_SPOTS.len())
		.map(|offset| HIDING_SPOTS[(start + offset) % HIDING_SPOTS.len()])
		.find(|x| taken.iter().all(|t| (t - x).abs() > 0.14))
		.unwrap_or(HIDING_SPOTS[start]);

	let mut worm = Actor::new("wormsley", member.kind.clone());
	worm.x = Some(spot);
	worm.y = Some((scene.setting.ground() + 70.0) / crate::art::STAGE);
	worm.size = 1.15;
	worm.z = 10;
	scene.actors.push(worm);
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::story;
	use crate::story::Direction;

	fn cast() -> Cast {
		Cast::parse("[stinky]\nname = \"Stinky\"\n[mum]\nname = \"Mum\"\nkind = \"grown-up\"\n")
			.unwrap()
	}

	#[test]
	fn unknown_settings_are_reported_with_line_numbers() {
		let error = run("# moon\n").unwrap_err();
		let message = format!("{error:#}");
		assert!(message.contains("line 5"), "{message}");
		assert!(message.contains("unknown setting `moon`"), "{message}");
	}

	#[test]
	fn a_kid_stands_as_tall_in_every_style() {
		let staged = |style: &str, build: Build| {
			let look =
				crate::cast::template_look(&StylePack::named(style).unwrap(), build).unwrap();
			figure_size(&look) * look.frame().height()
		};

		for build in [Build::Kid, Build::Grownup] {
			let (cartoon, anime) = (staged("cartoon", build), staged("anime", build));
			assert!(
				(cartoon - anime).abs() < 0.01,
				"{build:?}: {cartoon} vs {anime}"
			);
		}
	}

	fn run(script: &str) -> Result<Scene> {
		let story = story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n{script}"
		))?;
		let page = &story.pages[0];
		let mut scene = Scene::for_page(page, &story.settings)?;
		let mut memory = HashMap::new();

		for beat in &page.beats {
			if let Beat::Direct(directive) = beat {
				scene.apply(directive, &cast(), &mut memory)?;
			}
		}

		Ok(scene)
	}

	#[test]
	fn directions_accumulate_into_a_scene() {
		let scene = run(
			"# park wilt=0.5\n@stinky x=left defiant arms-crossed teeth=grotty stink=1 \
			 talk\n@bird.a fly\n@bird.a faint",
		)
		.unwrap();
		assert!((scene.weather.wilt - 0.5).abs() < f32::EPSILON);
		let stinky = &scene.actors[0];
		assert_eq!(stinky.mood, Mood::Defiant);
		assert_eq!(stinky.pose, Pose::ArmsCrossed);
		assert_eq!(stinky.teeth, Teeth::Grotty);
		assert!(stinky.talked);
		assert_eq!(stinky.x, Some(0.28));
		let bird = &scene.actors[1];
		assert!((bird.knocked - 1.0).abs() < f32::EPSILON);
		assert_eq!(bird.y, None, "a fainted bird drops to the ground");
	}

	#[test]
	fn automatic_positions_spread_actors_out() {
		let scene = run("# park\n@stinky\n@mum\n").unwrap();
		let xs: Vec<f32> = scene.placed().iter().map(|(_, x)| *x).collect();
		assert_eq!(xs.len(), 2);
		assert!(
			(xs[0] - 1.0 / 3.0).abs() < 0.01 && (xs[1] - 2.0 / 3.0).abs() < 0.01,
			"{xs:?}"
		);
	}

	#[test]
	fn mistakes_are_reported_with_line_numbers_and_options() {
		let message = format!("{:#}", run("# park\n@stinky mood=furious").unwrap_err());
		assert!(
			message.contains("line 6") && message.contains("grumpy"),
			"{message}"
		);

		let message = format!("{:#}", run("# park\n@stinky dance").unwrap_err());
		assert!(
			message.contains("not a mood, pose, stance or action"),
			"{message}"
		);

		let message = format!("{:#}", run("# park\n@granny").unwrap_err());
		assert!(message.contains("unknown actor `granny`"), "{message}");
	}

	#[test]
	fn camera_vocabulary_parses_into_the_scene_camera() {
		let scene =
			run("# park\n@camera shot=close on=stinky,mum move=push over=2.5 shake=0.5\n").unwrap();
		let camera = &scene.camera;

		assert_eq!(camera.shot, Some(ShotSize::Close));
		assert_eq!(camera.on, vec!["stinky".to_owned(), "mum".to_owned()]);
		assert_eq!(camera.travel, Some(Travel::Push));
		assert!((camera.over - 2.5).abs() < f32::EPSILON);
		assert!((camera.shake - 0.5).abs() < f32::EPSILON);
	}

	#[test]
	fn a_new_shot_frames_its_default_subjects_again() {
		let scene = run("# park\n@camera shot=close on=stinky\n@camera shot=wide\n").unwrap();

		assert!(scene.camera.on.is_empty(), "a new shot names nobody");
		assert_eq!(scene.camera.shot, Some(ShotSize::Wide));
	}

	#[test]
	fn cut_and_ease_are_camera_words() {
		let cut = run("# park\n@camera cut\n").unwrap();
		assert_eq!(cut.camera.travel, Some(Travel::Cut));

		let ease = run("# park\n@camera ease\n").unwrap();
		assert_eq!(ease.camera.travel, Some(Travel::Glide));

		let ease_over = run("# park\n@camera ease over=3\n").unwrap();
		assert_eq!(ease_over.camera.travel, Some(Travel::Glide));
		assert!((ease_over.camera.over - 3.0).abs() < f32::EPSILON);
	}

	#[test]
	fn a_light_is_scene_state_that_can_change_mid_page() {
		let scene = run("# park light=golden\n@stinky\n").unwrap();
		assert_eq!(
			scene.weather.light.preset,
			crate::art::light::Preset::Golden
		);

		// A bare `night` keeps today's behaviour: no light preset comes with it.
		let night = run("# park night\n@stinky\n").unwrap();
		assert!(night.weather.night);
		assert!(night.weather.light.is_day());

		let eased = run("# park light=golden\n@stinky\n@scene light=moonlit\n").unwrap();
		assert_eq!(
			eased.weather.light.preset,
			crate::art::light::Preset::Moonlit
		);
	}

	#[test]
	fn an_unknown_light_names_the_line_and_the_presets() {
		let message = format!("{:#}", run("# park light=sunset\n").unwrap_err());
		assert!(
			message.contains("line 5") && message.contains("candle") && message.contains("moonlit"),
			"{message}"
		);
	}

	#[test]
	fn bad_camera_values_name_the_line_and_the_options() {
		let message = format!("{:#}", run("# park\n@camera shot=cinematic").unwrap_err());
		assert!(
			message.contains("line 6") && message.contains("extreme-close"),
			"{message}"
		);

		let message = format!("{:#}", run("# park\n@camera move=dolly").unwrap_err());
		assert!(
			message.contains("line 6") && message.contains("pan-left"),
			"{message}"
		);

		let message = format!("{:#}", run("# park\n@camera shake=furious").unwrap_err());
		assert!(
			message.contains("line 6") && message.contains("needs a number"),
			"{message}"
		);

		let message = format!("{:#}", run("# park\n@camera on=").unwrap_err());
		assert!(message.contains("names nobody"), "{message}");

		let message = format!("{:#}", run("# park transition=blink\n").unwrap_err());
		assert!(
			message.contains("line 5") && message.contains("iris"),
			"{message}"
		);
	}

	#[test]
	fn transitions_and_shots_live_in_page_headings() {
		let story = story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park transition=iris shot=medium \
			 on=stinky\n@stinky\n# town transition=cut\n",
		)
		.unwrap();

		assert_eq!(story.pages[0].transition(), Some(Transition::Iris));
		assert_eq!(story.pages[1].transition(), Some(Transition::Cut));

		let scene = Scene::for_page(&story.pages[0], &story.settings).unwrap();
		assert_eq!(scene.camera.shot, Some(ShotSize::Medium));
		assert_eq!(scene.camera.on, vec!["stinky".to_owned()]);
	}

	#[test]
	fn the_video_table_defaults_to_the_new_camera() {
		let story = story::parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park\n").unwrap();
		assert_eq!(story.meta.video.direction, Direction::Auto);
		assert!(story.meta.video.depth_of_field);

		let locked = story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n[video]\ndirection = \"locked\"\ndepth-of-field \
			 = false\ntransition = \"dissolve\"\n+++\n# park\n",
		)
		.unwrap();
		assert_eq!(locked.meta.video.direction, Direction::Locked);
		assert!(!locked.meta.video.depth_of_field);
		assert_eq!(locked.meta.video.transition, Some(Transition::Dissolve));
	}

	#[test]
	fn every_number_a_script_supplies_is_finite() {
		for value in ["nan", "inf", "-inf"] {
			for script in [
				format!("# park\n@stinky size={value}"),
				format!("# park\n@stinky x={value}"),
				format!("# park wilt={value}"),
				format!("# park size={value}"),
				format!("# park\n@camera look={value},{value}"),
				format!("# park\n@camera shake={value}"),
			] {
				let message = format!("{:#}", run(&script).unwrap_err());
				assert!(
					message.contains("line ") && message.contains("finite"),
					"{script}: {message}"
				);
			}
		}

		// A page's text scale must also leave the type a size to set.
		for value in ["0", "-1"] {
			let message = format!("{:#}", run(&format!("# park size={value}")).unwrap_err());
			assert!(message.contains("more than zero"), "{value}: {message}");
		}
	}

	#[test]
	fn page_options_that_are_not_scene_state_parse() {
		let story = story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park size=0.9 no-wormsley\n@stinky\n",
		)
		.unwrap();
		let page = &story.pages[0];

		assert_eq!(
			page.option("size").and_then(|token| token.value.as_deref()),
			Some("0.9")
		);
		assert!(page.flag("no-wormsley"));
		Scene::for_page(page, &story.settings).unwrap();
	}

	#[test]
	fn traits_carry_between_pages() {
		let story = story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park\n@stinky teeth=grotty\n# \
			 bathroom\n@stinky\n",
		)
		.unwrap();
		let cast = cast();
		let mut memory = HashMap::new();
		let mut last = None;

		for page in &story.pages {
			let mut scene = Scene::for_page(page, &story.settings).unwrap();

			for beat in &page.beats {
				if let Beat::Direct(directive) = beat {
					scene.apply(directive, &cast, &mut memory).unwrap();
				}
			}

			last = Some(scene);
		}

		assert_eq!(last.unwrap().actors[0].teeth, Teeth::Grotty);
	}
}
