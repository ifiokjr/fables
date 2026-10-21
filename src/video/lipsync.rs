//! Lip-sync: which mouth shape a speaking character makes at each moment.
//!
//! Cartoon lip-sync does not need real phonemes. Animators work from a chart of nine mouth
//! shapes (see [`Viseme`]) and a rough idea of which sounds come when. This module gets that
//! from the line's text and its audio:
//!
//! - With **alignment** (per-character timings, which some TTS APIs return), each letter's
//!   shape lands exactly when it is spoken.
//! - Without it, the letters are paced by *voiced* time: the mouth only advances through the
//!   word while there is sound, and rests in the pauses, so commas and full stops line up.

use serde::Deserialize;
use serde::Serialize;

use crate::art::figure::Viseme;

/// One letter of a line with when it is spoken, from a TTS provider's alignment data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timed {
	pub ch: char,
	pub start: f32,
	pub end: f32,
}

/// A mouth shape held from `start` (seconds into the line) until the next one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
	pub start: f32,
	pub viseme: Viseme,
}

/// Mouth shapes for a whole spoken line, sorted by time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
	pub cues: Vec<Cue>,
}

impl Track {
	/// The shape at `t` seconds into the line.
	pub fn at(&self, t: f32) -> Viseme {
		self.cues
			.iter()
			.rev()
			.find(|cue| cue.start <= t)
			.map_or(Viseme::Rest, |cue| cue.viseme)
	}
}

/// Mouth shapes for each letter of `text`, with how long each should be held relative to the
/// others. Digraphs (th, sh, oo, ee…) count as one sound.
pub fn letters(text: &str) -> Vec<(Viseme, f32)> {
	let lower: Vec<char> = text.to_lowercase().chars().collect();
	let mut out: Vec<(Viseme, f32)> = Vec::with_capacity(lower.len());
	let mut i = 0;

	while i < lower.len() {
		let here = lower[i];
		let next = lower.get(i + 1).copied().unwrap_or(' ');
		let pair = [here, next];
		let (viseme, weight, used) = match pair {
			['t', 'h'] => (Viseme::Tongue, 1.0, 2),
			['s' | 'c', 'h'] => (Viseme::Teeth, 1.0, 2),
			['p', 'h'] => (Viseme::Fv, 1.0, 2),
			['o', 'o'] => (Viseme::Pucker, 1.5, 2),
			['e', 'e' | 'a'] => (Viseme::Teeth, 1.4, 2),
			['o', 'u' | 'w'] => (Viseme::Round, 1.5, 2),
			['a', 'i' | 'y'] => (Viseme::Open, 1.4, 2),
			_ => {
				let (viseme, weight) = letter(here);
				(viseme, weight, 1)
			}
		};

		if weight > 0.0 {
			match out.last_mut() {
				Some(last) if last.0 == viseme => last.1 += weight,
				_ => out.push((viseme, weight)),
			}
		}

		i += used;
	}

	out
}

fn letter(ch: char) -> (Viseme, f32) {
	match ch {
		'm' | 'b' | 'p' => (Viseme::Mbp, 0.9),
		'f' | 'v' => (Viseme::Fv, 0.9),
		'l' => (Viseme::Tongue, 0.8),
		'w' | 'q' => (Viseme::Pucker, 0.9),
		'o' => (Viseme::Round, 1.3),
		'a' => (Viseme::Wide, 1.4),
		'u' | 'e' | 'i' | 'y' => (Viseme::Open, 1.1),
		'r' => (Viseme::Round, 0.7),
		ch if ch.is_alphabetic() => (Viseme::Teeth, 0.7),
		// Punctuation and spaces take no time of their own: pauses come from the audio.
		_ => (Viseme::Rest, 0.0),
	}
}

/// Builds a track from exact per-letter timings.
pub fn from_alignment(timed: &[Timed]) -> Track {
	let mut cues: Vec<Cue> = Vec::with_capacity(timed.len());
	let mut previous_end = 0.0;

	for (i, letter_time) in timed.iter().enumerate() {
		if letter_time.start - previous_end > 0.12 {
			cues.push(Cue {
				start: previous_end,
				viseme: Viseme::Rest,
			});
		}

		// Look one letter ahead so digraphs like "th" read as one shape.
		let pair: String = timed[i..(i + 2).min(timed.len())]
			.iter()
			.map(|t| t.ch)
			.collect();
		let viseme = letters(&pair)
			.first()
			.map_or(Viseme::Rest, |(viseme, _)| *viseme);

		if viseme != Viseme::Rest && cues.last().is_none_or(|cue| cue.viseme != viseme) {
			cues.push(Cue {
				start: letter_time.start,
				viseme,
			});
		}

		previous_end = letter_time.end;
	}

	cues.push(Cue {
		start: previous_end,
		viseme: Viseme::Rest,
	});

	Track { cues }
}

/// Builds a track by pacing the text's sounds through the voiced parts of the audio.
///
/// `envelope` is loudness at `rate` frames per second (0..1).
pub fn from_envelope(text: &str, envelope: &[f32], rate: f32) -> Track {
	let sounds = letters(text);
	let total_weight: f32 = sounds.iter().map(|(_, weight)| weight).sum();
	let voiced: Vec<bool> = envelope.iter().map(|level| *level > 0.12).collect();
	let voiced_frames = voiced.iter().filter(|v| **v).count().max(1) as f32;

	if sounds.is_empty() || total_weight <= 0.0 {
		return Track::default();
	}

	let mut cues: Vec<Cue> = Vec::new();
	let mut spoken = 0.0;
	let mut silent_run = 0;

	for (frame, is_voiced) in voiced.iter().enumerate() {
		let time = frame as f32 / rate;

		if !is_voiced {
			silent_run += 1;

			// Short dips between syllables keep the shape; real pauses close the mouth.
			if silent_run == 5 {
				push(&mut cues, time - 4.0 / rate, Viseme::Rest);
			}

			continue;
		}

		silent_run = 0;
		let progress = spoken / voiced_frames * total_weight;
		spoken += 1.0;
		let mut cumulative = 0.0;
		let viseme = sounds
			.iter()
			.find(|(_, weight)| {
				cumulative += weight;
				cumulative > progress
			})
			.map_or(Viseme::Rest, |(viseme, _)| *viseme);
		push(&mut cues, time, viseme);
	}

	push(&mut cues, voiced.len() as f32 / rate, Viseme::Rest);

	Track { cues }
}

fn push(cues: &mut Vec<Cue>, start: f32, viseme: Viseme) {
	if cues.last().is_some_and(|cue| cue.viseme == viseme) {
		return;
	}

	cues.push(Cue {
		start: start.max(0.0),
		viseme,
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn words_become_mouth_shapes() {
		let shapes: Vec<Viseme> = letters("Mum").into_iter().map(|(v, _)| v).collect();
		assert_eq!(shapes, vec![Viseme::Mbp, Viseme::Open, Viseme::Mbp]);

		let shapes: Vec<Viseme> = letters("the moon").into_iter().map(|(v, _)| v).collect();
		assert_eq!(
			shapes,
			vec![
				Viseme::Tongue,
				Viseme::Open,
				Viseme::Mbp,
				Viseme::Pucker,
				Viseme::Teeth
			]
		);
	}

	#[test]
	fn punctuation_takes_no_time() {
		let with: f32 = letters("No, no!").iter().map(|(_, w)| w).sum();
		let without: f32 = letters("Nono").iter().map(|(_, w)| w).sum();
		assert!((with - without).abs() < 1e-5);
	}

	#[test]
	fn mouths_rest_in_pauses_and_move_while_voiced() {
		// Voiced, a long pause, voiced again.
		let mut envelope = vec![0.8; 30];
		envelope.extend(vec![0.0; 30]);
		envelope.extend(vec![0.8; 30]);
		let track = from_envelope("Hello. Hello.", &envelope, 60.0);
		assert_ne!(track.at(0.2), Viseme::Rest);
		assert_eq!(
			track.at(0.75),
			Viseme::Rest,
			"the mouth closes during the pause"
		);
		assert_ne!(track.at(1.2), Viseme::Rest);
		assert_eq!(track.at(2.0), Viseme::Rest, "and after the line");
	}

	#[test]
	fn alignment_places_shapes_exactly() {
		let timed: Vec<Timed> = "mama"
			.chars()
			.enumerate()
			.map(|(i, ch)| {
				Timed {
					ch,
					start: i as f32 * 0.1,
					end: i as f32 * 0.1 + 0.1,
				}
			})
			.collect();
		let track = from_alignment(&timed);
		assert_eq!(track.at(0.05), Viseme::Mbp);
		assert_eq!(track.at(0.15), Viseme::Wide);
		assert_eq!(track.at(0.25), Viseme::Mbp);
		assert_eq!(track.at(0.5), Viseme::Rest);
	}
}
