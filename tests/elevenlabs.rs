//! The `ElevenLabs` integration end to end, against a stand-in for the service on localhost.
//!
//! The stand-in answers the endpoints fables calls with real audio (WAV that ffmpeg decodes,
//! as it decodes `ElevenLabs`' MP3), records every request, and turns the first speech request
//! away as busy. One book is voiced and its sounds recorded, then voiced again: the second run
//! must not call the service at all. Then the key is taken away and a new line falls back
//! without a call.
//!
//! Everything runs in one test, because the account is set for the whole process.

use std::io::BufRead as _;
use std::io::BufReader;
use std::io::Read as _;
use std::io::Write as _;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;

use base64::Engine as _;
use fables::cast::Cast;
use fables::story::Story;
use fables::video;
use fables::video::audio::Sound;
use fables::video::elevenlabs;
use fables::world::World;

#[derive(Clone, Debug)]
struct Request {
	method: String,
	target: String,
	key: Option<String>,
	body: serde_json::Value,
}

/// A WAV of `seconds` of a soft tone at 44.1 kHz, as a service would send it.
fn wav(seconds: f32) -> Vec<u8> {
	let spec = hound::WavSpec {
		channels: 1,
		sample_rate: 44_100,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};
	let mut bytes = std::io::Cursor::new(Vec::new());
	let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();

	for index in 0..(seconds * 44_100.0) as usize {
		let t = index as f32 / 44_100.0;
		let value = (std::f32::consts::TAU * 330.0 * t).sin() * 0.4;
		writer
			.write_sample((value * f32::from(i16::MAX)) as i16)
			.unwrap();
	}

	writer.finalize().unwrap();
	bytes.into_inner()
}

fn respond(stream: &mut std::net::TcpStream, status: &str, kind: &str, extra: &str, body: &[u8]) {
	let head = format!(
		"HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{extra}Connection: \
		 close\r\n\r\n",
		body.len()
	);
	stream.write_all(head.as_bytes()).unwrap();
	stream.write_all(body).unwrap();
}

/// Serves the stand-in until the test ends; returns its address and the requests it saw.
fn serve() -> (String, Arc<Mutex<Vec<Request>>>) {
	let listener = TcpListener::bind("127.0.0.1:0").unwrap();
	let address = format!("http://{}", listener.local_addr().unwrap());
	let seen = Arc::new(Mutex::new(Vec::<Request>::new()));
	let log = Arc::clone(&seen);

	std::thread::spawn(move || {
		let mut turned_away = false;

		for stream in listener.incoming() {
			let mut stream = stream.unwrap();
			let mut reader = BufReader::new(stream.try_clone().unwrap());
			let mut line = String::new();
			reader.read_line(&mut line).unwrap();
			let mut parts = line.split_whitespace();
			let method = parts.next().unwrap_or_default().to_owned();
			let target = parts.next().unwrap_or_default().to_owned();
			let mut length = 0;
			let mut key = None;

			loop {
				let mut header = String::new();
				reader.read_line(&mut header).unwrap();

				if header.trim().is_empty() {
					break;
				}

				let (name, value) = header.split_once(':').unwrap();

				match name.to_ascii_lowercase().as_str() {
					"content-length" => length = value.trim().parse().unwrap(),
					"xi-api-key" => key = Some(value.trim().to_owned()),
					_ => {}
				}
			}

			let mut body = vec![0; length];
			reader.read_exact(&mut body).unwrap();
			let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
			log.lock().unwrap().push(Request {
				method: method.clone(),
				target: target.clone(),
				key,
				body: body.clone(),
			});

			if target.starts_with("/v1/text-to-speech/") {
				if !turned_away {
					turned_away = true;
					respond(
						&mut stream,
						"429 Too Many Requests",
						"application/json",
						"Retry-After: 0\r\n",
						br#"{"detail":{"status":"too_many_concurrent_requests","message":"busy"}}"#,
					);
					continue;
				}

				let text = body["text"].as_str().unwrap_or_default();
				let letters: Vec<String> = text.chars().map(String::from).collect();
				let each = 0.05;
				let starts: Vec<f32> = (0..letters.len()).map(|i| i as f32 * each).collect();
				let ends: Vec<f32> = starts.iter().map(|start| start + each).collect();
				let reply = serde_json::json!({
					"audio_base64": base64::engine::general_purpose::STANDARD
						.encode(wav(letters.len() as f32 * each)),
					"normalized_alignment": {
						"characters": letters,
						"character_start_times_seconds": starts,
						"character_end_times_seconds": ends,
					},
				});
				respond(
					&mut stream,
					"200 OK",
					"application/json",
					"",
					reply.to_string().as_bytes(),
				);
			} else if target.starts_with("/v1/sound-generation") {
				let seconds = body["duration_seconds"].as_f64().unwrap_or(1.0) as f32;
				respond(
					&mut stream,
					"200 OK",
					"audio/mpeg",
					"",
					&wav(seconds.min(3.0)),
				);
			} else if target.starts_with("/v1/user/subscription") {
				respond(
					&mut stream,
					"200 OK",
					"application/json",
					"",
					br#"{"character_count":100,"character_limit":10000,"tier":"starter"}"#,
				);
			} else {
				respond(&mut stream, "404 Not Found", "application/json", "", b"{}");
			}
		}
	});

	(address, seen)
}

const WORLD: &str = r#"
name = "Foley"
style = "cartoon"

[narrator]
provider = "elevenlabs"
id = "narrator-voice"

[sound]
provider = "elevenlabs"

[sound.effects]
pop = "a soap bubble popping"
"#;

const CAST: &str = r#"
[pip]
name = "Pip"

[pip.voice]
provider = "elevenlabs"
id = "pip-voice"
stability = 0.4
fallback = "Junior"
"#;

const BOOK: &str = r#"+++
title = "The Pot"
author = "A"

[video]
cast-voices = true
ambience = true
+++

# park
@pip happy talk
"Hello, pot!" said Pip.
@sfx bonk "a wooden spoon tapping a pot"
The pot went bong.
"#;

fn book(world: &World, source: &str) -> (Story, Cast) {
	let folder = world.dir.join("stories/the-pot");
	std::fs::create_dir_all(&folder).unwrap();
	let path = folder.join("story.md");
	std::fs::write(&path, source).unwrap();
	let mut story = Story::load(&path).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast_for(&story).unwrap();

	(story, cast)
}

#[test]
fn a_book_is_voiced_and_scored_by_elevenlabs_once() {
	if std::process::Command::new("ffmpeg")
		.arg("-version")
		.output()
		.is_err()
	{
		eprintln!("ffmpeg is not installed; skipping");
		return;
	}

	let (address, seen) = serve();
	elevenlabs::use_account(Some(elevenlabs::Account {
		key: Some("test-key".into()),
		host: Some(address.clone()),
	}));

	let dir = tempfile::tempdir().unwrap();
	let folder = dir.path().join("worlds/foley");
	std::fs::create_dir_all(&folder).unwrap();
	std::fs::write(folder.join("world.toml"), WORLD).unwrap();
	std::fs::write(folder.join("cast.toml"), CAST).unwrap();
	let world = World::load(&folder).unwrap();
	let (story, cast) = book(&world, BOOK);
	let cache = dir.path().join("out/the-pot/cache/voice");

	let timeline = video::plan(&story, &cast, &cache).unwrap();
	let requests = seen.lock().unwrap().clone();

	// Every request carried the key.
	assert!(
		requests
			.iter()
			.all(|request| request.key.as_deref() == Some("test-key")),
		"{requests:#?}"
	);

	// Speech: the busy reply was waited out, then every line recorded once.
	let speech: Vec<&Request> = requests
		.iter()
		.filter(|request| request.target.starts_with("/v1/text-to-speech/"))
		.collect();
	let texts: Vec<&str> = speech
		.iter()
		.filter_map(|request| request.body["text"].as_str())
		.collect();
	let pip = speech
		.iter()
		.find(|request| {
			request
				.target
				.starts_with("/v1/text-to-speech/pip-voice/with-timestamps")
		})
		.expect("Pip spoke in his own voice");
	assert_eq!(pip.method, "POST");
	assert!(
		pip.target.contains("output_format=mp3_44100_128"),
		"{}",
		pip.target
	);
	assert_eq!(pip.body["text"], "[cheerful] Hello, pot!");
	assert_eq!(pip.body["model_id"], "eleven_v4");
	assert!(pip.body["seed"].is_u64());
	assert!((pip.body["voice_settings"]["stability"].as_f64().unwrap() - 0.4).abs() < 1e-6);
	assert!(
		speech.iter().any(|request| {
			request
				.target
				.starts_with("/v1/text-to-speech/narrator-voice/")
		}),
		"{texts:?}"
	);
	let unique: std::collections::BTreeSet<&str> = texts.iter().copied().collect();
	assert_eq!(
		texts.len(),
		unique.len() + 1,
		"one line was sent twice, after the busy reply, and no other: {texts:?}"
	);

	// Lip-sync came from the timings, and the speech is real audio.
	assert!(
		timeline
			.speeches
			.iter()
			.all(|speech| !speech.samples.is_empty())
	);

	// Sounds: the described cue, the world's own pop, the built-in page turn and a looping bed.
	let sounds: Vec<&serde_json::Value> = requests
		.iter()
		.filter(|request| request.target.starts_with("/v1/sound-generation"))
		.map(|request| &request.body)
		.collect();
	let said = |text: &str| sounds.iter().any(|body| body["text"] == text);
	assert!(said("a wooden spoon tapping a pot"), "{sounds:#?}");
	assert!(
		sounds
			.iter()
			.any(|body| body["loop"] == true && body["duration_seconds"] == 20.0),
		"an ambience loop: {sounds:#?}"
	);
	assert!(
		sounds
			.iter()
			.all(|body| body["model_id"] == "eleven_text_to_sound_v3")
	);
	assert!(
		timeline
			.foley
			.effect(
				Sound::Bonk,
				Some(&Arc::from("a wooden spoon tapping a pot"))
			)
			.is_some()
	);
	assert!(timeline.foley.effect(Sound::PageTurn, None).is_some());
	assert!(
		requests
			.iter()
			.any(|request| request.target == "/v1/user/subscription"),
		"the allowance is read before recording"
	);

	// The soundtrack mixes the recorded sounds.
	let soundtrack = dir.path().join("soundtrack.wav");
	video::audio::write(&timeline, &soundtrack).unwrap();
	assert!(soundtrack.is_file());

	// A second build is all cache: not one call.
	let before = seen.lock().unwrap().len();
	let again = video::plan(&story, &cast, &cache).unwrap();
	assert_eq!(
		seen.lock().unwrap().len(),
		before,
		"a re-render never pays twice"
	);
	assert_eq!(again.speeches.len(), timeline.speeches.len());
	assert!(!again.foley.is_empty());

	// Without the key, a new line is never sent: it falls back, and the cached ones still play.
	elevenlabs::use_account(Some(elevenlabs::Account {
		key: None,
		host: Some(address),
	}));
	let (story, cast) = book(
		&world,
		&BOOK.replace("The pot went bong.", "The pot went BONG."),
	);
	let fallen = video::plan(&story, &cast, &cache).unwrap();
	assert_eq!(seen.lock().unwrap().len(), before, "no key, no call");
	assert_eq!(fallen.speeches.len(), timeline.speeches.len());
	assert!(
		!fallen.foley.is_empty(),
		"recorded sounds are used without the key"
	);
}
