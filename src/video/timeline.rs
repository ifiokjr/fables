//! The video timeline: the rehearsal laid out against the narrator's clock.
//!
//! Each video page becomes a shot. A shot holds scene keyframes (from stage directions), the
//! lines spoken over it and sound cues. Directions take effect at the moment the narrator
//! reaches them, which is what makes characters "do what the narrator says".
//!
//! With `cast-voices` on, a paragraph like `"Brush your teeth!" said Mum.` is split: Mum says
//! the quoted words in her own voice, in the mood the script has her in, and the narrator says
//! the rest (or, with `drop-tags`, nothing, so the scene plays like a show).

use anyhow::Result;

use crate::book;
use crate::cast::Cast;
use crate::cast::Kind;
use crate::stage::Camera;
use crate::stage::Rehearsal;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::stage::Travel;
use crate::story::Beat;
use crate::story::Medium;
use crate::story::Story;
use crate::story::Transition;
use crate::story::plain_text;
use crate::video::audio::Sound;
use crate::video::director;
use crate::video::voice::Delivery;
use crate::video::voice::Speech;
use crate::video::voice::Utterance;
use crate::video::voice::Voice;

/// Silence before the first line of a shot, so the new scene can land.
const LEAD_IN: f32 = 0.55;
/// Silence between paragraphs.
const GAP: f32 = 0.4;
/// Silence between a quote and its dialogue tag within one paragraph.
const BEAT: f32 = 0.18;
/// Silence after the last line, before the page turns.
const TAIL: f32 = 0.75;

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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
	pub time: f32,
	pub sound: Sound,
}

#[derive(Clone, Debug)]
pub struct Timeline {
	pub shots: Vec<Shot>,
	pub speeches: Vec<Speech>,
	pub cues: Vec<Cue>,
	pub duration: f32,
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
}

/// Turns an utterance into speech: TTS in production, silence of a known length in tests.
pub trait Speaker: FnMut(&Utterance<'_>) -> Result<Speech> {}
impl<F: FnMut(&Utterance<'_>) -> Result<Speech>> Speaker for F {}

struct Builder<'a, S: Speaker> {
	story: &'a Story,
	cast: &'a Cast,
	speak: S,
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

	let mut builder = Builder {
		story,
		cast,
		speak,
		timeline: Timeline {
			shots: Vec::new(),
			speeches: Vec::new(),
			cues: Vec::new(),
			duration: 0.0,
		},
	};
	let mut cursor = builder.title_shot(0.0)?;

	for rehearsal in rehearsals {
		if !story.pages[rehearsal.page].in_video() {
			continue;
		}

		builder.cue(cursor, Sound::PageTurn);
		cursor = builder.page_shot(rehearsal, cursor)?;
	}

	if let Some(moral) = &story.meta.moral {
		builder.cue(cursor, Sound::PageTurn);
		cursor = builder.end_shot(moral, cursor)?;
	}

	let mut timeline = builder.timeline;
	timeline.duration = cursor;
	timeline.cues.sort_by(|a, b| a.time.total_cmp(&b.time));

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

impl<S: Speaker> Builder<'_, S> {
	fn cue(&mut self, time: f32, sound: Sound) {
		self.timeline.cues.push(Cue { time, sound });
	}

	fn narrator(&self) -> &Voice {
		self.story.narrator()
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
		let scene = book::star_scene(
			story,
			self.cast,
			&pose,
			1.0,
			&mut std::collections::HashMap::new(),
		)?;
		let mut text = story.meta.title.clone();

		if let Some(subtitle) = &story.meta.subtitle {
			text = format!("{text}. {subtitle}.");
		}

		let line_start = start + 0.8;
		let narrator = self.narrator().clone();
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

		let line_start = start + LEAD_IN;
		let narrator = self.narrator().clone();
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
			}],
			transition: self.story.meta.video.transition.unwrap_or_default(),
			setups: Vec::new(),
			portrait_setups: Vec::new(),
		});

		Ok(end)
	}

	fn page_shot(&mut self, rehearsal: &Rehearsal, start: f32) -> Result<f32> {
		let page = &self.story.pages[rehearsal.page];
		let mut keys = vec![Key {
			time: start,
			scene: rehearsal.opening.clone(),
			line: page.line,
			after: None,
		}];
		let mut lines = Vec::new();
		let mut t = start + LEAD_IN;
		let mut opening = true;
		let mut pending: Vec<Pending> = Vec::new();
		let mut camera = keys[0].scene.camera.clone();
		camera.after = None;

		for (beat, scene) in page.beats.iter().zip(&rehearsal.after) {
			// A camera change written with `after=` lands later than its line. Land it once
			// the page's clock reaches it, in time order, so a key never jumps backwards over
			// an action that happens first.
			land(&mut keys, &mut pending, start, t, &mut camera);

			match beat {
				Beat::Direct(directive)
					if opening
						&& !(directive.actor == "camera"
							&& after(directive).is_some_and(|after| after > 0.0)) =>
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
						auto_cues(&previous.scene, scene, at, &mut self.timeline.cues);
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

					push(&mut keys, at, state, directive.line, after, start);
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
				Beat::Sfx { name, .. } => {
					if let Some(sound) = Sound::parse(name) {
						self.cue(t, sound);
					}
				}
			}
		}

		land(&mut keys, &mut pending, start, f32::INFINITY, &mut camera);

		let end = (t - GAP + TAIL).max(start + LEAD_IN + TAIL);

		self.timeline.shots.push(Shot {
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
		});

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
					};
					t += duration + BEAT;
					line
				} else {
					if options.drop_tags && is_dialogue_tag(&part) {
						continue;
					}

					let narrator = self.narrator().clone();
					let (speech, duration) = self.say(&part, &narrator, Delivery::default())?;
					let line = Line {
						start: t,
						speech,
						duration,
						speakers: Vec::new(),
						text: part,
						voiced_by: None,
						line: script_line,
					};
					t += duration + BEAT;
					line
				};

				lines.push(line);
			}

			return Ok(t - BEAT + GAP);
		}

		let narrator = self.narrator().clone();
		let (speech, duration) = self.say(text, &narrator, Delivery::default())?;
		lines.push(Line {
			start: t,
			speech,
			duration,
			speakers: talking.iter().map(|actor| actor.id.clone()).collect(),
			text: text.to_owned(),
			voiced_by: None,
			line: script_line,
		});

		Ok(t + duration + GAP)
	}
}

/// When a camera direction lands, in seconds after the page opens, when it says `after=`.
fn after(directive: &crate::story::Directive) -> Option<f32> {
	directive
		.tokens
		.iter()
		.find(|token| token.key == "after")
		.and_then(|token| token.value.as_deref())
		.and_then(|value| value.parse::<f32>().ok())
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
		);
	}
}

/// Adds a key, or rewrites the one already at `at`: a direction and an action at one moment are
/// one state. Only a landing that waits carries `after`; it names the setup's exact time.
fn push(keys: &mut Vec<Key>, at: f32, scene: Scene, line: usize, after: Option<f32>, start: f32) {
	// Only a landing that waits names its own time; `after=0` or an inherited value is
	// already where the key sits.
	let after = after.filter(|after| *after > 1e-3 && (start + after - at).abs() < 1e-3);

	match keys.last_mut() {
		Some(last) if (last.time - at).abs() < 1e-3 => {
			// An action landing on a pinned camera change keeps the change's exact time:
			// the action carries no `after`, so it cannot say when the camera should land.
			last.after = after.or(last.after);
			last.scene = scene;
		}
		_ => {
			keys.push(Key {
				time: at,
				scene,
				line,
				after,
			})
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
fn auto_cues(before: &Scene, after: &Scene, time: f32, cues: &mut Vec<Cue>) {
	for actor in &after.actors {
		let previous = before.actors.iter().find(|other| other.id == actor.id);

		match previous {
			None if !actor.hidden => {
				cues.push(Cue {
					time,
					sound: Sound::Pop,
				});
			}
			Some(previous) => {
				if actor.knocked > previous.knocked + 0.5 && !matches!(actor.kind, Kind::Bird(_)) {
					cues.push(Cue {
						time: time + 0.25,
						sound: Sound::Bonk,
					});
				}

				if actor.hops > previous.hops {
					cues.push(Cue {
						time,
						sound: Sound::Boing,
					});
				}

				if actor.lively > previous.lively + 0.5 && matches!(actor.kind, Kind::Bird(_)) {
					cues.push(Cue {
						time,
						sound: Sound::Tweet,
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
