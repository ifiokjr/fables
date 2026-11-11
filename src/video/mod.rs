//! Narrated videos: the book performed, for `YouTube` (16:9) and Shorts, `TikTok` and Reels (9:16).
//!
//! The rehearsal that lays out the book also drives the video. The narrator reads each
//! paragraph, stage directions land as the narrator reaches them, and the characters lip-flap
//! to the narration. Frames are rendered in parallel and piped straight into ffmpeg.

pub mod audio;
pub mod director;
pub mod frames;
pub mod lipsync;
pub mod motion;
pub mod timeline;
pub mod voice;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use rayon::prelude::*;

use crate::cast::Cast;
use crate::stage::rehearse;
use crate::story::Story;
use crate::story::plain_text;
use crate::video::frames::Format;
use crate::video::frames::Renderer;
use crate::video::timeline::Timeline;

/// Frames rendered together before being handed to ffmpeg in order.
const BATCH: usize = 48;

/// Options for a video build.
#[derive(Clone, Copy, Debug)]
pub struct Options {
	/// Half resolution at 15 fps, for checking timing quickly.
	pub preview: bool,
}

/// Narrates the story and lays out the timeline (shared by every format).
pub fn plan(story: &Story, cast: &Cast, cache: &Path) -> Result<Timeline> {
	let rehearsals = rehearse(story, cast)?;
	let mut spoken = 0;
	let mut characters = std::collections::BTreeSet::new();
	let timeline = timeline::build(story, cast, &rehearsals, |line: &voice::Utterance<'_>| {
		spoken += 1;

		if line.delivery.mood.is_some() {
			characters.insert(line.voice.name.clone());
		}

		voice::speak(line, cache)
	})?;

	eprintln!(
		"  voiced {spoken} lines ({} character voices), {:.0}s total",
		characters.len(),
		timeline.duration
	);

	Ok(timeline)
}

/// Builds the soundtrack, captions file and one MP4 per format. Returns the video paths.
pub fn make(
	story: &Story,
	timeline: &Timeline,
	formats: &[Format],
	options: Options,
	dir: &Path,
) -> Result<Vec<PathBuf>> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;

	let soundtrack = dir.join("soundtrack.wav");
	audio::write(timeline, &soundtrack)?;

	let captions = dir.join(format!("{}.srt", story.slug));
	std::fs::write(&captions, srt(timeline))
		.with_context(|| format!("could not write {}", captions.display()))?;

	write_covers(story, timeline, dir)?;

	let mut outputs = Vec::new();

	for format in formats {
		let suffix = if options.preview { "-preview" } else { "" };
		let path = dir.join(format!("{}-{}{suffix}.mp4", story.slug, format.suffix()));
		encode(story, timeline, *format, options, &soundtrack, &path)?;
		outputs.push(path);
	}

	Ok(outputs)
}

fn encode(
	story: &Story,
	timeline: &Timeline,
	format: Format,
	options: Options,
	soundtrack: &Path,
	path: &Path,
) -> Result<()> {
	let fps: u32 = if options.preview { 15 } else { 30 };
	let (width, height) = format.size(options.preview);
	let renderer = Renderer::new(timeline, story, format, width, height);
	let frames = (timeline.duration * fps as f32).ceil() as usize;
	let clip = Clip {
		width,
		height,
		fps,
		frames,
		audio: Some(soundtrack),
		label: format.suffix(),
	};

	clip.encode(path, |index| {
		renderer
			.render(index as f32 / fps as f32, index as u64)
			.map(resvg::tiny_skia::Pixmap::take)
	})
}

/// A video to encode: size, frame rate, length and an optional soundtrack.
pub struct Clip<'a> {
	pub width: u32,
	pub height: u32,
	pub fps: u32,
	pub frames: usize,
	pub audio: Option<&'a Path>,
	/// Shown in progress output.
	pub label: &'a str,
}

impl Clip<'_> {
	/// Renders every frame with `render` (RGBA bytes) and encodes an H.264 MP4 at `path`.
	pub fn encode(
		&self,
		path: &Path,
		render: impl Fn(usize) -> Result<Vec<u8>> + Sync,
	) -> Result<()> {
		let (width, height, frames) = (self.width, self.height, self.frames);
		let mut command = Command::new("ffmpeg");
		command
			.args([
				"-y",
				"-loglevel",
				"error",
				"-f",
				"rawvideo",
				"-pix_fmt",
				"rgba",
				"-s",
			])
			.arg(format!("{width}x{height}"))
			.args(["-r", &self.fps.to_string(), "-i", "-"]);

		if let Some(audio) = self.audio {
			command.arg("-i").arg(audio);
		}

		command.args([
			"-c:v",
			"libx264",
			"-preset",
			"medium",
			"-crf",
			"18",
			"-pix_fmt",
			"yuv420p",
			"-profile:v",
			"high",
			"-colorspace",
			"bt709",
			"-color_primaries",
			"bt709",
			"-color_trc",
			"bt709",
		]);

		if self.audio.is_some() {
			command.args([
				"-af",
				"loudnorm=I=-16:TP=-1.5:LRA=11",
				"-c:a",
				"aac",
				"-b:a",
				"192k",
				"-ar",
				"48000",
				"-shortest",
			]);
		}

		let mut ffmpeg = command
			.args(["-movflags", "+faststart"])
			.arg(path)
			.stdin(Stdio::piped())
			// Never on our stdout: under `fables mcp` that is the protocol's channel.
			.stdout(Stdio::null())
			.spawn()
			.context("could not run ffmpeg; install it to make videos")?;
		let mut stdin = ffmpeg.stdin.take().context("ffmpeg has no stdin")?;
		let (sender, receiver) = std::sync::mpsc::sync_channel::<Vec<Vec<u8>>>(2);

		// Performance: a writer thread feeds ffmpeg while the next batch renders on every core,
		// so neither the encoder nor the rasteriser sits idle; the small channel bounds memory.
		let rendered = std::thread::scope(|scope| -> Result<()> {
			let writer = scope.spawn(move || -> Result<()> {
				for batch in receiver {
					for frame in batch {
						stdin
							.write_all(&frame)
							.context("ffmpeg stopped accepting frames")?;
					}
				}

				Ok(())
			});
			let mut last_report = 0;

			for start in (0..frames).step_by(BATCH) {
				let end = (start + BATCH).min(frames);
				let batch: Vec<Vec<u8>> = (start..end)
					.into_par_iter()
					.map(&render)
					.collect::<Result<_>>()?;

				if sender.send(batch).is_err() {
					break;
				}

				let percent = end * 100 / frames.max(1);

				if percent >= last_report + 10 {
					last_report = percent / 10 * 10;
					eprint!("\r  {} {width}x{height}: {percent}%", self.label);
					let _ = std::io::stdout().flush();
				}
			}

			drop(sender);
			writer
				.join()
				.map_err(|_| anyhow::anyhow!("the frame writer panicked"))?
		});

		let status = ffmpeg.wait().context("ffmpeg did not finish")?;
		eprintln!();
		rendered?;

		if !status.success() {
			bail!("ffmpeg failed while encoding {}", path.display());
		}

		Ok(())
	}
}

/// Still images the platforms ask for: a 1280×720 `YouTube` thumbnail and a 1080×1920 cover for
/// Shorts, `TikTok` and Reels, both taken from the title card once it has popped in.
fn write_covers(story: &Story, timeline: &Timeline, dir: &Path) -> Result<()> {
	let moment = timeline
		.shots
		.first()
		.map_or(2.0, |shot| (shot.start + shot.duration() * 0.6).min(2.4));

	for (format, width, height, name) in [
		(Format::Landscape, 1280, 720, "thumbnail"),
		(Format::Portrait, 1080, 1920, "cover-9x16"),
	] {
		let renderer = Renderer::new(timeline, story, format, width, height);
		let path = dir.join(format!("{}-{name}.jpg", story.slug));
		let pixmap = renderer.render(moment, 0)?;
		std::fs::write(&path, crate::raster::jpeg(&pixmap, 90)?)
			.with_context(|| format!("could not write {}", path.display()))?;
	}

	Ok(())
}

/// `SubRip` captions for `YouTube`'s closed captions (one cue per narrated paragraph).
pub fn srt(timeline: &Timeline) -> String {
	let mut out = String::new();
	let mut index = 1;

	for shot in &timeline.shots {
		for line in &shot.lines {
			let text = plain_text(&line.text);

			if text.is_empty() {
				continue;
			}

			let end = line.start + timeline.speeches[line.speech].duration();
			let _ = write!(
				out,
				"{index}\n{} --> {}\n{text}\n\n",
				stamp(line.start),
				stamp(end)
			);
			index += 1;
		}
	}

	out
}

fn stamp(seconds: f32) -> String {
	let millis = (seconds.max(0.0) * 1000.0).round() as u64;

	format!(
		"{:02}:{:02}:{:02},{:03}",
		millis / 3_600_000,
		millis / 60_000 % 60,
		millis / 1000 % 60,
		millis % 1000
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn srt_timestamps_are_well_formed() {
		assert_eq!(stamp(0.0), "00:00:00,000");
		assert_eq!(stamp(83.4567), "00:01:23,457");
		assert_eq!(stamp(3725.0), "01:02:05,000");
	}
}
