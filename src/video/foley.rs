//! Foley: sound effects and ambience recorded by `ElevenLabs` instead of synthesised.
//!
//! A world (or one book) asks for it with a `[sound]` table:
//!
//! ```toml
//! [sound]
//! provider = "elevenlabs"
//!
//! [sound.effects]            # optional: what a built-in effect sounds like in this world
//! bonk = "a frying pan clanging off a cartoon head"
//!
//! [sound.ambience]           # optional: what a bed sounds like in this world
//! birds = "seagulls and a distant harbour bell"
//! ```
//!
//! Every `@sfx` cue and every ambience bed is then recorded once, from a description, and
//! cached by that description, so a re-render never pays twice. A script can also describe a
//! one-off effect after the built-in name that stands in for it: `@sfx bonk "a wooden spoon
//! tapping a pot"`.
//!
//! Nothing here is ever required. With no `[sound]` table, no key, or a recording that is
//! not cached yet when nothing may record, the built-in synthesised sound plays exactly as it
//! always has.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use rayon::prelude::*;
use serde::Deserialize;

use crate::art::seed_of;
use crate::art::settings::Bed;
use crate::story::Story;
use crate::video::audio::Sound;
use crate::video::elevenlabs;
use crate::video::elevenlabs::SoundRequest;
use crate::video::timeline::Timeline;
use crate::video::voice::SAMPLE_RATE;

/// Seconds of ambience recorded as one seamless loop.
const BED_SECONDS: f32 = 20.0;

/// Who makes a book's sound effects and ambience: `[sound]` in a world or a script.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct SoundSource {
	/// `built-in` (the default: synthesised) or `elevenlabs`.
	#[serde(default = "built_in")]
	pub provider: String,
	/// The sound-effects model, if not `eleven_text_to_sound_v3`.
	#[serde(default)]
	pub model: Option<String>,
	/// What a built-in effect sounds like in this world, by its `@sfx` name.
	#[serde(default)]
	pub effects: BTreeMap<String, String>,
	/// What an ambience bed sounds like in this world, by its name.
	#[serde(default)]
	pub ambience: BTreeMap<String, String>,
}

fn built_in() -> String {
	"built-in".to_owned()
}

impl SoundSource {
	/// Checks the table up front, so a misspelt sound fails before anything is recorded.
	pub fn validate(&self) -> Result<()> {
		match self.provider.as_str() {
			"built-in" | "elevenlabs" => {}
			other => bail!("unknown sound provider `{other}`; use `built-in` or `elevenlabs`"),
		}

		for name in self.effects.keys() {
			if Sound::parse(name).is_none() {
				bail!(
					"`[sound.effects]` names `{name}`, which is not a sound; try {}",
					Sound::names()
				);
			}
		}

		for name in self.ambience.keys() {
			if Bed::parse(name).is_none() {
				bail!(
					"`[sound.ambience]` names `{name}`, which is not an ambience bed; try {}",
					Bed::names()
				);
			}
		}

		Ok(())
	}

	fn records(&self) -> bool {
		self.provider == "elevenlabs"
	}

	fn model(&self) -> String {
		self.model
			.clone()
			.unwrap_or_else(|| elevenlabs::SOUND_MODEL.to_owned())
	}

	/// The request for an effect cue: its own description, the world's, or the built-in one.
	pub fn effect(&self, sound: Sound, described: Option<&str>) -> SoundRequest {
		let name = sound_name(sound);
		let (text, seconds) = if let Some(text) = described {
			(text.to_owned(), None)
		} else {
			let (default, seconds) = effect_description(sound);
			let text = self.effects.get(name).map_or(default, String::as_str);
			(text.to_owned(), Some(seconds))
		};

		SoundRequest {
			text,
			seconds,
			looped: false,
			model: self.model(),
		}
	}

	/// The request for an ambience bed: a seamless loop.
	pub fn bed(&self, bed: Bed) -> SoundRequest {
		let text = self
			.ambience
			.get(bed.name())
			.map_or_else(|| bed_description(bed).to_owned(), Clone::clone);

		SoundRequest {
			text,
			seconds: Some(BED_SECONDS),
			looped: true,
			model: self.model(),
		}
	}
}

fn sound_name(sound: Sound) -> &'static str {
	Sound::ALL
		.iter()
		.find(|(_, each)| *each == sound)
		.map_or("", |(name, _)| *name)
}

/// What each built-in effect is asked for, and for how long: cartoon timing, never music
/// under it and never a voice in it.
pub fn effect_description(sound: Sound) -> (&'static str, f32) {
	match sound {
		Sound::Pop => {
			(
				"a single short cartoon pop, bright and clean, like a cork",
				0.5,
			)
		}
		Sound::Plop => ("a cartoon plop of a small thing dropping into water", 0.6),
		Sound::Bonk => {
			(
				"a comic cartoon bonk on the head, a hollow wooden knock",
				0.8,
			)
		}
		Sound::Boing => ("a wobbly cartoon spring boing", 1.0),
		Sound::Whoosh => ("a fast cartoon whoosh as something zips past", 0.8),
		Sound::Sparkle => ("a magical sparkle, a shimmer of tiny twinkling chimes", 1.2),
		Sound::Tweet => ("one cheerful songbird tweet", 0.6),
		Sound::Blorp => {
			(
				"a silly squelchy cartoon blorp, like a bubble of goo popping",
				0.6,
			)
		}
		Sound::PageTurn => {
			(
				"a soft page turn in a picture book, a gentle paper rustle",
				0.6,
			)
		}
		Sound::Sting => {
			(
				"a short comic orchestral sting, two low notes, dun dun",
				1.5,
			)
		}
		Sound::Tada => ("a short triumphant cartoon ta-da fanfare on brass", 1.5),
		Sound::Twinkle => ("a gentle rising twinkle on a glockenspiel", 1.2),
		Sound::Thud => ("a soft heavy thud of something landing on the ground", 0.7),
		Sound::Splash => ("a cartoon water splash", 1.0),
		Sound::GaspWhoosh => {
			(
				"a quick dramatic whoosh like a sharp breath in, with no voice",
				0.8,
			)
		}
		Sound::Ding => ("a bright little bell ding, like a good idea", 1.0),
		Sound::DrumRoll => ("a snare drum roll building up", 2.5),
		Sound::Heartbeat => ("a slow soft heartbeat, two thumps", 1.5),
	}
}

/// What each ambience bed is asked for: a quiet bed with no music and no words in it.
pub fn bed_description(bed: Bed) -> &'static str {
	match bed {
		Bed::Birds => "gentle daytime birdsong in a quiet garden, distant songbirds, no music",
		Bed::Breeze => "a soft outdoor breeze through leaves, calm, no birds, no music",
		Bed::Waves => "small waves lapping on a sandy beach, a calm sea, no music",
		Bed::Rain => "steady gentle rain falling outside, no thunder, no music",
		Bed::Wind => "low gusting wind on a stormy day, no music",
		Bed::Crickets => "crickets chirping on a warm summer night, no music",
		Bed::Room => "the quiet room tone of a cosy house, very soft, no music",
		Bed::Crowd => "a friendly crowd murmuring in a town square, no clear words, no music",
	}
}

/// An effect cue's identity: its built-in sound and its own description, if it has one.
type EffectKey = (Sound, Option<Arc<str>>);

/// Recorded effects and beds for one timeline. Anything missing plays the built-in sound.
#[derive(Clone, Debug, Default)]
pub struct Foley {
	/// Effects by their cue's identity.
	effects: HashMap<EffectKey, Arc<Vec<f32>>>,
	/// Seamless ambience loops by bed.
	beds: HashMap<Bed, Arc<Vec<f32>>>,
}

impl Foley {
	/// The recorded effect for a cue, if there is one.
	pub fn effect(&self, sound: Sound, described: Option<&Arc<str>>) -> Option<&[f32]> {
		self.effects
			.get(&(sound, described.cloned()))
			.map(|samples| samples.as_slice())
	}

	/// The recorded loop for a bed, if there is one.
	pub fn bed(&self, bed: Bed) -> Option<&[f32]> {
		self.beds.get(&bed).map(|samples| samples.as_slice())
	}

	pub fn is_empty(&self) -> bool {
		self.effects.is_empty() && self.beds.is_empty()
	}
}

enum Slot {
	Effect(Sound, Option<Arc<str>>),
	Bed(Bed),
}

/// Gathers the recorded effects and beds a timeline plays. New ones are recorded when `record`
/// is true and the key is set; otherwise only cached ones are used. Notes for the reader go
/// through `report`.
pub fn gather(
	story: &Story,
	timeline: &Timeline,
	cache: &Path,
	record: bool,
	report: &mut dyn FnMut(String),
) -> Result<Foley> {
	let Some(source) = story.meta.sound.as_ref().filter(|source| source.records()) else {
		return Ok(Foley::default());
	};
	source.validate()?;

	let mut wanted: Vec<(Slot, SoundRequest)> = Vec::new();
	let mut seen = std::collections::HashSet::new();

	for cue in &timeline.cues {
		if seen.insert((cue.sound, cue.described.clone())) {
			let request = source.effect(cue.sound, cue.described.as_deref());
			wanted.push((Slot::Effect(cue.sound, cue.described.clone()), request));
		}
	}

	let mut beds: Vec<Bed> = timeline
		.ambience
		.iter()
		.flat_map(|cue| cue.beds.iter().copied())
		.collect();
	beds.sort_unstable();
	beds.dedup();
	wanted.extend(
		beds.into_iter()
			.map(|bed| (Slot::Bed(bed), source.bed(bed))),
	);

	if wanted.is_empty() {
		return Ok(Foley::default());
	}

	std::fs::create_dir_all(cache)
		.with_context(|| format!("could not create {}", cache.display()))?;
	let path =
		|request: &SoundRequest| cache.join(format!("{:016x}.wav", seed_of(&request.cache_key())));
	let missing: Vec<&SoundRequest> = wanted
		.iter()
		.map(|(_, request)| request)
		.filter(|request| !path(request).is_file())
		.collect();

	if !missing.is_empty() {
		if record && elevenlabs::has_key() {
			report(format!(
				"recording {} sound{} with ElevenLabs; {} already recorded",
				missing.len(),
				if missing.len() == 1 { "" } else { "s" },
				wanted.len() - missing.len()
			));
			let pool = rayon::ThreadPoolBuilder::new()
				.num_threads(elevenlabs::JOBS)
				.build()?;
			let failures: Vec<anyhow::Error> = pool.install(|| {
				missing
					.par_iter()
					.filter_map(|request| record_sound(request, &path(request)).err())
					.collect()
			});

			if let Some(first) = failures.into_iter().next() {
				return Err(first.context("a sound could not be recorded; the others are kept"));
			}
		} else {
			let why = if record {
				"`ELEVENLABS_API_KEY` is not set"
			} else {
				"nothing is recorded for this build"
			};
			report(format!(
				"{} sound{} not recorded yet ({why}), so the built-in {} play{}",
				missing.len(),
				if missing.len() == 1 { " is" } else { "s are" },
				if missing.len() == 1 { "one" } else { "ones" },
				if missing.len() == 1 { "s" } else { "" },
			));
		}
	}

	let mut foley = Foley::default();

	for (slot, request) in wanted {
		let file = path(&request);

		if !file.is_file() {
			continue;
		}

		let samples = Arc::new(read(&file)?);

		match slot {
			Slot::Effect(sound, described) => {
				foley.effects.insert((sound, described), samples);
			}
			Slot::Bed(bed) => {
				foley.beds.insert(bed, samples);
			}
		}
	}

	Ok(foley)
}

fn record_sound(request: &SoundRequest, wav: &Path) -> Result<()> {
	let raw = wav.with_extension("mp3");
	elevenlabs::sound(request, &raw)?;
	let normalised = crate::video::voice::normalise(&raw, wav);
	let _ = std::fs::remove_file(&raw);
	normalised
}

/// Reads a cached recording, untrimmed: a loop must keep every sample to stay seamless.
fn read(path: &Path) -> Result<Vec<f32>> {
	let mut reader = hound::WavReader::open(path)
		.with_context(|| format!("could not open {}", path.display()))?;
	let spec = reader.spec();

	if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE || spec.bits_per_sample != 16 {
		bail!("{} is not 48 kHz 16-bit mono", path.display());
	}

	reader
		.samples::<i16>()
		.map(|sample| sample.map(|value| f32::from(value) / f32::from(i16::MAX)))
		.collect::<std::result::Result<Vec<f32>, _>>()
		.with_context(|| format!("could not read samples from {}", path.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn eleven(extra: &str) -> SoundSource {
		toml::from_str(&format!("provider = \"elevenlabs\"\n{extra}")).unwrap()
	}

	#[test]
	fn every_built_in_sound_and_bed_has_a_description() {
		let source = eleven("");

		for (name, sound) in Sound::ALL {
			let request = source.effect(sound, None);
			assert!(request.text.len() > 10, "{name}");
			assert!(
				request.seconds.is_some_and(|s| (0.5..=30.0).contains(&s)),
				"{name}"
			);
			assert!(!request.looped);
		}

		for (name, bed) in Bed::ALL {
			let request = source.bed(bed);
			assert!(
				request.text.contains("no "),
				"{name}: beds ask for no music or words"
			);
			assert!(request.looped);
		}
	}

	#[test]
	fn a_world_and_a_cue_can_describe_their_own_sounds() {
		let source =
			eleven("[effects]\nbonk = \"a frying pan clang\"\n[ambience]\nbirds = \"seagulls\"\n");
		source.validate().unwrap();
		assert_eq!(source.effect(Sound::Bonk, None).text, "a frying pan clang");
		assert_eq!(source.bed(Bed::Birds).text, "seagulls");

		let cue = source.effect(Sound::Bonk, Some("a spoon on a pot"));
		assert_eq!(cue.text, "a spoon on a pot");
		assert_eq!(cue.seconds, None, "a described cue is as long as it needs");
	}

	#[test]
	fn a_misspelt_sound_fails_before_anything_is_recorded() {
		let error = eleven("[effects]\nbonkk = \"x\"\n").validate().unwrap_err();
		assert!(error.to_string().contains("`bonkk`"), "{error}");
		let error = eleven("[ambience]\nseagulls = \"x\"\n")
			.validate()
			.unwrap_err();
		assert!(error.to_string().contains("`seagulls`"), "{error}");
		let error: SoundSource = toml::from_str("provider = \"foley-co\"\n").unwrap();
		assert!(error.validate().is_err());
	}

	#[test]
	fn the_cache_key_changes_with_anything_that_changes_the_sound() {
		let source = eleven("");
		let one = source.effect(Sound::Bonk, None).cache_key();
		assert_eq!(one, source.effect(Sound::Bonk, None).cache_key());
		assert_ne!(one, source.effect(Sound::Bonk, Some("x")).cache_key());
		assert_ne!(
			one,
			eleven("model = \"eleven_text_to_sound_v2\"")
				.effect(Sound::Bonk, None)
				.cache_key()
		);
	}
}
