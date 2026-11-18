//! Sound measured rather than heard.
//!
//! Each test writes a small book into a temporary world, lays it out with a synthetic narrator
//! that reads at the rate it is asked for, writes the real `soundtrack.wav`, and then measures
//! the file: RMS and spectral-centroid windows across a cue change, the silence before a shouted
//! peak, the ambience's level under the narration, the worst sample-to-sample step at a cue
//! boundary, and words a minute for each age band. A passing listen is not available to a test,
//! so these numbers are the evidence.

use std::path::Path;
use std::path::PathBuf;

use fables::art::settings::Bed;
use fables::cast::Cast;
use fables::stage::rehearse;
use fables::story::Story;
use fables::video;
use fables::video::audio;
use fables::video::audio::Music;
use fables::video::audio::Sound;
use fables::video::lipsync;
use fables::video::timeline;
use fables::video::timeline::Timeline;
use fables::video::voice::SAMPLE_RATE;
use fables::video::voice::Speech;
use fables::video::voice::Utterance;
use fables::world::World;

const RATE: f32 = SAMPLE_RATE as f32;

/// A paragraph of exactly thirty words, the same one read at every age band.
const THIRTY_WORDS: &str = "One two three four five six seven eight nine ten eleven twelve \
                            thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty \
                            twenty-one twenty-two twenty-three twenty-four twenty-five twenty-six \
                            twenty-seven twenty-eight twenty-nine thirty.";

const WORLD: &str = "name = \"Soundtest\"\nstyle = \"cartoon\"\n";

const CAST: &str = "\
[pip]
name = \"Pip\"
[pip.voice]
name = \"Junior\"
[mum]
name = \"Mum\"
kind = \"grown-up\"
";

/// A temporary world to write books in, kept alive by its directory.
struct TestWorld {
	dir: tempfile::TempDir,
	world: World,
}

impl TestWorld {
	fn new() -> TestWorld {
		let dir = tempfile::tempdir().unwrap();
		let folder = dir.path().join("worlds/sound");
		std::fs::create_dir_all(&folder).unwrap();
		std::fs::write(folder.join("world.toml"), WORLD).unwrap();
		std::fs::write(folder.join("cast.toml"), CAST).unwrap();
		let world = World::load(&folder).unwrap();

		TestWorld { dir, world }
	}

	/// Writes a book and returns its script and cast, furnished by the world.
	fn book(&self, name: &str, source: &str) -> (Story, Cast) {
		let folder = self.world.dir.join("stories").join(name);
		std::fs::create_dir_all(&folder).unwrap();
		let path = folder.join("story.md");
		std::fs::write(&path, source).unwrap();
		let mut story = Story::load(&path).unwrap();
		self.world.furnish(&mut story).unwrap();
		let cast = self.world.cast_for(&story).unwrap();

		(story, cast)
	}

	fn path(&self, name: &str) -> PathBuf {
		self.dir.path().join(name)
	}
}

/// A narrator that reads at the rate it is asked for, like `say -r`: the model has to obey the
/// voice, or the words-a-minute measurement would measure the model instead of the book.
fn speech(line: &Utterance<'_>) -> Speech {
	let words = line.text.split_whitespace().count().max(1) as f32;
	let seconds = words / line.voice.rate() as f32 * 60.0;
	let samples: Vec<f32> = (0..(seconds * RATE) as usize)
		.map(|index| {
			let t = index as f32 / RATE;
			let envelope = (t / 0.02).min(1.0) * ((seconds - t) / 0.03).clamp(0.0, 1.0);
			(std::f32::consts::TAU * 220.0 * t).sin() * 0.3 * envelope
		})
		.collect();
	let envelope = vec![0.6_f32; (seconds * 60.0) as usize];
	let track = lipsync::from_envelope(line.text, &envelope, 60.0);

	Speech {
		text: line.text.to_owned(),
		samples,
		envelope,
		track,
	}
}

fn plan(story: &Story, cast: &Cast) -> Timeline {
	let rehearsals = rehearse(story, cast).unwrap();

	timeline::build(story, cast, &rehearsals, |line| Ok(speech(line))).unwrap()
}

/// Lays a book out and writes its real soundtrack, returning the timeline it came from.
fn soundtrack(story: &Story, cast: &Cast, path: &Path) -> Timeline {
	let timeline = plan(story, cast);
	audio::write(&timeline, path).unwrap();

	timeline
}

fn read(path: &Path) -> Vec<f32> {
	let mut reader = hound::WavReader::open(path).unwrap();
	let spec = reader.spec();
	assert_eq!(spec.channels, 2);
	let samples: Vec<i16> = reader
		.samples::<i16>()
		.map(|sample| sample.unwrap())
		.collect();

	samples
		.chunks(2)
		.map(|pair| (f32::from(pair[0]) + f32::from(pair[1])) / (2.0 * f32::from(i16::MAX)))
		.collect()
}

fn window(samples: &[f32], start: f32, end: f32) -> &[f32] {
	let from = ((start.max(0.0) * RATE) as usize).min(samples.len());
	let to = ((end.max(0.0) * RATE) as usize).min(samples.len());

	&samples[from..to.max(from)]
}

fn rms(samples: &[f32]) -> f32 {
	if samples.is_empty() {
		return 0.0;
	}

	(samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

/// The level of `here` against `reference`, in dB (negative is quieter).
fn db(reference: f32, here: f32) -> f32 {
	20.0 * (here / reference.max(1e-12)).log10()
}

/// A spectral centroid: the energy of 24 log-spaced bands weighted by their frequency.
fn centroid(samples: &[f32]) -> f32 {
	let mut energy = 0.0;
	let mut weighted = 0.0;

	for band in 0..24 {
		let freq = 80.0 * 2f32.powf(band as f32 / 4.0);
		let (mut real, mut imaginary) = (0.0_f32, 0.0_f32);

		for (index, sample) in samples.iter().enumerate() {
			let phase = std::f32::consts::TAU * freq * (index as f32 / RATE);
			real += sample * phase.cos();
			imaginary += sample * phase.sin();
		}

		let power = real * real + imaginary * imaginary;
		energy += power;
		weighted += power * freq;
	}

	weighted / energy.max(1e-12)
}

/// The worst sample-to-sample step in a stretch: a click would show up here.
fn worst_step(samples: &[f32]) -> f32 {
	samples
		.windows(2)
		.map(|pair| (pair[1] - pair[0]).abs())
		.fold(0.0_f32, f32::max)
}

/// The longest stretch of sound in a track: where a paragraph is read, start and length.
fn longest_run(samples: &[f32]) -> (f32, f32) {
	let threshold = 0.005;
	let mut best = (0.0_f32, 0.0_f32);
	let mut running: Option<usize> = None;
	let mut last_loud = 0;
	let gap = (0.2 * RATE) as usize;

	for (index, sample) in samples.iter().enumerate() {
		if sample.abs() > threshold {
			if running.is_none() {
				running = Some(index);
			}

			last_loud = index;
		} else if let Some(start) = running
			&& index - last_loud > gap
		{
			let length = (last_loud - start) as f32 / RATE;

			if length > best.1 {
				best = (start as f32 / RATE, length);
			}

			running = None;
		}
	}

	if let Some(start) = running {
		let length = (last_loud - start) as f32 / RATE;

		if length > best.1 {
			best = (start as f32 / RATE, length);
		}
	}

	best
}

/// A kitchen-sink book: every cue, a light-driven change, a shouted peak, `@hold`, `hold=`,
/// three ambiences and nine new effects.
const KITCHEN_SINK: &str = "+++
title = \"Every Sound\"
author = \"A\"
star = \"pip\"
age = \"3-5\"

[video]
cast-voices = true
+++

// A town morning: the default cue.
# park light=dawn
@pip x=0.4 happy talk
\"Good morning!\" said Pip.
@pause 0.4
@sfx tada
@pip cheer
@pause 0.4

// A golden afternoon: calm, with a reveal the script holds for.
# park light=golden hold=0.8
@pip x=0.5 grin talk
\"What a day.\" said Pip.
@pause 0.4
@sfx heartbeat
@pause 0.4

// A storm: tense, with rain and wind beds.
# hilltop light=storm
@pip x=0.45 worried talk
\"Oh no.\" said Pip.
@pause 0.3
@sfx drum-roll
@pause 0.4
@sfx thud
@pause 0.4

// Dusk: calm.
# garden light=dusk
@pip x=0.5 sad talk
\"I am lost.\" said Pip.
@pause 0.4
@sfx twinkle
@pause 0.4

// Night, in a room: sleepy, a room tone and no crickets.
# bathroom light=night
@pip x=0.5 sleepy talk
\"Bedtime.\" said Pip.
@pause 0.4
@sfx ding
@pause 0.4

// A dead stop in the middle of a page.
# playroom light=candle
@pip x=0.5 dreamy talk
\"Shh.\" said Pip.
@hold 0.9
@music none
@pause 1.0
Every sound, then nothing.

// Morning again: bright, and a shouted peak with a fresh strong mood.
# park light=day music=bright
@pip x=0.3 happy talk
\"I can shout!\" said Pip.
@pip cross talk
\"**STOP!**\" said Pip.
@pause 1.2

// A quiet place for the ambience to be heard alone.
# hilltop light=day
@pip x=0.5 calm talk
\"Listen.\" said Pip.
@pause 2.4
@sfx gasp-whoosh
@pause 0.6
@sfx splash
@pause 0.6
@sfx sting
@pause 0.6
";

#[test]
fn the_kitchen_sink_book_uses_every_sound_word() {
	let world = TestWorld::new();
	let (story, cast) = world.book("every-sound", KITCHEN_SINK);
	let path = world.path("every-sound.wav");
	let timeline = soundtrack(&story, &cast, &path);

	let cues: Vec<Music> = timeline.music.iter().map(|cue| cue.music).collect();
	for cue in [
		Music::Play,
		Music::Calm,
		Music::Tense,
		Music::Sleepy,
		Music::Bright,
		Music::None,
	] {
		assert!(cues.contains(&cue), "{cue:?} is missing from {cues:?}");
	}

	assert!(!timeline.dropouts.is_empty(), "the shouted peak drops out");
	assert_eq!(
		timeline.holds.len(),
		2,
		"an @hold and a page hold=, {:?}",
		timeline.holds
	);

	let beds: Vec<Bed> = timeline
		.ambience
		.iter()
		.flat_map(|cue| cue.beds.clone())
		.collect();
	for bed in [Bed::Birds, Bed::Breeze, Bed::Wind, Bed::Rain, Bed::Room] {
		assert!(beds.contains(&bed), "{bed:?} is missing from {beds:?}");
	}

	let sounds: Vec<Sound> = timeline.cues.iter().map(|cue| cue.sound).collect();
	for sound in [
		Sound::Tada,
		Sound::Heartbeat,
		Sound::DrumRoll,
		Sound::Thud,
		Sound::Twinkle,
		Sound::Ding,
		Sound::GaspWhoosh,
		Sound::Splash,
		Sound::Sting,
	] {
		assert!(
			sounds.contains(&sound),
			"{sound:?} is missing from {sounds:?}"
		);
	}

	// The file is real, and every part of it makes a sound.
	let track = read(&path);
	assert!(
		rms(&track) > 0.01,
		"the soundtrack is audible: {}",
		rms(&track)
	);

	// No cue boundary clicks: each change is no rougher than the music beside it.
	for cue in timeline.music.iter().skip(1) {
		let step = worst_step(window(&track, cue.time - 0.02, cue.time + 0.02));
		let around = worst_step(window(&track, cue.time - 1.0, cue.time + 1.0));
		println!(
			"cue {} at {:.2}s: step {step:.4} (around {around:.4})",
			cue.music.name(),
			cue.time
		);
		assert!(step < 0.1, "a click at {:.2}s: {step}", cue.time);
		assert!(
			step <= around.max(0.02) * 1.2,
			"a step bigger than the music at {:.2}s: {step:.4} against {around:.4}",
			cue.time
		);
	}

	// Leave a copy where a person can listen to it.
	let evidence = std::env::temp_dir().join("fables-sound-test");
	std::fs::create_dir_all(&evidence).unwrap();
	std::fs::copy(&path, evidence.join("soundtrack.wav")).unwrap();
}

#[test]
fn each_age_band_narrates_at_its_rate() {
	let world = TestWorld::new();
	let mut measured = Vec::new();

	for age in ["under-3", "3-5", "5-7"] {
		let source = format!(
			"+++\ntitle = \"Bands\"\nauthor = \"A\"\nstar = \"pip\"\nage = \
			 \"{age}\"\n\n[video]\nmusic = \"none\"\nambience = false\n+++\n\n# \
			 park\n{THIRTY_WORDS}\n"
		);
		let (story, cast) = world.book(&format!("bands-{age}"), &source);
		let path = world.path(&format!("bands-{age}.wav"));
		soundtrack(&story, &cast, &path);
		let track = read(&path);
		let (_, seconds) = longest_run(&track);
		let words = THIRTY_WORDS.split_whitespace().count() as f32;
		let wpm = words / seconds * 60.0;
		measured.push((age, wpm));
	}

	println!("words a minute, one thirty-word paragraph: {measured:?}");

	for (age, wpm) in &measured {
		let want = match *age {
			"under-3" => 125.0,
			"3-5" => 135.0,
			_ => 150.0,
		};
		assert!(
			(wpm - want).abs() / want < 0.02,
			"{age}: {wpm:.1} words a minute, wanted {want}"
		);
	}

	assert!(measured[0].1 < measured[1].1 && measured[1].1 < measured[2].1);
}

#[test]
fn a_light_change_cross_fades_to_the_light_s_cue() {
	let world = TestWorld::new();
	let (story, cast) = world.book(
		"cue-change",
		"+++\ntitle = \"Cues\"\nauthor = \"A\"\nstar = \"pip\"\nage = \
		 \"3-5\"\n\n[video]\nambience = false\n+++\n\n# park light=day\nA short line.\n\n# park \
		 light=moonlit\nAnother short line.\n",
	);
	let path = world.path("cue-change.wav");
	let timeline = soundtrack(&story, &cast, &path);
	let track = read(&path);
	let change = timeline
		.shots
		.iter()
		.filter(|shot| shot.kind.page().is_some())
		.nth(1)
		.map(|shot| shot.start)
		.expect("the moonlit page");

	assert_eq!(timeline.music_at(change - 0.05), Music::Play);
	assert_eq!(timeline.music_at(change + 0.05), Music::Sleepy);

	// Both windows are silent of narration: the old cue before the page turn, the new one after
	// the one-second cross-fade, inside the page's lead-in.
	let before = window(&track, change - 1.1, change - 0.2);
	let after = window(&track, change + 1.1, change + 1.7);
	let (before_rms, after_rms) = (rms(before), rms(after));
	let (before_centroid, after_centroid) = (centroid(before), centroid(after));

	println!(
		"cue change at {change:.2}s: before {before_rms:.4} rms, {before_centroid:.0} Hz; after \
		 {after_rms:.4} rms, {after_centroid:.0} Hz"
	);

	assert!(before_rms > 0.002, "the ukulele is playing: {before_rms}");
	assert!(
		db(before_rms, after_rms).abs() < 6.0,
		"the cues are levelled: {before_rms:.4} against {after_rms:.4}"
	);
	assert!(
		after_centroid > before_centroid * 1.3,
		"the music box is brighter than the ukulele: {after_centroid:.0} Hz against \
		 {before_centroid:.0} Hz"
	);

	// No click at the boundary: no bigger step than the music takes anywhere in that second.
	let music_step = worst_step(window(&track, change - 1.0, change + 1.0));
	let boundary_step = worst_step(window(&track, change - 0.02, change + 0.02));
	println!("click check: worst {boundary_step:.4} at the change, {music_step:.4} in the second");
	assert!(
		boundary_step < 0.1 && boundary_step <= music_step.max(0.02) * 1.2,
		"a click at the cue change: {boundary_step}"
	);
}

#[test]
fn the_music_drops_out_before_a_shouted_peak_and_comes_back() {
	let world = TestWorld::new();
	let (story, cast) = world.book(
		"shout",
		"+++\ntitle = \"Shout\"\nauthor = \"A\"\nstar = \"pip\"\nage = \
		 \"3-5\"\n\n[video]\ncast-voices = true\nambience = false\n+++\n\n# park\n@pip happy \
		 talk\n\"I can shout.\" said Pip.\n@pip cross talk\n\"**STOP!**\" said Pip.\n@pause 1.5\n",
	);
	let path = world.path("shout.wav");
	let timeline = soundtrack(&story, &cast, &path);
	let track = read(&path);

	let shout = timeline
		.shots
		.iter()
		.flat_map(|shot| &shot.lines)
		.find(|line| line.text.contains("STOP"))
		.expect("the shouted line");
	let page = &timeline.shots[1];
	let control = rms(window(&track, page.start + 1.1, page.start + 1.7));
	let dropout = rms(window(&track, shout.start - 0.4, shout.start - 0.02));
	let back = rms(window(
		&track,
		shout.start + shout.duration + 0.5,
		shout.start + shout.duration + 1.2,
	));

	println!(
		"shout at {:.2}s: music {control:.4}, half a second before {dropout:.4} ({:.1} dB), after \
		 {back:.4}",
		shout.start,
		db(control, dropout)
	);

	assert!(
		db(control, dropout) < -18.0,
		"the half second before the shout is silent: {dropout:.5} against {control:.4}"
	);
	assert!(dropout < 0.002, "below the silence threshold: {dropout}");
	assert!(
		back > control * 0.2,
		"the music returns after the line: {back:.4} against {control:.4}"
	);
	assert_eq!(timeline.dropouts.len(), 1);
	assert!((timeline.dropouts[0].start - (shout.start - 0.5)).abs() < 1e-3);
}

#[test]
fn ambience_sits_twenty_five_db_under_the_narration() {
	let world = TestWorld::new();
	let (story, cast) = world.book(
		"ambience",
		"+++\ntitle = \"Ambience\"\nauthor = \"A\"\nstar = \"pip\"\nage = \
		 \"3-5\"\n\n[video]\nmusic = \"none\"\n+++\n\n# park\n@scene ambience=birds\nA short line \
		 of words.\n@pause 2.6\n",
	);
	let path = world.path("ambience.wav");
	let timeline = soundtrack(&story, &cast, &path);
	let track = read(&path);
	let page = &timeline.shots[1];
	let line = &page.lines[0];
	let line_end = line.start + line.duration;

	assert_eq!(timeline.ambience_at(page.start + 0.1), &[Bed::Birds]);

	let narration = rms(window(&track, line.start + 0.1, line_end - 0.1));
	// The duck's release is half a second, so a second after the line the ambience is alone.
	let ambience = rms(window(&track, line_end + 1.0, line_end + 2.2));
	let under = db(narration, ambience);

	println!("narration {narration:.4}, ambience {ambience:.4}: {under:.1} dB under");

	assert!(narration > 0.1, "a real narration level: {narration}");
	assert!(ambience > 1e-4, "the place is audible: {ambience}");
	assert!(
		(under + 25.0).abs() < 3.0,
		"the ambience sits about 25 dB under the narration: {under:.1} dB"
	);
}

#[test]
fn an_action_s_sound_lands_after_its_contact_frame() {
	let world = TestWorld::new();
	let legacy = world.book(
		"legacy-bonk",
		"+++\ntitle = \"Bonk\"\nauthor = \"A\"\nstar = \"pip\"\n+++\n\n# park\n@pip\nOne.\n@pip \
		 faint\nTwo.\n",
	);
	let aged = world.book(
		"aged-bonk",
		"+++\ntitle = \"Bonk\"\nauthor = \"A\"\nstar = \"pip\"\nage = \"3-5\"\n+++\n\n# \
		 park\n@pip\nOne.\n@pip faint\nTwo.\n",
	);
	let bonk = |story: &Story, cast: &Cast| {
		let timeline = plan(story, cast);
		let key = timeline.shots[1].keys[1].time;
		let cue = timeline
			.cues
			.iter()
			.find(|cue| cue.sound == Sound::Bonk)
			.map(|cue| cue.time)
			.expect("a bonk");

		cue - key
	};

	let old = bonk(&legacy.0, &legacy.1);
	let new = bonk(&aged.0, &aged.1);
	println!("a faint's bonk: {old:.2}s after the direction, {new:.2}s with an age band");

	assert!(
		(old - 0.25).abs() < 1e-3,
		"the timing every book had: {old}"
	);
	assert!(
		(new - 0.35).abs() < 1e-3,
		"the contact frame plus three frames: {new}"
	);
	assert!(
		new > old,
		"and never before the contact: {new} against {old}"
	);
}

/// A diagnostic, not a check: it prints one row a second with the soundtrack's RMS and the
/// music cue playing, for a book that has already been rendered so its voice cache exists.
///
/// ```text
/// FABLES_STORY=worlds/mumbleton/stories/the-visitor/story.md \
/// FABLES_CACHE=out/the-visitor/riso/video/cache/voice \
///   cargo test --test sound plot_a_rendered_soundtrack -- --ignored --nocapture
/// ```
#[test]
#[ignore = "a diagnostic: needs a rendered book's voice cache"]
fn plot_a_rendered_soundtrack() {
	let Ok(story_path) = std::env::var("FABLES_STORY") else {
		return;
	};
	let cache = PathBuf::from(std::env::var("FABLES_CACHE").expect("FABLES_CACHE"));
	let mut story = Story::load(Path::new(&story_path)).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast_for(&story).unwrap();
	let timeline = video::plan(&story, &cast, &cache).unwrap();

	if let Ok(path) = std::env::var("FABLES_OUT") {
		audio::write(&timeline, Path::new(&path)).unwrap();
		println!("wrote {path}");
	}

	let track = audio::render(&timeline);

	println!("second\trms\tcue");
	for second in 0..timeline.duration.ceil() as usize {
		let here = rms(window(&track.mix, second as f32, second as f32 + 1.0));
		let cue = timeline.music_at(second as f32 + 0.5);

		println!("{second}\t{here:.4}\t{}", cue.name());
	}
}
