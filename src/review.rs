//! A critique of a rehearsed story: what a director would change about the script, as
//! findings with a severity, the script line, an observation and a suggestion.
//!
//! [`review`] is pure: it reads a story, its rehearsal and a timeline with estimated
//! durations (the same plan the storyboard draws), and gives advice. It never refuses a
//! script — `fables check` says whether a script works; this says whether it works *well*.

use std::collections::HashSet;

use serde::Serialize;

use crate::art::Bounds;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::light::Preset;
use crate::compose;
use crate::stage::Actor;
use crate::stage::Rehearsal;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::story::Beat;
use crate::story::Directive;
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
/// Past this share of the smaller figure's drawn shape covered by another, two people read
/// as one shape.
pub const MAX_OVERLAP: f32 = 0.25;
/// Two positions this close, as a fraction of the stage, look the same at proof size.
const SAME_X: f32 = 0.05;
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
	// A book `fables new` has just written still has its bracketed placeholders; say so
	// plainly instead of critiquing lines the writer has not written yet.
	if let Some(unwritten) = unwritten(story) {
		return vec![unwritten];
	}

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
	findings.extend(text_picture(story, rehearsals));
	findings.extend(reaction_order(story, rehearsals));
	findings.extend(screen_direction(story, rehearsals));
	findings.extend(refrain(story));
	findings.extend(goal_early(story, rehearsals));
	findings.extend(return_image(story));
	findings.extend(wind_down(story, rehearsals));
	findings.extend(page_budget(story));

	findings.sort_by_key(|finding| (finding.severity != Severity::Fix, finding.line));
	findings
}

/// The state of the stage at the end of each page.
fn finales(rehearsals: &[Rehearsal]) -> Vec<&Scene> {
	rehearsals.iter().map(Rehearsal::finale).collect()
}

/// A scaffold's unwritten lines: the narration paragraphs that are still `[bracketed]`
/// placeholders. A book with them is not ready to be critiqued, so the review says how many
/// are left and stops.
fn unwritten(story: &Story) -> Option<Finding> {
	let mut count = 0;
	let mut line = None;

	for page in &story.pages {
		for text in page.book_text() {
			if text.contains('[') && text.contains(']') {
				count += 1;
				line.get_or_insert(page.line);
			}
		}
	}

	let line = line?;

	Some(Finding::new(
		"unwritten",
		Severity::Consider,
		line,
		format!(
			"The book still has {count} unwritten line{} (the [bracketed] ones).",
			if count == 1 { "" } else { "s" }
		),
		"Replace each bracketed line with the story, then run the review again.",
	))
}

/// Whether two pages show the same picture: one setting, light and shot, and everybody in
/// the same place (within [`SAME_X`]), pose and stance. Moods are left out: a face barely
/// changes the picture at proof size.
fn same_stage(a: &Scene, b: &Scene) -> bool {
	let (left, right) = (a.placed(), b.placed());

	a.setting == b.setting
		&& a.weather.light == b.weather.light
		&& a.weather.night == b.weather.night
		&& (a.weather.wilt - b.weather.wilt).abs() < 1e-4
		&& a.camera.shot == b.camera.shot
		&& a.camera.on == b.camera.on
		&& left.len() == right.len()
		&& left.iter().all(|(actor, x)| {
			right.iter().any(|(other, y)| {
				other.id == actor.id
					&& (x - y).abs() <= SAME_X
					&& other.pose == actor.pose
					&& other.stance == actor.stance
					&& other.prop == actor.prop
					&& other.y == actor.y
					&& (other.size - actor.size).abs() < 1e-3
					&& (other.knocked > 0.5) == (actor.knocked > 0.5)
			})
		})
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
				"This page's picture is the same as the last one's; at most a face changes.",
				"Change something to look at: a pose, an `x=`, an entrance, the `shot=` or the \
				 `light=`.",
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
		if run.len() > 3 && run.iter().all(|&index| quiet(story, rehearsals, index)) {
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

/// Whether a page leaves its run visually quiet: no shot of its own, and nobody's mood
/// changed since the page before it (the first page of a run is quiet by definition).
fn quiet(story: &Story, rehearsals: &[Rehearsal], index: usize) -> bool {
	let page = &story.pages[index];

	if page.option("shot").is_some() || page.directed_by_camera() {
		return false;
	}

	let Some(before) = index.checked_sub(1).and_then(|last| rehearsals.get(last)) else {
		return true;
	};
	let here = rehearsals[index].finale();
	let before = before.finale();

	before
		.actors
		.iter()
		.filter(|actor| !actor.hidden)
		.all(|actor| {
			here.actors
				.iter()
				.any(|other| other.id == actor.id && other.mood == actor.mood)
		})
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
	let flow = flow_words(story);
	let (min, max) = flow.unwrap_or(BOOK_WORDS);

	if words < min || words > max {
		let observation = if flow.is_some() {
			format!("The book reads {words} words; the flow it names wants {min}–{max}.")
		} else {
			format!("The book reads {words} words; picture books land between {min} and {max}.")
		};
		let suggestion = if flow.is_some() {
			"Trim to the flow's budget, or choose a flow that fits the story."
		} else {
			"Grow the middle beats, or trim towards one clear idea per page."
		};

		findings.push(Finding::new(
			"word-budget",
			Severity::Consider,
			story.pages[0].line,
			observation,
			suggestion,
		));
	}

	findings
}

/// The word range a flow sets for the books that name it, when its front matter does.
fn flow_words(story: &Story) -> Option<(usize, usize)> {
	let name = story.meta.flow.as_deref()?;
	let world = crate::world::World::for_story(story, None).ok();

	crate::flow::find(world.as_ref(), name)
		.ok()
		.map(|flow| flow.words)
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
	let phases: HashSet<&str> = words.iter().filter_map(|w| phase_of(w)).collect();

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

/// How much of the smaller of two drawn shapes the other covers, from 0 to 1.
fn overlap(a: Bounds, b: Bounds) -> f32 {
	let area = |r: Bounds| (r.2 - r.0).max(0.0) * (r.3 - r.1).max(0.0);
	let width = (a.2.min(b.2) - a.0.max(b.0)).max(0.0);
	let height = (a.3.min(b.3) - a.1.max(b.1)).max(0.0);
	let smaller = area(a).min(area(b));

	if smaller > 0.0 {
		width * height / smaller
	} else {
		0.0
	}
}

/// Two people whose drawn shapes overlap so much they read as one, measured from the puppets
/// the page draws: raised arms, held props, sizes and all.
fn crowding(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for (index, scene) in finales(rehearsals).iter().enumerate() {
		let shapes: Vec<(String, Bounds)> = compose::puppets(scene, 0.0, true)
			.iter()
			.filter(|puppet| puppet.kind.is_figure())
			.map(|puppet| (puppet.id.clone(), puppet.extents().subject))
			.collect();

		for (i, (a, first)) in shapes.iter().enumerate() {
			for (b, second) in shapes.iter().skip(i + 1) {
				let covered = overlap(*first, *second);

				if covered > MAX_OVERLAP {
					findings.push(Finding::new(
						"crowding",
						Severity::Consider,
						story.pages[index].line,
						format!(
							"@{a} and @{b} overlap by {:.0}% of the smaller figure, so they read \
							 as one shape.",
							covered * 100.0
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

/// Verbs a page's text may use, and the direction words that would show them. `move` stands
/// for anybody's `x` changing on the page, or an entrance or exit.
const ACTIONS: [(&[&str], &[&str]); 13] = [
	(
		&[
			"jump", "jumps", "jumped", "jumping", "leap", "leaps", "leapt", "leaped", "hop",
			"hops", "hopped", "hopping",
		],
		&["jump", "hop"],
	),
	(&["waved", "waving"], &["wave"]),
	(
		&["run", "runs", "ran", "running", "raced", "dashed"],
		&["run", "move"],
	),
	(
		&["hid", "hide", "hides", "hiding"],
		&["cover-mouth", "crouch", "hide"],
	),
	(&["points", "pointed", "pointing"], &["point"]),
	(&["sat", "sit", "sits", "sitting"], &["sit"]),
	(&["laughed", "laughing", "giggled"], &["laugh"]),
	(&["cried", "cries", "crying", "wept", "sobbed"], &["sad"]),
	(
		&["slept", "sleeps", "sleeping", "asleep", "yawned", "dozed"],
		&["sleepy"],
	),
	(
		&[
			"hug", "hugs", "hugged", "hugging", "held", "holding", "caught", "catches",
		],
		&["hold", "reach-out", "holds", "catch", "pick"],
	),
	(&["cheered", "cheering"], &["cheer"]),
	(&["fainted", "faints"], &["faint", "fainted"]),
	(&["shrugged", "shrugs"], &["shrug"]),
];

/// Phrases that use an action verb for something else: "ran the post office", "laughed till
/// he cried". Their words are passed over.
const IDIOMS: [&[&str]; 5] = [
	&["ran", "the"],
	&["runs", "the"],
	&["run", "the"],
	&["till", "he", "cried"],
	&["till", "she", "cried"],
];

/// Words that make somebody the subject of a sentence, besides the cast's own names: an
/// action verb only counts in a sentence about a character ("The cakes sat down" does not).
const SUBJECTS: [&str; 6] = ["he", "she", "they", "we", "everyone", "everybody"];

/// Words that start a thing's name: "the wind held" is about the wind.
const ARTICLES: [&str; 3] = ["the", "a", "an"];

/// Creatures the text may name, as the cast's `kind`s call them.
const CRITTERS: [&str; 4] = ["dog", "bird", "worm", "flower"];

/// What a setting draws for itself, so its page need not cast it.
const SETTING_SHOWS: [(&str, &[&str]); 1] = [("garden", &["flower"])];

/// A paragraph's narrated sentences as words, with quoted speech left out: speech may talk
/// about the past or the future, so it is not held to the picture. Hyphenated words stay
/// whole ("hide-and-seek").
fn narrated_sentences(text: &str) -> Vec<Vec<String>> {
	let mut narration = String::new();
	let mut quoted = false;

	for ch in plain_text(text).chars() {
		match ch {
			'"' => quoted = !quoted,
			'\u{201c}' => quoted = true,
			'\u{201d}' => quoted = false,
			_ if !quoted => narration.push(ch),
			_ => narration.push(' '),
		}
	}

	narration
		.split(['.', '!', '?', ';'])
		.map(|sentence| {
			sentence
				.split(|ch: char| !ch.is_alphanumeric() && ch != '-')
				.map(|word| word.trim_matches('-'))
				.filter(|word| !word.is_empty())
				.map(str::to_owned)
				.collect()
		})
		.collect()
}

/// Every scene a page passes through, from its opening to its last beat.
fn scenes(rehearsal: &Rehearsal) -> impl Iterator<Item = &Scene> {
	std::iter::once(&rehearsal.opening).chain(&rehearsal.after)
}

/// The direction words a page uses, with `move` for any walk, entrance or exit.
fn shown(page: &Page, rehearsal: &Rehearsal) -> HashSet<String> {
	let mut shown = HashSet::new();

	for beat in &page.beats {
		let Beat::Direct(directive) = beat else {
			continue;
		};

		for token in &directive.tokens {
			if token.key == "holds" && matches!(token.value.as_deref(), Some("nothing" | "none")) {
				continue;
			}

			if matches!(token.key.as_str(), "enter" | "exit") {
				shown.insert("move".to_owned());
			}

			shown.insert(token.key.clone());
			shown.extend(token.value.clone());
		}
	}

	let walks = rehearsal.after.windows(2).any(|pair| {
		pair[1].actors.iter().any(|actor| {
			pair[0].actors.iter().any(|before| {
				before.id == actor.id
					&& !actor.hidden
					&& !before.hidden
					&& before
						.x
						.zip(actor.x)
						.is_some_and(|(a, b)| (a - b).abs() > 0.01)
			})
		})
	});

	if walks {
		shown.insert("move".to_owned());
	}

	shown
}

/// Who and what a page's picture holds: everyone on stage at any beat (by id and by kind),
/// what they hold, and the words of the setting's name.
fn present(page: &Page, rehearsal: &Rehearsal) -> HashSet<String> {
	let mut present: HashSet<String> = page.setting.split('-').map(str::to_owned).collect();

	for (setting, shows) in SETTING_SHOWS {
		if page.setting == setting {
			present.extend(shows.iter().map(|thing| (*thing).to_owned()));
		}
	}

	for actor in scenes(rehearsal).flat_map(|scene| &scene.actors) {
		if !actor.hidden {
			present.insert(base(&actor.id).to_owned());
			present.insert(actor.kind.name().to_owned());
		}
	}

	present.extend(held(page));
	present
}

/// The props a page's directions hand somebody (`holds=`), whatever props there are.
fn held(page: &Page) -> Vec<String> {
	page.beats
		.iter()
		.filter_map(|beat| {
			match beat {
				Beat::Direct(directive) => Some(&directive.tokens),
				_ => None,
			}
		})
		.flatten()
		.filter(|token| token.key == "holds")
		.filter_map(|token| token.value.clone())
		.filter(|value| !matches!(value.as_str(), "nothing" | "none"))
		.collect()
}

/// An actor id without its instance suffix: `bird` for `bird.a`.
fn base(id: &str) -> &str {
	id.split('.').next().unwrap_or(id)
}

/// The thing a word names, when it is `name` or its plural.
fn names<'a>(word: &str, name: &'a str) -> Option<&'a str> {
	let lower = word.to_lowercase();
	(lower == name || lower.strip_suffix('s') == Some(name)).then_some(name)
}

/// A page whose words name an action its directions never show, or somebody (a cast member,
/// a creature, a held prop) its picture does not hold.
fn text_picture(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let cast: Vec<String> = cast_ids(story)
		.into_iter()
		.filter(|id| !matches!(id.as_str(), "camera" | "scene"))
		.collect();
	let props: Vec<String> = story.pages.iter().flat_map(held).collect();
	let mut findings = Vec::new();

	for (index, page) in story.pages.iter().enumerate() {
		let rehearsal = &rehearsals[index];
		let shown = shown(page, rehearsal);
		let present = present(page, rehearsal);
		// Each verb, the direction that shows it, and the character the sentence was about.
		let mut verbs: Vec<(String, &str, Option<String>)> = Vec::new();
		let mut missing: Vec<String> = Vec::new();
		let mut line = None;
		let mut who = None;

		for beat in &page.beats {
			let Beat::Say { text, line: at, .. } = beat else {
				continue;
			};

			for words in narrated_sentences(text) {
				let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
				let mut subject = false;
				let mut skip = 0;

				for (i, word) in words.iter().enumerate() {
					let here = &lower[i];

					if cast.contains(here) && word.starts_with(char::is_uppercase) {
						subject = true;

						if present.contains(here) {
							who = Some(here.clone());
						}
					}

					subject |= SUBJECTS.contains(&here.as_str());

					if let Some(idiom) = IDIOMS.iter().find(|idiom| {
						lower.len() - i >= idiom.len()
							&& lower[i..].iter().zip(idiom.iter()).all(|(a, b)| a == b)
					}) {
						skip = idiom.len();
					}

					if skip > 0 {
						skip -= 1;
						continue;
					}

					// "the wind held its breath": a thing named just before the verb is its
					// subject, not the character.
					let thing = i >= 2 && ARTICLES.contains(&lower[i - 2].as_str());

					if subject
						&& !thing && let Some((_, shows)) = ACTIONS
						.iter()
						.find(|(forms, _)| forms.contains(&here.as_str()))
						&& !shows.iter().any(|direction| shown.contains(*direction))
						&& !verbs.iter().any(|(_, first, _)| *first == shows[0])
					{
						verbs.push((here.clone(), shows[0], who.clone()));
						line.get_or_insert(*at);
					}

					// A cast member is named with a capital (Mo, Mum); a creature or a prop in
					// any spelling.
					let proper = word.starts_with(char::is_uppercase);
					let named = cast
						.iter()
						.filter(|_| proper)
						.chain(props.iter())
						.find_map(|name| names(word, name))
						.or_else(|| CRITTERS.iter().find_map(|name| names(word, name)));

					if let Some(name) = named
						&& !present.contains(name)
						&& !missing.iter().any(|seen| seen == name)
					{
						missing.push(name.to_owned());
						line.get_or_insert(*at);
					}
				}
			}
		}

		let Some(line) = line else {
			continue;
		};
		let star = story
			.meta
			.star
			.clone()
			.filter(|star| present.contains(star));
		let who = who
			.or_else(|| star.clone())
			.unwrap_or_else(|| "who".to_owned());

		if !verbs.is_empty() {
			let said: Vec<String> = verbs
				.iter()
				.map(|(word, ..)| format!("\"{word}\""))
				.collect();
			let directions: Vec<String> = verbs
				.iter()
				.map(|(_, direction, by)| {
					format!("`@{} {direction}`", by.as_deref().unwrap_or(&who))
				})
				.collect();

			findings.push(Finding::new(
				"text-picture",
				Severity::Consider,
				line,
				format!(
					"The text says {}, but no direction on this page shows it.",
					join_names(&said)
				),
				format!(
					"Show it on the line above ({}), or leave it out of the words.",
					directions.join(", ")
				),
			));
		}

		if !missing.is_empty() {
			let things: Vec<String> = missing.iter().map(|name| format!("the {name}")).collect();
			let entrances: Vec<String> = missing
				.iter()
				.map(|name| {
					if props.contains(name) {
						format!("`@{who} holds={name}`")
					} else {
						format!("`@{name} enter=left`")
					}
				})
				.collect();
			let them = if missing.len() == 1 { "it" } else { "them" };

			findings.push(Finding::new(
				"text-picture",
				Severity::Consider,
				line,
				format!(
					"The text names {}, but the picture does not show {them}.",
					join_names(&things)
				),
				format!(
					"Put {them} in the picture ({}), or leave {them} out of the words.",
					entrances.join(", ")
				),
			));
		}
	}

	findings
}

/// A list of names with commas and a final `and`.
fn join_names(names: &[String]) -> String {
	match names {
		[] => String::new(),
		[one] => one.clone(),
		[rest @ .., last] => format!("{} and {last}", rest.join(", ")),
	}
}

/// Moods that are a reaction to something: a take nobody should do before its cause.
const TAKES: [Mood; 3] = [Mood::Shocked, Mood::Disgusted, Mood::Fainted];

/// The take a direction gives, if any.
fn take(directive: &Directive) -> Option<Mood> {
	directive.tokens.iter().find_map(|token| {
		let mood = match (token.key.as_str(), token.value.as_deref()) {
			("mood", Some(value)) => Mood::parse(value),
			(word, None) => Mood::parse(word),
			_ => None,
		}?;
		TAKES.contains(&mood).then_some(mood)
	})
}

/// Whether a beat is something to react to — an entrance, a shout, a faint, a hop or a sound
/// effect — and who does it (nobody, an empty id, for a sound).
fn cause(beat: &Beat, scene: &Scene) -> Option<String> {
	match beat {
		Beat::Direct(directive) => {
			directive
				.tokens
				.iter()
				.any(|token| {
					token.key == "enter"
						|| token.value.is_none()
							&& matches!(token.key.as_str(), "shout" | "faint" | "hop")
				})
				.then(|| base(&directive.actor).to_owned())
		}
		Beat::Sfx { .. } => Some(String::new()),
		Beat::Say { text, .. } if text.contains("**") => {
			Some(
				scene
					.actors
					.iter()
					.find(|actor| actor.talking)
					.map(|actor| base(&actor.id).to_owned())
					.unwrap_or_default(),
			)
		}
		_ => None,
	}
}

/// The script line a beat was written on.
fn beat_line(beat: &Beat) -> usize {
	match beat {
		Beat::Direct(directive) => directive.line,
		Beat::Say { line, .. }
		| Beat::Pause { line, .. }
		| Beat::Sfx { line, .. }
		| Beat::Hold { line, .. }
		| Beat::Music { line, .. } => *line,
	}
}

/// A strong reaction directed before anything has happened to react to, when its only cause
/// on the page (an entrance, a shout, a faint, a hop, a sound) comes later.
fn reaction_order(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for (index, page) in story.pages.iter().enumerate() {
		let causes: Vec<(usize, String)> = page
			.beats
			.iter()
			.zip(&rehearsals[index].after)
			.enumerate()
			.filter_map(|(at, (beat, scene))| Some((at, cause(beat, scene)?)))
			.collect();

		for (at, beat) in page.beats.iter().enumerate() {
			let Beat::Direct(directive) = beat else {
				continue;
			};
			let Some(mood) = take(directive) else {
				continue;
			};
			let who = base(&directive.actor);
			let by_another = |by: &String| by != who;
			// Narration before the take may be what it reacts to, and a stink on stage is
			// reason enough to be disgusted.
			let earlier = page.beats[..at]
				.iter()
				.any(|beat| matches!(beat, Beat::Say { .. }))
				|| causes
					.iter()
					.any(|(when, by)| *when <= at && by_another(by))
				|| mood == Mood::Disgusted
					&& rehearsals[index].after[at]
						.actors
						.iter()
						.any(|actor| actor.stink > 0.0 && !actor.hidden);
			// Directions and the paragraph they stand over land together, so the cause must
			// come in a later paragraph.
			let lands = page.beats[at..]
				.iter()
				.position(|beat| matches!(beat, Beat::Say { .. }))
				.map_or(page.beats.len(), |offset| at + offset);
			let later = causes
				.iter()
				.find(|(when, by)| *when > lands && by_another(by));

			if let Some((when, _)) = later
				&& !earlier
			{
				let mood = mood.name();
				let cause = beat_line(&page.beats[*when]);

				findings.push(Finding::new(
					"reaction-order",
					Severity::Consider,
					directive.line,
					format!(
						"@{who} is {mood} before anything happens: the cause is on line {cause}."
					),
					format!(
						"Move `@{who} {mood}` below line {cause}, so the reaction follows what \
						 causes it."
					),
				));
			}
		}
	}

	findings
}

/// Two figures who swap sides between one page and the next in the same setting, with no
/// entrance to explain it: the reader loses track of who is where.
fn screen_direction(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let mut findings = Vec::new();

	for pair in rehearsals.windows(2) {
		let (before, after) = (pair[0].finale(), &pair[1]);
		let page = &story.pages[after.page];

		if before.setting != after.finale().setting {
			continue;
		}

		let figures: Vec<(&Actor, f32)> = before
			.placed()
			.into_iter()
			.filter(|(actor, _)| actor.kind.is_figure())
			.collect();
		let enters = |id: &str| {
			page.beats.iter().any(|beat| {
				matches!(beat, Beat::Direct(directive) if directive.actor == id
					&& directive.tokens.iter().any(|token| token.key == "enter"))
			})
		};

		for (i, (a, was_a)) in figures.iter().enumerate() {
			for (b, was_b) in figures.iter().skip(i + 1) {
				// Where the two first stand together on the new page.
				let now = after.after.iter().find_map(|scene| {
					let placed = scene.placed();
					let at = |id: &str| {
						placed
							.iter()
							.find(|(actor, _)| actor.id == id)
							.map(|(_, x)| *x)
					};
					at(&a.id).zip(at(&b.id))
				});
				let Some((now_a, now_b)) = now else {
					continue;
				};

				if (was_a - was_b).abs() > SAME_X
					&& (now_a - now_b).abs() > SAME_X
					&& (was_a < was_b) != (now_a < now_b)
					&& !enters(&a.id)
					&& !enters(&b.id)
				{
					let (left, right) = if was_a < was_b { (a, b) } else { (b, a) };

					findings.push(Finding::new(
						"screen-direction",
						Severity::Consider,
						page.line,
						format!(
							"@{} and @{} swap sides from the last page in the same {}, with no \
							 walk between.",
							left.id, right.id, page.setting
						),
						format!(
							"Keep @{} on the left and @{} on the right, or walk them across on \
							 the page (`@{} x=0.7` after a line) so the reader sees them swap.",
							left.id, right.id, left.id
						),
					));
				}
			}
		}
	}

	findings
}

/// The flows whose shape each flow-aware rule checks.
const REFRAIN_FLOWS: [&str; 2] = ["three-tries", "pile-up"];
const GOAL_FLOWS: [&str; 2] = ["three-tries", "there-and-back"];
const RETURN_FLOWS: [&str; 2] = ["round-we-go", "there-and-back"];
const WIND_DOWN_FLOWS: [&str; 1] = ["goodnight-tour"];

/// Whether the book's front matter names one of these flows.
fn flow_is(story: &Story, flows: &[&str]) -> bool {
	story
		.meta
		.flow
		.as_deref()
		.is_some_and(|flow| flows.contains(&flow))
}

/// Words that tag speech rather than carry a refrain: "said Mo".
const SPEECH_TAGS: [&str; 5] = ["said", "says", "asked", "shouted", "mumbled"];

/// The book's lines a phrase at a time (split at commas and full stops), lower-cased, with
/// the page each is on.
fn phrases(story: &Story) -> Vec<(usize, String)> {
	let mut phrases = Vec::new();

	for (index, page) in story.pages.iter().enumerate() {
		for text in page.book_text() {
			for phrase in plain_text(text).split([',', '.', '!', '?', ';', ':']) {
				let words: Vec<String> = phrase
					.split(|ch: char| !ch.is_alphanumeric())
					.filter(|word| !word.is_empty())
					.map(str::to_lowercase)
					.collect();

				if words.len() >= 2 && !words.iter().any(|word| SPEECH_TAGS.contains(&&word[..])) {
					phrases.push((index, words.join(" ")));
				}
			}
		}
	}

	phrases
}

/// A flow built on a refrain with no line that comes back three times, or a refrain that
/// never changes.
fn refrain(story: &Story) -> Vec<Finding> {
	if !flow_is(story, &REFRAIN_FLOWS) {
		return Vec::new();
	}

	let phrases = phrases(story);
	let count = |line: &str| phrases.iter().filter(|(_, other)| other == line).count();
	let Some((first, line)) = phrases
		.iter()
		.filter(|(_, line)| count(line) >= 3)
		.max_by_key(|(index, line)| (count(line), std::cmp::Reverse(*index)))
	else {
		return vec![Finding::new(
			"refrain",
			Severity::Consider,
			story.pages[0].line,
			"The flow wants a refrain, but no line comes back three times.",
			"Choose one short line a child can join in with, repeat it word for word on three \
			 pages, and change it once near the end.",
		)];
	};

	// A changed refrain keeps at least half its words and adds or swaps the rest.
	let words: Vec<&str> = line.split(' ').collect();
	let changed = phrases.iter().any(|(index, other)| {
		let kept = other.split(' ').filter(|word| words.contains(word)).count();
		index > first && other != line && kept * 2 >= words.len()
	});

	if changed {
		return Vec::new();
	}

	vec![Finding::new(
		"refrain",
		Severity::Consider,
		story.pages[story.pages.len() - 1].line,
		format!("The refrain \"{line}\" comes back, but never changes."),
		"Change it once, near the end, so the last time lands differently.",
	)]
}

/// A flow that starts from a want, where the star neither speaks nor reaches in the first two
/// pages.
fn goal_early(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let Some(star) = story.meta.star.as_deref() else {
		return Vec::new();
	};

	if !flow_is(story, &GOAL_FLOWS) {
		return Vec::new();
	}

	let wants = rehearsals.iter().take(2).flat_map(scenes).any(|scene| {
		scene.actors.iter().any(|actor| {
			base(&actor.id) == star
				&& !actor.hidden
				&& (actor.talked || matches!(actor.pose, Pose::ReachOut | Pose::Point))
		})
	});

	if wants {
		return Vec::new();
	}

	vec![Finding::new(
		"goal-early",
		Severity::Consider,
		story.pages[0].line,
		format!("@{star} does not say or reach for what they want in the first two pages."),
		format!(
			"Give @{star} a line that says the want on page 1 or 2 (`@{star} talk`), or a \
			 `reach-out` towards it."
		),
	)]
}

/// A circular or journey flow that does not end where it began.
fn return_image(story: &Story) -> Vec<Finding> {
	let (Some(first), Some(last)) = (story.pages.first(), story.pages.last()) else {
		return Vec::new();
	};

	if !flow_is(story, &RETURN_FLOWS) || story.pages.len() < 2 || first.setting == last.setting {
		return Vec::new();
	}

	vec![Finding::new(
		"return-image",
		Severity::Consider,
		last.line,
		format!(
			"The book ends in the {}, not where it began, in the {}.",
			last.setting, first.setting
		),
		format!(
			"End in the {}, framed as the first page was, with one thing changed.",
			first.setting
		),
	)]
}

/// How lively a mood is: 2 for a take, 1 for an ordinary face, 0 for a sleepy one.
fn energy(mood: Mood) -> u8 {
	match mood {
		Mood::Sleepy | Mood::Dreamy => 0,
		Mood::Laugh
		| Mood::Shocked
		| Mood::Disgusted
		| Mood::Fainted
		| Mood::Cross
		| Mood::Defiant
		| Mood::Dizzy => 2,
		_ => 1,
	}
}

/// How bright a light reads at bedtime: candlelight is as small as moonlight.
fn brightness(light: Preset) -> u8 {
	match light {
		Preset::Day | Preset::Overcast | Preset::Indoor => 3,
		Preset::Dawn | Preset::Golden | Preset::Storm => 2,
		Preset::Dusk => 1,
		Preset::Night | Preset::Moonlit | Preset::Candle => 0,
	}
}

/// The words a page reads aloud.
fn page_words(page: &Page) -> usize {
	page.book_text()
		.iter()
		.map(|text| text.split_whitespace().count())
		.sum()
}

/// A bedtime flow whose last third does not wind down: the words, the star's mood or the
/// light rise again, or somebody new arrives in the last three pages.
fn wind_down(story: &Story, rehearsals: &[Rehearsal]) -> Vec<Finding> {
	let pages = story.pages.len();

	if !flow_is(story, &WIND_DOWN_FLOWS) || pages < 3 {
		return Vec::new();
	}

	let mut findings = Vec::new();
	let third = pages - pages.div_ceil(3);
	let words: Vec<usize> = story.pages.iter().map(page_words).collect();
	let mean = |words: &[usize]| words.iter().sum::<usize>() as f32 / words.len().max(1) as f32;

	if mean(&words[third..]) >= mean(&words[..third]) {
		findings.push(Finding::new(
			"wind-down",
			Severity::Consider,
			story.pages[third].line,
			format!(
				"The last third reads about {:.0} words a page, no fewer than the pages before it.",
				mean(&words[third..])
			),
			"Let each goodnight page say less than the last, down to a whisper on the final page.",
		));
	}

	let star = story.meta.star.as_deref();
	let light = |index: usize| brightness(rehearsals[index].finale().weather.light.preset);
	let liveliness = |index: usize| {
		rehearsals[index]
			.finale()
			.actors
			.iter()
			.find(|actor| Some(base(&actor.id)) == star && !actor.hidden)
			.map(|actor| energy(actor.mood))
	};

	for (index, page) in story.pages.iter().enumerate().skip(third.max(1)) {
		if light(index) > light(index - 1) {
			findings.push(Finding::new(
				"wind-down",
				Severity::Consider,
				page.line,
				"The light brightens again near the end of a bedtime book.",
				"Keep the last third in `dusk`, `moonlit` or `candle`, each page no brighter than \
				 the one before.",
			));
		}

		if let (Some(now), Some(before)) = (liveliness(index), liveliness(index - 1))
			&& now > before
		{
			findings.push(Finding::new(
				"wind-down",
				Severity::Consider,
				page.line,
				"The star livens up again near the end of a bedtime book.",
				"Keep the last pages calm (`sleepy`, `dreamy`, `happy`), never livelier than the \
				 page before.",
			));
		}
	}

	let mut seen: HashSet<&str> = HashSet::new();

	for (index, rehearsal) in rehearsals.iter().enumerate() {
		for actor in scenes(rehearsal)
			.flat_map(|scene| &scene.actors)
			.filter(|actor| !actor.hidden)
		{
			if seen.insert(base(&actor.id)) && index > 0 && index + 3 >= pages {
				findings.push(Finding::new(
					"wind-down",
					Severity::Consider,
					story.pages[index].line,
					format!(
						"@{} arrives in the last three pages of a bedtime book.",
						actor.id
					),
					"Bring nobody new on near the end: say goodnight to somebody the reader has \
					 already met.",
				));
			}
		}
	}

	findings
}

/// The plain pages a short book is padded with, as `fables check` says it: the story pages,
/// the blanks they leave, and the room that gives.
pub fn padding(story: &Story) -> Option<String> {
	let story_pages = story.pages.iter().filter(|page| page.in_book()).count();
	let blanks = crate::book::blank_pages(story_pages);

	if blanks == 0 {
		return None;
	}

	// The title, copyright, "Meet the folk" and activity pages, then the blanks.
	let printed = story_pages + 4 + blanks;
	let count =
		|count: usize, word: &str| format!("{count} {word}{}", if count == 1 { "" } else { "s" });

	Some(format!(
		"{} {} {}; a {printed}-page book has room for {blanks} more {}.",
		count(story_pages, "story page"),
		if story_pages == 1 { "leaves" } else { "leave" },
		count(blanks, "blank page"),
		if blanks == 1 { "page" } else { "pages" }
	))
}

/// A book that prints blank pages it could fill.
fn page_budget(story: &Story) -> Vec<Finding> {
	let Some(padding) = padding(story) else {
		return Vec::new();
	};

	vec![Finding::new(
		"page-budget",
		Severity::Consider,
		story.pages[story.pages.len() - 1].line,
		format!("The book is padded: {padding}"),
		"Give a rushed beat its own page, or let the blanks stand as endpapers.",
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
	fn a_flow_sets_the_book_word_range() {
		let short = pages_of(6);
		let plain = review_script("", &short);
		assert!(plain.iter().any(|finding| {
			finding.rule == "word-budget" && finding.observation.contains("picture books land")
		}));

		let flowed = review_script("flow = \"gag-short\"\n", &short);
		assert!(
			!flowed.iter().any(|finding| {
				finding.rule == "word-budget" && finding.observation.contains("book reads")
			}),
			"{flowed:#?}"
		);

		let long = pages_of(20);
		let over = review_script("flow = \"gag-short\"\n", &long);
		assert!(
			over.iter().any(|finding| {
				finding.rule == "word-budget"
					&& finding.observation.contains("flow it names wants 40–120")
			}),
			"{over:#?}"
		);
	}

	/// Seven pages of one repeated word, `words` words a page, with lights that change.
	fn pages_of(words: usize) -> String {
		let text = std::iter::repeat_n("cake", words)
			.collect::<Vec<_>>()
			.join(" ");
		let mut script = String::new();

		for index in 0..7 {
			let _ = write!(
				script,
				"# street light={}\n@mo x=0.{} happy\n{text}\n",
				if index % 2 == 0 { "golden" } else { "dusk" },
				3 + index % 3
			);
		}

		script
	}

	#[test]
	fn findings_carry_their_line_and_a_summary_counts_them() {
		let findings = review_script("", "# park\n@mo x=0.3 wave\n\"Hello,\" said Mo.\n");

		assert!(
			findings
				.iter()
				.all(|finding| finding.line > 0 && !finding.suggestion.is_empty())
		);
		assert_eq!(summary(&findings), "1 fix, 2 consider.");
	}

	#[test]
	fn a_scaffold_says_how_many_lines_are_unwritten() {
		let findings = review_script(
			"",
			"# park\n@mo x=0.3 happy\n[The want.]\n\n[The first try.]\n\n# garden\n@mo x=0.4 \
			 happy\n[The failure.]\n",
		);

		assert_eq!(findings.len(), 1, "{findings:#?}");
		assert_eq!(findings[0].rule, "unwritten");
		assert_eq!(findings[0].severity, Severity::Consider);
		assert!(
			findings[0].observation.contains("3 unwritten lines"),
			"{}",
			findings[0].observation
		);
	}

	#[test]
	fn overlap_is_the_share_of_the_smaller_shape_covered() {
		assert!((overlap((0.0, 0.0, 2.0, 2.0), (1.0, 0.0, 2.0, 1.0)) - 1.0).abs() < 1e-6);
		assert!((overlap((0.0, 0.0, 2.0, 2.0), (1.0, 0.0, 3.0, 2.0)) - 0.5).abs() < 1e-6);
		assert!(overlap((0.0, 0.0, 1.0, 1.0), (2.0, 0.0, 3.0, 1.0)).abs() < 1e-6);
	}

	#[test]
	fn a_page_that_repeats_the_last_picture_is_flagged_whatever_the_faces_do() {
		let same = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park\n@mo x=0.32 sad\nTwo.\n",
		);
		assert!(has(&same, "static-page"), "{:?}", rules(&same));

		let posed = review_script(
			"",
			"# park\n@mo x=0.3 happy\nOne.\n# park\n@mo x=0.32 sad droop\nTwo.\n",
		);
		assert!(!has(&posed, "static-page"), "{:?}", rules(&posed));
	}

	#[test]
	fn words_that_name_an_action_the_picture_never_shows_are_flagged() {
		let told = review_script("", "# park\n@mo x=0.3 happy\nMo jumped over the puddle.\n");
		let finding = told
			.iter()
			.find(|finding| finding.rule == "text-picture")
			.expect("a text-picture finding");
		assert_eq!(finding.line, 8);
		assert!(finding.observation.contains("\"jumped\""), "{finding:?}");
		assert!(finding.suggestion.contains("`@mo jump`"), "{finding:?}");

		let shown = review_script(
			"",
			"# park\n@mo x=0.3 happy jump\nMo jumped over the puddle.\n",
		);
		assert!(!has(&shown, "text-picture"), "{:?}", rules(&shown));

		// A walk across the stage shows a run.
		let ran = review_script(
			"",
			"# park\n@mo x=0.3 happy\nMo ran.\n@mo x=0.7\nAnd ran.\n",
		);
		assert!(!has(&ran, "text-picture"), "{:?}", rules(&ran));

		// Speech, idioms and sentences about things are not held to the picture.
		for quiet in [
			"@mo x=0.3 talk\n\"I jumped!\" said Mo.\n",
			"@mo x=0.3 happy\nMo ran the shop.\n",
			"@mo x=0.3 happy\nThe cakes sat down.\n",
			"@mo x=0.3 talk\n\"Look,\" said Mo, and the wind held its breath.\n",
		] {
			let findings = review_script("", &format!("# park\n{quiet}"));
			assert!(!has(&findings, "text-picture"), "{quiet}: {findings:#?}");
		}
	}

	#[test]
	fn words_that_name_somebody_off_stage_are_flagged() {
		// Hana is in the book, but not on this page.
		let missing = review_script(
			"",
			"# park\n@mo x=0.3 happy\nMo looked for Hana and a dog.\n# garden\n@hana x=0.5 \
			 wave\nThere she was.\n",
		);
		let finding = missing
			.iter()
			.find(|finding| finding.rule == "text-picture")
			.expect("a text-picture finding");
		assert!(
			finding.observation.contains("the hana and the dog"),
			"{finding:?}"
		);

		let cast = review_script(
			"",
			"# park\n@mo x=0.3 happy\n@hana x=0.7 wave\nMo looked for Hana.\n",
		);
		assert!(!has(&cast, "text-picture"), "{:?}", rules(&cast));

		// A prop the book hands somebody must be held when the words name it.
		let dropped = review_script(
			"",
			"# park\n@mo x=0.3 happy holds=parcel\nMo had a parcel.\n# garden\n@mo x=0.4 sad\nMo \
			 lost the parcel.\n",
		);
		assert!(
			dropped.iter().any(|finding| {
				finding.rule == "text-picture" && finding.suggestion.contains("holds=parcel")
			}),
			"{dropped:#?}"
		);
	}

	#[test]
	fn a_take_before_its_cause_is_flagged() {
		let early = review_script(
			"",
			"# park\n@mo x=0.3 shocked\nSomething was coming.\n@hana enter=right x=0.7 wave\nIt \
			 was Hana.\n",
		);
		let finding = early
			.iter()
			.find(|finding| finding.rule == "reaction-order")
			.expect("a reaction-order finding");
		assert_eq!(finding.line, 7);
		assert!(finding.observation.contains("line 9"), "{finding:?}");

		let after = review_script(
			"",
			"# park\n@mo x=0.3 happy\nSomething was coming.\n@hana enter=right x=0.7 wave\n@mo \
			 shocked\nIt was Hana.\n",
		);
		assert!(!has(&after, "reaction-order"), "{:?}", rules(&after));

		// A take in the same block as its cause lands with it.
		let together = review_script(
			"",
			"# park\n@mo x=0.3 shocked\n@hana enter=right x=0.7 wave\nIt was Hana.\n",
		);
		assert!(!has(&together, "reaction-order"), "{:?}", rules(&together));
	}

	#[test]
	fn two_figures_who_swap_sides_without_a_walk_are_flagged() {
		let script = "# park\n@mo x=0.3 happy\n@hana x=0.7 wave\nOne.\n# park\n@mo x=0.7 \
		              sad\n@hana x=0.3 wave\nTwo.\n";
		let swapped = review_script("", script);
		assert!(has(&swapped, "screen-direction"), "{:?}", rules(&swapped));

		let walked = review_script(
			"",
			"# park\n@mo x=0.3 happy\n@hana x=0.7 wave\nOne.\n# park\n@mo x=0.3 sad\n@hana x=0.7 \
			 wave\nTwo.\n@mo x=0.9\nThree.\n",
		);
		assert!(!has(&walked, "screen-direction"), "{:?}", rules(&walked));

		let elsewhere = script.replace("# park\n@mo x=0.7", "# garden\n@mo x=0.7");
		assert!(!has(&review_script("", &elsewhere), "screen-direction"));
	}

	#[test]
	fn a_flow_with_a_refrain_wants_one_that_comes_back_and_changes() {
		let flow = "flow = \"pile-up\"\n";
		let none = review_script(flow, &pages_of(30));
		assert!(has(&none, "refrain"), "{:?}", rules(&none));

		let mut chorus = String::new();

		for (index, line) in [
			"Mo hummed, and the cat looked up.",
			"Mo hummed, and the dog looked up.",
			"Mo hummed, and the bird looked up.",
			"Mo hummed one note, and everybody sang.",
		]
		.iter()
		.enumerate()
		{
			let _ = write!(chorus, "# street\n@mo x=0.{} happy\n{line}\n", 3 + index);
		}

		assert!(!has(&review_script(flow, &chorus), "refrain"));

		let unchanged = chorus.replace("Mo hummed one note", "Mo hummed");
		let stuck = review_script(flow, &unchanged);
		assert!(
			stuck
				.iter()
				.any(|finding| finding.rule == "refrain" && finding.observation.contains("never")),
			"{stuck:#?}"
		);

		// Books in other flows are not asked for a refrain.
		assert!(!has(&review_script("", &pages_of(30)), "refrain"));
	}

	#[test]
	fn a_want_flow_wants_the_star_to_speak_or_reach_by_page_two() {
		let flow = "flow = \"three-tries\"\n";
		let silent = review_script(flow, &pages_of(30));
		assert!(has(&silent, "goal-early"), "{:?}", rules(&silent));

		let said = pages_of(30).replacen("happy", "happy talk", 1);
		assert!(!has(&review_script(flow, &said), "goal-early"));
	}

	#[test]
	fn a_circular_flow_wants_to_end_where_it_began() {
		let flow = "flow = \"round-we-go\"\n";
		let away = "# park\n@mo x=0.3 happy\nOne.\n# garden\n@mo x=0.5 sad\nTwo.\n";
		let finding = review_script(flow, away);
		assert!(has(&finding, "return-image"), "{:?}", rules(&finding));

		let home = format!("{away}# park light=golden\n@mo x=0.4 proud\nThree.\n");
		assert!(!has(&review_script(flow, &home), "return-image"));
		assert!(!has(&review_script("", away), "return-image"));
	}

	#[test]
	fn a_bedtime_flow_wants_its_last_third_to_wind_down() {
		let flow = "flow = \"goodnight-tour\"\n";
		let calm = "# street golden\n@mo x=0.3 happy\nOne two three four five six.\n# street \
		            dusk\n@mo x=0.4 happy\nOne two three four five.\n# park moonlit\n@mo x=0.4 \
		            sleepy\nOne two three.\n# bedroom candle\n@mo x=0.5 sleepy\nGoodnight.\n";
		let settled = review_script(flow, calm);
		assert!(!has(&settled, "wind-down"), "{settled:#?}");

		let loud = calm
			.replace(
				"# bedroom candle\n@mo x=0.5 sleepy",
				"# bedroom day\n@mo x=0.5 laugh",
			)
			.replace(
				"Goodnight.",
				"One two three four five six seven eight nine ten.\n@hana x=0.8 wave",
			);
		let lively = review_script(flow, &loud);
		let observations: Vec<&str> = lively
			.iter()
			.filter(|finding| finding.rule == "wind-down")
			.map(|finding| finding.observation.as_str())
			.collect();
		assert_eq!(observations.len(), 4, "{observations:#?}");
	}

	#[test]
	fn a_short_book_notes_the_blank_pages_it_is_padded_with() {
		let mut script = String::new();

		for index in 0..8 {
			let _ = write!(
				script,
				"# park\n@mo x=0.{} happy\nPage {index}.\n",
				2 + index % 6
			);
		}

		let story = crate::story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n{script}"
		))
		.unwrap();
		assert_eq!(
			padding(&story).as_deref(),
			Some("8 story pages leave 12 blank pages; a 24-page book has room for 12 more pages.")
		);
		assert!(has(&review_script("", &script), "page-budget"));
	}
}
