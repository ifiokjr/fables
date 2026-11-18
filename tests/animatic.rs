//! The animatic is judged by ear as well as by eye, so its soundtrack must be the video's own:
//! the same narration, music cues, drop-outs, dead stops and ambience, byte for byte.

use std::process::Command;

use fables::cast::Cast;
use fables::publish;
use fables::video::frames::Format;

/// A book with everything sound mixes: an age band (so ambience is on and the pacing is its
/// own), a heading's `hold=`, an `@hold` dead stop, a music cue change, a shout on a turn to a
/// strong mood (a drop-out) and an effect. The narrator and Kid are `command` voices that write
/// tones, so no speech engine is needed, only ffmpeg.
const SCRIPT: &str = r#"+++
title = "Loud and Quiet"
author = "A"
age = "3-5"
star = "kid"

[video]
cast-voices = true

[voice]
provider = "command"
command = ["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=220:duration=0.9", "-f", "wav", "{out}"]
+++
# park hold=0.6 music=calm
@kid x=0.4 happy
The kite went up.
@hold 0.8
@music tense
@sfx boing
@kid shocked talk
"**UP IT GOES!**" said Kid.
"#;

/// Kid's own voice: a higher tone, written the same way.
const TONE: &str = r#"["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=0.7", "-f", "wav", "{out}"]"#;

fn ffmpeg() -> bool {
	Command::new("ffmpeg")
		.arg("-version")
		.output()
		.is_ok_and(|output| output.status.success())
}

#[test]
fn the_animatic_soundtrack_is_the_preview_videos_byte_for_byte() {
	if !ffmpeg() {
		eprintln!("skipped: the animatic and the video need ffmpeg");
		return;
	}

	let story = fables::story::parse(SCRIPT).unwrap();
	let cast = Cast::parse(&format!(
		"[kid]\nname = \"Kid\"\n[kid.voice]\nprovider = \"command\"\ncommand = {TONE}\n"
	))
	.unwrap();
	let out = tempfile::tempdir().unwrap();

	// The video records the takes into the cache; the animatic reads them back from it, as the
	// two commands do one after the other.
	publish::video(&story, &cast, out.path(), &[Format::Landscape], true).unwrap();
	let cut = publish::animatic(
		&story,
		&cast,
		out.path(),
		Format::Landscape,
		publish::Narration::Voiced,
	)
	.unwrap();
	let timeline = &cut.timeline;

	assert!(cut.path.is_file(), "{}", cut.path.display());
	assert_eq!(cut.silent, 0, "every line has a take: {:?}", cut.note);
	assert!(!timeline.holds.is_empty(), "the book has dead stops");
	assert!(timeline.music.len() > 1, "the music changes cue");
	assert!(
		!timeline.dropouts.is_empty(),
		"the shout drops the music out"
	);
	assert!(
		!timeline.ambience.is_empty(),
		"an age band turns ambience on"
	);

	let book = publish::book_dir(out.path(), &story);
	let video = std::fs::read(book.join("video/soundtrack.wav")).unwrap();
	let heard = std::fs::read(book.join("animatic/soundtrack.wav")).unwrap();

	assert!(video.len() > 44, "the video's soundtrack has samples");
	assert!(
		video == heard,
		"the animatic's soundtrack ({} bytes) is not the video's ({} bytes)",
		heard.len(),
		video.len()
	);
}
