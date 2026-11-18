//! Voices: text to speech for the narrator and for every character, cached on disk.
//!
//! A [`Voice`] is configuration: which provider speaks and as whom. The narrator's lives in the
//! script's `[voice]` table and each character's in their cast entry, so a character sounds the
//! same in every line of every book. Each line is spoken with a [`Delivery`] taken from the
//! character's mood at that moment, which providers turn into acting:
//!
//! | Provider | Offline | Acting | Lip-sync timing |
//! | --- | --- | --- | --- |
//! | `say` (macOS) | yes | speaking rate | from the audio |
//! | `command` (Piper, mlx-audio, anything) | yes | `{mood}` in the command | from the audio |
//! | `elevenlabs` | no | `[defiant]`-style audio tags | exact, per letter |
//! | `openai` | no | free-text acting instructions | from the audio |
//! | `cartesia` (Sonic) | no | an emotion and a speed | from the audio |
//! | `hume` (Octave) | no | free-text acting directions | from the audio |
//!
//! Everything is normalised to 48 kHz mono, and every take is cached by provider, voice, text
//! and delivery, so editing one line re-records one line.

use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use base64::Engine as _;
use serde::Deserialize;

use crate::art::figure::Mood;
use crate::art::seed_of;
use crate::video::lipsync;
use crate::video::lipsync::Timed;
use crate::video::lipsync::Track;

pub const SAMPLE_RATE: u32 = 48_000;
/// Mouth-movement frames per second derived from the audio.
pub const ENVELOPE_RATE: f32 = 60.0;

/// Who speaks a line and how. Used for the narrator (`[voice]` in a script) and for characters
/// (`[stinky.voice]` in the cast file).
#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Voice {
	/// `say` (default), `command`, `elevenlabs`, `openai`, `cartesia` or `hume`.
	#[serde(default = "default_provider")]
	pub provider: String,
	/// The voice name: a macOS voice for `say`, a built-in voice for `openai` (e.g. "marin").
	#[serde(default = "default_name")]
	pub name: String,
	/// Words per minute (`say`). Left out, a book's age band sets it, and with no band either
	/// the voice reads at [`Voice::rate`]'s 165, exactly as every book did before age bands.
	#[serde(default)]
	pub rate: Option<u32>,
	/// The external program for `command`, with `{text}`, `{out}`, `{mood}` placeholders.
	#[serde(default)]
	pub command: Option<Vec<String>>,
	/// The provider's voice id (`elevenlabs`, `cartesia`, `hume`).
	#[serde(default)]
	pub id: Option<String>,
	/// The provider's model, e.g. `eleven_v4` or `gpt-4o-mini-tts`.
	#[serde(default)]
	pub model: Option<String>,
	/// How this character talks, sent as acting direction (`openai`, `hume`), e.g.
	/// "A cheeky six-year-old boy, loud and bouncy, with a British accent".
	#[serde(default)]
	pub style: Option<String>,
}

/// The rate a voice reads at when nobody sets one: the pace every book had before age bands.
pub(crate) const RATE: u32 = 165;

impl std::fmt::Debug for Voice {
	/// Prints the effective rate, so a take's cache key is the same as it was when the rate was
	/// a plain number and no band was in play.
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Voice")
			.field("provider", &self.provider)
			.field("name", &self.name)
			.field("rate", &self.rate())
			.field("command", &self.command)
			.field("id", &self.id)
			.field("model", &self.model)
			.field("style", &self.style)
			.finish()
	}
}

fn default_provider() -> String {
	"say".to_owned()
}

fn default_name() -> String {
	"Daniel".to_owned()
}

impl Default for Voice {
	fn default() -> Self {
		Self {
			provider: default_provider(),
			name: default_name(),
			rate: None,
			command: None,
			id: None,
			model: None,
			style: None,
		}
	}
}

impl Voice {
	/// Words a minute this voice is asked to read at.
	pub fn rate(&self) -> u32 {
		self.rate.unwrap_or(RATE)
	}

	/// Checks the configuration up front, so a typo fails before any audio is made.
	pub fn validate(&self) -> Result<()> {
		match self.provider.as_str() {
			"say" | "openai" => Ok(()),
			"command" if self.command.as_ref().is_some_and(|c| !c.is_empty()) => Ok(()),
			"command" => {
				bail!("voice provider `command` needs `command = [\"program\", \"{{out}}\", …]`")
			}
			"elevenlabs" | "cartesia" | "hume" if self.id.is_some() => Ok(()),
			provider @ ("elevenlabs" | "cartesia" | "hume") => {
				bail!("voice provider `{provider}` needs `id` (a voice id from its voice library)")
			}
			other => {
				bail!(
					"unknown voice provider `{other}`; use one of: {}",
					PROVIDERS.join(", ")
				)
			}
		}
	}
}

/// Every voice provider, by its `provider` name.
pub const PROVIDERS: [&str; 6] = ["say", "command", "elevenlabs", "openai", "cartesia", "hume"];

/// How a line should be delivered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Delivery {
	/// The speaker's mood; `None` is the narrator's neutral storytelling voice.
	pub mood: Option<Mood>,
	/// The line is shouted (`**WORDS**` in the script).
	pub shout: bool,
}

impl Delivery {
	/// An audio tag (for tag-driven models) and a plain-English direction for this delivery.
	fn direction(self) -> Option<(&'static str, &'static str)> {
		if self.shout {
			return Some(("shouting", "shouting it at the top of their voice"));
		}

		Some(match self.mood? {
			Mood::Happy => ("cheerful", "cheerfully"),
			Mood::Grin => ("playful", "playfully, with a big grin"),
			Mood::Laugh => ("laughs", "laughing as they say it"),
			Mood::Sad => ("sad", "sadly, close to tears"),
			Mood::Grumpy => ("grumpy", "grumpily"),
			Mood::Cross => ("angry", "crossly, losing patience"),
			Mood::Defiant => ("defiant", "defiantly, like a cheeky child refusing"),
			Mood::Shocked => ("gasps", "in total shock"),
			Mood::Disgusted => ("disgusted", "with disgust, holding their nose"),
			Mood::Fainted => ("weakly", "weakly, completely dazed"),
			Mood::Dizzy => ("dazed", "dizzily"),
			Mood::Sleepy => ("sleepy", "sleepily, mid-yawn"),
			Mood::Dreamy => ("dreamy", "dreamily, delighted"),
			Mood::Worried => ("nervous", "nervously"),
			Mood::Proud => ("proud", "proudly"),
		})
	}

	pub(crate) fn rate_scale(self) -> f32 {
		if self.shout {
			return 1.08;
		}

		match self.mood {
			Some(Mood::Sad | Mood::Sleepy | Mood::Dreamy | Mood::Fainted) => 0.85,
			Some(Mood::Cross | Mood::Shocked | Mood::Laugh | Mood::Worried) => 1.1,
			_ => 1.0,
		}
	}
}

/// One line to speak.
#[derive(Clone, Copy, Debug)]
pub struct Utterance<'a> {
	/// Plain text, markup removed.
	pub text: &'a str,
	pub voice: &'a Voice,
	pub delivery: Delivery,
}

/// One spoken line.
#[derive(Clone, Debug)]
pub struct Speech {
	pub text: String,
	/// Mono samples at [`SAMPLE_RATE`].
	pub samples: Vec<f32>,
	/// Loudness at [`ENVELOPE_RATE`], scaled so speech peaks near 1.
	pub envelope: Vec<f32>,
	/// Mouth shapes over the line.
	pub track: Track,
}

impl Speech {
	pub fn duration(&self) -> f32 {
		self.samples.len() as f32 / SAMPLE_RATE as f32
	}

	/// How open a speaking mouth is at `t` seconds into the line.
	pub fn mouth(&self, t: f32) -> f32 {
		if t < 0.0 {
			return 0.0;
		}

		let index = (t * ENVELOPE_RATE) as usize;
		self.envelope.get(index).copied().unwrap_or(0.0)
	}
}

/// The key a take is cached under: the voice, how the line is delivered, and the words.
///
/// This is the exact string cached takes were first written under, so it is deliberately kept
/// byte for byte: changing it would re-record every line of every book, and adding a field to
/// [`Voice`] means deciding here whether old takes stay valid.
pub fn cache_key(voice: &Voice, delivery: Delivery, text: &str) -> String {
	format!("{voice:?}|{delivery:?}|{text}")
}

/// Speaks a line, reusing a cached take when nothing about it has changed.
pub fn speak(utterance: &Utterance<'_>, cache: &Path) -> Result<Speech> {
	std::fs::create_dir_all(cache)
		.with_context(|| format!("could not create {}", cache.display()))?;

	let voice = utterance.voice;
	let key = cache_key(voice, utterance.delivery, utterance.text);
	let stem = cache.join(format!("{:016x}", seed_of(&key)));
	let wav = stem.with_extension("wav");
	let alignment_file = stem.with_extension("align.json");

	if !wav.is_file() {
		let take = record(utterance, &stem)?;
		normalise(&take.audio, &wav)?;
		let _ = std::fs::remove_file(&take.audio);

		if let Some(alignment) = take.alignment {
			std::fs::write(&alignment_file, serde_json::to_vec(&alignment)?)
				.with_context(|| format!("could not write {}", alignment_file.display()))?;
		}
	}

	let (samples, trimmed) = read_wav(&wav)?;
	let envelope = envelope(&samples);
	let track = match std::fs::read(&alignment_file) {
		Ok(bytes) => {
			let mut alignment: Vec<Timed> =
				serde_json::from_slice(&bytes).context("cached alignment is corrupt")?;

			// Leading silence was trimmed from the audio, so shift the timings to match.
			for letter in &mut alignment {
				letter.start -= trimmed;
				letter.end -= trimmed;
			}

			lipsync::from_alignment(&alignment)
		}
		Err(_) => lipsync::from_envelope(utterance.text, &envelope, ENVELOPE_RATE),
	};

	Ok(Speech {
		text: utterance.text.to_owned(),
		samples,
		envelope,
		track,
	})
}

/// Raw provider output before normalisation.
struct Take {
	audio: PathBuf,
	alignment: Option<Vec<Timed>>,
}

fn record(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let voice = utterance.voice;
	voice.validate()?;

	match voice.provider.as_str() {
		"say" => say(utterance, stem),
		"command" => run_command(utterance, stem),
		"elevenlabs" => elevenlabs(utterance, stem),
		"openai" => openai(utterance, stem),
		"cartesia" => cartesia(utterance, stem),
		"hume" => hume(utterance, stem),
		other => bail!("unknown voice provider `{other}`"),
	}
}

fn say(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let audio = stem.with_extension("aiff");
	let text_file = stem.with_extension("txt");
	let rate = (utterance.voice.rate() as f32 * utterance.delivery.rate_scale()).round() as u32;
	std::fs::write(&text_file, utterance.text)
		.with_context(|| format!("could not write {}", text_file.display()))?;

	let output = Command::new("say")
		.arg("-v")
		.arg(&utterance.voice.name)
		.arg("-r")
		.arg(rate.to_string())
		.arg("-o")
		.arg(&audio)
		.arg("-f")
		.arg(&text_file)
		.output()
		.context("could not run `say`; on other systems use another voice provider")?;
	let _ = std::fs::remove_file(&text_file);

	if !output.status.success() {
		bail!(
			"`say` failed for voice `{}`: {}",
			utterance.voice.name,
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}

	Ok(Take {
		audio,
		alignment: None,
	})
}

fn run_command(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let template = utterance.voice.command.as_deref().unwrap_or_default();
	let Some((program, args)) = template.split_first() else {
		bail!("`command` voice has no program");
	};

	let out = stem.with_extension("raw-tts");
	let out_str = out.to_string_lossy();
	let mood = utterance
		.delivery
		.direction()
		.map_or("neutral", |(tag, _)| tag);
	let uses_text = template.iter().any(|arg| arg.contains("{text}"));
	let args: Vec<String> = args
		.iter()
		.map(|arg| {
			arg.replace("{out}", &out_str)
				.replace("{text}", utterance.text)
				.replace("{mood}", mood)
				.replace("{voice}", &utterance.voice.name)
		})
		.collect();
	let mut child = Command::new(program)
		.args(&args)
		.stdin(if uses_text {
			Stdio::null()
		} else {
			Stdio::piped()
		})
		.stdout(Stdio::null())
		.stderr(Stdio::piped())
		.spawn()
		.with_context(|| format!("could not run TTS command `{program}`"))?;

	if !uses_text && let Some(mut stdin) = child.stdin.take() {
		stdin
			.write_all(utterance.text.as_bytes())
			.context("could not send text to the TTS command")?;
	}

	let output = child
		.wait_with_output()
		.context("TTS command did not finish")?;

	if !output.status.success() {
		bail!(
			"TTS command `{program}` failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}

	if !out.is_file() {
		bail!(
			"TTS command `{program}` did not write {}; include `{{out}}` in its arguments",
			out.display()
		);
	}

	Ok(Take {
		audio: out,
		alignment: None,
	})
}

fn api_key(variable: &str) -> Result<String> {
	std::env::var(variable).with_context(|| {
		format!(
			"{variable} is not set; set it in the environment for this run, e.g. `{variable}=… \
			 fables video …`"
		)
	})
}

/// An HTTP agent that hands error responses back instead of failing, so the provider's own
/// message reaches the user.
fn agent() -> ureq::Agent {
	ureq::Agent::config_builder()
		.http_status_as_error(false)
		.build()
		.into()
}

fn check_status(provider: &str, response: &mut ureq::http::Response<ureq::Body>) -> Result<()> {
	let status = response.status();

	if status.is_success() {
		return Ok(());
	}

	let body = response.body_mut().read_to_string().unwrap_or_default();

	// No retries: a failing paid API call is reported once and never looped.
	bail!(
		"{provider} refused the request ({status}): {}",
		body.chars().take(400).collect::<String>()
	)
}

/// The text with the delivery as a leading audio tag, the way `ElevenLabs` v3/v4 models take
/// direction: `[defiant] Nope!`.
pub fn elevenlabs_text(utterance: &Utterance<'_>) -> String {
	match utterance.delivery.direction() {
		Some((tag, _)) => format!("[{tag}] {}", utterance.text),
		None => utterance.text.to_owned(),
	}
}

#[derive(Deserialize)]
struct ElevenLabsReply {
	audio_base64: String,
	#[serde(default)]
	alignment: Option<ElevenLabsAlignment>,
	#[serde(default)]
	normalized_alignment: Option<ElevenLabsAlignment>,
}

#[derive(Deserialize)]
struct ElevenLabsAlignment {
	characters: Vec<String>,
	character_start_times_seconds: Vec<f32>,
	character_end_times_seconds: Vec<f32>,
}

impl ElevenLabsAlignment {
	fn timed(&self) -> Vec<Timed> {
		self.characters
			.iter()
			.zip(&self.character_start_times_seconds)
			.zip(&self.character_end_times_seconds)
			.filter_map(|((ch, start), end)| {
				Some(Timed {
					ch: ch.chars().next()?,
					start: *start,
					end: *end,
				})
			})
			.collect()
	}
}

fn elevenlabs(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let key = api_key("ELEVENLABS_API_KEY")?;
	let voice_id = utterance.voice.id.as_deref().unwrap_or_default();
	let model = utterance.voice.model.as_deref().unwrap_or("eleven_v4");
	let url = format!(
		"https://api.elevenlabs.io/v1/text-to-speech/{voice_id}/with-timestamps?output_format=mp3_44100_128"
	);
	let body = serde_json::json!({ "text": elevenlabs_text(utterance), "model_id": model });
	let mut response = agent()
		.post(&url)
		.header("xi-api-key", &key)
		.send_json(&body)
		.context("could not reach ElevenLabs")?;
	check_status("ElevenLabs", &mut response)?;

	let reply: ElevenLabsReply = response
		.body_mut()
		.read_json()
		.context("ElevenLabs sent an unexpected reply")?;
	let audio = stem.with_extension("mp3");
	let bytes = base64::engine::general_purpose::STANDARD
		.decode(reply.audio_base64)
		.context("ElevenLabs audio was not valid base64")?;
	std::fs::write(&audio, bytes)
		.with_context(|| format!("could not write {}", audio.display()))?;

	let alignment = reply
		.normalized_alignment
		.or(reply.alignment)
		.map(|a| a.timed());

	Ok(Take { audio, alignment })
}

/// Acting direction for `OpenAI`'s instruction-following TTS.
pub fn openai_instructions(utterance: &Utterance<'_>) -> String {
	let style =
		utterance.voice.style.as_deref().unwrap_or(
			"A warm, playful storyteller reading a picture book aloud to a young child.",
		);

	match utterance.delivery.direction() {
		Some((_, how)) => format!("{style} Say this line {how}."),
		None => style.to_owned(),
	}
}

fn openai(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let key = api_key("OPENAI_API_KEY")?;
	let body = serde_json::json!({
		"model": utterance.voice.model.as_deref().unwrap_or("gpt-4o-mini-tts"),
		"voice": utterance.voice.name,
		"input": utterance.text,
		"instructions": openai_instructions(utterance),
		"response_format": "wav",
	});
	let mut response = agent()
		.post("https://api.openai.com/v1/audio/speech")
		.header("Authorization", &format!("Bearer {key}"))
		.send_json(&body)
		.context("could not reach OpenAI")?;
	check_status("OpenAI", &mut response)?;

	write_audio("OpenAI", &mut response, &stem.with_extension("openai.wav"))
}

/// Cartesia Sonic's emotion for a delivery, from the emotions every Sonic 3 voice takes.
fn cartesia_emotion(delivery: Delivery) -> &'static str {
	if delivery.shout {
		return "angry";
	}

	match delivery.mood {
		None => "neutral",
		Some(Mood::Happy | Mood::Grin | Mood::Laugh | Mood::Proud | Mood::Dreamy) => "content",
		Some(Mood::Sleepy) => "calm",
		Some(Mood::Sad | Mood::Fainted | Mood::Dizzy) => "sad",
		Some(Mood::Cross | Mood::Grumpy | Mood::Defiant | Mood::Disgusted) => "angry",
		Some(Mood::Shocked | Mood::Worried) => "scared",
	}
}

/// The request body for Cartesia's bytes endpoint: WAV at the sample rate fables works in.
pub fn cartesia_body(utterance: &Utterance<'_>) -> serde_json::Value {
	serde_json::json!({
		"model_id": utterance.voice.model.as_deref().unwrap_or("sonic-3.6"),
		"transcript": utterance.text,
		"voice": { "id": utterance.voice.id.as_deref().unwrap_or_default() },
		"language": "en",
		"output_format": { "container": "wav", "encoding": "pcm_s16le", "sample_rate": SAMPLE_RATE },
		"generation_config": {
			"speed": utterance.delivery.rate_scale().clamp(0.6, 1.5),
			"emotion": cartesia_emotion(utterance.delivery),
		},
	})
}

fn cartesia(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let key = api_key("CARTESIA_API_KEY")?;
	let mut response = agent()
		.post("https://api.cartesia.ai/tts/bytes")
		.header("Authorization", &format!("Bearer {key}"))
		.header("Cartesia-Version", "2026-08-14")
		.send_json(cartesia_body(utterance))
		.context("could not reach Cartesia")?;
	check_status("Cartesia", &mut response)?;
	write_audio(
		"Cartesia",
		&mut response,
		&stem.with_extension("cartesia.wav"),
	)
}

/// The request body for Hume's Octave file endpoint. The character's style and the line's
/// delivery become its acting directions, kept short as Hume recommends.
pub fn hume_body(utterance: &Utterance<'_>) -> serde_json::Value {
	let how = utterance.delivery.direction().map(|(_, how)| how);
	let description = match (utterance.voice.style.as_deref(), how) {
		(Some(style), Some(how)) => format!("{style}; {how}"),
		(Some(style), None) => style.to_owned(),
		(None, Some(how)) => format!("Storytelling, {how}"),
		(None, None) => "A warm storyteller reading aloud to a young child".to_owned(),
	};
	let description: String = description.chars().take(100).collect();

	serde_json::json!({
		"utterances": [{
			"text": utterance.text,
			"description": description,
			"voice": { "id": utterance.voice.id.as_deref().unwrap_or_default() },
		}],
		"format": { "type": "wav" },
		"version": 2,
		"num_generations": 1,
	})
}

fn hume(utterance: &Utterance<'_>, stem: &Path) -> Result<Take> {
	let key = api_key("HUME_API_KEY")?;
	let mut response = agent()
		.post("https://api.hume.ai/v0/tts/file")
		.header("X-Hume-Api-Key", &key)
		.send_json(hume_body(utterance))
		.context("could not reach Hume")?;
	check_status("Hume", &mut response)?;
	write_audio("Hume", &mut response, &stem.with_extension("hume.wav"))
}

/// Saves a provider's audio reply.
fn write_audio(
	provider: &str,
	response: &mut ureq::http::Response<ureq::Body>,
	audio: &Path,
) -> Result<Take> {
	let bytes = response
		.body_mut()
		.with_config()
		.limit(64 * 1024 * 1024)
		.read_to_vec()
		.with_context(|| format!("could not read {provider} audio"))?;
	std::fs::write(audio, bytes).with_context(|| format!("could not write {}", audio.display()))?;

	Ok(Take {
		audio: audio.to_path_buf(),
		alignment: None,
	})
}

fn normalise(input: &Path, output: &Path) -> Result<()> {
	let status = Command::new("ffmpeg")
		.args(["-y", "-loglevel", "error", "-i"])
		.arg(input)
		.args([
			"-ac",
			"1",
			"-ar",
			&SAMPLE_RATE.to_string(),
			"-sample_fmt",
			"s16",
		])
		.arg(output)
		// Never on our stdout: under `fables mcp` that is the protocol's channel.
		.stdout(Stdio::null())
		.status()
		.context("could not run ffmpeg; install it to make videos")?;

	if !status.success() {
		bail!("ffmpeg could not convert {}", input.display());
	}

	Ok(())
}

/// Reads a 16-bit mono WAV into floats with leading and trailing silence trimmed. Also returns
/// how many seconds were trimmed from the start, so timings can be shifted to match.
pub fn read_wav(path: &Path) -> Result<(Vec<f32>, f32)> {
	let mut reader = hound::WavReader::open(path)
		.with_context(|| format!("could not open {}", path.display()))?;
	let spec = reader.spec();

	if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE || spec.bits_per_sample != 16 {
		bail!("{} is not 48 kHz 16-bit mono", path.display());
	}

	let samples = reader
		.samples::<i16>()
		.map(|sample| sample.map(|value| f32::from(value) / f32::from(i16::MAX)))
		.collect::<std::result::Result<Vec<f32>, _>>()
		.with_context(|| format!("could not read samples from {}", path.display()))?;
	let (start, end) = speech_bounds(&samples);

	Ok((
		samples[start..end].to_vec(),
		start as f32 / SAMPLE_RATE as f32,
	))
}

/// Where speech starts and ends, keeping a little padding, so pauses between lines are ours.
fn speech_bounds(samples: &[f32]) -> (usize, usize) {
	let threshold = 0.01;
	let start = samples
		.iter()
		.position(|s| s.abs() > threshold)
		.unwrap_or(0);
	let end = samples
		.iter()
		.rposition(|s| s.abs() > threshold)
		.map_or(samples.len(), |i| i + 1);
	let pad = (SAMPLE_RATE as usize) / 50;
	let start = start.saturating_sub(pad);
	let end = (end + pad).min(samples.len());

	if start >= end {
		(0, samples.len())
	} else {
		(start, end)
	}
}

/// RMS loudness per mouth frame, normalised and smoothed so lips move with syllables.
fn envelope(samples: &[f32]) -> Vec<f32> {
	let window = (SAMPLE_RATE as f32 / ENVELOPE_RATE) as usize;
	let rms: Vec<f32> = samples
		.chunks(window.max(1))
		.map(|chunk| (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt())
		.collect();
	let peak = rms.iter().copied().fold(0.0_f32, f32::max).max(1e-4);
	let mut smoothed = Vec::with_capacity(rms.len());
	let mut level = 0.0;

	for value in rms {
		let target = (value / peak * 1.6).min(1.0);
		// Mouths snap open quickly and close a little slower, like real speech.
		let rate = if target > level { 0.6 } else { 0.35 };
		level += (target - level) * rate;
		smoothed.push(level);
	}

	smoothed
}

#[cfg(test)]
mod tests {
	use super::*;

	fn voice(provider: &str) -> Voice {
		Voice {
			provider: provider.into(),
			id: Some("voice-123".into()),
			style: Some("A bouncy six-year-old".into()),
			..Voice::default()
		}
	}

	#[test]
	fn cartesia_gets_an_emotion_and_wav_at_our_rate() {
		let voice = voice("cartesia");
		let body = cartesia_body(&Utterance {
			text: "Nope!",
			voice: &voice,
			delivery: Delivery {
				mood: Some(Mood::Defiant),
				shout: false,
			},
		});
		assert_eq!(body["voice"]["id"], "voice-123");
		assert_eq!(body["generation_config"]["emotion"], "angry");
		assert_eq!(body["output_format"]["container"], "wav");
		assert_eq!(body["output_format"]["sample_rate"], SAMPLE_RATE);
		assert_eq!(body["model_id"], "sonic-3.6");
	}

	#[test]
	fn hume_gets_short_acting_directions() {
		let voice = voice("hume");
		let body = hume_body(&Utterance {
			text: "Brush your teeth!",
			voice: &voice,
			delivery: Delivery {
				mood: Some(Mood::Cross),
				shout: true,
			},
		});
		let line = &body["utterances"][0];
		assert_eq!(line["voice"]["id"], "voice-123");
		let description = line["description"].as_str().unwrap();
		assert!(
			description.starts_with("A bouncy six-year-old; shouting"),
			"{description}"
		);
		assert!(description.chars().count() <= 100);
		assert_eq!(body["format"]["type"], "wav");
	}

	#[test]
	fn library_voices_need_an_id() {
		for provider in ["elevenlabs", "cartesia", "hume"] {
			let mut voice = voice(provider);
			assert!(voice.validate().is_ok());
			voice.id = None;
			let error = voice.validate().unwrap_err().to_string();
			assert!(error.contains("needs `id`"), "{provider}: {error}");
		}
	}

	#[test]
	fn trims_silence_but_keeps_a_little_padding() {
		let mut samples = vec![0.0; 48_000];
		samples.extend(std::iter::repeat_n(0.5, 4800));
		samples.extend(vec![0.0; 48_000]);
		let (start, end) = speech_bounds(&samples);
		assert!(
			end - start > 4800 && end - start < 4800 + 2000,
			"{}",
			end - start
		);
		assert_eq!(start, 48_000 - 960);
	}

	#[test]
	fn envelope_follows_loudness() {
		let mut samples = vec![0.0; 4800];
		samples.extend((0..9600).map(|i| (i as f32 * 0.05).sin() * 0.8));
		let env = envelope(&samples);
		assert!(env[2] < 0.05);
		assert!(env.iter().copied().fold(0.0, f32::max) > 0.8);
	}

	#[test]
	fn mouth_is_closed_outside_the_line() {
		let speech = Speech {
			text: String::new(),
			samples: vec![0.0; 480],
			envelope: vec![0.5; 6],
			track: Track::default(),
		};
		assert!(speech.mouth(-1.0).abs() < f32::EPSILON);
		assert!(speech.mouth(10.0).abs() < f32::EPSILON);
		assert!((speech.mouth(0.01) - 0.5).abs() < f32::EPSILON);
	}

	#[test]
	fn moods_become_audio_tags_and_directions() {
		let voice = Voice {
			style: Some("A cheeky six-year-old.".into()),
			..Voice::default()
		};
		let line = Utterance {
			text: "Nope!",
			voice: &voice,
			delivery: Delivery {
				mood: Some(Mood::Defiant),
				shout: false,
			},
		};
		assert_eq!(elevenlabs_text(&line), "[defiant] Nope!");
		assert_eq!(
			openai_instructions(&line),
			"A cheeky six-year-old. Say this line defiantly, like a cheeky child refusing."
		);

		let narrator = Utterance {
			delivery: Delivery::default(),
			..line
		};
		assert_eq!(elevenlabs_text(&narrator), "Nope!");

		let shouted = Utterance {
			delivery: Delivery {
				mood: Some(Mood::Happy),
				shout: true,
			},
			..line
		};
		assert_eq!(elevenlabs_text(&shouted), "[shouting] Nope!");
	}

	#[test]
	fn elevenlabs_alignment_becomes_timed_letters() {
		let reply: ElevenLabsReply = serde_json::from_str(
			r#"{"audio_base64":"","alignment":{"characters":["H","i"],"character_start_times_seconds":[0.0,0.1],"character_end_times_seconds":[0.1,0.25]}}"#,
		)
		.unwrap();
		let timed = reply.alignment.unwrap().timed();
		assert_eq!(
			timed,
			vec![
				Timed {
					ch: 'H',
					start: 0.0,
					end: 0.1
				},
				Timed {
					ch: 'i',
					start: 0.1,
					end: 0.25
				}
			]
		);
	}

	#[test]
	fn a_take_is_cached_under_the_key_it_was_first_written_with() {
		let voice = Voice::default();
		let delivery = Delivery {
			mood: Some(Mood::Cross),
			shout: false,
		};

		assert_eq!(
			cache_key(&voice, delivery, "Stop that."),
			r#"Voice { provider: "say", name: "Daniel", rate: 165, command: None, id: None, model: None, style: None }|Delivery { mood: Some(Cross), shout: false }|Stop that."#
		);
	}

	#[test]
	fn an_unset_rate_reads_at_the_old_speed_and_keeps_the_cache_key() {
		let voice = Voice::default();
		assert_eq!(voice.rate(), 165);
		// The take cache is keyed by this debug string, so an unset rate must print exactly the
		// number the field held before age bands existed.
		assert_eq!(
			format!("{voice:?}"),
			"Voice { provider: \"say\", name: \"Daniel\", rate: 165, command: None, id: None, \
			 model: None, style: None }"
		);

		let explicit: Voice = toml::from_str("name = \"Daniel\"\nrate = 158\n").unwrap();
		assert_eq!(explicit.rate(), 158);
		assert!(format!("{explicit:?}").contains("rate: 158"));

		let unset: Voice = toml::from_str("name = \"Moira\"\n").unwrap();
		assert_eq!(unset.rate, None);
		assert_eq!(unset.rate(), 165);
	}

	#[test]
	fn voices_are_validated() {
		assert!(Voice::default().validate().is_ok());
		let eleven = Voice {
			provider: "elevenlabs".into(),
			..Voice::default()
		};
		assert!(format!("{:#}", eleven.validate().unwrap_err()).contains("needs `id`"));
		let typo = Voice {
			provider: "elevenlab".into(),
			..Voice::default()
		};
		assert!(typo.validate().is_err());
	}
}
