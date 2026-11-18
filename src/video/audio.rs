//! The soundtrack: narration, a plucked-ukulele music bed and cartoon sound effects.
//!
//! Everything except the narrator's voice is synthesised here, so videos need no sample
//! libraries and carry no licensing questions. The music ducks under the narrator.

use std::f32::consts::TAU;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;

use crate::art::noise;
use crate::video::timeline::Timeline;
use crate::video::voice::SAMPLE_RATE;

const SR: f32 = SAMPLE_RATE as f32;

/// Built-in sound effects, usable from scripts with `@sfx name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl Sound {
	pub const ALL: [(&'static str, Sound); 9] = [
		("pop", Sound::Pop),
		("plop", Sound::Plop),
		("bonk", Sound::Bonk),
		("boing", Sound::Boing),
		("whoosh", Sound::Whoosh),
		("sparkle", Sound::Sparkle),
		("tweet", Sound::Tweet),
		("blorp", Sound::Blorp),
		("page", Sound::PageTurn),
	];

	pub fn parse(name: &str) -> Option<Sound> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, sound)| *sound)
	}

	/// Synthesises the effect as mono samples.
	pub fn render(self) -> Vec<f32> {
		match self {
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

/// Four bars of a bouncy I–vi–IV–V ukulele strum with a soft bass, ready to tile.
fn music_loop() -> (Vec<f32>, usize) {
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

/// Renders the full soundtrack and writes it as 48 kHz 16-bit stereo WAV.
pub fn write(timeline: &Timeline, path: &Path) -> Result<()> {
	super::check_length(timeline.duration)?;
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
		let rendered = cue.sound.render();
		mix_into(&mut effects, &rendered, cue.time);
	}

	effects.truncate(total);

	let (bed, loop_len) = music_loop();
	let duck = ducking(&voice);
	let fade_out = len(2.5);
	let mut mix = Vec::with_capacity(total);

	for i in 0..total {
		// Overlap-add the loop with the previous repeat's ringing tail so the seam is inaudible.
		let phase = i % loop_len;
		let mut music = bed[phase];

		if i >= loop_len {
			music += bed.get(phase + loop_len).copied().unwrap_or(0.0);
		}

		let fade = (i as f32 / SR).min(1.0) * ((total - i) as f32 / fade_out as f32).min(1.0);
		let music_gain = 0.2 * (1.0 - 0.6 * duck[i]) * fade;
		mix.push(voice[i] + effects[i] * 0.55 + music * music_gain);
	}

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

	#[test]
	fn every_sound_is_audible_and_safe() {
		for (name, sound) in Sound::ALL {
			let samples = sound.render();
			let peak = samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
			assert!(!samples.is_empty(), "{name} is empty");
			assert!(peak > 0.05, "{name} is too quiet ({peak})");
			assert!(peak < 1.5, "{name} clips hard ({peak})");
			assert_eq!(Sound::parse(name), Some(sound));
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
}
