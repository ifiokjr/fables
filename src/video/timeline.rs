//! The video timeline: the rehearsal laid out against the narrator's clock.
//!
//! Each video page becomes a shot. A shot holds scene keyframes (from stage directions), the
//! lines spoken over it and sound cues. Directions take effect at the moment the narrator
//! reaches them, which is what makes characters "do what the narrator says".
//!
//! With `cast-voices` on, a paragraph like `"Brush your teeth!" said Mum.` is split: Mum says
//! the quoted words in her own voice, in the mood the script has her in, and the narrator says
//! the rest (or, with `drop-tags`, nothing, so the scene plays like a show).
//!
//! The same pass plans the soundtrack's changes: which music cue plays when, where the music
//! drops out for a shouted peak, where a `@hold` stops everything, and which ambience beds a
//! scene's setting and light ask for.

use std::collections::HashMap;

use anyhow::Result;

use crate::art::figure::Mood;
use crate::art::light::Preset;
use crate::art::settings::Bed;
use crate::book;
use crate::cast::Cast;
use crate::cast::Kind;
use crate::stage::Ambience;
use crate::stage::Camera;
use crate::stage::Rehearsal;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::stage::Travel;
use crate::story::Acting;
use crate::story::Beat;
use crate::story::Direction;
use crate::story::Medium;
use crate::story::Pacing;
use crate::story::Story;
use crate::story::Transition;
use crate::story::plain_text;
use crate::video::audio::Music;
use crate::video::audio::MusicDefault;
use crate::video::audio::Sound;
use crate::video::director;
use crate::video::perform;
use crate::video::voice::Delivery;
use crate::video::voice::Speech;
use crate::video::voice::Utterance;
use crate::video::voice::Voice;

/// How long an action takes to reach its contact frame: a fall lands, a hop comes down.
const CONTACT: f32 = 0.25;
/// Effects triggered by an action land this long after its contact frame: three frames at 30.
const AFTER_CONTACT: f32 = 0.1;
/// How long before a shouted peak the music drops out.
const SHOUT_DROPOUT: f32 = 0.5;
/// How long after a shouted line the music comes back.
const SHOUT_RETURN: f32 = 0.2;

/// A stretch of the timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
	pub start: f32,
	pub end: f32,
}

impl Span {
	pub fn contains(self, time: f32) -> bool {
		time >= self.start && time < self.end
	}
}

/// A music cue change: the cue from `time` until the next change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicCue {
	pub time: f32,
	pub music: Music,
}

/// An ambience change: the beds from `time` until the next change. No beds is silence.
#[derive(Clone, Debug, PartialEq)]
pub struct AmbienceCue {
	pub time: f32,
	pub beds: Vec<Bed>,
}

/// A scene state that takes effect at `time` (seconds from the start of the video).
#[derive(Clone, Debug)]
pub struct Key {
	pub time: f32,
	pub scene: Scene,
	/// The script line of the direction that made this state, or the heading's line for the
	/// state a page opens on. Zero for the title and end cards.
	pub line: usize,
	/// Seconds after the page opens at which this camera direction lands; `None` lands where
	/// it is written.
	pub after: Option<f32>,
}

/// A spoken line: a whole narrated paragraph, or one quote or tag from it.
#[derive(Clone, Debug)]
pub struct Line {
	pub start: f32,
	/// Index into [`Timeline::speeches`].
	pub speech: usize,
	/// How long the line lasts, in seconds.
	pub duration: f32,
	/// Actors whose mouths move with this line.
	pub speakers: Vec<String>,
	/// The words with markup, for captions.
	pub text: String,
	/// The character voicing the line, or `None` for the narrator.
	pub voiced_by: Option<String>,
	/// The script line the paragraph starts on (zero for the cards).
	pub line: usize,
	/// The actor on stage a narrated line opens by naming: the camera frames them.
	pub about: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
	Title,
	Page(usize),
	End,
}

impl ShotKind {
	/// The script page a page shot shows.
	pub fn page(self) -> Option<usize> {
		match self {
			ShotKind::Page(page) => Some(page),
			ShotKind::Title | ShotKind::End => None,
		}
	}
}

#[derive(Clone, Debug)]
pub struct Shot {
	pub kind: ShotKind,
	pub start: f32,
	pub end: f32,
	pub keys: Vec<Key>,
	pub lines: Vec<Line>,
	/// How the page is entered from the previous one.
	pub transition: Transition,
	/// The camera's work on this shot, decided once when the timeline is built: one setup per
	/// framing or move. Landscape first; portrait shots are planned separately, because a
	/// phone frame prefers different sizes.
	pub setups: Vec<Setup>,
	pub portrait_setups: Vec<Setup>,
}

impl Shot {
	pub fn duration(&self) -> f32 {
		self.end - self.start
	}

	/// The camera setups for a frame shape.
	pub fn setups_for(&self, portrait: bool) -> &[Setup] {
		if portrait && !self.portrait_setups.is_empty() {
			&self.portrait_setups
		} else {
			&self.setups
		}
	}

	/// Whether characters are moving between keys at `time`: within `tween` seconds of a key
	/// that changes the scene (the shot's first key only sets it up).
	pub fn moving(&self, time: f32, tween: f32) -> bool {
		self.keys
			.iter()
			.rposition(|key| key.time <= time)
			.is_some_and(|index| index > 0 && time - self.keys[index].time < tween)
	}

	/// The scene the shot is in just after `time`.
	pub fn scene_at(&self, time: f32) -> &Scene {
		let index = self
			.keys
			.iter()
			.rposition(|key| key.time <= time + 1e-3)
			.unwrap_or(0);
		&self.keys[index].scene
	}
}

/// What a camera setup frames.
#[derive(Clone, Debug, PartialEq)]
pub enum Framing {
	/// The automatic framing: everyone on stage, fitted by the format (the older camera's view).
	Auto,
	/// A zoom and look point, as fractions of the stage.
	Zoom { zoom: f32, look: (f32, f32) },
	/// A shot size framed on the named actors. Empty names mean everyone on stage, or the
	/// current speaker for a close shot.
	Shot { size: ShotSize, on: Vec<String> },
}

/// Where a camera setup came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
	/// Written as an `@camera` direction in the script.
	Script,
	/// Chosen by the automatic director.
	Director,
	/// The title or end card, which no script page starts.
	Card,
}

impl Origin {
	pub fn name(self) -> &'static str {
		match self {
			Origin::Script => "script",
			Origin::Director => "director",
			Origin::Card => "card",
		}
	}
}

/// One camera setup: a framing the camera holds or moves to, starting at `time`.
#[derive(Clone, Debug)]
pub struct Setup {
	/// When the camera begins the setup (seconds from the start of the video). A cut lands
	/// exactly here; a glide or move starts here and takes `over`.
	pub time: f32,
	/// The scene moment the framing frames, so subject positions are settled.
	pub anchor: f32,
	pub framing: Framing,
	pub travel: Travel,
	/// Seconds a glide or move takes; zero means the rest of the setup.
	pub over: f32,
	/// A decaying shake that starts with the setup (0 = none).
	pub shake: f32,
	/// The script line of the direction or line that starts the setup; zero for a card.
	pub line: usize,
	/// Whether the script wrote this setup or the director chose it.
	pub origin: Origin,
}

/// A sound effect at a moment in the video.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
	pub time: f32,
	/// The built-in effect, which plays unless a recorded one is in the timeline's foley.
	pub sound: Sound,
	/// The script's own description of the effect (`@sfx bonk "a spoon on a pot"`), for a
	/// recorded effect.
	pub described: Option<std::sync::Arc<str>>,
}

#[derive(Clone, Debug)]
pub struct Timeline {
	pub shots: Vec<Shot>,
	pub speeches: Vec<Speech>,
	pub cues: Vec<Cue>,
	pub duration: f32,
	/// The music cue changes, the first at time zero.
	pub music: Vec<MusicCue>,
	/// Where the music drops out for a shouted peak.
	pub dropouts: Vec<Span>,
	/// Dead stops (`@hold`): nothing sounds, and the performance layer can read [`in_hold`]
	/// as its flag to stop the idle movement too.
	///
	/// [`in_hold`]: Timeline::in_hold
	pub holds: Vec<Span>,
	/// The ambience changes, the first at time zero; an empty list means no ambience at all.
	pub ambience: Vec<AmbienceCue>,
	/// Recorded effects and ambience that play in place of the built-in ones; empty unless
	/// the book asks for them (see [`foley`](crate::video::foley)).
	pub foley: crate::video::foley::Foley,
}

impl Timeline {
	/// The shot playing at `time`.
	pub fn shot_at(&self, time: f32) -> &Shot {
		self.shots
			.iter()
			.find(|shot| time < shot.end)
			.unwrap_or_else(|| {
				self.shots
					.last()
					.unwrap_or_else(|| unreachable!("a timeline always has a title shot"))
			})
	}

	/// Whether the video is in a dead stop at `time`. The performance layer reads this to stop
	/// the idle movement as well as the sound.
	pub fn in_hold(&self, time: f32) -> bool {
		self.holds.iter().any(|hold| hold.contains(time))
	}

	/// The music cue playing at `time`.
	pub fn music_at(&self, time: f32) -> Music {
		self.music
			.iter()
			.rev()
			.find(|cue| cue.time <= time)
			.map_or(Music::Play, |cue| cue.music)
	}

	/// The ambience beds playing at `time`.
	pub fn ambience_at(&self, time: f32) -> &[Bed] {
		self.ambience
			.iter()
			.rev()
			.find(|cue| cue.time <= time)
			.map_or(&[], |cue| cue.beds.as_slice())
	}
}

/// Turns an utterance into speech: TTS in production, silence of a known length in tests.
pub trait Speaker: FnMut(&Utterance<'_>) -> Result<Speech> {}
impl<F: FnMut(&Utterance<'_>) -> Result<Speech>> Speaker for F {}

struct Builder<'a, S: Speaker> {
	story: &'a Story,
	cast: &'a Cast,
	speak: S,
	/// The narrator as it will read this book: the age band's rate fills in only when the
	/// narrator does not name one.
	narrator: Voice,
	pacing: Pacing,
	/// `@music` moments, merged with the scene keys when the timeline is finished.
	music_sets: Vec<MusicCue>,
	timeline: Timeline,
}

/// Lays the story out in time.
pub fn build(
	story: &Story,
	cast: &Cast,
	rehearsals: &[Rehearsal],
	speak: impl Speaker,
) -> Result<Timeline> {
	story.narrator().validate()?;

	let pacing = story.pacing();
	let mut narrator = story.narrator().clone();

	if narrator.rate.is_none()
		&& let Some(age) = story.age
	{
		narrator.rate = Some(age.pacing().rate);
	}

	let mut builder = Builder {
		story,
		cast,
		speak,
		narrator,
		pacing,
		music_sets: Vec::new(),
		timeline: Timeline {
			shots: Vec::new(),
			speeches: Vec::new(),
			cues: Vec::new(),
			duration: 0.0,
			music: Vec::new(),
			dropouts: Vec::new(),
			holds: Vec::new(),
			ambience: Vec::new(),
			foley: crate::video::foley::Foley::default(),
		},
	};
	let mut cursor = builder.title_shot(0.0)?;
	let mut shown: Option<(String, Preset)> = None;

	for rehearsal in rehearsals {
		if !story.pages[rehearsal.page].in_video() {
			continue;
		}

		let opening = &rehearsal.opening;
		let place = (
			opening.setting.name().to_owned(),
			opening.weather.light.preset,
		);
		let new_place = shown.as_ref().is_some_and(|shown| *shown != place);
		shown = Some(place);
		builder.cue(cursor, Sound::PageTurn);
		cursor = builder.page_shot(rehearsal, cursor, new_place)?;
	}

	if let Some(moral) = &story.meta.moral {
		builder.cue(cursor, Sound::PageTurn);
		cursor = builder.end_shot(moral, cursor)?;
	}

	let mut timeline = builder.timeline;
	timeline.duration = cursor;
	timeline.cues.sort_by(|a, b| a.time.total_cmp(&b.time));
	timeline.music = music_plan(&timeline.shots, &builder.music_sets, story);
	timeline.ambience = ambience_plan(&timeline.shots, story);
	timeline.dropouts = shouted_dropouts(&timeline);

	// The director plans every shot's camera once, so the frame renderer only reads setups.
	for shot in &mut timeline.shots {
		let explicit = shot.kind == ShotKind::Title
			|| shot.kind == ShotKind::End
			|| story
				.pages
				.get(shot.kind.page().unwrap_or(usize::MAX))
				.is_some_and(crate::story::Page::directed_by_camera);

		director::direct(story, shot, explicit)?;
	}

	Ok(timeline)
}

/// The music cue changes: every scene key asks for a cue, `@music` can override one mid-page,
/// and a book's own `[video] music` wins over the light.
fn music_plan(shots: &[Shot], sets: &[MusicCue], story: &Story) -> Vec<MusicCue> {
	let default = story.meta.video.music;
	let mut moments: Vec<(f32, Option<Music>, Preset)> = Vec::new();

	for shot in shots {
		for key in &shot.keys {
			moments.push((key.time, key.scene.music, key.scene.weather.light.preset));
		}
	}

	for set in sets {
		moments.push((set.time, Some(set.music), Preset::Day));
	}

	// The keys went in first, so a stable sort puts a scene at an `@music` moment before the
	// `@music` itself, and the explicit cue wins.
	moments.sort_by(|a, b| a.0.total_cmp(&b.0));

	let mut cues: Vec<MusicCue> = Vec::new();

	for (time, music, light) in moments {
		let music = match music {
			Some(music) => music,
			None => {
				match default {
					MusicDefault::Auto => Music::for_light(light),
					MusicDefault::Cue(music) => music,
				}
			}
		};

		if cues.last().map(|cue| cue.music) != Some(music) {
			cues.push(MusicCue {
				time: time.max(0.0),
				music,
			});
		}
	}

	if cues.is_empty() {
		cues.push(MusicCue {
			time: 0.0,
			music: Music::Play,
		});
	} else if cues[0].time > 0.0 {
		let music = cues[0].music;
		cues.insert(0, MusicCue { time: 0.0, music });
	}

	cues
}

/// The ambience bed changes. Empty when the book has not asked for ambience: a book with an
/// `age` gets it, and so does one with `[video] ambience = true`.
fn ambience_plan(shots: &[Shot], story: &Story) -> Vec<AmbienceCue> {
	if !story.ambience() {
		return Vec::new();
	}

	let mut cues: Vec<AmbienceCue> = Vec::new();

	for shot in shots {
		for key in &shot.keys {
			let beds = beds_at(&key.scene);

			if cues.last().map(|cue| &cue.beds) != Some(&beds) {
				cues.push(AmbienceCue {
					time: key.time.max(0.0),
					beds,
				});
			}
		}
	}

	cues
}

/// The beds a scene asks for: its setting's own, plus what its light adds, unless the script
/// turned ambience off or named its own.
fn beds_at(scene: &Scene) -> Vec<Bed> {
	match &scene.ambience {
		Ambience::Off => Vec::new(),
		Ambience::Beds(beds) => beds.clone(),
		Ambience::Default => {
			let mut beds = scene.setting.ambience().to_vec();

			for bed in Bed::for_light(scene.weather.light.preset, scene.setting.indoors()) {
				if !beds.contains(bed) {
					beds.push(*bed);
				}
			}

			beds
		}
	}
}

/// The music drops out before a shouted peak, but only when an actor's mood has just turned
/// to a strong one: a shout from nowhere is not a peak, it is a loud line.
fn shouted_dropouts(timeline: &Timeline) -> Vec<Span> {
	let mut moods: HashMap<String, Mood> = HashMap::new();
	let mut spans = Vec::new();

	for shot in &timeline.shots {
		// The title card is not a scene: the star's cover pose is not a mood that changes.
		if shot.kind == ShotKind::Title {
			continue;
		}

		for (index, key) in shot.keys.iter().enumerate() {
			let next = shot.keys.get(index + 1).map_or(shot.end, |next| next.time);
			let changed: Vec<(String, Mood)> = key
				.scene
				.actors
				.iter()
				.filter(|actor| moods.get(&actor.id).is_some_and(|mood| *mood != actor.mood))
				.map(|actor| (actor.id.clone(), actor.mood))
				.collect();

			for line in &shot.lines {
				if line.start + 1e-3 < key.time || line.start >= next {
					continue;
				}

				let Some(speaker) = &line.voiced_by else {
					continue;
				};

				if !line.text.contains("**") {
					continue;
				}

				if changed
					.iter()
					.any(|(id, mood)| id == speaker && is_strong(*mood))
				{
					spans.push(Span {
						start: line.start - SHOUT_DROPOUT,
						end: line.start + line.duration + SHOUT_RETURN,
					});
				}
			}

			for actor in &key.scene.actors {
				moods.insert(actor.id.clone(), actor.mood);
			}
		}
	}

	spans
}

/// The moods a shout can peak from: anger, defiance, shock and laughter. A proud or happy line
/// can be loud without the music needing to get out of its way.
fn is_strong(mood: Mood) -> bool {
	matches!(
		mood,
		Mood::Cross | Mood::Defiant | Mood::Shocked | Mood::Laugh
	)
}

impl<S: Speaker> Builder<'_, S> {
	fn cue(&mut self, time: f32, sound: Sound) {
		self.timeline.cues.push(Cue {
			time,
			sound,
			described: None,
		});
	}

	/// Speaks `text` and returns the speech index and its length.
	fn say(&mut self, text: &str, voice: &Voice, delivery: Delivery) -> Result<(usize, f32)> {
		let plain = plain_text(text);
		let speech = (self.speak)(&Utterance {
			text: &plain,
			voice,
			delivery,
		})?;
		let duration = speech.duration();
		self.timeline.speeches.push(speech);

		Ok((self.timeline.speeches.len() - 1, duration))
	}

	fn title_shot(&mut self, start: f32) -> Result<f32> {
		let story = self.story;
		let pose = story
			.meta
			.cover
			.as_ref()
			.and_then(|cover| cover.pose.clone())
			.unwrap_or_else(|| "x=0.5 grin wave".into());
		let scene = book::star_scene(story, self.cast, &pose, 1.0, &mut HashMap::new())?;
		let mut text = story.meta.title.clone();

		if let Some(subtitle) = &story.meta.subtitle {
			text = format!("{text}. {subtitle}.");
		}

		let line_start = start + 0.8;
		let narrator = self.narrator.clone();
		let (speech, duration) = self.say(&text, &narrator, Delivery::default())?;
		let speakers = scene
			.actors
			.iter()
			.filter(|actor| actor.talked)
			.map(|actor| actor.id.clone())
			.collect();
		let end = line_start + duration + 1.0;

		self.timeline.shots.push(Shot {
			kind: ShotKind::Title,
			start,
			end,
			keys: vec![Key {
				time: start,
				scene,
				line: 0,
				after: None,
			}],
			lines: vec![Line {
				start: line_start,
				speech,
				duration,
				speakers,
				text: String::new(),
				voiced_by: None,
				line: 0,
				about: None,
			}],
			transition: Transition::Cut,
			setups: Vec::new(),
			portrait_setups: Vec::new(),
		});

		Ok(end)
	}

	fn end_shot(&mut self, moral: &str, start: f32) -> Result<f32> {
		let back = book::back_cover(self.story, self.cast)?;
		let mut scene = back.scene;

		for actor in &mut scene.actors {
			actor.x = Some(0.5);
			actor.talked = false;
		}

		let line_start = start + self.pacing.lead_in;
		let narrator = self.narrator.clone();
		let (speech, duration) = self.say(moral, &narrator, Delivery::default())?;
		let end = line_start + duration + 1.6;

		self.timeline.shots.push(Shot {
			kind: ShotKind::End,
			start,
			end,
			keys: vec![Key {
				time: start,
				scene,
				line: 0,
				after: None,
			}],
			lines: vec![Line {
				start: line_start,
				speech,
				duration,
				speakers: Vec::new(),
				text: moral.to_owned(),
				voiced_by: None,
				line: 0,
				about: None,
			}],
			transition: self.story.meta.video.transition.unwrap_or_default(),
			setups: Vec::new(),
			portrait_setups: Vec::new(),
		});

		Ok(end)
	}

	fn page_shot(&mut self, rehearsal: &Rehearsal, start: f32, new_place: bool) -> Result<f32> {
		let page = &self.story.pages[rehearsal.page];
		let mut keys = vec![Key {
			time: start,
			scene: rehearsal.opening.clone(),
			line: page.line,
			after: None,
		}];
		let mut lines = Vec::new();
		// A reveal holds the page for the script's `hold=`, and a page in a new place is seen
		// before it is talked over; both only apply to a book that has an age band, so a book
		// written before any of this keeps the timing it was rendered with.
		let mut lead = self.pacing.lead_in;

		if new_place {
			lead = lead.max(self.pacing.reveal);
		}

		let reveal = page.hold()?;

		if reveal > 0.0 {
			self.timeline.holds.push(Span {
				start,
				end: start + reveal,
			});
		}

		lead += reveal;

		let mut t = start + lead;
		let mut opening = true;
		let mut pending: Vec<Pending> = Vec::new();
		let mut camera = keys[0].scene.camera.clone();
		camera.after = None;
		// An action's sound lands on its contact frame plus three frames, not before it. A book
		// without an age keeps the timing every earlier video was rendered with.
		let (action, fall) = if self.story.age.is_some() {
			(CONTACT + AFTER_CONTACT, CONTACT + AFTER_CONTACT)
		} else {
			(0.0, CONTACT)
		};

		for (beat, scene) in page.beats.iter().zip(&rehearsal.after) {
			// A camera change written with `after=` lands later than its line. Land it once
			// the page's clock reaches it, in time order, so a key never jumps backwards over
			// an action that happens first.
			land(&mut keys, &mut pending, start, t, &mut camera);

			match beat {
				Beat::Direct(directive)
					if opening
						&& !(directive.actor == "camera"
							&& scene.camera.after.is_some_and(|after| after > 0.0)) =>
				{
					// Directions before the first line set the scene the page opens on. A camera
					// direction with `after=` lands later by its own clock, even here.
					keys[0].scene = scene.clone();
					keys[0].scene.camera.after = None;

					if directive.actor == "camera" {
						keys[0].after = scene.camera.after;
					}

					camera = keys[0].scene.camera.clone();
					camera.after = None;
				}
				Beat::Direct(directive) => {
					let after = if directive.actor == "camera" {
						scene.camera.after
					} else {
						None
					};
					let at = after.map_or(t, |after| (start + after).max(t));

					if directive.actor == "camera" && at > t + 1e-3 {
						let mut held = scene.clone();
						held.camera = camera.clone();
						held.apply_camera(directive)?;
						held.camera.after = None;

						pending.push(Pending {
							at,
							line: directive.line,
							camera: held.camera,
						});
						continue;
					}

					// A camera direction moves no one, so it never makes a sound of its own.
					if directive.actor != "camera"
						&& let Some(previous) = keys.last()
					{
						auto_cues(
							&previous.scene,
							scene,
							at,
							action,
							fall,
							&mut self.timeline.cues,
						);
					}

					// A stage direction does not move the camera: only the camera that has
					// actually landed carries, and a move, its length and a shake were
					// written for the camera's own line. A camera direction builds on that
					// camera and applies only its own tokens.
					let mut state = scene.clone();
					state.camera = camera.clone();

					if directive.actor == "camera" {
						state.apply_camera(directive)?;
						state.camera.after = None;
						camera = state.camera.clone();
					} else {
						state.camera.travel = None;
						state.camera.over = 0.0;
						state.camera.shake = 0.0;
					}

					push(
						&mut keys,
						at,
						state,
						directive.line,
						after,
						start,
						directive.actor == "camera",
					);
				}
				Beat::Say {
					medium: Medium::BookOnly,
					..
				} => {}
				Beat::Say { text, line, .. } => {
					opening = false;
					t = self.paragraph(text, *line, scene, t, &mut lines)?;
				}
				Beat::Pause { seconds, .. } => {
					opening = false;
					t += seconds.max(0.0);
				}
				Beat::Sfx {
					name, described, ..
				} => {
					if let Some(sound) = Sound::parse(name) {
						self.timeline.cues.push(Cue {
							time: t,
							sound,
							described: described.as_deref().map(std::sync::Arc::from),
						});
					}
				}
				Beat::Hold { seconds, .. } => {
					opening = false;
					let seconds = seconds.max(0.0);

					if seconds > 0.0 {
						self.timeline.holds.push(Span {
							start: t,
							end: t + seconds,
						});
						t += seconds;
					}
				}
				Beat::Music { music, .. } => {
					self.music_sets.push(MusicCue {
						time: t,
						music: *music,
					});
				}
			}
		}

		land(&mut keys, &mut pending, start, f32::INFINITY, &mut camera);

		let end = (t - self.pacing.gap + self.pacing.tail).max(start + lead + self.pacing.tail);

		// The performance pass: what the page's plain directions mean on screen.
		if self.story.meta.video.acting == Acting::Full {
			perform::perform(&mut keys, lines.first().map_or(end, |line| line.start));
		}

		let mut shot = Shot {
			kind: ShotKind::Page(rehearsal.page),
			start,
			end,
			keys,
			lines,
			transition: page
				.transition()
				.unwrap_or(self.story.meta.video.transition.unwrap_or_default()),
			setups: Vec::new(),
			portrait_setups: Vec::new(),
		};

		// A reaction at the very end of a page gets its shot: the page's tail makes the room.
		// A page with its own `@camera` gets the room too, so a pinned plan runs as long.
		if self.story.meta.video.direction == Direction::Auto
			&& let Some(end) = director::room_for_reaction(&shot)
		{
			shot.end = end;
		}

		let end = shot.end;
		self.timeline.shots.push(shot);

		Ok(end)
	}

	/// Speaks one paragraph starting at `t`; returns when the next one may start.
	fn paragraph(
		&mut self,
		text: &str,
		script_line: usize,
		scene: &Scene,
		mut t: f32,
		lines: &mut Vec<Line>,
	) -> Result<f32> {
		let talking: Vec<&crate::stage::Actor> = scene
			.actors
			.iter()
			.filter(|actor| actor.talking && !actor.hidden)
			.collect();
		let speaker = talking.first().copied();
		let speaker_voice = speaker
			.and_then(|actor| self.cast.member(&actor.id))
			.and_then(|member| member.voice.clone());
		let options = &self.story.meta.video;

		if let (true, Some(actor), Some(voice)) = (
			options.cast_voices && has_quotes(text),
			speaker,
			speaker_voice,
		) {
			for (quoted, part) in split_dialogue(text) {
				let line = if quoted {
					let delivery = Delivery {
						mood: Some(actor.mood),
						shout: part.contains("**"),
					};
					let (speech, duration) = self.say(&part, &voice, delivery)?;
					let line = Line {
						start: t,
						speech,
						duration,
						speakers: vec![actor.id.clone()],
						text: format!("\u{201c}{part}\u{201d}"),
						voiced_by: Some(actor.id.clone()),
						line: script_line,
						about: None,
					};
					t += duration + self.pacing.beat;
					line
				} else {
					if options.drop_tags && is_dialogue_tag(&part) {
						continue;
					}

					let narrator = self.narrator.clone();
					let (speech, duration) = self.say(&part, &narrator, Delivery::default())?;
					let line = Line {
						start: t,
						speech,
						duration,
						speakers: Vec::new(),
						about: self.subject(&part, scene),
						text: part,
						voiced_by: None,
						line: script_line,
					};
					t += duration + self.pacing.beat;
					line
				};

				lines.push(line);
			}

			return Ok(t - self.pacing.beat + self.pacing.gap);
		}

		let narrator = self.narrator.clone();
		let (speech, duration) = self.say(text, &narrator, Delivery::default())?;
		let about = if talking.is_empty() {
			self.subject(text, scene)
		} else {
			None
		};
		lines.push(Line {
			start: t,
			speech,
			duration,
			speakers: talking.iter().map(|actor| actor.id.clone()).collect(),
			text: text.to_owned(),
			voiced_by: None,
			line: script_line,
			about,
		});

		Ok(t + duration + self.pacing.gap)
	}

	/// The figure on stage a narrated line is about: the one whose name opens it ("Mo stood
	/// tall."). Dialogue is never about anyone; its speaker is framed instead.
	fn subject(&self, text: &str, scene: &Scene) -> Option<String> {
		if has_quotes(text) {
			return None;
		}

		let plain = plain_text(text);
		let opens = |name: &str| {
			!name.is_empty()
				&& plain
					.strip_prefix(name)
					.is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric()))
		};

		scene
			.actors
			.iter()
			.filter(|actor| actor.kind.is_figure() && !actor.hidden)
			.find(|actor| {
				self.cast.member(&actor.id).is_some_and(|member| {
					opens(&member.name)
						|| opens(member.name.split_whitespace().next().unwrap_or_default())
				})
			})
			.map(|actor| actor.id.clone())
	}
}

/// A camera change written with `after=`, waiting for its moment on the page's clock.
struct Pending {
	/// When it lands, in seconds from the start of the video.
	at: f32,
	/// The script line that wrote it.
	line: usize,
	/// The camera it changes to.
	camera: Camera,
}

/// Lands every pending camera change due by `until`, earliest first. A landing takes the scene
/// the page has reached by then, so it never wipes an action it lands after.
fn land(
	keys: &mut Vec<Key>,
	pending: &mut Vec<Pending>,
	start: f32,
	until: f32,
	camera: &mut Camera,
) {
	while let Some(index) = pending
		.iter()
		.enumerate()
		.filter(|(_, change)| change.at <= until + 1e-3)
		.min_by(|(_, a), (_, b)| a.at.total_cmp(&b.at))
		.map(|(index, _)| index)
	{
		let change = pending.remove(index);
		let mut scene = keys
			.iter()
			.rev()
			.find(|key| key.time <= change.at + 1e-3)
			.map_or_else(|| keys[0].scene.clone(), |key| key.scene.clone());
		scene.camera = change.camera;
		scene.camera.after = None;
		*camera = scene.camera.clone();

		push(
			keys,
			change.at,
			scene,
			change.line,
			Some(change.at - start),
			start,
			true,
		);
	}
}

/// Adds a key, or rewrites the one already at `at`: a camera change and an action at one
/// moment are one state. The action brings the stage, the camera direction brings the camera,
/// and a change that waits carries `after` to name its exact time.
fn push(
	keys: &mut Vec<Key>,
	at: f32,
	scene: Scene,
	line: usize,
	after: Option<f32>,
	start: f32,
	camera: bool,
) {
	// Only a landing that waits names its own time; `after=0` or an inherited value is
	// already where the key sits.
	let after = after.filter(|after| *after > 1e-3 && (start + after - at).abs() < 1e-3);

	match keys.last_mut() {
		Some(last) if (last.time - at).abs() < 1e-3 => {
			if camera {
				last.scene = scene;
				last.after = after.or(last.after);
			} else {
				// The action landed on a camera change: keep the framing and the move, take
				// the action's stage.
				let framing = last.scene.camera.clone();
				last.scene = scene;
				last.scene.camera = framing;
			}
		}
		_ => {
			keys.push(Key {
				time: at,
				scene,
				line,
				after,
			});
		}
	}
}

fn has_quotes(text: &str) -> bool {
	text.contains(['"', '\u{201c}', '\u{201d}'])
}

/// Splits a paragraph into `(is_quote, text)` parts on straight or curly double quotes.
/// Quotes lose a trailing comma (`"Nope," said…` speaks as "Nope").
pub fn split_dialogue(text: &str) -> Vec<(bool, String)> {
	let mut parts = Vec::new();
	let mut current = String::new();
	let mut quoted = false;

	for ch in text.chars() {
		if matches!(ch, '"' | '\u{201c}' | '\u{201d}') {
			push_part(&mut parts, quoted, &current);
			current.clear();
			quoted = !quoted;
			continue;
		}

		current.push(ch);
	}

	push_part(&mut parts, quoted, &current);
	parts
}

fn push_part(parts: &mut Vec<(bool, String)>, quoted: bool, text: &str) {
	let trimmed = text.trim();
	let cleaned = if quoted {
		trimmed.trim_end_matches(',').trim()
	} else {
		trimmed.trim_start_matches(',').trim()
	};

	if !plain_text(cleaned).chars().any(char::is_alphanumeric) {
		return;
	}

	parts.push((quoted, cleaned.to_owned()));
}

/// Short narrator bits like "said Mum." that a show can skip.
pub fn is_dialogue_tag(text: &str) -> bool {
	const VERBS: [&str; 12] = [
		"said",
		"shouted",
		"sighed",
		"asked",
		"called",
		"cried",
		"yelled",
		"whispered",
		"laughed",
		"added",
		"replied",
		"grumbled",
	];
	let words: Vec<String> = text
		.split_whitespace()
		.map(|word| {
			word.trim_matches(|ch: char| !ch.is_alphanumeric())
				.to_lowercase()
		})
		.collect();

	words.len() <= 5
		&& words
			.iter()
			.take(2)
			.any(|word| VERBS.contains(&word.as_str()))
}

/// Sound effects that follow from what happens on stage, so authors only add the special ones.
/// `action` is how long after the direction an action's sound lands and `fall` the same for a
/// body hitting the floor: an older book keeps its timing, and a book with an age band lands
/// both on the contact frame plus three frames.
fn auto_cues(
	before: &Scene,
	after: &Scene,
	time: f32,
	action: f32,
	fall: f32,
	cues: &mut Vec<Cue>,
) {
	for actor in &after.actors {
		let previous = before.actors.iter().find(|other| other.id == actor.id);

		match previous {
			None if !actor.hidden => {
				cues.push(Cue {
					time: time + action,
					sound: Sound::Pop,
					described: None,
				});
			}
			Some(previous) => {
				if actor.knocked > previous.knocked + 0.5 && !matches!(actor.kind, Kind::Bird(_)) {
					cues.push(Cue {
						time: time + fall,
						sound: Sound::Bonk,
						described: None,
					});
				}

				if actor.hops > previous.hops {
					cues.push(Cue {
						time: time + action,
						sound: Sound::Boing,
						described: None,
					});
				}

				if actor.lively > previous.lively + 0.5 && matches!(actor.kind, Kind::Bird(_)) {
					cues.push(Cue {
						time: time + action,
						sound: Sound::Tweet,
						described: None,
					});
				}
			}
			None => {}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::stage::rehearse;
	use crate::story;
	use crate::story::Age;
	use crate::video::lipsync::Track;
	use crate::video::voice::SAMPLE_RATE;

	fn silence(seconds: f32) -> Speech {
		Speech {
			text: String::new(),
			samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
			envelope: Vec::new(),
			track: Track::default(),
		}
	}

	const CAST: &str = "[kid]\nname = \"Kid\"\n[kid.voice]\nname = \"Junior\"\n[mum]\nname = \
	                    \"Mum\"\nkind = \"grown-up\"\n";

	fn timeline_with(front: &str, script: &str) -> (Timeline, Vec<String>) {
		let story = story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\nmoral = \"Be \
			 kind.\"\n{front}\n+++\n{script}"
		))
		.unwrap();
		let cast = Cast::parse(CAST).unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let mut voices = Vec::new();
		let timeline = build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			voices.push(format!("{}: {}", line.voice.name, line.text));
			Ok(silence(line.text.len() as f32 * 0.1))
		})
		.unwrap();

		(timeline, voices)
	}

	fn timeline(script: &str) -> Timeline {
		timeline_with("", script).0
	}

	/// Like [`timeline_with`], but records the rate each line was actually asked for, so the
	/// age band's pace can be measured.
	fn paced(front: &str, script: &str) -> (Timeline, Vec<u32>) {
		let story = story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\nmoral = \"Be \
			 kind.\"\n{front}\n+++\n{script}"
		))
		.unwrap();
		let cast = Cast::parse(CAST).unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let mut rates = Vec::new();
		let timeline = build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			rates.push(line.voice.rate());
			Ok(silence(line.text.len() as f32 * 0.08))
		})
		.unwrap();

		(timeline, rates)
	}

	#[test]
	fn a_book_with_no_age_keeps_the_timing_it_was_rendered_with() {
		let (tl, rates) = paced("", "# park\nOne two.\n# garden\nThree four.\n");
		assert!(rates.iter().all(|rate| *rate == 165), "{rates:?}");

		let page = &tl.shots[1];
		assert!(
			(page.lines[0].start - page.start - Pacing::LEGACY.lead_in).abs() < 1e-4,
			"the old lead-in"
		);

		// One play cue, no ambience and nothing that could move the sound.
		assert_eq!(
			tl.music,
			vec![MusicCue {
				time: 0.0,
				music: Music::Play
			}]
		);
		assert!(tl.ambience.is_empty(), "no ambience");
		assert!(tl.holds.is_empty(), "no holds");
		assert!(tl.dropouts.is_empty(), "no drop-outs");
	}

	#[test]
	fn an_age_band_slows_the_narrator_and_leaves_room_to_look() {
		let (tl, rates) = paced(
			"age = \"under-3\"",
			"# park\nOne two.\n# garden light=dusk\nThree.\n",
		);
		assert_eq!(rates[0], 125, "the band sets an unset narrator's rate");

		let first = &tl.shots[1];
		assert!((first.lines[0].start - first.start - Age::Under3.pacing().lead_in).abs() < 1e-4);

		let new_place = &tl.shots[2];
		assert!(
			new_place.lines[0].start - new_place.start >= 1.5,
			"a new place is seen first: {}",
			new_place.lines[0].start - new_place.start
		);
	}

	#[test]
	fn a_narrator_with_its_own_rate_keeps_it() {
		let (_, rates) = paced(
			"age = \"under-3\"\n[voice]\nname = \"Daniel\"\nrate = 158",
			"# park\nOne.\n",
		);
		assert_eq!(rates[0], 158);
	}

	#[test]
	fn music_follows_the_light_and_the_script_can_say_otherwise() {
		let (tl, _) = paced(
			"age = \"3-5\"",
			"# park light=storm\nOne.\n# park light=dusk\nTwo.\n# park light=moonlit\nThree.\n# \
			 park light=day music=bright\nFour.\n# park light=day\n@music sad\nFive.\n",
		);
		let cues: Vec<Music> = tl.music.iter().map(|cue| cue.music).collect();
		assert_eq!(
			cues,
			vec![
				Music::Play,
				Music::Tense,
				Music::Calm,
				Music::Sleepy,
				Music::Bright,
				Music::Play,
				Music::Sad,
				// The moral card is its own scene, back in the book's default light.
				Music::Play,
			],
			"the title on day, then each page's light, then the script's own cues"
		);

		for pair in tl.music.windows(2) {
			assert!(pair[0].time <= pair[1].time, "cues are in time order");
		}
	}

	#[test]
	fn a_book_that_names_play_holds_one_loop_through_every_light() {
		let (tl, _) = paced(
			"[video]\nmusic = \"play\"",
			"# park light=storm\nOne.\n# park light=night\nTwo.\n",
		);
		assert_eq!(tl.music.len(), 1);
		assert_eq!(tl.music[0].music, Music::Play);
	}

	#[test]
	fn a_shout_without_a_change_of_mood_keeps_the_music() {
		let (tl, _) = paced(
			"[video]\ncast-voices = true",
			"# park\n@kid cross talk\n\"**STOP!**\" said Kid.\n# park\n@kid cross \
			 talk\n\"**AGAIN!**\" said Kid.\n",
		);
		assert!(tl.dropouts.is_empty(), "{:?}", tl.dropouts);
	}

	#[test]
	fn hold_and_reveal_stop_everything() {
		let (tl, _) = paced(
			"age = \"3-5\"",
			"# park\nOne.\n@hold 1.5\nTwo.\n# garden hold=2\nThree.\n",
		);
		assert_eq!(tl.holds.len(), 2);

		let hold = tl.holds[0];
		assert!((hold.end - hold.start - 1.5).abs() < 1e-4);
		assert!(tl.in_hold(hold.start + 0.2));
		assert!(!tl.in_hold(hold.start - 0.1));

		let page = &tl.shots[2];
		let reveal = tl.holds[1];
		assert!((reveal.start - page.start).abs() < 1e-4);
		assert!(
			page.lines[0].start - page.start >= 2.0,
			"the reveal holds the page: {}",
			page.lines[0].start - page.start
		);
	}

	#[test]
	fn ambience_is_off_unless_the_book_asks_for_it_or_has_an_age() {
		let (legacy, _) = paced("", "# park light=night\nOne.\n");
		assert!(legacy.ambience.is_empty(), "legacy has no ambience");

		let (aged, _) = paced("age = \"3-5\"\n[video]\nambience = false", "# park\nOne.\n");
		assert!(
			aged.ambience.is_empty(),
			"`ambience = false` wins over the age"
		);

		let (asked, _) = paced("[video]\nambience = true", "# park\nOne.\n");
		assert!(!asked.ambience.is_empty(), "ambience = true turns it on");
	}

	#[test]
	fn directions_land_when_the_narrator_reaches_them() {
		let tl = timeline("# park\n@kid x=0.3\nOne two.\n@kid faint\nThree.\n");
		let shot = &tl.shots[1];
		assert_eq!(shot.kind, ShotKind::Page(0));
		assert_eq!(shot.keys.len(), 2, "opening + faint");
		assert_eq!(shot.lines.len(), 2);
		assert!(
			(shot.keys[1].time - shot.lines[1].start).abs() < 1e-4,
			"the faint lands as the second line starts"
		);
		assert!(tl.cues.iter().any(|cue| cue.sound == Sound::Bonk));
	}

	#[test]
	fn book_only_lines_are_skipped_and_video_only_lines_are_spoken() {
		let tl = timeline("# park\n~ Only in the book.\n> Only in the video.\n");
		let shot = &tl.shots[1];
		assert_eq!(shot.lines.len(), 1);
		assert_eq!(shot.lines[0].text, "Only in the video.");
	}

	#[test]
	fn shots_are_contiguous_with_title_and_end_cards() {
		let tl = timeline("# park\nHello.\n# garden video=skip\nSkipped.\n# town\nBye.\n");
		let kinds: Vec<ShotKind> = tl.shots.iter().map(|shot| shot.kind).collect();
		assert_eq!(
			kinds,
			vec![
				ShotKind::Title,
				ShotKind::Page(0),
				ShotKind::Page(2),
				ShotKind::End
			]
		);

		for pair in tl.shots.windows(2) {
			assert!((pair[0].end - pair[1].start).abs() < 1e-4);
		}

		assert!((tl.duration - tl.shots.last().unwrap().end).abs() < 1e-4);
	}

	#[test]
	fn a_camera_direction_can_land_inside_a_hold() {
		let tl = timeline(
			"# park\n@kid x=0.3 talk\nHello there today my friend.\n@camera shot=close on=kid \
			 after=4\nWhy hello to you too.\n",
		);
		let shot = &tl.shots[1];
		let late = shot
			.keys
			.iter()
			.find(|key| key.scene.camera.shot == Some(ShotSize::Close))
			.expect("the late close-up is a key");

		assert!(
			(late.time - (shot.start + 4.0)).abs() < 1e-3,
			"the direction lands 4s after the page opens, not at {:.2}s",
			late.time
		);
	}

	#[test]
	fn speakers_follow_talk_directions() {
		let tl = timeline("# park\n@kid talk\n\"Hi!\"\n\nNarrator again.\n");
		let lines = &tl.shots[1].lines;
		assert_eq!(lines[0].speakers, vec!["kid".to_owned()]);
		assert_eq!(lines[1].speakers, Vec::<String>::new());
	}

	#[test]
	fn cast_voices_speak_their_own_quotes() {
		let (tl, voices) = timeline_with(
			"[video]\ncast-voices = true",
			"# park\n@kid defiant talk\n\"**NOPE**,\" said Kid.\n",
		);
		let lines = &tl.shots[1].lines;
		assert_eq!(lines.len(), 2);
		assert_eq!(lines[0].voiced_by.as_deref(), Some("kid"));
		assert_eq!(lines[0].speakers, vec!["kid".to_owned()]);
		assert_eq!(lines[1].voiced_by, None);
		assert!(
			lines[1].speakers.is_empty(),
			"the kid's mouth stays shut while the narrator says the tag"
		);
		assert!(voices.contains(&"Junior: NOPE".to_owned()), "{voices:?}");
		assert!(
			voices.contains(&"Daniel: said Kid.".to_owned()),
			"{voices:?}"
		);
	}

	#[test]
	fn dropped_tags_make_a_show() {
		let (tl, _) = timeline_with(
			"[video]\ncast-voices = true\ndrop-tags = true",
			"# park\n@kid talk\n\"Hello!\" said Kid.\n",
		);
		assert_eq!(tl.shots[1].lines.len(), 1);
	}

	#[test]
	fn characters_without_voices_are_narrated() {
		let (tl, voices) = timeline_with(
			"[video]\ncast-voices = true",
			"# park\n@mum talk\n\"Hello!\" said Mum.\n",
		);
		assert_eq!(tl.shots[1].lines.len(), 1);
		assert!(
			voices.iter().any(|v| v == "Daniel: \"Hello!\" said Mum."),
			"{voices:?}"
		);
	}

	/// Plans a book, recording the rate each line is asked for, with speech that lasts as long
	/// as its words so the timing differences come from the pacing alone.
	fn planned(front: &str, script: &str) -> (Timeline, Vec<u32>) {
		let story = story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\nmoral = \"Be \
			 kind.\"\n{front}\n+++\n{script}"
		))
		.unwrap();
		let cast = Cast::parse(CAST).unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let mut rates = Vec::new();
		let timeline = build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			rates.push(line.voice.rate());
			Ok(silence(line.text.split_whitespace().count() as f32 * 0.4))
		})
		.unwrap();

		(timeline, rates)
	}

	fn pages(timeline: &Timeline) -> Vec<&Shot> {
		timeline
			.shots
			.iter()
			.filter(|shot| shot.kind.page().is_some())
			.collect()
	}

	#[test]
	fn a_book_with_no_age_keeps_its_rate_its_gaps_and_the_play_loop() {
		let (timeline, rates) = planned("", "# park\nOne two.\n# garden\nThree.\n");
		assert!(rates.iter().all(|rate| *rate == 165), "{rates:?}");
		assert_eq!(
			timeline.music,
			vec![MusicCue {
				time: 0.0,
				music: Music::Play
			}]
		);
		assert!(timeline.ambience.is_empty(), "no ambience");
		assert!(timeline.holds.is_empty(), "no holds");
		assert!(timeline.dropouts.is_empty(), "no drop-outs");

		let first = pages(&timeline)[0];
		assert!(
			(first.lines[0].start - first.start - 0.55).abs() < 1e-4,
			"today's lead-in"
		);
	}

	#[test]
	fn an_age_band_slows_the_narrator_and_lengthens_the_gaps() {
		let (legacy, _) = planned("", "# park\nOne two three four.\n");
		let (slow, rates) = planned("age = \"under-3\"", "# park\nOne two three four.\n");
		assert_eq!(rates[0], 125, "the band's rate reaches the narrator");
		assert!(
			slow.duration > legacy.duration + 1.0,
			"slower: {} against {}",
			slow.duration,
			legacy.duration
		);

		let first = pages(&slow)[0];
		assert!(
			first.lines[0].start - first.start >= 1.0,
			"the band's lead-in"
		);
	}

	#[test]
	fn a_page_in_a_new_place_is_seen_before_it_is_talked_over() {
		let (timeline, _) = planned(
			"age = \"3-5\"",
			"# park\nOne.\n# park\nTwo.\n# garden light=night\nThree.\n",
		);
		let page_shots = pages(&timeline);
		// Two pages in the same place, then a garden at night: a new place for the first line.
		let same_place = page_shots[1].lines[0].start - page_shots[1].start;
		let new_place = page_shots[2].lines[0].start - page_shots[2].start;
		assert!(
			(same_place - 1.0).abs() < 1e-4,
			"the band's lead-in: {same_place}"
		);
		assert!(
			new_place >= 1.5,
			"a new place gets room to look: {new_place}"
		);
		assert!(new_place > same_place);

		let (under, _) = planned(
			"age = \"under-3\"",
			"# park\nOne.\n# garden light=night\nTwo.\n",
		);
		let under = pages(&under)[1].lines[0].start - pages(&under)[1].start;
		assert!(under > new_place, "younger: {under} against {new_place}");
	}

	#[test]
	fn a_page_hold_opens_silently_and_delays_the_first_line() {
		let (timeline, _) = planned("age = \"3-5\"", "# park hold=2.0\nOne two.\n");
		let page = pages(&timeline)[0];
		assert!(
			page.lines[0].start - page.start >= 2.0,
			"the reveal: {}",
			page.lines[0].start - page.start
		);
		assert_eq!(timeline.holds.len(), 1);
		assert!((timeline.holds[0].start - page.start).abs() < 1e-4);
		assert!((timeline.holds[0].end - timeline.holds[0].start - 2.0).abs() < 1e-4);
	}

	#[test]
	fn a_mid_page_hold_stops_the_clock_and_every_sound() {
		let (timeline, _) = planned("age = \"3-5\"", "# park\nOne.\n@hold 1.5\nTwo.\n");
		let page = pages(&timeline)[0];
		assert_eq!(timeline.holds.len(), 1);
		let hold = timeline.holds[0];
		assert!((hold.end - hold.start - 1.5).abs() < 1e-4);
		assert!(timeline.in_hold(hold.start + 0.1));
		assert!(!timeline.in_hold(hold.end + 0.1));
		assert!(
			page.lines[1].start >= hold.end - 1e-4,
			"the next line waits for the hold"
		);
	}

	#[test]
	fn a_scene_light_picks_the_music_cue_and_a_page_can_override_it() {
		let (timeline, _) = planned(
			"age = \"3-5\"",
			"# park light=storm\nOne.\n# park light=dusk\nTwo.\n# park light=golden \
			 music=bright\nThree.\n# park light=moonlit\nFour.\n",
		);
		let cues: Vec<Music> = timeline.music.iter().map(|cue| cue.music).collect();
		assert!(cues.contains(&Music::Tense), "{cues:?}");
		assert!(cues.contains(&Music::Calm), "{cues:?}");
		assert!(cues.contains(&Music::Bright), "{cues:?}");
		assert!(cues.contains(&Music::Sleepy), "{cues:?}");

		let bright_page = pages(&timeline)[2];
		let cue = timeline
			.music
			.iter()
			.find(|cue| (cue.time - bright_page.start).abs() < 1e-3)
			.expect("a cue at the bright page");
		assert_eq!(cue.music, Music::Bright, "the heading wins over golden");
	}

	#[test]
	fn a_mid_page_music_direction_changes_the_cue_there() {
		let (timeline, _) = planned(
			"age = \"3-5\"",
			"# park light=day\nOne.\n@music sad\nTwo.\n@scene light=storm\nThree.\n",
		);
		let page = pages(&timeline)[0];
		let sad = timeline
			.music
			.iter()
			.find(|cue| (cue.time - page.lines[1].start).abs() < 1e-3)
			.expect("a cue at the second line");
		assert_eq!(sad.music, Music::Sad);
		let storm = timeline
			.music
			.iter()
			.find(|cue| (cue.time - page.lines[2].start).abs() < 1e-3)
			.expect("a cue at the third line");
		assert_eq!(storm.music, Music::Tense, "a mid-page storm overrides sad");
	}

	#[test]
	fn music_play_holds_one_loop_through_every_light() {
		let (timeline, _) = planned(
			"age = \"3-5\"\n[video]\nmusic = \"play\"",
			"# park light=storm\nOne.\n# park light=moonlit\nTwo.\n",
		);
		assert_eq!(timeline.music.len(), 1, "{:?}", timeline.music);
		assert_eq!(timeline.music[0].music, Music::Play);
	}

	#[test]
	fn a_shouted_peak_from_a_fresh_strong_mood_drops_the_music_out() {
		let (timeline, _) = planned(
			"[video]\ncast-voices = true",
			"# park\n@kid happy talk\n\"Hello.\"\n# park\n@kid defiant talk\n\"**STOP!**\" said \
			 Kid.\n",
		);
		let line = timeline
			.shots
			.iter()
			.flat_map(|shot| &shot.lines)
			.find(|line| line.text.contains("STOP"))
			.expect("the shouted line");
		assert_eq!(timeline.dropouts.len(), 1);
		let dropout = timeline.dropouts[0];
		assert!((dropout.start - (line.start - 0.5)).abs() < 1e-3);
		assert!(dropout.end >= line.start + line.duration);
	}

	#[test]
	fn a_shout_without_a_fresh_strong_mood_keeps_its_music() {
		let (timeline, _) = planned(
			"[video]\ncast-voices = true",
			"# park\n@kid happy talk\n\"Hello.\"\n# park\n@kid happy talk\n\"**STOP!**\" said \
			 Kid.\n",
		);
		assert!(timeline.dropouts.is_empty(), "{:?}", timeline.dropouts);
	}

	#[test]
	fn ambience_comes_from_the_setting_and_the_light() {
		let (timeline, _) = planned(
			"age = \"3-5\"",
			"# park\nOne.\n# bathroom light=night\nTwo.\n# park light=night\nThree.\n# hilltop \
			 light=storm\nFour.\n",
		);
		let page_shots = pages(&timeline);
		assert_eq!(
			timeline.ambience_at(page_shots[0].start + 0.1),
			&[Bed::Birds, Bed::Breeze]
		);
		assert_eq!(
			timeline.ambience_at(page_shots[1].start + 0.1),
			&[Bed::Room],
			"an indoor night keeps the crickets outside"
		);
		assert_eq!(
			timeline.ambience_at(page_shots[2].start + 0.1),
			&[Bed::Birds, Bed::Breeze, Bed::Crickets]
		);
		assert_eq!(
			timeline.ambience_at(page_shots[3].start + 0.1),
			&[Bed::Wind, Bed::Breeze, Bed::Rain]
		);
	}

	#[test]
	fn a_scene_can_silence_its_ambience_and_a_legacy_book_has_none() {
		let (timeline, _) = planned("age = \"3-5\"", "# park\n@scene ambience=none\nOne.\n");
		let page = pages(&timeline)[0];
		assert!(
			timeline.ambience_at(page.start + 0.1).is_empty(),
			"silenced"
		);

		let (legacy, _) = planned("[video]\nambience = true", "# park\nOne.\n");
		assert!(!legacy.ambience.is_empty(), "asked for, it is on");
		let (off, _) = planned("age = \"3-5\"\n[video]\nambience = false", "# park\nOne.\n");
		assert!(off.ambience.is_empty(), "and false turns it off");
	}

	#[test]
	fn dialogue_splits_on_quotes() {
		assert_eq!(
			split_dialogue("\"**NOPE**,\" said Stanley. \"Teeth are boring.\""),
			vec![
				(true, "**NOPE**".to_owned()),
				(false, "said Stanley.".to_owned()),
				(true, "Teeth are boring.".to_owned())
			]
		);
		assert!(is_dialogue_tag("said Mum."));
		assert!(is_dialogue_tag("shouted Shouty McShoutface."));
		assert!(!is_dialogue_tag("The postman fainted flat on his back."));
	}
}
