//! The soundtrack: narration, procedural music cues and ambience, and cartoon sound effects.
//!
//! Everything except the narrator's voice is synthesised here, so videos need no sample
//! libraries and carry no licensing questions. The music and the ambience duck under the
//! narrator, a cue change cross-fades so it never clicks, and a shouted peak can drop the music
//! out for half a second so the line lands in air.
//!
//! The music is a handful of four-bar loops from the same simple instruments. Each [`Music`]
//! cue differs in mode, tempo and voicing; a page's light asks for one, and a script can say
//! otherwise. A book that never sets a light and never names a cue hears [`Music::Play`]
//! throughout, exactly as every book did before cues existed.

use std::f32::consts::FRAC_PI_2;
use std::f32::consts::TAU;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Deserializer;

use crate::art::light::Preset;
use crate::art::noise;
use crate::art::settings::Bed;
use crate::video::timeline::Span;
use crate::video::timeline::Timeline;
use crate::video::voice::SAMPLE_RATE;

const SR: f32 = SAMPLE_RATE as f32;

/// How long a music cue takes to cross-fade into the next.
const CUE_FADE: f32 = 1.0;
/// How quickly the music drops out for a shouted peak, so the cut has no click in it.
const DROP_RAMP: f32 = 0.03;
/// Ambience sits this many dB under the narrator's own loudness.
const AMBIENCE_UNDER: f32 = 25.0;

/// The music a scene plays under.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Music {
	/// Today's bouncy I–vi–IV–V ukulele strum, the default everywhere else.
	#[default]
	Play,
	/// Slower, sparser and major: golden light, a safe moment.
	Calm,
	/// Minor with a low pulse, no strum: a storm, an overcast threat.
	Tense,
	/// Slow, minor, single notes: the sad page.
	Sad,
	/// A music box at about 70 bpm: bedtime, moonlight, candlelight.
	Sleepy,
	/// Faster and fuller: a celebration.
	Bright,
	/// No music at all.
	None,
}

impl Music {
	pub const ALL: [(&'static str, Music); 7] = [
		("play", Music::Play),
		("calm", Music::Calm),
		("tense", Music::Tense),
		("sad", Music::Sad),
		("sleepy", Music::Sleepy),
		("bright", Music::Bright),
		("none", Music::None),
	];

	pub fn parse(name: &str) -> Option<Music> {
		Self::ALL
			.iter()
			.find(|(key, _)| key.eq_ignore_ascii_case(name))
			.map(|(_, music)| *music)
	}

	pub fn name(self) -> &'static str {
		Self::ALL
			.iter()
			.find(|(_, music)| *music == self)
			.map_or("", |(name, _)| *name)
	}

	/// The names, for error messages that list the options.
	pub fn names() -> String {
		Self::ALL.map(|(name, _)| name).join(", ")
	}

	/// The cue a page's light asks for when the script does not choose one.
	pub fn for_light(light: Preset) -> Music {
		match light {
			Preset::Storm | Preset::Overcast => Music::Tense,
			Preset::Dusk | Preset::Golden => Music::Calm,
			Preset::Night | Preset::Moonlit | Preset::Candle => Music::Sleepy,
			Preset::Day | Preset::Dawn | Preset::Indoor => Music::Play,
		}
	}
}

/// A book's `[video] music` choice: a cue, or `auto` so each page's light picks one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MusicDefault {
	/// Each scene's light picks the cue (`Music::for_light`), unless the scene names one.
	#[default]
	Auto,
	Cue(Music),
}

impl MusicDefault {
	pub const NAMES: [&'static str; 8] = [
		"auto", "play", "calm", "tense", "sad", "sleepy", "bright", "none",
	];

	pub fn parse(value: &str) -> Option<MusicDefault> {
		if value.eq_ignore_ascii_case("auto") {
			return Some(MusicDefault::Auto);
		}

		Music::parse(value).map(MusicDefault::Cue)
	}

	pub fn name(self) -> &'static str {
		match self {
			MusicDefault::Auto => "auto",
			MusicDefault::Cue(music) => music.name(),
		}
	}
}

impl<'de> Deserialize<'de> for MusicDefault {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
		let value = String::deserialize(deserializer)?;
		Self::parse(&value).ok_or_else(|| {
			serde::de::Error::custom(format!(
				"unknown music `{value}`; use one of: {}",
				Self::NAMES.join(", ")
			))
		})
	}
}

/// Built-in sound effects, usable from scripts with `@sfx name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
	Pop,
	Plop,
	Bonk,
	Boing,
	Whoosh,
	Sparkle,
	Tweet,
	Blorp,
	PageTurn,
	Sting,
	Tada,
	Twinkle,
	Thud,
	Splash,
	GaspWhoosh,
	Ding,
	DrumRoll,
	Heartbeat,
}

impl Sound {
	pub const ALL: [(&'static str, Sound); 18] = [
		("pop", Sound::Pop),
		("plop", Sound::Plop),
		("bonk", Sound::Bonk),
		("boing", Sound::Boing),
		("whoosh", Sound::Whoosh),
		("sparkle", Sound::Sparkle),
		("tweet", Sound::Tweet),
		("blorp", Sound::Blorp),
		("page", Sound::PageTurn),
		("sting", Sound::Sting),
		("tada", Sound::Tada),
		("twinkle", Sound::Twinkle),
		("thud", Sound::Thud),
		("splash", Sound::Splash),
		("gasp-whoosh", Sound::GaspWhoosh),
		("ding", Sound::Ding),
		("drum-roll", Sound::DrumRoll),
		("heartbeat", Sound::Heartbeat),
	];

	/// The names, for error messages that list the options.
	pub fn names() -> String {
		Self::ALL.map(|(name, _)| name).join(", ")
	}

	pub fn parse(name: &str) -> Option<Sound> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, sound)| *sound)
	}

	/// Whether the effect is one of the ones added with the sound vocabulary, which are
	/// normalised to the loudness of the nine that came first.
	pub fn is_new(self) -> bool {
		!matches!(
			self,
			Sound::Pop
				| Sound::Plop
				| Sound::Bonk
				| Sound::Boing
				| Sound::Whoosh
				| Sound::Sparkle
				| Sound::Tweet
				| Sound::Blorp
				| Sound::PageTurn
		)
	}

	/// Synthesises the effect as mono samples.
	pub fn render(self) -> Vec<f32> {
		let samples = match self {
			Sound::Pop => sweep(620.0, 240.0, 0.09, 0.5),
			Sound::Plop => {
				let mut out = sweep(1100.0, 280.0, 0.13, 0.6);
				mix_into(&mut out, &sweep(700.0, 200.0, 0.1, 0.3), 0.05);
				out
			}
			Sound::Bonk => {
				let mut out = sweep(150.0, 55.0, 0.32, 0.9);
				mix_into(&mut out, &noise_burst(0.03, 0.4, 4000.0), 0.0);
				mix_into(&mut out, &sweep(1300.0, 380.0, 0.45, 0.18), 0.05);
				out
			}
			Sound::Boing => boing(),
			Sound::Whoosh => whoosh(0.55, 0.5),
			Sound::PageTurn => whoosh(0.3, 0.16),
			Sound::Sparkle => sparkle(),
			Sound::Tweet => tweet(),
			Sound::Blorp => blorp(),
			Sound::Sting => sting(),
			Sound::Tada => tada(),
			Sound::Twinkle => twinkle(),
			Sound::Thud => thud(),
			Sound::Splash => splash(),
			Sound::GaspWhoosh => gasp_whoosh(),
			Sound::Ding => ding(),
			Sound::DrumRoll => drum_roll(),
			Sound::Heartbeat => heartbeat(),
		};

		if self.is_new() {
			at_loudness(samples)
		} else {
			samples
		}
	}
}

fn len(seconds: f32) -> usize {
	(seconds * SR) as usize
}

/// A sine whose pitch glides from `from` to `to` Hz, with a fast attack and exponential decay.
fn sweep(from: f32, to: f32, seconds: f32, gain: f32) -> Vec<f32> {
	let n = len(seconds);
	let mut phase = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / n as f32;
			let freq = from * (to / from).powf(t);
			phase += TAU * freq / SR;
			let env = (i as f32 / (SR * 0.004)).min(1.0) * (-t * 5.0).exp();
			phase.sin() * env * gain
		})
		.collect()
}

fn noise_burst(seconds: f32, gain: f32, cutoff: f32) -> Vec<f32> {
	let n = len(seconds);
	let alpha = (TAU * cutoff / SR).min(1.0);
	let mut low = 0.0;

	(0..n)
		.map(|i| {
			low += (noise(91, i as u64) - low) * alpha;
			low * gain * (1.0 - i as f32 / n as f32)
		})
		.collect()
}

fn whoosh(seconds: f32, gain: f32) -> Vec<f32> {
	let n = len(seconds);
	let mut low = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / n as f32;
			let cutoff = 300.0 + 2600.0 * (std::f32::consts::PI * t).sin();
			low += (noise(92, i as u64) - low) * (TAU * cutoff / SR).min(1.0);
			low * gain * (std::f32::consts::PI * t).sin().powi(2)
		})
		.collect()
}

fn boing() -> Vec<f32> {
	let n = len(0.5);
	let mut phase = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let wobble = (TAU * 14.0 * t).sin() * 40.0 * (-t * 4.0).exp();
			let freq = 180.0 + 150.0 * (1.0 - (-t * 10.0).exp()) + wobble;
			phase += TAU * freq / SR;
			phase.sin() * (-t * 5.0).exp() * 0.5
		})
		.collect()
}

fn sparkle() -> Vec<f32> {
	let mut out = vec![0.0; len(1.1)];

	for (k, freq) in [1046.5, 1318.5, 1568.0, 2093.0].iter().enumerate() {
		let start = len(0.07 * k as f32);
		let note: Vec<f32> = (0..len(0.8))
			.map(|i| {
				let t = i as f32 / SR;
				let tone = (TAU * freq * t).sin() + 0.3 * (TAU * freq * 2.76 * t).sin();
				tone * (-t * 6.0).exp() * 0.18
			})
			.collect();
		mix_into(&mut out, &note, start as f32 / SR);
	}

	out
}

fn tweet() -> Vec<f32> {
	let mut out = vec![0.0; len(0.6)];

	for k in 0..3 {
		let chirp = sweep(2600.0 + k as f32 * 200.0, 4200.0, 0.07, 0.22);
		mix_into(&mut out, &chirp, 0.12 * k as f32);
	}

	out
}

fn blorp() -> Vec<f32> {
	let n = len(0.55);
	let mut phase = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let freq = 95.0 + (TAU * 18.0 * t).sin() * 20.0 - t * 40.0;
			phase += TAU * freq / SR;
			let buzz = phase.sin() + 0.5 * (2.0 * phase).sin() + 0.33 * (3.0 * phase).sin();
			let env = (t / 0.03).min(1.0) * (1.0 - t / 0.55).max(0.0);
			buzz * env * 0.32
		})
		.collect()
}

/// A short rising figure: four quick notes up a major arpeggio.
fn sting() -> Vec<f32> {
	let mut out = vec![0.0; len(0.75)];

	for (k, note) in [60.0, 64.0, 67.0, 72.0].iter().enumerate() {
		let freq = midi(*note);
		let rise: Vec<f32> = (0..len(0.24))
			.map(|i| {
				let t = i as f32 / SR;
				let phase = TAU * freq * t * (1.0 + 0.06 * t);
				phase.sin() * (t / 0.008).min(1.0) * (-t * 7.0).exp() * 0.4
			})
			.collect();
		mix_into(&mut out, &rise, 0.11 * k as f32);
	}

	out
}

/// A fanfare: a three-note run into a held major chord.
fn tada() -> Vec<f32> {
	let mut out = vec![0.0; len(1.4)];

	for (k, note) in [60.0, 64.0, 67.0].iter().enumerate() {
		mix_into(
			&mut out,
			&pluck(midi(*note), 0.35, 0.4, 300 + k as u64),
			0.09 * k as f32,
		);
	}

	for (k, note) in [72.0, 76.0, 79.0, 84.0].iter().enumerate() {
		let chord: Vec<f32> = (0..len(1.0))
			.map(|i| {
				let t = i as f32 / SR;
				let tone =
					(TAU * midi(*note) * t).sin() + 0.25 * (TAU * midi(*note) * 2.0 * t).sin();
				tone * (t / 0.02).min(1.0) * (-t * 2.6).exp() * 0.17
			})
			.collect();
		mix_into(&mut out, &chord, 0.28 + 0.012 * k as f32);
	}

	out
}

/// Three high bells falling: a small sparkle of a sound.
fn twinkle() -> Vec<f32> {
	let mut out = vec![0.0; len(0.9)];

	for (k, note) in [100.0, 96.0, 91.0].iter().enumerate() {
		let at = 0.13 * k as f32;
		let bell: Vec<f32> = (0..len(0.6))
			.map(|i| {
				let t = i as f32 / SR;
				let f = midi(*note);
				let tone = (TAU * f * t).sin() + 0.3 * (TAU * f * 2.0 * t).sin();
				tone * (t / 0.004).min(1.0) * (-t * 9.0).exp() * 0.22
			})
			.collect();
		mix_into(&mut out, &bell, at);
	}

	out
}

/// A body landing: a very low thump with a little grit on the attack.
fn thud() -> Vec<f32> {
	let mut out = sweep(120.0, 42.0, 0.4, 0.9);
	mix_into(&mut out, &noise_burst(0.05, 0.5, 900.0), 0.0);
	mix_into(&mut out, &sweep(300.0, 90.0, 0.12, 0.25), 0.0);
	out
}

/// A splash: noise falling away into bubbles.
fn splash() -> Vec<f32> {
	let n = len(0.7);
	let mut low = 0.0;
	let mut out: Vec<f32> = (0..n)
		.map(|i| {
			let t = i as f32 / n as f32;
			let cutoff = 5200.0 - 4200.0 * t;
			low += (noise(97, i as u64) - low) * (TAU * cutoff / SR).min(1.0);
			low * (1.0 - t).powi(2) * 0.9
		})
		.collect();

	for k in 0..4 {
		let note = sweep(500.0 + 90.0 * k as f32, 1400.0, 0.08, 0.18);
		mix_into(&mut out, &note, 0.16 + 0.11 * k as f32);
	}

	out
}

/// A gasp of breath in, then a whoosh past.
fn gasp_whoosh() -> Vec<f32> {
	let n = len(0.7);
	let mut low = 0.0;
	let mut out: Vec<f32> = (0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let rise = (t / 0.3).min(1.0);
			let cutoff = 600.0 + 3000.0 * rise;
			low += (noise(98, i as u64) - low) * (TAU * cutoff / SR).min(1.0);
			low * rise * (1.0 - (t / 0.34).min(1.0)) * 0.9
		})
		.collect();
	mix_into(&mut out, &whoosh(0.5, 0.45), 0.22);
	out
}

/// A struck bell with a long tail.
fn ding() -> Vec<f32> {
	let mut out = vec![0.0; len(1.5)];

	for (k, ratio) in [1.0, 2.0, 2.98, 4.2].iter().enumerate() {
		let amp = 0.3 / (k as f32 + 1.0);
		let partial: Vec<f32> = (0..len(1.4))
			.map(|i| {
				let t = i as f32 / SR;
				(TAU * 880.0 * ratio * t).sin() * (t / 0.003).min(1.0) * (-t * 2.4).exp() * amp
			})
			.collect();
		mix_into(&mut out, &partial, 0.0);
	}

	out
}

/// A drum roll that speeds up into a final hit.
fn drum_roll() -> Vec<f32> {
	let mut out = vec![0.0; len(1.3)];
	let mut at = 0.0f32;
	let mut gap = 0.13f32;

	while at < 0.95 {
		mix_into(&mut out, &noise_burst(0.05, 0.35, 2600.0), at);
		mix_into(&mut out, &sweep(180.0, 90.0, 0.06, 0.35), at);
		at += gap;
		gap = (gap * 0.86).max(0.035);
	}

	mix_into(&mut out, &noise_burst(0.16, 0.7, 1600.0), 1.0);
	mix_into(&mut out, &sweep(150.0, 55.0, 0.28, 0.8), 1.0);
	out
}

/// A heartbeat: lub-dub, then a beat of rest.
fn heartbeat() -> Vec<f32> {
	let mut out = vec![0.0; len(1.1)];

	for (at, gain) in [(0.0, 0.9), (0.24, 0.65)] {
		let thump: Vec<f32> = (0..len(0.2))
			.map(|i| {
				let t = i as f32 / SR;
				(TAU * (62.0 - 18.0 * t) * t).sin()
					* (t / 0.004).min(1.0)
					* (-t * 16.0).exp()
					* gain
			})
			.collect();
		mix_into(&mut out, &thump, at);
	}

	out
}

/// Scales an effect to the loudness of the original nine, so a new `@sfx` sits at the same
/// level as an old one.
fn at_loudness(mut samples: Vec<f32>) -> Vec<f32> {
	const EFFECT_RMS: f32 = 0.1;
	let rms = rms(&samples).max(1e-6);
	let gain = EFFECT_RMS / rms;

	for sample in &mut samples {
		*sample *= gain;
	}

	samples
}

/// A recorded effect made to sit like a built-in one: silence trimmed from the front so it
/// lands on its cue, and from the end, then brought to the same loudness, its peak kept clear
/// of clipping.
fn recorded_effect(recorded: &[f32]) -> Vec<f32> {
	let threshold = 0.01;
	let start = recorded
		.iter()
		.position(|s| s.abs() > threshold)
		.unwrap_or(0);
	let end = recorded
		.iter()
		.rposition(|s| s.abs() > threshold)
		.map_or(start, |i| i + 1);
	let mut samples = at_loudness(recorded[start..end].to_vec());
	let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));

	if peak > 0.9 {
		for sample in &mut samples {
			*sample *= 0.9 / peak;
		}
	}

	samples
}

/// A recorded ambience loop repeated for `seconds`, starting at a different place in the
/// loop for each segment so two scenes with the same bed do not begin identically, and
/// normalised to unit RMS like a procedural bed.
fn recorded_bed(recorded: &[f32], seconds: f32, segment: u64) -> Vec<f32> {
	let n = len(seconds);

	if recorded.is_empty() {
		return vec![0.0; n];
	}

	let offset = (segment.wrapping_mul(0x9E37_79B9) as usize) % recorded.len();
	let mut out: Vec<f32> = (0..n)
		.map(|i| recorded[(offset + i) % recorded.len()])
		.collect();
	let rms = rms(&out).max(1e-6);

	for sample in &mut out {
		*sample /= rms;
	}

	out
}

/// Adds `source` into `target` starting `at` seconds in, growing `target` if needed.
fn mix_into(target: &mut Vec<f32>, source: &[f32], at: f32) {
	let offset = (at.max(0.0) * SR) as usize;

	if target.len() < offset + source.len() {
		target.resize(offset + source.len(), 0.0);
	}

	for (i, sample) in source.iter().enumerate() {
		target[offset + i] += sample;
	}
}

/// A plucked string (Karplus–Strong): a burst of noise circulating through a damped delay line.
fn pluck(freq: f32, seconds: f32, gain: f32, seed: u64) -> Vec<f32> {
	let period = (SR / freq).round().max(2.0) as usize;
	let mut line: Vec<f32> = (0..period).map(|i| noise(seed, i as u64) * gain).collect();
	let n = len(seconds);
	let mut out = Vec::with_capacity(n);
	let mut index = 0;

	for _ in 0..n {
		let next = (index + 1) % period;
		let value = line[index];
		line[index] = f32::midpoint(value, line[next]) * 0.996;
		out.push(value);
		index = next;
	}

	out
}

fn midi(note: f32) -> f32 {
	440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// A music box or celesta note: a sine with two quiet upper partials and a fast decay.
fn bell_note(freq: f32, seconds: f32, gain: f32) -> Vec<f32> {
	let n = len(seconds);

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let tone = (TAU * freq * t).sin()
				+ 0.28 * (TAU * freq * 2.0 * t).sin()
				+ 0.12 * (TAU * freq * 3.0 * t).sin();
			tone * (t / 0.004).min(1.0) * (-t * 5.5).exp() * gain
		})
		.collect()
}

/// Four bars of a bouncy I–vi–IV–V ukulele strum with a soft bass, ready to tile.
fn play_loop() -> (Vec<f32>, usize) {
	let tempo = 104.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(2.0)];
	// Re-entrant ukulele voicings (G4 C4 E4 A4 strings): C, Am, F, G.
	let chords: [[f32; 4]; 4] = [
		[67.0, 60.0, 64.0, 72.0],
		[69.0, 60.0, 64.0, 69.0],
		[69.0, 60.0, 65.0, 69.0],
		[67.0, 62.0, 67.0, 71.0],
	];
	let roots = [48.0, 45.0, 41.0, 43.0];
	// Strum pattern in beats: down, down-up, up-down-up.
	let strums = [0.0, 1.0, 1.5, 2.5, 3.0, 3.5];

	for (b, chord) in chords.iter().enumerate() {
		for (s, offset) in strums.iter().enumerate() {
			let at = b as f32 * bar + offset * beat;
			let accent = if s == 0 || s == 4 { 1.0 } else { 0.65 };

			for (k, note) in chord.iter().enumerate() {
				let order = if s % 2 == 0 { k } else { 3 - k };
				let stagger = order as f32 * 0.012;
				let seed = (b * 100 + s * 10 + k) as u64;
				let string = pluck(midi(*note), 1.4, 0.32 * accent, seed);
				mix_into(&mut out, &string, at + stagger);
			}
		}

		for half in [0.0, 2.0] {
			let at = b as f32 * bar + half * beat;
			let freq = midi(roots[b]);
			let bass: Vec<f32> = (0..len(beat * 1.6))
				.map(|i| {
					let t = i as f32 / SR;
					(TAU * freq * t).sin() * (t / 0.01).min(1.0) * (-t * 2.2).exp() * 0.42
				})
				.collect();
			mix_into(&mut out, &bass, at);
		}
	}

	(out, loop_len)
}

/// Slow, sparse and major: two soft strums a bar over a long bass.
fn calm_loop() -> (Vec<f32>, usize) {
	let tempo = 72.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(2.2)];
	// C, F, G, C: a plain major cadence.
	let chords: [[f32; 3]; 4] = [
		[64.0, 67.0, 72.0],
		[65.0, 69.0, 72.0],
		[62.0, 67.0, 71.0],
		[64.0, 67.0, 72.0],
	];
	let roots = [48.0, 41.0, 43.0, 48.0];

	for (b, chord) in chords.iter().enumerate() {
		for (s, offset) in [0.0, 2.0].iter().enumerate() {
			let at = b as f32 * bar + offset * beat;

			for (k, note) in chord.iter().enumerate() {
				let stagger = k as f32 * 0.02;
				let seed = (b * 40 + s * 6 + k) as u64;
				let string = pluck(midi(*note), 2.0, 0.3, seed);
				mix_into(&mut out, &string, at + stagger);
			}
		}

		let at = b as f32 * bar;
		let freq = midi(roots[b]);
		let bass: Vec<f32> = (0..len(beat * 3.4))
			.map(|i| {
				let t = i as f32 / SR;
				(TAU * freq * t).sin() * (t / 0.02).min(1.0) * (-t * 1.2).exp() * 0.3
			})
			.collect();
		mix_into(&mut out, &bass, at);
	}

	(out, loop_len)
}

/// Minor with a low pulse and no strum: a slow dread under the page.
fn tense_loop() -> (Vec<f32>, usize) {
	let tempo = 84.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(1.6)];
	// D minor: D, D, Bb, A, one low pulse a beat and a fifth on the off beats.
	let roots = [38.0, 38.0, 34.0, 33.0];

	for (b, root) in roots.iter().enumerate() {
		for (k, offset) in [0.0, 1.0, 2.0, 3.0].iter().enumerate() {
			let at = b as f32 * bar + offset * beat;
			let accent = if k == 0 { 0.5 } else { 0.32 };
			let freq = midi(*root);
			let pulse: Vec<f32> = (0..len(beat * 0.9))
				.map(|i| {
					let t = i as f32 / SR;
					(TAU * freq * t).sin() * (t / 0.008).min(1.0) * (-t * 4.5).exp() * accent
				})
				.collect();
			mix_into(&mut out, &pulse, at);
		}

		// A hollow fifth, once a bar, high and quiet.
		let fifth = midi(root + 19.0);
		let drone: Vec<f32> = (0..len(beat * 3.0))
			.map(|i| {
				let t = i as f32 / SR;
				let swell = (t / 0.4).min(1.0) * (1.0 - (t / (beat * 3.0)).min(1.0));
				(TAU * fifth * t).sin() * swell * 0.06
			})
			.collect();
		mix_into(&mut out, &drone, b as f32 * bar);
	}

	(out, loop_len)
}

/// Slow, minor and single notes: one plucked note a bar, walking down.
fn sad_loop() -> (Vec<f32>, usize) {
	let tempo = 60.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(2.6)];
	// A minor, falling: A, G, F, E.
	let notes = [57.0, 55.0, 53.0, 52.0];

	for (b, note) in notes.iter().enumerate() {
		let at = b as f32 * bar;
		mix_into(&mut out, &pluck(midi(*note), 2.4, 0.4, 500 + b as u64), at);
		mix_into(
			&mut out,
			&pluck(midi(*note - 12.0), 2.4, 0.18, 520 + b as u64),
			at,
		);
	}

	(out, loop_len)
}

/// A music box at about 70 bpm: a small lullaby in C.
fn sleepy_loop() -> (Vec<f32>, usize) {
	let tempo = 70.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(1.8)];
	let melody = [72.0, 76.0, 79.0, 76.0, 74.0, 77.0, 81.0, 77.0];

	for (k, note) in melody.iter().enumerate() {
		let at = k as f32 * beat * 2.0;
		mix_into(&mut out, &bell_note(midi(*note), 1.6, 0.24), at);
	}

	for (b, note) in [48.0, 43.0].iter().enumerate() {
		mix_into(
			&mut out,
			&bell_note(midi(*note), 2.4, 0.12),
			b as f32 * bar * 2.0,
		);
	}

	(out, loop_len)
}

/// Faster and fuller: a strum on every beat, a bass and a high answer.
fn bright_loop() -> (Vec<f32>, usize) {
	let tempo = 132.0;
	let beat = 60.0 / tempo;
	let bar = beat * 4.0;
	let loop_len = len(bar * 4.0);
	let mut out = vec![0.0; loop_len + len(2.0)];
	// C, G, Am, F: a bright pop turnaround.
	let chords: [[f32; 3]; 4] = [
		[64.0, 67.0, 72.0],
		[62.0, 67.0, 71.0],
		[64.0, 69.0, 72.0],
		[65.0, 69.0, 74.0],
	];
	let roots = [48.0, 43.0, 45.0, 41.0];

	for (b, chord) in chords.iter().enumerate() {
		for (k, offset) in [0.0, 1.0, 2.0, 3.0].iter().enumerate() {
			let at = b as f32 * bar + offset * beat;
			let accent = if k % 2 == 0 { 1.0 } else { 0.6 };

			for (string, note) in chord.iter().enumerate() {
				let seed = (b * 70 + k * 7 + string) as u64;
				let plucked = pluck(midi(*note), 1.2, 0.26 * accent, seed);
				mix_into(&mut out, &plucked, at + string as f32 * 0.008);
			}

			if k % 2 == 1 {
				let answer = bell_note(midi(chord[2] + 12.0), 0.5, 0.12);
				mix_into(&mut out, &answer, at + beat * 0.5);
			}
		}

		for half in [0.0, 2.0] {
			let at = b as f32 * bar + half * beat;
			let freq = midi(roots[b]);
			let bass: Vec<f32> = (0..len(beat * 1.1))
				.map(|i| {
					let t = i as f32 / SR;
					(TAU * freq * t).sin() * (t / 0.01).min(1.0) * (-t * 3.2).exp() * 0.36
				})
				.collect();
			mix_into(&mut out, &bass, at);
		}
	}

	(out, loop_len)
}

/// The loop for a cue, with its ringing tail, ready to tile. Cues after `play` are normalised
/// to its loudness so a cue change is a change of colour, not of volume.
fn music_loop(cue: Music) -> (Vec<f32>, usize) {
	match cue {
		Music::Play => play_loop(),
		Music::None => (vec![0.0; len(1.0)], len(1.0)),
		other => {
			let (mut bed, loop_len) = match other {
				Music::Calm => calm_loop(),
				Music::Tense => tense_loop(),
				Music::Sad => sad_loop(),
				Music::Sleepy => sleepy_loop(),
				Music::Bright => bright_loop(),
				Music::Play | Music::None => unreachable!("the play and silent cues are above"),
			};
			let (target, _) = play_loop();
			let level = rms(&target[..target.len() - len(2.0)]).max(1e-6);
			let here = rms(&bed[..loop_len]).max(1e-6);
			let gain = level / here;

			for sample in &mut bed {
				*sample *= gain;
			}

			(bed, loop_len)
		}
	}
}

/// One sample of a cue's loop at `local` samples from its start, overlapping the previous
/// repeat's ringing tail so the seam is inaudible.
fn cue_sample(bed: &[f32], loop_len: usize, local: usize) -> f32 {
	let phase = local % loop_len;
	let mut value = bed[phase];

	if local >= loop_len {
		value += bed.get(phase + loop_len).copied().unwrap_or(0.0);
	}

	value
}

/// The music bed over the whole timeline, one cue after another, cross-faded at each change.
fn music_track(timeline: &Timeline, total: usize) -> Vec<f32> {
	let mut track = vec![0.0_f32; total];
	let fade = len(CUE_FADE);

	for (index, cue) in timeline.music.iter().enumerate() {
		let start = len(cue.time.max(0.0));

		if start >= total {
			break;
		}

		let next = timeline
			.music
			.get(index + 1)
			.map(|next| len(next.time.max(0.0)));
		let end = next.map_or(total, |next| (next + fade).min(total));
		let (bed, loop_len) = music_loop(cue.music);

		for (offset, slot) in track[start..end].iter_mut().enumerate() {
			let here = start + offset;
			let value = cue_sample(&bed, loop_len, offset);
			let mut gain = 1.0_f32;

			// A change fades the new cue in while the old one fades out under it.
			if index > 0 && here < start + fade {
				let t = offset as f32 / fade as f32;
				gain *= (t * FRAC_PI_2).sin();
			}

			if let Some(next) = next
				&& here >= next
				&& index + 1 < timeline.music.len()
			{
				let t = (here - next) as f32 / fade as f32;
				gain *= (t.min(1.0) * FRAC_PI_2).cos();
			}

			// A gain of one is exact, so a book with a single cue mixes the loop untouched.
			*slot += value * gain;
		}
	}

	track
}

/// The ambience beds over the whole timeline, already at their mix loudness: about 25 dB under
/// the narrator, and adjusted per bed (`room` is near silence with a faint tone).
fn ambience_track(timeline: &Timeline, voice: &[f32], total: usize) -> Vec<f32> {
	if timeline.ambience.is_empty() {
		return vec![0.0; total];
	}

	let level = speech_rms(voice) * 10f32.powf(-AMBIENCE_UNDER / 20.0);
	let mut track = vec![0.0_f32; total];
	let fade = len(CUE_FADE);

	for (index, cue) in timeline.ambience.iter().enumerate() {
		if cue.beds.is_empty() {
			continue;
		}

		let start = len(cue.time.max(0.0));

		if start >= total {
			break;
		}

		let next = timeline
			.ambience
			.get(index + 1)
			.map(|next| len(next.time.max(0.0)));
		let end = next.map_or(total, |next| (next + fade).min(total));
		let seconds = (end - start) as f32 / SR;
		let mut segment = vec![0.0_f32; end - start];

		for bed in &cue.beds {
			let signal = match timeline.foley.bed(*bed) {
				Some(recorded) => recorded_bed(recorded, seconds, index as u64),
				None => bed_signal(*bed, seconds, index as u64),
			};
			let gain = level * bed.level();

			for (i, sample) in signal.iter().take(segment.len()).enumerate() {
				segment[i] += sample * gain;
			}
		}

		for (i, sample) in segment.into_iter().enumerate() {
			let mut gain = 1.0_f32;

			if index > 0 && i < fade {
				let t = i as f32 / fade as f32;
				gain *= (t * FRAC_PI_2).sin();
			}

			if let Some(next) = next
				&& start + i >= next
				&& index + 1 < timeline.ambience.len()
			{
				let t = (start + i - next) as f32 / fade as f32;
				gain *= (t.min(1.0) * FRAC_PI_2).cos();
			}

			track[start + i] += sample * gain;
		}
	}

	track
}

/// A procedural ambience bed, normalised to unit RMS so beds can be mixed by level. Filtered
/// noise and simple oscillators, like [`whoosh`].
fn bed_signal(bed: Bed, seconds: f32, segment: u64) -> Vec<f32> {
	let n = len(seconds);
	let seed = segment
		.wrapping_mul(0x9E37_79B9_7F4A_7C15)
		.wrapping_add(bed as u64 * 1009);
	let mut out = match bed {
		Bed::Birds => birds(n, seed),
		Bed::Breeze => wind(n, seed, 1400.0, 0.12),
		Bed::Waves => waves(n, seed),
		Bed::Rain => rain(n, seed),
		Bed::Wind => wind(n, seed, 500.0, 0.05),
		Bed::Crickets => crickets(n, seed),
		Bed::Room => room(n, seed),
		Bed::Crowd => crowd(n, seed),
	};
	let rms = rms(&out).max(1e-6);

	for sample in &mut out {
		*sample /= rms;
	}

	out
}

/// Sparse chirps over a very quiet air.
fn birds(n: usize, seed: u64) -> Vec<f32> {
	let mut out = vec![0.0_f32; n];
	let mut at = len(0.2);

	while at < n {
		for k in 0..3 {
			let chirp = sweep(
				2400.0 + noise(seed, k) * 300.0,
				3800.0 + noise(seed, k + 9) * 400.0,
				0.07,
				0.3,
			);
			let start = (at + len(0.075 * k as f32)).min(n.saturating_sub(1));
			let room = n - start;
			let take = chirp.len().min(room);

			for (i, sample) in chirp.iter().take(take).enumerate() {
				out[start + i] += sample;
			}
		}

		let gap = 0.45 + 0.75 * noise(seed, (at / 997) as u64).abs();
		at += len(gap);
	}

	// A faint high air under the chirps, so the gaps are not digital silence.
	let mut low = 0.0;
	for (i, sample) in out.iter_mut().enumerate() {
		low += (noise(seed ^ 0x51, i as u64) - low) * (TAU * 6000.0 / SR).min(1.0);
		*sample += low * 0.03;
	}

	out
}

/// Filtered noise with a slow swell: wind at a cutoff, a breeze higher and gentler.
fn wind(n: usize, seed: u64, cutoff: f32, gust: f32) -> Vec<f32> {
	let mut low = 0.0;
	let mut slow = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let lfo = (TAU * 0.09 * t + noise(seed, 3)).sin() * 0.5 + 0.5;
			let gust = 0.55 + gust * 4.0 * lfo;
			let freq = cutoff * (0.6 + 0.9 * lfo);
			low += (noise(seed, i as u64) - low) * (TAU * freq / SR).min(1.0);
			slow += (low - slow) * 0.002;
			slow * gust
		})
		.collect()
}

/// Slow swells of low noise: a shore.
fn waves(n: usize, seed: u64) -> Vec<f32> {
	let mut low = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let swell = ((TAU * t / 6.0).sin() * 0.5 + 0.5).powi(2);
			low += (noise(seed, i as u64) - low) * (TAU * (420.0 + 380.0 * swell) / SR).min(1.0);
			low * (0.35 + 0.9 * swell)
		})
		.collect()
}

/// A close, hissing rain with a slow change in density.
fn rain(n: usize, seed: u64) -> Vec<f32> {
	let mut low = 0.0;
	let mut lower = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let density = (TAU * 0.13 * t + 1.3).sin() * 0.25 + 0.75;
			let value = noise(seed, i as u64);
			low += (value - low) * (TAU * 5200.0 / SR).min(1.0);
			lower += (low - lower) * (TAU * 900.0 / SR).min(1.0);
			(low - lower * 0.55) * density
		})
		.collect()
}

/// A pulse train of high chirps in a slow chorus: summer night.
fn crickets(n: usize, seed: u64) -> Vec<f32> {
	let mut out = vec![0.0_f32; n];
	let voices = [(0.0, 4300.0, 0.0), (0.31, 4700.0, 0.0), (0.62, 3950.0, 0.0)];

	for (offset, freq, _) in voices {
		let mut phase = 0.0;
		let mut gate = 0.0;
		let start = len(offset);

		for (index, slot) in out[start..].iter_mut().enumerate() {
			let t = index as f32 / SR;
			// Three chirps, then a rest: the classic cricket phrase.
			let phrase = (t % 1.1) / 1.1;
			let target = if phrase < 0.42 && (phrase * 34.0) % 1.0 < 0.55 {
				1.0
			} else {
				0.0
			};
			gate += (target - gate) * 0.35;
			phase += TAU * freq / SR;
			let chorus = (TAU * 0.07 * t + noise(seed, offset as u64 * 7)).sin() * 0.2 + 0.8;
			*slot += phase.sin() * gate * chorus * 0.25;
		}
	}

	out
}

/// Near silence with a faint tone: the room's own hum.
fn room(n: usize, seed: u64) -> Vec<f32> {
	let mut low = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let hum = (TAU * 58.0 * t).sin() * 0.5 + (TAU * 116.0 * t).sin() * 0.2;
			low += (noise(seed, i as u64) - low) * (TAU * 240.0 / SR).min(1.0);
			hum + low * 0.35
		})
		.collect()
}

/// A low murmur of many voices, swelling and settling.
fn crowd(n: usize, seed: u64) -> Vec<f32> {
	let mut low = 0.0;
	let mut lower = 0.0;

	(0..n)
		.map(|i| {
			let t = i as f32 / SR;
			let voices = (TAU * 2.7 * t + noise(seed, 1)).sin().abs()
				* (TAU * 1.9 * t + noise(seed, 2)).sin().abs();
			let swell = (TAU * 0.03 * t + noise(seed, 3)).sin() * 0.3 + 0.7;
			low += (noise(seed, i as u64) - low) * (TAU * 900.0 / SR).min(1.0);
			lower += (low - lower) * (TAU * 180.0 / SR).min(1.0);
			(lower + low * 0.2) * (0.4 + 0.6 * voices) * swell
		})
		.collect()
}

/// The RMS of some samples.
fn rms(samples: &[f32]) -> f32 {
	if samples.is_empty() {
		return 0.0;
	}

	(samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

/// The RMS of the parts of the narrator's track that are actually speaking, so "25 dB under the
/// narration" uses the voice's own level and not its silences.
fn speech_rms(voice: &[f32]) -> f32 {
	let mut sum = 0.0;
	let mut count = 0;

	for sample in voice {
		if sample.abs() > 1e-4 {
			sum += sample * sample;
			count += 1;
		}
	}

	if count == 0 {
		0.0
	} else {
		(sum / count as f32).sqrt()
	}
}

/// How much of the music is left at `time`: nothing inside a drop-out, ramping in and out at
/// its edges so the cut is silent but never clicks.
fn dropout_gain(spans: &[Span], time: f32) -> f32 {
	let mut gain = 1.0_f32;

	for span in spans {
		if time <= span.start - DROP_RAMP || time >= span.end + DROP_RAMP {
			continue;
		}

		let here = if time < span.start {
			(time - (span.start - DROP_RAMP)) / DROP_RAMP
		} else if time > span.end {
			((span.end + DROP_RAMP) - time) / DROP_RAMP
		} else {
			0.0
		};
		gain = gain.min(here.clamp(0.0, 1.0));
	}

	gain
}

/// Everything the soundtrack is made of, before it is mastered into 16-bit samples. The parts
/// are kept apart so a test can measure one against another.
pub struct Soundtrack {
	/// The narrator alone.
	pub voice: Vec<f32>,
	/// The sound effects alone.
	pub effects: Vec<f32>,
	/// The music beds alone, before the music's gain and ducking.
	pub music: Vec<f32>,
	/// The ambience beds alone, already 25 dB under the narrator, before ducking and fading.
	pub ambience: Vec<f32>,
	/// Everything, mixed.
	pub mix: Vec<f32>,
}

/// Renders the full soundtrack, without writing it.
pub fn render(timeline: &Timeline) -> Soundtrack {
	let total = len(timeline.duration + 0.5);
	let mut voice = vec![0.0_f32; total];

	for shot in &timeline.shots {
		for line in &shot.lines {
			let samples = &timeline.speeches[line.speech].samples;
			let offset = len(line.start);

			for (i, sample) in samples.iter().enumerate() {
				if let Some(slot) = voice.get_mut(offset + i) {
					*slot += sample;
				}
			}
		}
	}

	let mut effects = vec![0.0_f32; total];

	for cue in &timeline.cues {
		let rendered = match timeline.foley.effect(cue.sound, cue.described.as_ref()) {
			Some(recorded) => recorded_effect(recorded),
			None => cue.sound.render(),
		};
		mix_into(&mut effects, &rendered, cue.time);
	}

	effects.truncate(total);

	let duck = ducking(&voice);
	let music = music_track(timeline, total);
	let ambience = ambience_track(timeline, &voice, total);
	let fade_out = len(2.5);
	let mut mix = Vec::with_capacity(total);

	for i in 0..total {
		if timeline.in_hold(i as f32 / SR) {
			mix.push(0.0);
			continue;
		}

		let fade = (i as f32 / SR).min(1.0) * ((total - i) as f32 / fade_out as f32).min(1.0);
		let mut music_gain = 0.2 * (1.0 - 0.6 * duck[i]) * fade;

		if !timeline.dropouts.is_empty() {
			music_gain *= dropout_gain(&timeline.dropouts, i as f32 / SR);
		}

		let ambience_gain = (1.0 - 0.6 * duck[i]) * fade;
		mix.push(
			voice[i] + effects[i] * 0.55 + music[i] * music_gain + ambience[i] * ambience_gain,
		);
	}

	Soundtrack {
		voice,
		effects,
		music,
		ambience,
		mix,
	}
}

/// Renders the full soundtrack and writes it as 48 kHz 16-bit stereo WAV.
pub fn write(timeline: &Timeline, path: &Path) -> Result<()> {
	super::check_length(timeline.duration)?;
	let Soundtrack { mix, .. } = render(timeline);
	let peak = mix.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
	let scale = if peak > 0.95 { 0.95 / peak } else { 1.0 };

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.with_context(|| format!("could not create {}", parent.display()))?;
	}

	let spec = hound::WavSpec {
		channels: 2,
		sample_rate: SAMPLE_RATE,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};
	let mut writer = hound::WavWriter::create(path, spec)
		.with_context(|| format!("could not create {}", path.display()))?;

	for sample in mix {
		let value = (sample * scale * f32::from(i16::MAX)) as i16;
		writer.write_sample(value)?;
		writer.write_sample(value)?;
	}

	writer
		.finalize()
		.context("could not finish the soundtrack")?;

	Ok(())
}

/// 0..1 "someone is talking" signal: fast attack, slow release.
fn ducking(voice: &[f32]) -> Vec<f32> {
	let attack = 1.0 / (SR * 0.05);
	let release = 1.0 / (SR * 0.5);
	let mut level = 0.0_f32;

	voice
		.iter()
		.map(|sample| {
			let target = if sample.abs() > 0.02 { 1.0 } else { 0.0 };
			level += (target - level)
				* if target > level {
					attack * 20.0
				} else {
					release
				};
			level = level.clamp(0.0, 1.0);
			level
		})
		.collect()
}
#[cfg(test)]
mod tests {
	use super::*;

	/// The largest sample-to-sample step in a stretch of samples: what a click would look like.
	fn biggest_step(samples: &[f32]) -> f32 {
		samples
			.windows(2)
			.map(|pair| (pair[1] - pair[0]).abs())
			.fold(0.0_f32, f32::max)
	}

	/// A timeline that plays one cue for the whole of it, for measuring a cue on its own.
	fn one_cue(cue: Music) -> Timeline {
		Timeline {
			shots: Vec::new(),
			speeches: Vec::new(),
			cues: Vec::new(),
			duration: 24.0,
			music: vec![crate::video::timeline::MusicCue {
				time: 0.0,
				music: cue,
			}],
			dropouts: Vec::new(),
			holds: Vec::new(),
			ambience: Vec::new(),
			foley: crate::video::foley::Foley::default(),
		}
	}

	#[test]
	fn every_sound_is_audible_and_safe() {
		for (name, sound) in Sound::ALL {
			let samples = sound.render();
			let peak = samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
			assert!(!samples.is_empty(), "{name} is empty");
			assert!(peak > 0.05, "{name} is too quiet ({peak})");
			assert!(peak < 2.0, "{name} clips hard ({peak})");
			assert_eq!(Sound::parse(name), Some(sound));
		}
	}

	#[test]
	fn the_new_effects_sit_at_the_old_ones_loudness() {
		let old: Vec<f32> = Sound::ALL
			.iter()
			.filter(|(_, sound)| !sound.is_new())
			.map(|(_, sound)| rms(&sound.render()))
			.collect();
		let mean = old.iter().sum::<f32>() / old.len() as f32;

		for (name, sound) in Sound::ALL {
			if !sound.is_new() {
				continue;
			}

			let here = rms(&sound.render());
			let db = 20.0 * (here / mean).log10();
			assert!(
				db.abs() < 3.0,
				"{name} is {db:.1} dB from the old effects' {mean:.3}"
			);
		}
	}

	#[test]
	fn plucked_strings_decay() {
		let note = pluck(440.0, 1.0, 0.5, 1);
		let early: f32 = note[..4800].iter().map(|s| s.abs()).sum();
		let late: f32 = note[note.len() - 4800..].iter().map(|s| s.abs()).sum();
		assert!(late < early * 0.6, "early {early}, late {late}");
	}

	#[test]
	fn ducking_rises_with_speech_and_falls_after() {
		let mut voice = vec![0.0; 48_000];
		voice.extend(vec![0.5; 48_000]);
		voice.extend(vec![0.0; 96_000]);
		let duck = ducking(&voice);
		assert!(duck[40_000] < 0.01);
		assert!(duck[90_000] > 0.9);
		assert!(duck[191_000] < 0.2);
	}

	#[test]
	fn every_cue_loops_at_a_steady_loudness_and_no_rougher_than_its_own_material() {
		let (play_bed, play_len) = play_loop();
		let play = rms(&play_bed[..play_len]);

		for (name, cue) in Music::ALL {
			let (bed, loop_len) = music_loop(cue);

			if cue == Music::None {
				assert!(bed.iter().all(|s| *s == 0.0), "{name} is silent");
				continue;
			}

			// The seam has to be a repeat, not a step: its jump is no bigger than the loop's
			// own sharpest attack, which is what a new strum is.
			let seam = (cue_sample(&bed, loop_len, loop_len - 1)
				- cue_sample(&bed, loop_len, loop_len))
			.abs();
			let own = biggest_step(&bed[..loop_len]);
			assert!(
				seam <= own * 1.2,
				"{name} jumps {seam} at its loop seam; its own sharpest step is {own}"
			);

			let here = rms(&bed[..loop_len]);
			let db = 20.0 * (here / play).log10();
			assert!(db.abs() < 1.0, "{name} is {db:.2} dB from play");
		}
	}

	/// The spectral centroid of a stretch of samples, by a Goertzel sweep of 24 bands from 80 Hz.
	fn centroid(samples: &[f32]) -> f32 {
		let mut energy = 0.0;
		let mut weighted = 0.0;

		for band in 0..24 {
			let freq = 80.0 * 2f32.powf(band as f32 / 4.0);
			let (mut real, mut imaginary) = (0.0_f32, 0.0_f32);

			for (index, sample) in samples.iter().enumerate() {
				let phase = TAU * freq * (index as f32 / SR);
				real += sample * phase.cos();
				imaginary += sample * phase.sin();
			}

			let power = real * real + imaginary * imaginary;
			energy += power;
			weighted += power * freq;
		}

		weighted / energy.max(1e-6)
	}

	/// The middle of a cue's first bar, clear of the loop's attack and its tail.
	fn colour_window(cue: Music) -> Vec<f32> {
		let (bed, loop_len) = music_loop(cue);
		let start = loop_len / 3;
		bed[start..start + 4096].to_vec()
	}

	#[test]
	fn every_cue_has_its_own_colour() {
		let play = centroid(&colour_window(Music::Play));
		let sleepy = centroid(&colour_window(Music::Sleepy));
		let tense = centroid(&colour_window(Music::Tense));
		let sad = centroid(&colour_window(Music::Sad));

		// The music box rings an octave or two above the others, so it is unmistakably brighter.
		assert!(
			sleepy > play * 1.15,
			"the music box is brighter than the ukulele: {sleepy:.0} vs {play:.0}"
		);
		assert!(
			sleepy > tense * 1.2 && sleepy > sad * 1.2,
			"and brighter than the low pulse and the single notes: {sleepy:.0} against {tense:.0} \
			 and {sad:.0}"
		);
	}

	#[test]
	fn a_cue_change_cross_fades_and_never_clicks() {
		let timeline = Timeline {
			music: vec![
				crate::video::timeline::MusicCue {
					time: 0.0,
					music: Music::Play,
				},
				crate::video::timeline::MusicCue {
					time: 12.0,
					music: Music::Tense,
				},
			],
			..one_cue(Music::Play)
		};
		let track = music_track(&timeline, len(timeline.duration));
		let change = len(12.0);

		// A click would be a step bigger than anything the music itself does; the change is no
		// rougher than the cue beside it.
		let own = biggest_step(&track[change - len(1.0)..change]);
		let at_change = biggest_step(&track[change - 32..change + 32]);
		assert!(
			at_change <= own * 1.2,
			"a click at the change: {at_change} against the music's own {own}"
		);

		// The old cue is still audible just before the change and the new one a second later.
		let before = rms(&track[change - len(1.0)..change]);
		let during = rms(&track[change..change + len(0.5)]);
		let after = rms(&track[change + len(1.0)..change + len(2.0)]);
		assert!(before > 0.001, "the old cue is playing: {before}");
		assert!(during > 0.001, "the cross-fade is not a hole: {during}");
		assert!(after > 0.001, "the new cue is playing: {after}");
	}

	#[test]
	fn a_dropout_empties_the_music_and_comes_back() {
		let spans = [Span {
			start: 2.0,
			end: 4.0,
		}];
		assert_eq!(dropout_gain(&spans, 1.0), 1.0);
		assert_eq!(dropout_gain(&spans, 3.0), 0.0);
		// A 30 ms ramp on each edge, so the cut is silent but never clicks.
		assert!(
			(dropout_gain(&spans, 1.985) - 0.5).abs() < 0.1,
			"the ramp's middle"
		);
		assert!(dropout_gain(&spans, 1.98) < 0.4);
		assert!(dropout_gain(&spans, 1.96) > 0.99, "before the ramp");
		assert!(dropout_gain(&spans, 4.02) < 0.4);
		assert!(dropout_gain(&spans, 4.05) > 0.99, "after the ramp");
		assert_eq!(dropout_gain(&spans, 5.0), 1.0);
	}

	#[test]
	fn a_book_that_names_no_music_and_no_light_plays_the_old_loop() {
		assert_eq!(Music::for_light(Preset::Day), Music::Play);
		assert_eq!(Music::parse("play"), Some(Music::Play));
		assert_eq!(MusicDefault::parse("auto"), Some(MusicDefault::Auto));
		assert_eq!(
			MusicDefault::parse("none"),
			Some(MusicDefault::Cue(Music::None))
		);
		assert!(MusicDefault::parse("loud").is_none());
	}

	#[test]
	fn every_bed_is_a_texture_with_a_faint_room() {
		for (name, bed) in Bed::ALL {
			let signal = bed_signal(bed, 4.0, 1);
			let here = rms(&signal);
			assert!((here - 1.0).abs() < 0.01, "{name} is normalised: {here}");
			assert!(
				signal.iter().all(|sample| sample.abs() < 20.0),
				"{name} is a wild signal"
			);
		}

		assert!(Bed::Room.level() < 0.2, "the room is near silence");
		assert_eq!(
			Bed::for_light(Preset::Storm, false),
			&[Bed::Rain, Bed::Wind]
		);
		assert_eq!(Bed::for_light(Preset::Night, true), &[]);
		assert_eq!(Bed::for_light(Preset::Moonlit, false), &[Bed::Crickets]);
	}
}
