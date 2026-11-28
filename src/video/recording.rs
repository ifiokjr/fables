//! The recording session: every line a video speaks, recorded before the video is cut.
//!
//! A book is read through once with every line timed by its estimate, so the session knows
//! every line it will need before it records one. Then:
//!
//! - lines already in the take cache are used as they are, whatever this machine can do;
//! - new lines for an online voice (`ElevenLabs`, `OpenAI`, Cartesia, Hume) are recorded a few
//!   at a time, after one line says how many characters that will cost and, for `ElevenLabs`,
//!   how much of the plan's allowance is left;
//! - a voice that cannot record here (no API key, no program) is stood in for by its
//!   offline `fallback` voice, and if that cannot speak either, its new lines are silent and
//!   timed by estimate. Either way the build says so, once per voice.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use crate::cast::Cast;
use crate::video::elevenlabs;
use crate::video::voice;
use crate::video::voice::Delivery;
use crate::video::voice::Speech;
use crate::video::voice::Utterance;
use crate::video::voice::Voice;

/// One line as the session holds it.
#[derive(Clone, Debug)]
pub struct Line {
	pub text: String,
	pub voice: Voice,
	pub delivery: Delivery,
}

impl Line {
	pub fn of(utterance: &Utterance<'_>) -> Line {
		Line {
			text: utterance.text.to_owned(),
			voice: utterance.voice.clone(),
			delivery: utterance.delivery,
		}
	}

	fn utterance(&self) -> Utterance<'_> {
		Utterance {
			text: &self.text,
			voice: &self.voice,
			delivery: self.delivery,
		}
	}
}

/// Who speaks a voice's new lines on this machine.
#[derive(Clone, Debug, PartialEq)]
pub enum Speaker {
	/// The voice itself.
	Itself,
	/// An offline voice in its place.
	StandIn(Box<Voice>),
	/// Nobody: the line is silent and timed by its estimate.
	Silent,
}

/// The offline voice that stands in for `voice` when it cannot record here: its `fallback`
/// macOS voice (or Daniel), at its own rate. `None` when `voice` is already that voice.
pub fn stand_in(voice: &Voice) -> Option<Voice> {
	let name = voice
		.fallback
		.clone()
		.unwrap_or_else(|| Voice::default().name);
	let stand_in = Voice {
		name,
		rate: voice.rate,
		..Voice::default()
	};

	(stand_in != *voice).then_some(stand_in)
}

/// The session's decisions for one book.
pub struct Session {
	/// Who speaks each voice's new lines, by the voice's cache key.
	speakers: BTreeMap<String, Speaker>,
}

/// Online providers record a few lines at a time; local ones keep to one.
fn records_in_parallel(voice: &Voice) -> bool {
	matches!(
		voice.provider.as_str(),
		"elevenlabs" | "openai" | "cartesia" | "hume"
	)
}

impl Session {
	/// Decides who speaks every new line and records the online ones, reporting what it does
	/// through `report`, one line at a time.
	pub fn prepare(
		lines: &[Line],
		cast: &Cast,
		cache: &Path,
		report: &mut dyn FnMut(String),
	) -> Result<Session> {
		// A line repeated word for word in the same voice and delivery is one take, however
		// often the book says it: it is counted, paid for and recorded once.
		let mut seen = std::collections::HashSet::new();
		let takes: Vec<&Line> = lines
			.iter()
			.filter(|line| seen.insert(voice::cache_key(&line.voice, line.delivery, &line.text)))
			.collect();
		let cached = takes
			.iter()
			.filter(|line| voice::is_cached(&line.utterance(), cache))
			.count();
		let new: Vec<&Line> = takes
			.into_iter()
			.filter(|line| !voice::is_cached(&line.utterance(), cache))
			.collect();
		let mut speakers = BTreeMap::new();
		let mut waiting: BTreeMap<String, (Speaker, usize, String)> = BTreeMap::new();

		for line in &new {
			let key = format!("{:?}", line.voice);

			if speakers.contains_key(&key) {
				if let Some(entry) = waiting.get_mut(&key) {
					entry.1 += 1;
				}

				continue;
			}

			let speaker = match voice::unavailable(&line.voice) {
				None => Speaker::Itself,
				Some(reason) => {
					let speaker = match stand_in(&line.voice) {
						Some(stand_in) if voice::unavailable(&stand_in).is_none() => {
							Speaker::StandIn(Box::new(stand_in))
						}
						_ => Speaker::Silent,
					};
					waiting.insert(key.clone(), (speaker.clone(), 1, reason));
					speaker
				}
			};
			speakers.insert(key, speaker);
		}

		for (key, (speaker, count, reason)) in &waiting {
			let who = who(lines, key, cast);
			let lines = if *count == 1 { "line" } else { "lines" };
			let what = if let Speaker::StandIn(voice) = speaker {
				format!(
					"the macOS voice {} reads its {count} new {lines}",
					voice.name
				)
			} else {
				let are = if *count == 1 { "is" } else { "are" };
				format!(
					"its {count} new {lines} {are} silent, timed by estimate (no offline voice \
					 can stand in on this machine)"
				)
			};
			report(format!("{who} cannot record here ({reason}), so {what}"));
		}

		let online: Vec<&Line> = new
			.iter()
			.copied()
			.filter(|line| {
				records_in_parallel(&line.voice)
					&& speakers.get(&format!("{:?}", line.voice)) == Some(&Speaker::Itself)
			})
			.collect();

		if !online.is_empty() {
			record(&online, cached, cache, report)?;
		}

		Ok(Session { speakers })
	}

	/// Speaks a line: the cached take if there is one, else whoever the session chose.
	pub fn speak(&self, utterance: &Utterance<'_>, cache: &Path) -> Result<Speech> {
		if voice::is_cached(utterance, cache) {
			return voice::speak(utterance, cache);
		}

		match self.speakers.get(&format!("{:?}", utterance.voice)) {
			Some(Speaker::StandIn(stand_in)) => {
				voice::speak(
					&Utterance {
						voice: stand_in,
						..*utterance
					},
					cache,
				)
			}
			Some(Speaker::Silent) => {
				Ok(voice::silent(
					utterance,
					voice::estimated_seconds(utterance),
				))
			}
			Some(Speaker::Itself) | None => voice::speak(utterance, cache),
		}
	}
}

/// A voice as a reader knows it: the cast member or narrator it belongs to.
fn who(lines: &[Line], key: &str, cast: &Cast) -> String {
	let Some(voice) = lines
		.iter()
		.map(|line| &line.voice)
		.find(|voice| format!("{voice:?}") == key)
	else {
		return "a voice".to_owned();
	};

	let member = cast
		.members
		.values()
		.find(|member| member.voice.as_ref() == Some(voice));

	match member {
		Some(member) => format!("{}'s `{}` voice", member.name, voice.provider),
		None => format!("the narrator's `{}` voice", voice.provider),
	}
}

/// Records new lines for online voices, a few at a time, after saying what it will cost.
fn record(
	lines: &[&Line],
	cached: usize,
	cache: &Path,
	report: &mut dyn FnMut(String),
) -> Result<()> {
	let characters: usize = lines.iter().map(|line| line.text.chars().count()).sum();
	let mut providers: Vec<&str> = lines
		.iter()
		.map(|line| {
			match line.voice.provider.as_str() {
				"elevenlabs" => "ElevenLabs",
				"openai" => "OpenAI",
				"cartesia" => "Cartesia",
				"hume" => "Hume",
				other => other,
			}
		})
		.collect();
	providers.sort_unstable();
	providers.dedup();
	report(format!(
		"recording {} new line{} ({} characters) with {}; {cached} already recorded",
		lines.len(),
		if lines.len() == 1 { "" } else { "s" },
		thousands(characters as u64),
		providers.join(" and ")
	));

	let eleven: u64 = lines
		.iter()
		.filter(|line| line.voice.provider == "elevenlabs")
		.map(|line| line.text.chars().count() as u64)
		.sum();

	if eleven > 0
		&& let Some(allowance) = elevenlabs::allowance()
	{
		report(allowance_note(&allowance, eleven));
	}

	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(elevenlabs::JOBS)
		.build()?;
	let failures: Vec<anyhow::Error> = pool.install(|| {
		lines
			.par_iter()
			.filter_map(|line| voice::speak(&line.utterance(), cache).err())
			.collect()
	});

	if let Some(first) = failures.into_iter().next() {
		// The takes that did record are cached, so the next build only retries the rest.
		return Err(first.context("a line could not be recorded; the others are kept"));
	}

	Ok(())
}

/// What the plan has left, and a warning when the book needs more than that.
pub fn allowance_note(allowance: &elevenlabs::Allowance, needed: u64) -> String {
	let left = allowance.left();
	let mut note = format!(
		"ElevenLabs plan: {} of {} characters left this period",
		thousands(left),
		thousands(allowance.limit)
	);

	if needed > left {
		let _ = write!(
			note,
			"; this book needs {}, so some lines will be refused until it resets",
			thousands(needed)
		);
	}

	note
}

/// 12345 as "12,345".
fn thousands(value: u64) -> String {
	let digits = value.to_string();
	let mut out = String::new();

	for (index, digit) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			out.push(',');
		}

		out.push(digit);
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn eleven() -> Voice {
		Voice {
			provider: "elevenlabs".into(),
			id: Some("abc".into()),
			..Voice::default()
		}
	}

	#[test]
	fn an_online_voice_is_stood_in_for_by_its_fallback() {
		let mut voice = eleven();
		voice.fallback = Some("Moira".into());
		voice.rate = Some(150);
		let moira = stand_in(&voice).unwrap();
		assert_eq!(moira.provider, "say");
		assert_eq!(moira.name, "Moira");
		assert_eq!(moira.rate, Some(150));

		assert_eq!(stand_in(&eleven()).unwrap().name, "Daniel");
		assert_eq!(
			stand_in(&Voice::default()),
			None,
			"Daniel stands in for nobody but others"
		);
	}

	#[test]
	fn a_voice_that_cannot_record_says_who_reads_instead() {
		// On a machine with a key the voice can record, which is a different test.
		if elevenlabs::has_key() {
			return;
		}

		let cache = tempfile::tempdir().unwrap();
		let lines = vec![
			Line {
				text: "Nope.".into(),
				voice: eleven(),
				delivery: Delivery::default(),
			},
			Line {
				text: "Never.".into(),
				voice: eleven(),
				delivery: Delivery::default(),
			},
		];
		let mut notes = Vec::new();
		let session = Session::prepare(&lines, &Cast::default(), cache.path(), &mut |note| {
			notes.push(note);
		})
		.unwrap();

		assert_eq!(notes.len(), 1, "{notes:?}");
		assert!(
			notes[0].starts_with(
				"the narrator's `elevenlabs` voice cannot record here (`ELEVENLABS_API_KEY` is \
				 not set)"
			),
			"{notes:?}"
		);
		assert!(notes[0].contains("2 new lines"), "{notes:?}");

		let expected = if voice::unavailable(&Voice::default()).is_none() {
			Speaker::StandIn(Box::new(stand_in(&eleven()).unwrap()))
		} else {
			Speaker::Silent
		};
		assert_eq!(session.speakers.values().next(), Some(&expected));
	}

	#[test]
	fn the_allowance_warns_before_a_book_overspends() {
		let allowance = elevenlabs::Allowance {
			used: 9_000,
			limit: 10_000,
			resets: None,
		};
		assert_eq!(
			allowance_note(&allowance, 400),
			"ElevenLabs plan: 1,000 of 10,000 characters left this period"
		);
		assert!(
			allowance_note(&allowance, 2_400)
				.ends_with("this book needs 2,400, so some lines will be refused until it resets")
		);
		assert_eq!(thousands(1_234_567), "1,234,567");
		assert_eq!(thousands(12), "12");
	}
}
