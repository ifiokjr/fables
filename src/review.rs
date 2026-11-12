//! A critique of a rehearsed story: what a director would change about the script, as
//! findings with a severity, the script line, an observation and a suggestion.
//!
//! [`review`] is pure: it reads a story, its rehearsal and a timeline with estimated
//! durations (the same plan the storyboard draws), and gives advice. It never refuses a
//! script — `fables check` says whether a script works; this says whether it works *well*.

use serde::Serialize;

use crate::art::figure::Mood;
use crate::cast::Kind;
use crate::stage::Actor;
use crate::stage::Rehearsal;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::story::Beat;
use crate::story::Medium;
use crate::story::Page;
use crate::story::Story;
use crate::story::plain_text;
use crate::video::timeline::ShotKind;
use crate::video::timeline::Timeline;

/// The most words a picture-book page should ask a reader to say in one breath.
pub const PAGE_WORD_LIMIT: usize = 45;
/// A picture book reads best between these word counts.
pub const BOOK_WORDS: (usize, usize) = (250, 900);
/// Shorts cap a portrait video at three minutes.
pub const VIDEO_LIMIT: f32 = 180.0;
/// Two people need at least this much stage between them to read as two people.
pub const MIN_X_GAP: f32 = 0.15;
/// Where the page's text card sits, as fractions of the stage: top and bottom of each band.
const CARD_TOP: (f32, f32) = (0.05, 0.41);
const CARD_BOTTOM: (f32, f32) = (0.65, 0.97);

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	/// The script will build, but this works against the book or the video; change it.
	Fix,
	/// Worth a thought; often right to keep as it is.
	Consider,
}

/// One thing a director would change, with where and how.
#[derive(Clone, Debug, Serialize)]
pub struct Finding {
	/// The rule that found it, e.g. `static-page`.
	pub rule: &'static str,
	pub severity: Severity,
	/// The script line it points at.
	pub line: usize,
	/// One sentence on what was seen.
	pub observation: String,
	/// What to do about it, in script vocabulary.
	pub suggestion: String,
}

impl Finding {
	fn new(
		rule: &'static str,
		severity: Severity,
		line: usize,
		observation: impl Into<String>,
		suggestion: impl Into<String>,
	) -> Finding {
		Finding {
			rule,
			severity,
			line,
			observation: observation.into(),
			suggestion: suggestion.into(),
		}
	}
}

/// The counts at the end of a review.
pub fn summary(findings: &[Finding]) -> String {
	let fix = findings
		.iter()
		.filter(|f| f.severity == Severity::Fix)
		.count();
	let consider = findings.len() - fix;
	format!("{fix} fix, {consider} consider.")
}

/// Reads a rehearsed story and its planned video, and says what a director would change.
/// Findings come back fixes first, then in script order.
pub fn review(story: &Story, rehearsals: &[Rehearsal], timeline: &Timeline) -> Vec<Finding> {
	let mut findings = Vec::new();
	findings.extend(static_pages(story, rehearsals));
	findings.extend(long_runs(story, rehearsals));
	findings.extend(mood_arc(story, rehearsals));
	findings.extend(absent_star(story, rehearsals));
	findings.extend(speech(story, rehearsals));
	findings.extend(word_budget(story));
	findings.extend(close_shots(story));
	findings.extend(flat_light(story, rehearsals));
	findings.extend(idle_arrivals(story));
	findings.extend(crowding(story, rehearsals));
	findings.extend(under_the_card(story, rehearsals));
	findings.extend(video_length(story, timeline));
	findings.extend(reactions(story));
	findings.extend(time_jumps(story));

	findings.sort_by_key(|finding| (finding.severity != Severity::Fix, finding.line));
	findings
}

/// The state of the stage at the end of each page.
fn finales(rehearsals: &[Rehearsal]) -> Vec<&Scene> {
	rehearsals.iter().map(Rehearsal::finale).collect()
}

/// What a page looks like: where everyone stands and how they feel. Two pages with the same
/// tableau look identical to a reader.
fn tableau(scene: &Scene) -> Vec<(String, Option<f32>, f32, Mood, bool)> {
	scene
		.actors
		.iter()
		.filter(|actor| !actor.hidden)
		.map(|actor| {
			(
				actor.id.clone(),
				actor.x,
				actor.size,
				actor.mood,
				actor.knocked > 0.5,
			)
		})
		.collect()
}

/// Whether the previous page's stage has visibly changed.
fn same_stage(a: &Scene, b: &Scene) -> bool {
	a.setting == b.setting
		&& a.weather.light == b.weather.light
		&& a.weather.night == b.weather.night
		&& (a.weather.wilt - b.weather.wilt).abs() < 1e-4
		&& tableau(a) == tableau(b)
}

/// A page with narration but no stage direction, or whose stage is identical to the previous
/// page's: nothing changed to look at.
fn static_pages(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for page in &story.pages {
		let narrates = page
			.beats
			.iter()
			.any(|beat| matches!(beat, Beat::Say { medium, .. } if *medium != Medium::BookOnly));
		let directs = page
			.beats
			.iter()
			.any(|beat| matches!(beat, Beat::Direct(_)));

		if narrates && !directs {
			findings.push(Finding::new(
				"static-page",
				Severity::Consider,
				page.line,
				"This page narrates but nobody on stage does anything.",
				"Add a direction — a mood, a move (`@mo x=0.7`), a `talk` — so the picture \
				 changes.",
			));
		}
	}

	for pair in rehearsals.windows(2) {
		if same_stage(pair[0].finale(), pair[1].finale()) {
			let page = &story.pages[pair[1].page];

			findings.push(Finding::new(
				"static-page",
				Severity::Consider,
				page.line,
				"This page's stage is identical to the last one's.",
				"Change something to look at: a mood, an `x=`, an entrance, or the `light=`.",
			));
		}
	}

	findings
}

/// A run of more than three pages in the same setting and light where nobody's mood changes
/// and no shot is written.
fn long_runs(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();
	let mut run: Vec<usize> = Vec::new();

	let flush = |run: &[usize], findings: &mut Vec<Finding>| {
		if run.len() > 3 {
			let index = run[3];
			let page = &story.pages[index];
			let setting = page.setting.as_str();

			findings.push(Finding::new(
				"long-run",
				Severity::Consider,
				page.line,
				format!(
					"{} pages run in the {setting} under the same light with no shot or mood \
					 change.",
					run.len()
				),
				"Change the `light=`, cut to a new setting, or write `shot=close on=…` somewhere \
				 in the run.",
			));
		}
	};

	for index in 0..story.pages.len() {
		let continues = run.last().is_some_and(|&last| {
			let before = rehearsals[last].finale();
			let here = rehearsals[index].finale();
			before.setting == here.setting && before.weather.light == here.weather.light
		});

		if continues {
			run.push(index);
		} else {
			flush(&run, &mut findings);
			run.clear();
			run.push(index);
		}
	}

	flush(&run, &mut findings);
	findings
}

/// The star's mood never changes across the book, or ends where it began.
fn mood_arc(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let Some(star) = story.meta.star.as_deref() else {
		return Vec::new();
	};

	let moods: Vec<(usize, Mood)> = finales(rehearsals)
		.iter()
		.enumerate()
		.filter_map(|(index, scene)| {
			let actor = scene
				.actors
				.iter()
				.find(|actor| actor.id.split('.').next() == Some(star) && !actor.hidden)?;
			Some((index, actor.mood))
		})
		.collect();

	if moods.len() < 2 {
		return Vec::new();
	}

	let distinct: Vec<Mood> = {
		let mut seen = moods.iter().map(|(_, mood)| *mood).collect::<Vec<_>>();
		seen.dedup();
		seen
	};

	if distinct.len() == 1 {
		return vec![Finding::new(
			"mood-arc",
			Severity::Consider,
			story.pages[moods[0].0].line,
			format!("{star}'s mood never changes across the book."),
			format!(
				"Let the story move them: direct a mood on the page that turns it (`@{star} \
				 shocked`)."
			),
		)];
	}

	if story.pages.len() > 2 && moods[0].1 == moods[moods.len() - 1].1 {
		return vec![Finding::new(
			"mood-arc",
			Severity::Consider,
			story.pages[moods[moods.len() - 1].0].line,
			format!("{star}'s last page feels like their first: the arc ends where it began."),
			format!("End on the mood the story earned (`@{star} proud`, `@{star} grin`)."),
		)];
	}

	Vec::new()
}

/// The star off stage for more than two pages running.
fn absent_star(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let Some(star) = story.meta.star.as_deref() else {
		return Vec::new();
	};

	let mut findings = Vec::new();
	let mut absent = 0;

	for (index, scene) in finales(rehearsals).iter().enumerate() {
		let on = scene
			.actors
			.iter()
			.any(|actor| actor.id.split('.').next() == Some(star) && !actor.hidden);

		absent = if on { 0 } else { absent + 1 };

		if absent == 3 {
			findings.push(Finding::new(
				"absent-star",
				Severity::Consider,
				story.pages[index].line,
				format!("{star} has been off stage for three pages."),
				format!(
					"Bring them back (`@{star} enter=left`), or make the guest the star of these \
					 pages."
				),
			));
		}
	}

	findings
}

/// Whether a paragraph carries quoted dialogue.
fn has_dialogue(text: &str) -> bool {
	text.contains(['"', '\u{201c}', '\u{201d}'])
}

/// Quoted dialogue with no `talk` on anybody, or `talk` on somebody the reader cannot see.
fn speech(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for (index, page) in story.pages.iter().enumerate() {
		for (beat, scene) in page.beats.iter().zip(&rehearsals[index].after) {
			let Beat::Say { text, line, .. } = beat else {
				continue;
			};

			if !has_dialogue(text) {
				continue;
			}

			let speakers: Vec<&Actor> = scene.actors.iter().filter(|actor| actor.talking).collect();

			if speakers.is_empty() {
				findings.push(Finding::new(
					"speech",
					Severity::Fix,
					*line,
					"Quoted dialogue, but nobody has `talk`.",
					"Put `@who talk` on the line above so the speaker owns the words.",
				));
			}

			for speaker in speakers.iter().filter(|speaker| speaker.hidden) {
				findings.push(Finding::new(
					"speech",
					Severity::Fix,
					*line,
					format!("`talk` is on @{}, but they are `hide`-den.", speaker.id),
					format!(
						"`@{} show` them first, or give the line to somebody on stage.",
						speaker.id
					),
				));
			}
		}
	}

	findings
}

/// Pages over the read-aloud limit, and books outside the picture-book word count.
fn word_budget(story: &Story) -> Vec<Finding> {
	let mut findings = Vec::new();

	for page in &story.pages {
		let words: usize = page
			.book_text()
			.iter()
			.map(|text| text.split_whitespace().count())
			.sum();

		if words > PAGE_WORD_LIMIT {
			findings.push(Finding::new(
				"word-budget",
				Severity::Fix,
				page.line,
				format!("The page reads {words} words; about {PAGE_WORD_LIMIT} is a breath."),
				"Split the page, or move words to a `~` book-only line.",
			));
		}
	}

	let words = story_words(story);

	if words < BOOK_WORDS.0 || words > BOOK_WORDS.1 {
		findings.push(Finding::new(
			"word-budget",
			Severity::Consider,
			story.pages[0].line,
			format!(
				"The book reads {words} words; picture books land between {} and {}.",
				BOOK_WORDS.0, BOOK_WORDS.1
			),
			"Grow the middle beats, or trim towards one clear idea per page.",
		));
	}

	findings
}

/// The words a reader says aloud, as `fables check` counts them.
pub fn story_words(story: &Story) -> usize {
	story
		.pages
		.iter()
		.flat_map(Page::book_text)
		.map(|text| text.split_whitespace().count())
		.sum()
}

/// Whether a page or its directions ask for a close framing.
fn close_on_page(page: &Page) -> bool {
	let heading = page
		.option("shot")
		.and_then(|token| token.value.as_deref())
		.is_some_and(|value| {
			ShotSize::parse(value)
				.is_some_and(|size| matches!(size, ShotSize::Close | ShotSize::ExtremeClose))
		});

	let camera = page.beats.iter().any(|beat| {
		match beat {
			Beat::Direct(directive) if directive.actor == "camera" => {
				directive.tokens.iter().any(|token| {
					token.key == "shot"
						&& token.value.as_deref().is_some_and(|value| {
							matches!(
								ShotSize::parse(value),
								Some(ShotSize::Close | ShotSize::ExtremeClose)
							)
						})
				})
			}
			_ => false,
		}
	});

	heading || camera
}

/// The mood changes a page directs, counted by mood words in its directions.
fn mood_directions(page: &Page) -> usize {
	page.beats
		.iter()
		.filter_map(|beat| {
			match beat {
				Beat::Direct(directive) => {
					Some(
						directive
							.tokens
							.iter()
							.filter(|token| {
								Mood::parse(&token.key).is_some()
									|| token.key == "mood"
										&& token
											.value
											.as_deref()
											.is_some_and(|value| Mood::parse(value).is_some())
							})
							.count(),
					)
				}
				_ => None,
			}
		})
		.sum()
}

/// No close or extreme-close shot in a long book, or none on the page with the strongest
/// mood change.
fn close_shots(story: &Story) -> Vec<Finding> {
	if story.pages.len() <= 6 {
		return Vec::new();
	}

	let mut findings = Vec::new();
	let anywhere = story.pages.iter().any(close_on_page);

	if !anywhere {
		findings.push(Finding::new(
			"close-shots",
			Severity::Consider,
			story.pages[0].line,
			"A book of this length never gets close to anybody.",
			"Write `shot=close on=…` (or `@camera shot=extreme-close on=…`) on the page that \
			 matters most.",
		));
	}

	let strongest = story
		.pages
		.iter()
		.enumerate()
		.map(|(index, page)| (index, mood_directions(page)))
		.max_by_key(|(_, changes)| *changes)
		.filter(|(_, changes)| *changes > 0)
		.map(|(index, _)| index);

	if let Some(index) = strongest
		&& anywhere
		&& !close_on_page(&story.pages[index])
	{
		findings.push(Finding::new(
			"close-shots",
			Severity::Consider,
			story.pages[index].line,
			"The page with the strongest mood change stays at a distance.",
			"Frame it: `shot=close on=…` in the heading, or an `@camera` direction mid-page.",
		));
	}

	findings
}

/// A time of day a word in the text implies.
fn phase_of(word: &str) -> Option<&'static str> {
	Some(match word.to_lowercase().as_str() {
		"dawn" | "sunrise" | "morning" => "dawn",
		"noon" | "midday" | "afternoon" => "day",
		"dusk" | "sunset" | "evening" | "golden" => "dusk",
		"night" | "midnight" | "moon" | "moonlit" | "dark" | "stars" => "night",
		_ => return None,
	})
}

/// Every page in the same light while the words move from morning to night.
fn flat_light(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let Some(first) = rehearsals.first() else {
		return Vec::new();
	};

	if !rehearsals
		.iter()
		.all(|rehearsal| rehearsal.finale().weather.light == first.finale().weather.light)
	{
		return Vec::new();
	}

	let words: Vec<String> = story
		.pages
		.iter()
		.flat_map(|page| page.book_text())
		.flat_map(|text| {
			plain_text(text)
				.split(|ch: char| !ch.is_alphanumeric())
				.filter(|word| !word.is_empty())
				.map(str::to_owned)
				.collect::<Vec<_>>()
		})
		.collect();
	let phases: std::collections::HashSet<&str> =
		words.iter().filter_map(|w| phase_of(w)).collect();

	if phases.len() < 2 {
		return Vec::new();
	}

	vec![Finding::new(
		"flat-light",
		Severity::Consider,
		story.pages[0].line,
		"The words move through the day, but every page keeps the same light.",
		"Give each sequence its own `light=` (`dawn`, `golden`, `dusk`, `moonlit`) at the story's \
		 turns.",
	)]
}

/// An actor who is directed but never does anything or speaks: every direction they get is
/// placement.
fn idle_arrivals(story: &Story) -> Vec<Finding> {
	const PLACEMENT: [&str; 9] = [
		"x", "y", "size", "z", "face", "enter", "exit", "colour", "color",
	];

	let mut findings = Vec::new();

	for id in cast_ids(story) {
		let mut does_something = false;
		let mut first_line = None;

		for page in &story.pages {
			for beat in &page.beats {
				let Beat::Direct(directive) = beat else {
					continue;
				};

				if directive.actor.split('.').next() != Some(id.as_str()) {
					continue;
				}

				first_line.get_or_insert(directive.line);

				does_something |= directive
					.tokens
					.iter()
					.any(|token| !PLACEMENT.contains(&token.key.as_str()));
			}
		}

		if let Some(line) = first_line
			&& !does_something
		{
			findings.push(Finding::new(
				"idle-arrival",
				Severity::Consider,
				line,
				format!("@{id} arrives but never does anything or speaks."),
				format!("Give them a beat — `@{id} talk`, a mood, a pose — or leave them out."),
			));
		}
	}

	findings
}

/// The distinct actor ids a script directs, in first-seen order.
fn cast_ids(story: &Story) -> Vec<String> {
	let mut ids: Vec<String> = Vec::new();

	for page in &story.pages {
		for beat in &page.beats {
			if let Beat::Direct(directive) = beat {
				let base = directive
					.actor
					.split('.')
					.next()
					.unwrap_or(&directive.actor);

				if !ids.iter().any(|id| id == base) {
					ids.push(base.to_owned());
				}
			}
		}
	}

	ids
}

/// Two people staged so close they read as one shape.
fn crowding(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for (index, scene) in finales(rehearsals).iter().enumerate() {
		let figures: Vec<&Actor> = scene
			.actors
			.iter()
			.filter(|actor| {
				!actor.hidden && matches!(actor.kind, Kind::Figure(_)) && actor.x.is_some()
			})
			.collect();

		for i in 0..figures.len() {
			for other in figures.iter().skip(i + 1) {
				let (a, b) = (figures[i], other);

				if (a.x.unwrap_or(0.0) - b.x.unwrap_or(0.0)).abs() < MIN_X_GAP {
					findings.push(Finding::new(
						"crowding",
						Severity::Consider,
						story.pages[index].line,
						format!(
							"@{} and @{} stand within {:.0}% of each other.",
							a.id,
							b.id,
							MIN_X_GAP * 100.0
						),
						"Give each their own space (`x=0.3`, `x=0.7`), or drop the `x=`s and let \
						 the spacing place them.",
					));
				}
			}
		}
	}

	findings
}

/// A figure placed where the page's text card will sit.
fn under_the_card(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for (index, scene) in finales(rehearsals).iter().enumerate() {
		let band = match scene.text {
			crate::story::TextPlace::Top => CARD_TOP,
			crate::story::TextPlace::Bottom => CARD_BOTTOM,
			crate::story::TextPlace::None => continue,
		};

		for actor in scene.actors.iter().filter(|actor| !actor.hidden) {
			if actor.y.is_some_and(|y| y >= band.0 && y <= band.1) {
				findings.push(Finding::new(
					"under-card",
					Severity::Consider,
					story.pages[index].line,
					format!("@{} sits where the text card lands.", actor.id),
					"Move them clear of the card (`y=`), or move the card (`text=bottom`).",
				));
			}
		}
	}

	findings
}

/// A portrait video over the Shorts limit, or a page that flashes past.
fn video_length(story: &Story, timeline: &Timeline) -> Vec<Finding> {
	let mut findings = Vec::new();

	if timeline.duration > VIDEO_LIMIT {
		findings.push(Finding::new(
			"video-length",
			Severity::Fix,
			story.pages[0].line,
			format!(
				"The video runs {:.0}s; Shorts cap at {:.0}s.",
				timeline.duration, VIDEO_LIMIT
			),
			"Trim narration into `~` book-only lines, or `@pause` less between beats.",
		));
	}

	for shot in &timeline.shots {
		if let ShotKind::Page(page) = shot.kind
			&& shot.duration() < 2.0
		{
			findings.push(Finding::new(
				"video-length",
				Severity::Consider,
				story.pages[page].line,
				format!("Page {} flashes past in {:.1}s.", page + 1, shot.duration()),
				"Give it a line to land, or a `@pause 0.5` of its own.",
			));
		}
	}

	findings
}

/// The moods that play as takes: a shock or a laugh needs a beat to land.
const STRONG_MOODS: [&str; 3] = ["shocked", "fainted", "laugh"];

/// A strong mood directed in the same line as a `talk`, with no `@pause` before the next
/// line: the reaction has no time to land.
fn reactions(story: &Story) -> Vec<Finding> {
	let mut findings = Vec::new();

	for page in &story.pages {
		for (index, beat) in page.beats.iter().enumerate() {
			let Beat::Direct(directive) = beat else {
				continue;
			};

			let strong = directive.tokens.iter().any(|token| {
				STRONG_MOODS.contains(&token.key.as_str())
					|| token.key == "mood"
						&& token
							.value
							.as_deref()
							.is_some_and(|value| STRONG_MOODS.contains(&value))
			});
			let talks = directive
				.tokens
				.iter()
				.any(|token| matches!(token.key.as_str(), "talk" | "says" | "shout"));

			if !strong || !talks {
				continue;
			}

			// This directive's paragraph, then the beats up to the next paragraph.
			let Some(say) = page.beats[index..]
				.iter()
				.position(|beat| matches!(beat, Beat::Say { .. }))
				.map(|offset| offset + index)
			else {
				continue;
			};

			let paused = page.beats[say + 1..]
				.iter()
				.take_while(|beat| !matches!(beat, Beat::Say { .. }))
				.any(|beat| matches!(beat, Beat::Pause { .. }));

			if !paused {
				findings.push(Finding::new(
					"reaction",
					Severity::Consider,
					directive.line,
					"A strong mood and a `talk` share one line, and the next line follows at once.",
					"Put `@pause 0.8` after the line so the reaction lands before the next one.",
				));
			}
		}
	}

	findings
}

/// Words that say time has jumped between pages.
const TIME_JUMP_WORDS: [&str; 2] = ["later", "tomorrow"];
const TIME_JUMP_PHRASES: [&str; 1] = ["next day"];

/// Whether a paragraph says time has jumped.
fn mentions_jump(text: &str) -> bool {
	let plain = plain_text(text).to_lowercase();
	let words: Vec<&str> = plain
		.split(|ch: char| !ch.is_alphanumeric())
		.filter(|word| !word.is_empty())
		.collect();

	words.iter().any(|word| TIME_JUMP_WORDS.contains(word))
		|| TIME_JUMP_PHRASES
			.iter()
			.any(|phrase| plain.contains(phrase))
}

/// Every time jump entering the same way, where a dissolve or a fade would say time has
/// passed.
fn time_jumps(story: &Story) -> Vec<Finding> {
	let default = story.meta.video.transition.unwrap_or_default();
	let jumps: Vec<&Page> = story
		.pages
		.iter()
		.filter(|page| page.book_text().iter().any(|text| mentions_jump(text)))
		.collect();

	if jumps.len() < 2 {
		return Vec::new();
	}

	if !jumps
		.iter()
		.all(|page| page.transition().unwrap_or(default) == default)
	{
		return Vec::new();
	}

	vec![Finding::new(
		"time-jumps",
		Severity::Consider,
		jumps[1].line,
		format!("Every time jump enters with a {}.", default.name()),
		"Say the time has passed instead: `transition=dissolve` or `transition=fade`.",
	)]
}

#[cfg(test)]
mod tests {
	use std::fmt::Write as _;

	use super::*;
	use crate::cast::Cast;
	use crate::stage::rehearse;
	use crate::video::lipsync::Track;
	use crate::video::timeline;
	use crate::video::voice::SAMPLE_RATE;
	use crate::video::voice::Speech;
	use crate::video::voice::Utterance;

	const CAST: &str = "[mo]\nname = \"Mo\"\nkind = \"kid\"\n[hana]\nname = \"Hana\"\n";

	/// Reviews a script with the offline cast, the same word-count timings the storyboard
	/// estimates.
	fn review_script(front: &str, script: &str) -> Vec<Finding> {
		let story = crate::story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"mo\"\n{front}+++\n{script}"
		))
		.unwrap();
		let cast = Cast::parse(CAST).unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let built = timeline::build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			let seconds = (plain_text(line.text).split_whitespace().count() as f32 / 2.6).max(0.3);

			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap();

		review(&story, &rehearsals, &built)
	}

	fn rules(findings: &[Finding]) -> Vec<&'static str> {
		findings.iter().map(|finding| finding.rule).collect()
	}

	fn has(findings: &[Finding], rule: &str) -> bool {
		findings.iter().any(|finding| finding.rule == rule)
	}

	#[test]
	fn a_page_with_no_direction_and_a_repeated_stage_are_flagged() {
		let flagged = review_script("", "# park\nMo waited.\n# park\nMo waited some more.\n");
		assert!(has(&flagged, "static-page"), "{:?}", rules(&flagged));

		let staged = review_script(
			"",
			"# park\n@mo x=0.3 happy\nMo waited.\n# park\n@mo x=0.5 happy\nMo waited some more.\n",
		);
		assert!(!has(&staged, "static-page"), "{:?}", rules(&staged));
	}

	#[test]
	fn four_pages_in_one_place_under_one_light_are_flagged() {
		let flat = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park\n@mo x=0.4 happy\nTwo.\n# park\n@mo x=0.5 \
			 happy\nThree.\n# park\n@mo x=0.6 happy\nFour.\n",
		);
		assert!(has(&flat, "long-run"), "{:?}", rules(&flat));

		let varied = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park light=golden\n@mo x=0.4 happy\nTwo.\n# \
			 garden\n@mo x=0.5 happy\nThree.\n# park shot=wide\n@mo x=0.6 happy\nFour.\n",
		);
		assert!(!has(&varied, "long-run"), "{:?}", rules(&varied));
	}

	#[test]
	fn a_star_whose_mood_never_moves_or_comes_home_is_flagged() {
		let flat = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park light=golden\n@mo x=0.5 happy\nTwo.\n",
		);
		assert!(has(&flat, "mood-arc"), "{:?}", rules(&flat));

		let returns = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park light=golden\n@mo x=0.5 shocked\nTwo.\n# park \
			 light=moonlit\n@mo x=0.5 happy\nThree.\n",
		);
		assert!(has(&returns, "mood-arc"), "{:?}", rules(&returns));

		let arc = review_script(
			"",
			"# park\n@mo x=0.3 worried\nOne.\n# park light=golden\n@mo x=0.5 shocked\nTwo.\n# \
			 park light=moonlit\n@mo x=0.5 proud\nThree.\n",
		);
		assert!(!has(&arc, "mood-arc"), "{:?}", rules(&arc));
	}

	#[test]
	fn a_star_off_stage_for_three_pages_is_flagged() {
		let alone = review_script(
			"",
			"# park\n@hana x=0.3 happy\nOne.\n# park light=golden\n@hana x=0.4 happy\nTwo.\n# \
			 park light=moonlit\n@hana x=0.5 happy\nThree.\n",
		);
		assert!(has(&alone, "absent-star"), "{:?}", rules(&alone));

		let back = review_script(
			"",
			"# park\n@hana x=0.3 happy\nOne.\n# park light=golden\n@hana x=0.4 happy\nTwo.\n# \
			 park light=moonlit\n@mo x=0.5 happy\nThree.\n",
		);
		assert!(!has(&back, "absent-star"), "{:?}", rules(&back));
	}

	#[test]
	fn dialogue_without_a_talker_and_talk_on_a_hidden_actor_are_flagged() {
		let flagged = review_script(
			"",
			"# park\n@mo x=0.3\n\"Hello!\" said Mo.\n# park light=golden\n@mo hide\n@hana \
			 talk\n\"Anybody?\"\n@mo talk\n\"Over here.\"\n",
		);

		let speech: Vec<&Finding> = flagged
			.iter()
			.filter(|finding| finding.rule == "speech")
			.collect();
		assert_eq!(speech.len(), 2, "{flagged:#?}");
		assert_eq!(speech[0].severity, Severity::Fix);
		assert!(speech[0].observation.contains("nobody has `talk`"));
		assert!(speech[1].observation.contains("@mo"));

		let spoken = review_script("", "# park\n@mo x=0.3 talk\n\"Hello!\" said Mo.\n");
		assert!(!has(&spoken, "speech"), "{:?}", rules(&spoken));
	}

	#[test]
	fn a_page_over_the_read_aloud_limit_and_a_book_outside_its_words_are_flagged() {
		let words = "word ".repeat(50);
		let chatty = review_script("", &format!("# park\n@mo x=0.3\n{words}\n"));
		assert!(has(&chatty, "word-budget"), "{:?}", rules(&chatty));
		assert!(
			chatty
				.iter()
				.find(|finding| finding.rule == "word-budget")
				.is_some_and(|finding| {
					finding.severity == Severity::Fix && finding.observation.contains("page reads")
				})
		);

		let thin = review_script("", "# park\n@mo x=0.3\nHi.\n");
		assert!(thin.iter().any(|finding| {
			finding.rule == "word-budget" && finding.observation.contains("book reads")
		}));

		// Eight pages of forty-one words: inside both budgets.
		let wordy: String = std::iter::repeat_n("wind", 36)
			.collect::<Vec<_>>()
			.join(" ");
		let mut full = String::new();

		for i in 0..8 {
			let _ = write!(
				full,
				"# park light=golden\n@mo x=0.{} happy talk\n{wordy} and kites counted {i} \
				 today.\n",
				i % 2 + 3
			);
		}

		assert_eq!(story_words_of("", &full), 8 * 41);
		assert!(!has(&review_script("", &full), "word-budget"));
	}

	/// The words a script's pages carry, the way [`story_words`] counts them.
	fn story_words_of(front: &str, script: &str) -> usize {
		let story = crate::story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"mo\"\n{front}+++\n{script}"
		))
		.unwrap();

		story_words(&story)
	}

	#[test]
	fn a_long_book_with_no_close_shot_is_flagged() {
		let mut script = String::new();

		for i in 0..7 {
			let _ = write!(
				script,
				"# park light=golden\n@mo x=0.{} grin talk\nLine number {i} of the book.\n",
				i % 2 + 3
			);
		}

		let wide = review_script("", &script);
		assert!(has(&wide, "close-shots"), "{:?}", rules(&wide));

		let close = script.replace("# park light=golden", "# park light=golden shot=close");
		assert!(!has(&review_script("", &close), "close-shots"));
	}

	#[test]
	fn day_words_under_one_light_are_flagged() {
		let flat = review_script(
			"",
			"# park\n@mo x=0.3 happy\nIt was morning.\n# park\n@mo x=0.5 happy\nThen night fell.\n",
		);
		assert!(has(&flat, "flat-light"), "{:?}", rules(&flat));

		let lit = review_script(
			"",
			"# park light=golden\n@mo x=0.3 happy\nIt was morning.\n# park light=moonlit\n@mo \
			 x=0.5 happy\nThen night fell.\n",
		);
		assert!(!has(&lit, "flat-light"), "{:?}", rules(&lit));
	}

	#[test]
	fn an_actor_who_never_does_anything_is_flagged() {
		let idle = review_script(
			"",
			"# park\n@mo x=0.3 talk\n@hana x=0.7\n\"Hello,\" said Mo.\n",
		);
		assert!(has(&idle, "idle-arrival"), "{:?}", rules(&idle));

		let busy = review_script(
			"",
			"# park\n@mo x=0.3 talk\n@hana x=0.7 wave\n\"Hello,\" said Mo.\n",
		);
		assert!(!has(&busy, "idle-arrival"), "{:?}", rules(&busy));
	}

	#[test]
	fn two_figures_staged_close_are_flagged() {
		let crowded = review_script(
			"",
			"# park\n@mo x=0.5 talk\n@hana x=0.45 wave\n\"Hello,\" said Mo.\n",
		);
		assert!(has(&crowded, "crowding"), "{:?}", rules(&crowded));

		let spaced = review_script(
			"",
			"# park\n@mo x=0.3 talk\n@hana x=0.7 wave\n\"Hello,\" said Mo.\n",
		);
		assert!(!has(&spaced, "crowding"), "{:?}", rules(&spaced));
	}

	#[test]
	fn a_figure_in_the_text_card_band_is_flagged() {
		let high = review_script(
			"",
			"# park\n@mo x=0.3 talk\n@hana y=0.2 x=0.7 wave\n\"Hello,\" said Mo.\n",
		);
		assert!(has(&high, "under-card"), "{:?}", rules(&high));

		let clear = review_script(
			"",
			"# park text=bottom\n@mo x=0.3 talk\n@hana x=0.7 wave\n\"Hello,\" said Mo.\n",
		);
		assert!(!has(&clear, "under-card"), "{:?}", rules(&clear));
	}

	#[test]
	fn a_video_over_three_minutes_and_a_page_under_two_seconds_are_flagged() {
		let mut script = String::new();

		for i in 0..40 {
			let _ = write!(
				script,
				"# park light=golden\n@mo x=0.{} grin talk\nThe wind and the kite and the boy and \
				 the harbour and the hill counted {i} together today, loudly.\n",
				i % 2 + 3
			);
		}

		let long = review_script("", &script);
		assert!(has(&long, "video-length"), "{:?}", rules(&long));

		let flash = review_script("", "# park\n@mo x=0.3\nHi.\n");
		assert!(flash.iter().any(|finding| {
			finding.rule == "video-length" && finding.observation.contains("flashes past")
		}));
	}

	#[test]
	fn a_strong_mood_with_a_talk_needs_a_pause_before_the_next_line() {
		let rushed = review_script(
			"",
			"# park\n@mo x=0.3 shocked talk\n\"What!\" said Mo.\n@hana x=0.7 wave\n\"Hello,\" \
			 said Hana.\n",
		);
		assert!(has(&rushed, "reaction"), "{:?}", rules(&rushed));

		let landing = review_script(
			"",
			"# park\n@mo x=0.3 shocked talk\n\"What!\" said Mo.\n@pause 0.8\n@hana x=0.7 \
			 wave\n\"Hello,\" said Hana.\n",
		);
		assert!(!has(&landing, "reaction"), "{:?}", rules(&landing));

		// A strong mood without a talk in the same line is fine.
		let apart = review_script(
			"",
			"# park\n@mo x=0.3 shocked\n\"What!\" said Mo.\n@hana x=0.7 wave\n\"Hello,\" said \
			 Hana.\n",
		);
		assert!(!has(&apart, "reaction"), "{:?}", rules(&apart));
	}

	#[test]
	fn time_jumps_that_all_enter_the_same_way_are_flagged() {
		let same = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne day.\n# park\n@mo x=0.5 happy\nLater, two.\n# park\n@mo \
			 x=0.6 happy\nThe next day, three.\n",
		);
		assert!(has(&same, "time-jumps"), "{:?}", rules(&same));

		let dissolved = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne day.\n# park transition=dissolve\n@mo x=0.5 \
			 happy\nLater, two.\n# park transition=fade\n@mo x=0.6 happy\nThe next day, three.\n",
		);
		assert!(!has(&dissolved, "time-jumps"), "{:?}", rules(&dissolved));
	}

	#[test]
	fn findings_carry_their_line_and_a_summary_counts_them() {
		let findings = review_script("", "# park\n@mo x=0.3 wave\n\"Hello,\" said Mo.\n");

		assert!(
			findings
				.iter()
				.all(|finding| finding.line > 0 && !finding.suggestion.is_empty())
		);
		assert_eq!(summary(&findings), "1 fix, 1 consider.");
	}
}
