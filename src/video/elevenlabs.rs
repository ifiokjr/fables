//! `ElevenLabs`: character voices, sound effects, ambience, the voice library and voice design.
//!
//! Every call fables makes to `ElevenLabs` goes through here, with the key from
//! `ELEVENLABS_API_KEY`. A busy reply (too many requests at once, or the service under load)
//! made no audio and cost nothing, so it is waited out and sent again, a few times at most;
//! any other failure is reported once, with `ElevenLabs`' own message, and never retried.
//!
//! Speech is recorded with timestamps, so lip-sync is exact to the letter. Models that take
//! direction as audio tags (`eleven_v3`, `eleven_v4`) hear the line's mood as a tag such as
//! `[defiant]`; older models would read a tag aloud, so they get the mood as a speaking speed
//! instead. Each take is asked for with a seed made from its cache key, so a line recorded
//! again (a cleared cache, a second machine) comes back as close to the first take as the
//! service allows.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use base64::Engine as _;
use serde::Deserialize;
use serde::Serialize;

use crate::art::seed_of;
use crate::video::lipsync::Timed;
use crate::video::voice;
use crate::video::voice::Utterance;

/// The environment variable the key is read from.
pub const KEY: &str = "ELEVENLABS_API_KEY";
/// The speech model a voice uses unless it names one: the most expressive, and it takes
/// audio tags.
pub const SPEECH_MODEL: &str = "eleven_v4";
/// The sound-effects model: effects and looping ambience.
pub const SOUND_MODEL: &str = "eleven_text_to_sound_v3";
/// The voice-design model, which can design from a casting note.
pub const DESIGN_MODEL: &str = "eleven_ttv_v3";
/// Requests sent at once. Every paid plan allows at least this many, and the free plan's
/// lower limit is met by waiting out the busy replies.
pub const JOBS: usize = 3;

/// The environment variable that points fables at another `ElevenLabs` host, such as a data
/// residency one (`https://api.eu.residency.elevenlabs.io`).
pub const HOST: &str = "ELEVENLABS_API_URL";
const API: &str = "https://api.elevenlabs.io";

/// The account fables calls `ElevenLabs` with when it is not read from the environment: for
/// a program that embeds fables and holds the key itself, and for tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
	/// The API key; `None` means there is none, whatever the environment says.
	pub key: Option<String>,
	/// The API host; `None` is the global one.
	pub host: Option<String>,
}

static ACCOUNT: std::sync::RwLock<Option<Account>> = std::sync::RwLock::new(None);

/// Uses `account` for every call from now on, in place of `ELEVENLABS_API_KEY` and
/// `ELEVENLABS_API_URL`; `None` goes back to the environment.
pub fn use_account(account: Option<Account>) {
	*ACCOUNT
		.write()
		.unwrap_or_else(std::sync::PoisonError::into_inner) = account;
}

/// The account in use: the one set with [`use_account`], or the environment's.
fn account() -> Account {
	let set = ACCOUNT
		.read()
		.unwrap_or_else(std::sync::PoisonError::into_inner)
		.clone();

	set.unwrap_or_else(|| {
		let read = |variable| {
			std::env::var(variable)
				.ok()
				.filter(|value: &String| !value.is_empty())
		};
		Account {
			key: read(KEY),
			host: read(HOST),
		}
	})
}

/// The API host: `ELEVENLABS_API_URL`, or the global one.
fn api() -> String {
	account().host.map_or_else(
		|| API.to_owned(),
		|host| host.trim_end_matches('/').to_owned(),
	)
}
/// Every plan can ask for this format.
const FORMAT: &str = "mp3_44100_128";
/// How many times a busy reply is sent again before giving up.
const RETRIES: u32 = 4;

/// Whether the key is set for this run.
pub fn has_key() -> bool {
	account().key.is_some()
}

fn key() -> Result<String> {
	account().key.with_context(|| {
		format!(
			"{KEY} is not set; set it in the environment for this run, e.g. `{KEY}=… fables video \
			 …`"
		)
	})
}

/// Whether `model` takes direction as audio tags in the text. Older models read a tag aloud.
pub fn takes_tags(model: &str) -> bool {
	model.starts_with("eleven_v3") || model.starts_with("eleven_v4")
}

/// Eleven v4 has no style or speed setting.
fn has_style_and_speed(model: &str) -> bool {
	!model.starts_with("eleven_v4")
}

fn agent() -> ureq::Agent {
	ureq::Agent::config_builder()
		.http_status_as_error(false)
		.timeout_global(Some(Duration::from_secs(180)))
		.build()
		.into()
}

type Response = ureq::http::Response<ureq::Body>;

/// Sends a request built by `send`, waiting out busy replies, and returns a successful reply
/// or `ElevenLabs`' own reason for refusing.
fn call(
	what: &str,
	send: impl Fn(&ureq::Agent) -> Result<Response, ureq::Error>,
) -> Result<Response> {
	let agent = agent();
	let mut attempt = 0;

	loop {
		let mut response =
			send(&agent).with_context(|| format!("could not reach ElevenLabs for {what}"))?;
		let status = response.status().as_u16();

		if (200..300).contains(&status) {
			return Ok(response);
		}

		let busy = status == 429 || status == 503;

		if busy && attempt < RETRIES {
			let wait = response
				.headers()
				.get("retry-after")
				.and_then(|value| value.to_str().ok())
				.and_then(|value| value.trim().parse::<u64>().ok())
				.unwrap_or(1 << attempt)
				.min(30);
			std::thread::sleep(Duration::from_secs(wait));
			attempt += 1;
			continue;
		}

		let body = response.body_mut().read_to_string().unwrap_or_default();
		bail!("ElevenLabs refused {what} ({status}): {}", reason(&body));
	}
}

/// The readable part of an `ElevenLabs` error body: `detail.message` when there is one.
pub fn reason(body: &str) -> String {
	let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
	let message = parsed.as_ref().and_then(|value| {
		let detail = value.get("detail")?;
		detail
			.get("message")
			.and_then(serde_json::Value::as_str)
			.or_else(|| detail.as_str())
			.map(str::to_owned)
	});

	message.unwrap_or_else(|| body.chars().take(400).collect())
}

fn read_bytes(what: &str, response: &mut Response) -> Result<Vec<u8>> {
	response
		.body_mut()
		.with_config()
		.limit(64 * 1024 * 1024)
		.read_to_vec()
		.with_context(|| format!("could not read the ElevenLabs reply for {what}"))
}

// Speech.

/// The text sent for a line: with the delivery as a leading audio tag for models that take
/// tags (`[defiant] Nope!`), and as written for those that would read the tag aloud.
pub fn speech_text(utterance: &Utterance<'_>) -> String {
	if takes_tags(model(utterance)) {
		voice::elevenlabs_text(utterance)
	} else {
		utterance.text.to_owned()
	}
}

fn model<'a>(utterance: &Utterance<'a>) -> &'a str {
	utterance.voice.model.as_deref().unwrap_or(SPEECH_MODEL)
}

/// The request body for a line of speech.
pub fn speech_body(utterance: &Utterance<'_>) -> serde_json::Value {
	let voice = utterance.voice;
	let model = model(utterance);
	let mut settings = serde_json::Map::new();
	let mut set = |name: &str, value: Option<f32>| {
		if let Some(value) = value {
			settings.insert(name.to_owned(), serde_json::json!(value));
		}
	};

	set("stability", voice.stability);
	set("similarity_boost", voice.similarity);

	if has_style_and_speed(model) {
		set("style", voice.exaggeration);

		// A model without tags hears the mood as a pace instead: slower when sad, quicker
		// when cross.
		let pace = if takes_tags(model) {
			1.0
		} else {
			utterance.delivery.rate_scale()
		};
		let speed = voice.speed.unwrap_or(1.0) * pace;

		if voice.speed.is_some() || (speed - 1.0).abs() > f32::EPSILON {
			set("speed", Some(speed.clamp(0.7, 1.2)));
		}
	}

	let key = voice::cache_key(voice, utterance.delivery, utterance.text);
	let mut body = serde_json::json!({
		"text": speech_text(utterance),
		"model_id": model,
		"seed": seed_of(&key) % (u64::from(u32::MAX) + 1),
	});

	if !settings.is_empty() {
		body["voice_settings"] = serde_json::Value::Object(settings);
	}

	body
}

#[derive(Deserialize)]
struct SpeechReply {
	audio_base64: String,
	#[serde(default)]
	alignment: Option<Alignment>,
	#[serde(default)]
	normalized_alignment: Option<Alignment>,
}

#[derive(Deserialize)]
struct Alignment {
	characters: Vec<String>,
	character_start_times_seconds: Vec<f32>,
	character_end_times_seconds: Vec<f32>,
}

impl Alignment {
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

/// Records a line with its timings. Writes the audio to `audio` and returns the per-letter
/// alignment.
pub fn speak(utterance: &Utterance<'_>, audio: &Path) -> Result<Option<Vec<Timed>>> {
	let key = key()?;
	let voice_id = utterance.voice.id.as_deref().unwrap_or_default();
	let url = format!("{}/v1/text-to-speech/{voice_id}/with-timestamps", api());
	let body = speech_body(utterance);
	let mut response = call("a line of speech", |agent| {
		agent
			.post(&url)
			.query("output_format", FORMAT)
			.header("xi-api-key", &key)
			.send_json(&body)
	})?;
	let reply: SpeechReply = response
		.body_mut()
		.with_config()
		.limit(64 * 1024 * 1024)
		.read_json()
		.context("ElevenLabs sent an unexpected reply")?;
	let bytes = base64::engine::general_purpose::STANDARD
		.decode(reply.audio_base64)
		.context("ElevenLabs audio was not valid base64")?;
	std::fs::write(audio, bytes).with_context(|| format!("could not write {}", audio.display()))?;

	Ok(reply
		.normalized_alignment
		.or(reply.alignment)
		.map(|alignment| alignment.timed()))
}

// Sound effects and ambience.

/// What to ask the sound-effects model for.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundRequest {
	pub text: String,
	/// Seconds, or `None` to let the model choose.
	pub seconds: Option<f32>,
	/// A seamless loop, for ambience.
	pub looped: bool,
	pub model: String,
}

impl SoundRequest {
	/// The key the recording is cached under.
	pub fn cache_key(&self) -> String {
		format!(
			"{}|{}|{:?}|{}",
			self.model, self.looped, self.seconds, self.text
		)
	}

	pub fn body(&self) -> serde_json::Value {
		let mut body = serde_json::json!({
			"text": self.text,
			"model_id": self.model,
			"loop": self.looped,
		});

		if let Some(seconds) = self.seconds {
			body["duration_seconds"] = serde_json::json!(seconds.clamp(0.5, 30.0));
		}

		body
	}
}

/// Records a sound effect or an ambience loop into `audio`.
pub fn sound(request: &SoundRequest, audio: &Path) -> Result<()> {
	let key = key()?;
	let body = request.body();
	let what = format!("the sound \"{}\"", request.text);
	let mut response = call(&what, |agent| {
		agent
			.post(&format!("{}/v1/sound-generation", api()))
			.query("output_format", FORMAT)
			.header("xi-api-key", &key)
			.send_json(&body)
	})?;
	let bytes = read_bytes(&what, &mut response)?;
	std::fs::write(audio, bytes).with_context(|| format!("could not write {}", audio.display()))
}

// The voice library.

/// A voice in the account's library.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct LibraryVoice {
	/// The id to put in a cast file: `id = "…"`.
	pub voice_id: String,
	pub name: String,
	/// `premade`, `cloned`, `generated` or `professional`.
	#[serde(default)]
	pub category: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	/// Accent, age, gender, use case and the like.
	#[serde(default)]
	pub labels: BTreeMap<String, String>,
	/// A short sample to listen to.
	#[serde(default)]
	pub preview_url: Option<String>,
}

impl LibraryVoice {
	/// The labels as one short line: "british, young, male, narration".
	pub fn summary(&self) -> String {
		let mut parts: Vec<&str> = ["accent", "age", "gender", "use_case", "descriptive"]
			.iter()
			.filter_map(|label| self.labels.get(*label).map(String::as_str))
			.collect();

		if parts.is_empty() {
			parts.extend(self.description.as_deref());
		}

		parts.join(", ")
	}
}

#[derive(Deserialize)]
struct VoicePage {
	voices: Vec<LibraryVoice>,
	#[serde(default)]
	has_more: bool,
	#[serde(default)]
	next_page_token: Option<String>,
}

/// The voices in the account's library (its own, designed and saved ones), optionally only
/// those matching `search` by name, description or label.
pub fn voices(search: Option<&str>) -> Result<Vec<LibraryVoice>> {
	let key = key()?;
	let mut found = Vec::new();
	let mut token: Option<String> = None;

	// Ten pages of a hundred is far more than any library a book draws on.
	for _ in 0..10 {
		let mut response = call("the voice list", |agent| {
			let mut request = agent
				.get(&format!("{}/v2/voices", api()))
				.query("page_size", "100")
				.query("include_total_count", "false")
				.header("xi-api-key", &key);

			if let Some(search) = search {
				request = request.query("search", search);
			}

			if let Some(token) = &token {
				request = request.query("next_page_token", token);
			}

			request.call()
		})?;
		let page: VoicePage = response
			.body_mut()
			.read_json()
			.context("ElevenLabs sent an unexpected voice list")?;
		found.extend(page.voices);

		match page.next_page_token {
			Some(next) if page.has_more => token = Some(next),
			_ => break,
		}
	}

	Ok(found)
}

// The plan's allowance.

/// How much of the plan's monthly allowance is used.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct Allowance {
	#[serde(rename = "character_count")]
	pub used: u64,
	#[serde(rename = "character_limit")]
	pub limit: u64,
	/// When the allowance resets, in Unix seconds.
	#[serde(rename = "next_character_count_reset_unix", default)]
	pub resets: Option<i64>,
}

impl Allowance {
	pub fn left(&self) -> u64 {
		self.limit.saturating_sub(self.used)
	}
}

/// The plan's allowance, if the key may read it (a key can be limited so it cannot).
pub fn allowance() -> Option<Allowance> {
	let key = key().ok()?;
	let mut response = call("the plan's allowance", |agent| {
		agent
			.get(&format!("{}/v1/user/subscription", api()))
			.header("xi-api-key", &key)
			.call()
	})
	.ok()?;

	response.body_mut().read_json().ok()
}

// Voice design.

/// A designed voice to listen to before keeping it.
#[derive(Clone, Debug)]
pub struct Preview {
	/// The id to keep it by.
	pub generated_voice_id: String,
	/// MP3 audio of the voice reading a sample.
	pub audio: Vec<u8>,
	pub seconds: f32,
}

#[derive(Deserialize)]
struct DesignReply {
	previews: Vec<DesignPreview>,
}

#[derive(Deserialize)]
struct DesignPreview {
	generated_voice_id: String,
	audio_base_64: String,
	#[serde(default)]
	duration_secs: f32,
}

/// The request body for designing a voice from a casting note, reading `sample` (100 to 1,000
/// characters) or a line the model writes when there is none.
pub fn design_body(description: &str, sample: Option<&str>) -> serde_json::Value {
	let mut body = serde_json::json!({
		"voice_description": description,
		"model_id": DESIGN_MODEL,
		"auto_generate_text": sample.is_none(),
	});

	if let Some(sample) = sample {
		body["text"] = serde_json::json!(sample);
	}

	body
}

/// Designs voices from a casting note: a few previews to listen to.
pub fn design(description: &str, sample: Option<&str>) -> Result<Vec<Preview>> {
	let key = key()?;
	let body = design_body(description, sample);
	let mut response = call("a voice design", |agent| {
		agent
			.post(&format!("{}/v1/text-to-voice/design", api()))
			.query("output_format", FORMAT)
			.header("xi-api-key", &key)
			.send_json(&body)
	})?;
	let reply: DesignReply = response
		.body_mut()
		.with_config()
		.limit(64 * 1024 * 1024)
		.read_json()
		.context("ElevenLabs sent an unexpected voice design")?;

	reply
		.previews
		.into_iter()
		.map(|preview| {
			Ok(Preview {
				audio: base64::engine::general_purpose::STANDARD
					.decode(preview.audio_base_64)
					.context("a designed voice's audio was not valid base64")?,
				generated_voice_id: preview.generated_voice_id,
				seconds: preview.duration_secs,
			})
		})
		.collect()
}

#[derive(Deserialize)]
struct Kept {
	voice_id: String,
}

/// Saves a designed voice to the account's library and returns its voice id.
pub fn keep(name: &str, description: &str, generated_voice_id: &str) -> Result<String> {
	let key = key()?;
	let body = serde_json::json!({
		"voice_name": name,
		"voice_description": description,
		"generated_voice_id": generated_voice_id,
	});
	let mut response = call("saving a designed voice", |agent| {
		agent
			.post(&format!("{}/v1/text-to-voice", api()))
			.header("xi-api-key", &key)
			.send_json(&body)
	})?;
	let kept: Kept = response
		.body_mut()
		.read_json()
		.context("ElevenLabs sent an unexpected reply when saving the voice")?;

	Ok(kept.voice_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure::Mood;
	use crate::video::voice::Delivery;
	use crate::video::voice::Voice;

	fn voice(model: Option<&str>) -> Voice {
		Voice {
			provider: "elevenlabs".into(),
			id: Some("voice-123".into()),
			model: model.map(str::to_owned),
			..Voice::default()
		}
	}

	fn cross(voice: &Voice) -> Utterance<'_> {
		Utterance {
			text: "Bed. Now.",
			voice,
			delivery: Delivery {
				mood: Some(Mood::Cross),
				shout: false,
			},
		}
	}

	#[test]
	fn tag_models_hear_the_mood_as_a_tag_and_older_models_as_a_pace() {
		let v4 = voice(None);
		let body = speech_body(&cross(&v4));
		assert_eq!(body["model_id"], "eleven_v4");
		assert_eq!(body["text"], "[angry] Bed. Now.");
		assert!(body.get("voice_settings").is_none(), "{body}");

		let v2 = voice(Some("eleven_multilingual_v2"));
		let body = speech_body(&cross(&v2));
		assert_eq!(
			body["text"], "Bed. Now.",
			"an older model would read a tag aloud"
		);
		let speed = body["voice_settings"]["speed"].as_f64().unwrap();
		assert!((speed - 1.1).abs() < 1e-6, "{speed}");
	}

	#[test]
	fn a_voices_settings_reach_the_model_that_has_them() {
		let mut v4 = voice(None);
		v4.stability = Some(0.3);
		v4.similarity = Some(0.8);
		v4.exaggeration = Some(0.4);
		v4.speed = Some(1.1);
		let body = speech_body(&cross(&v4));
		let settings = &body["voice_settings"];
		assert!((settings["stability"].as_f64().unwrap() - 0.3).abs() < 1e-6);
		assert!((settings["similarity_boost"].as_f64().unwrap() - 0.8).abs() < 1e-6);
		assert!(settings.get("style").is_none(), "v4 has no style setting");
		assert!(settings.get("speed").is_none(), "v4 has no speed setting");

		let mut flash = v4.clone();
		flash.model = Some("eleven_flash_v2_5".into());
		let body = speech_body(&cross(&flash));
		let settings = &body["voice_settings"];
		assert!((settings["style"].as_f64().unwrap() - 0.4).abs() < 1e-6);
		// 1.1 for the voice, quicker again for a cross line, held to the model's range.
		assert!((settings["speed"].as_f64().unwrap() - 1.2).abs() < 1e-6);
	}

	#[test]
	fn the_same_line_asks_for_the_same_seed() {
		let v4 = voice(None);
		let one = speech_body(&cross(&v4));
		let two = speech_body(&cross(&v4));
		assert_eq!(one["seed"], two["seed"]);
		assert!(u32::try_from(one["seed"].as_u64().unwrap()).is_ok());

		let other = Utterance {
			text: "Bed. Now!",
			..cross(&v4)
		};
		assert_ne!(speech_body(&other)["seed"], one["seed"]);
	}

	#[test]
	fn alignment_becomes_timed_letters() {
		let reply: SpeechReply = serde_json::from_str(
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
	fn a_sound_request_holds_its_length_to_the_models_range() {
		let request = SoundRequest {
			text: "a cartoon bonk".into(),
			seconds: Some(60.0),
			looped: true,
			model: SOUND_MODEL.into(),
		};
		let body = request.body();
		assert_eq!(body["duration_seconds"], 30.0);
		assert_eq!(body["loop"], true);
		assert_eq!(body["model_id"], "eleven_text_to_sound_v3");

		let free = SoundRequest {
			seconds: None,
			..request
		};
		assert!(free.body().get("duration_seconds").is_none());
	}

	#[test]
	fn errors_read_as_elevenlabs_wrote_them() {
		assert_eq!(
			reason(
				r#"{"detail":{"status":"quota_exceeded","message":"You have 12 credits left."}}"#
			),
			"You have 12 credits left."
		);
		assert_eq!(reason(r#"{"detail":"Not found"}"#), "Not found");
		assert_eq!(reason("plain words"), "plain words");
	}

	#[test]
	fn library_voices_read_their_labels() {
		let page: VoicePage = serde_json::from_str(
			r#"{"voices":[{"voice_id":"abc","name":"Pip","category":"generated","labels":{"accent":"british","age":"young","gender":"male"},"preview_url":"https://example.com/p.mp3"}],"has_more":false}"#,
		)
		.unwrap();
		assert_eq!(page.voices[0].summary(), "british, young, male");
		assert!(!page.has_more);
	}

	#[test]
	fn the_allowance_counts_what_is_left() {
		let allowance: Allowance = serde_json::from_str(
			r#"{"character_count":9000,"character_limit":10000,"next_character_count_reset_unix":1798000000,"tier":"starter"}"#,
		)
		.unwrap();
		assert_eq!(allowance.left(), 1000);
		assert_eq!(allowance.resets, Some(1_798_000_000));
	}

	#[test]
	fn a_design_without_a_sample_asks_for_one() {
		let body = design_body("A gravelly old sea captain", None);
		assert_eq!(body["auto_generate_text"], true);
		assert!(body.get("text").is_none());
		assert_eq!(body["model_id"], DESIGN_MODEL);
	}
}
