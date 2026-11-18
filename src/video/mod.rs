//! Narrated videos: the book performed, for `YouTube` (16:9) and Shorts, `TikTok` and Reels (9:16).
//!
//! The rehearsal that lays out the book also drives the video. The narrator reads each
//! paragraph, stage directions land as the narrator reaches them, and the characters lip-flap
//! to the narration. Frames are rendered in parallel and piped straight into ffmpeg.

pub mod acting;
pub mod audio;
pub mod director;
pub mod ffmpeg;
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
use anyhow::ensure;
use rayon::prelude::*;

use crate::art::pt;
use crate::cast::Cast;
use crate::press::storyboard;
use crate::stage::rehearse;
use crate::story::Story;
use crate::story::plain_text;
use crate::video::director::View;
use crate::video::frames::Format;
use crate::video::frames::Renderer;
use crate::video::timeline::Timeline;

/// Frames rendered together before being handed to ffmpeg in order.
const BATCH: usize = 48;

/// The longest video fables will make: half an hour. The frame count and the soundtrack are
/// sized from the timeline's duration, so no script may ask for more than this.
pub const MAX_SECONDS: f32 = 30.0 * 60.0;

/// Refuses a timeline too long to be a book's video, before anything is allocated from its
/// duration.
pub fn check_length(duration: f32) -> Result<()> {
	ensure!(
		duration.is_finite() && duration <= MAX_SECONDS,
		"the video would run {duration:.0}s; fables makes videos of at most {MAX_SECONDS:.0} \
		 seconds (30 minutes): trim the script with `~` book-only paragraphs, or shorten a \
		 `@pause` or `@hold`"
	);

	Ok(())
}

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
	check_length(timeline.duration)?;
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
	/// The ffmpeg arguments that encode raw RGBA frames from stdin, and the soundtrack if any,
	/// into an H.264 MP4 at `path`.
	fn args(&self, path: &Path) -> Vec<std::ffi::OsString> {
		let mut args: Vec<std::ffi::OsString> = Vec::new();
		let mut push = |values: &[&str]| args.extend(values.iter().map(Into::into));

		push(&[
			"-y",
			"-loglevel",
			"error",
			"-f",
			"rawvideo",
			"-pix_fmt",
			"rgba",
			"-s",
		]);
		push(&[&format!("{}x{}", self.width, self.height)]);
		push(&["-r", &self.fps.to_string(), "-i", "-"]);

		if let Some(audio) = self.audio {
			push(&["-i"]);
			args.push(audio.into());
		}

		let mut push = |values: &[&str]| args.extend(values.iter().map(Into::into));
		push(&[
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
			push(&[
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

		push(&["-movflags", "+faststart"]);
		args.push(path.into());

		args
	}

	/// Renders every frame with `render` (RGBA bytes) and encodes an H.264 MP4 at `path`.
	pub fn encode(
		&self,
		path: &Path,
		render: impl Fn(usize) -> Result<Vec<u8>> + Sync,
	) -> Result<()> {
		let (width, height, frames) = (self.width, self.height, self.frames);
		let args = self.args(path);

		ffmpeg::require(&args)?;

		let mut ffmpeg = Command::new("ffmpeg")
			.args(&args)
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

/// An animatic: the storyboard's shots held for their real durations over the video's
/// soundtrack, so timing can be judged before a full render. One plate is drawn per shot and
/// ffmpeg holds it, cropping a push or pan across the plate rather than rendering every frame.
/// The soundtrack is written beside it as `soundtrack.wav`, mixed exactly as the video's.
///
/// Every caption is drawn by fables and laid over its shot, and a progress bar runs along the
/// top, so the graph needs only filters every ffmpeg build has (see [`ANIMATIC_FILTERS`]).
pub fn animatic(
	story: &Story,
	cast: &Cast,
	timeline: &Timeline,
	format: Format,
	dir: &Path,
) -> Result<PathBuf> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
	let scratch = Scratch::new()?;
	let soundtrack = dir.join("soundtrack.wav");
	let path = dir.join(format!("{}-animatic.mp4", story.slug));
	let args = animatic_args(
		story,
		cast,
		timeline,
		format,
		&soundtrack,
		&scratch.path,
		&path,
	)?;

	ffmpeg::require(&args)?;
	audio::write(timeline, &soundtrack)?;

	let status = Command::new("ffmpeg")
		.args(&args)
		// Never on our stdout: under `fables mcp` that is the protocol's channel.
		.stdout(Stdio::null())
		.status()
		.context("could not run ffmpeg; install it to make videos")?;

	anyhow::ensure!(
		status.success(),
		"ffmpeg failed while writing {}",
		path.display()
	);

	Ok(path)
}

/// The filters an animatic's graph is built from. Each is part of libavfilter itself, with no
/// external library behind it, so every ffmpeg build has them; `drawtext`, which needs
/// freetype, is deliberately not among them.
pub const ANIMATIC_FILTERS: &[&str] =
	&["zoompan", "setsar", "format", "overlay", "concat", "color"];

/// Draws the animatic's plates, captions and progress rail into `scratch` and returns the
/// ffmpeg arguments that cut them over `soundtrack` into `path`.
fn animatic_args(
	story: &Story,
	cast: &Cast,
	timeline: &Timeline,
	format: Format,
	soundtrack: &Path,
	scratch: &Path,
	path: &Path,
) -> Result<Vec<std::ffi::OsString>> {
	let portrait = format == Format::Portrait;
	let (width, height) = if portrait { (540, 960) } else { (960, 540) };
	let fps = 24;
	let rows = storyboard::plan(story, timeline, cast, portrait);

	anyhow::ensure!(!rows.is_empty(), "the animatic has no shots to hold");

	let views = storyboard::views(story, timeline, &rows, portrait);
	let bounds = storyboard::frame_bounds(&rows, fps);
	let total = bounds.last().map_or(1, |(_, last)| (*last).max(1));
	let count = rows.len();
	let mut plates = Vec::new();
	let mut captions = Vec::new();
	let mut filters = Vec::new();
	let mut parts = String::new();

	for (index, ((row, (start, end)), (first, last))) in
		rows.iter().zip(&views).zip(&bounds).enumerate()
	{
		let (union, plate_aspect) = union_view(*start, *end, format.aspect());
		// A still shot is cropped one to one. Only a move needs a larger plate to crop from,
		// and drawing the rest at output size keeps an animatic to a handful of seconds.
		let supersample = if row.movement.is_some() { 1.6 } else { 1.0 };
		let plate_h = (f64::from(height) * supersample).round() as u32;
		let plate_w = (f64::from(plate_h) * f64::from(plate_aspect)).round() as u32;
		let renderer =
			Renderer::new(timeline, story, format, plate_w, plate_h).with_aspect(plate_aspect);
		let plate = renderer.render_view(row.start + row.hold * 0.5, 0, union)?;
		let plate_path = scratch.join(format!("plate-{index}.png"));
		std::fs::write(&plate_path, crate::raster::png(&plate)?)?;
		plates.push(plate_path);

		let caption = storyboard::animatic_caption(row, width, portrait)?;
		let caption_path = scratch.join(format!("caption-{index}.png"));
		std::fs::write(&caption_path, crate::raster::png(&caption)?)?;
		captions.push(caption_path);

		let frame = Plate {
			union,
			aspect: format.aspect(),
			shape: plate_aspect,
			width: plate_w,
		};
		let crop = crop(row, *start, *end, &frame, *first, *last, fps);
		let frames = last - first;

		// The caption is laid over the cropped frame, not the plate, so it stays still while
		// the camera moves.
		filters.push(format!(
			"[{index}:v]zoompan=z='{z}':x='{x}':y='{y}':d={frames}:s={width}x{height}:fps={fps},\
			 setsar=1[z{index}];[z{index}][{caption}:v]overlay=x=0:y=H-h,format=yuv420p[v{index}]",
			z = crop.z,
			x = crop.x,
			y = crop.y,
			caption = count + index,
		));
		let _ = write!(parts, "[v{index}]");
	}

	let rail = scratch.join("rail.png");
	std::fs::write(
		&rail,
		crate::raster::png(&progress_rail(&bounds, total, width)?)?,
	)?;

	let seconds = total as f32 / fps as f32;
	let rail_index = count * 2;
	let audio_index = rail_index + 1;
	// The bar slides in from the left as the cut plays: at the end it spans the frame. It runs
	// a second past the cut so `shortest` ends on the cut's last frame, never the bar's.
	let graph = format!(
		"{};{parts}concat=n={count}:v=1:a=0[cut];[cut][{rail_index}:v]overlay=x=0:y=0[railed];\
		 color=c=0xffc93c:s={width}x{BAR}:r={fps}:d={bar}[bar];[railed][bar]overlay=x='-w+w*t/\
		 {length}':y=0:eval=frame:shortest=1,format=yuv420p[out]",
		filters.join(";"),
		bar = decimal(seconds + 1.0),
		length = decimal(seconds),
	);

	let mut args: Vec<std::ffi::OsString> = ["-y", "-loglevel", "error"]
		.into_iter()
		.map(Into::into)
		.collect();

	for input in plates.iter().chain(&captions).chain([&rail]) {
		args.push("-i".into());
		args.push(input.into());
	}

	args.push("-i".into());
	args.push(soundtrack.into());
	args.extend(
		[
			"-filter_complex",
			&graph,
			"-map",
			"[out]",
			"-map",
			&format!("{audio_index}:a"),
			"-c:v",
			"libx264",
			"-preset",
			"veryfast",
			"-crf",
			"20",
			"-pix_fmt",
			"yuv420p",
			"-c:a",
			"aac",
			"-b:a",
			"192k",
			"-ar",
			"48000",
			"-shortest",
			"-movflags",
			"+faststart",
		]
		.into_iter()
		.map(Into::into),
	);
	args.push(path.into());

	Ok(args)
}

/// The progress bar's height, in pixels.
const BAR: u32 = 6;

/// The track the progress bar runs along: a dark rail with a tick where each shot begins, so
/// the bar shows where the cut is among its shots.
fn progress_rail(
	bounds: &[(usize, usize)],
	total: usize,
	width: u32,
) -> Result<resvg::tiny_skia::Pixmap> {
	let mut svg = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{BAR}"><rect width="{width}" height="{BAR}" fill="#2a2238" fill-opacity="0.6"/>"##
	);

	for (first, _) in bounds.iter().skip(1) {
		let x = *first as f32 / total as f32 * width as f32;
		let _ = write!(
			svg,
			r##"<rect x="{:.1}" width="2" height="{BAR}" fill="#f4ede1" fill-opacity="0.85"/>"##,
			x - 1.0
		);
	}

	svg.push_str("</svg>");

	crate::raster::render(&svg)
}

/// One shot's crop of its plate: the zoom and the top-left corner, as ffmpeg expressions in
/// the output frame number.
struct Crop {
	z: String,
	x: String,
	y: String,
}

/// How far into its move a shot is, eased like the video's camera, at output frame `on`.
fn progress(first: usize, last: usize) -> String {
	let span = last.saturating_sub(first).max(1);
	let p = format!("min(max((on-{first})/{span},0),1)");

	format!("({p})*({p})*(3-2*({p}))")
}

/// The plate a shot is cropped from: the view it covers and the shape it was drawn at.
struct Plate {
	union: View,
	aspect: f32,
	shape: f32,
	width: u32,
}

/// The zoom and corner that frame `start` and, later, `end` out of the plate.
fn crop(
	row: &storyboard::Row,
	start: View,
	end: Option<View>,
	plate: &Plate,
	first: usize,
	last: usize,
	fps: u32,
) -> Crop {
	let scale = plate.width as f32 / plate.union.width(plate.shape);
	let rect = |view: View| {
		(
			(view.centre.x
				- view.width(plate.aspect) / 2.0
				- (plate.union.centre.x - plate.union.width(plate.shape) / 2.0))
				* scale,
			(view.centre.y - view.height / 2.0 - (plate.union.centre.y - plate.union.height / 2.0))
				* scale,
			view.width(plate.aspect) * scale,
			view.height * scale,
		)
	};
	let (sx, sy, sw, _) = rect(start);
	let Some(end) = end else {
		return Crop {
			z: decimal(plate.width as f32 / sw),
			x: decimal(sx),
			y: decimal(sy),
		};
	};
	let (ex, ey, ew, _) = rect(end);
	let move_first = row
		.move_start
		.map_or(first, |time| frame(time, row.start, first, last, fps));
	let move_last = row
		.move_end
		.map_or(last, |time| frame(time, row.start, first, last, fps));

	if move_last <= move_first {
		return Crop {
			z: decimal(plate.width as f32 / sw),
			x: decimal(sx),
			y: decimal(sy),
		};
	}

	let p = progress(move_first, move_last);
	let width = plate.width as f32;

	Crop {
		z: format!(
			"({width})/(({}) + (({}) - ({}))*{p})",
			decimal(sw),
			decimal(ew),
			decimal(sw)
		),
		x: format!(
			"({}) + (({}) - ({}))*{p}",
			decimal(sx),
			decimal(ex),
			decimal(sx)
		),
		y: format!(
			"({}) + (({}) - ({}))*{p}",
			decimal(sy),
			decimal(ey),
			decimal(sy)
		),
	}
}

/// An absolute time as an output frame, kept inside the shot.
fn frame(time: f32, start: f32, first: usize, last: usize, fps: u32) -> usize {
	let fps = f32::from(u16::try_from(fps).unwrap_or(u16::MAX));
	let frame = ((time - start) * fps).round().max(0.0) as usize;

	frame.clamp(0, last.saturating_sub(first).max(1))
}

/// The bounding view that holds both the opening and closing views, and the plate's aspect.
fn union_view(start: View, end: Option<View>, aspect: f32) -> (View, f32) {
	let Some(end) = end else {
		return (start, aspect);
	};
	let rect = |view: View| {
		(
			view.centre.x - view.width(aspect) / 2.0,
			view.centre.y - view.height / 2.0,
			view.centre.x + view.width(aspect) / 2.0,
			view.centre.y + view.height / 2.0,
		)
	};
	let (sl, st, sr, sb) = rect(start);
	let (el, et, er, eb) = rect(end);
	let (left, right) = (sl.min(el), sr.max(er));
	let (top, bottom) = (st.min(et), sb.max(eb));
	let height = bottom - top;

	(
		View {
			centre: pt(f32::midpoint(left, right), f32::midpoint(top, bottom)),
			height,
		},
		(right - left) / height.max(1.0),
	)
}

/// A number for an ffmpeg expression, without exponents.
fn decimal(value: f32) -> String {
	format!("{value:.4}")
}

/// A directory under the system temp folder, removed when the animatic is done.
struct Scratch {
	path: PathBuf,
}

impl Scratch {
	fn new() -> Result<Scratch> {
		let stamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_or(0, |elapsed| elapsed.as_nanos());
		let path =
			std::env::temp_dir().join(format!("fables-animatic-{}-{stamp}", std::process::id()));
		std::fs::create_dir_all(&path)
			.with_context(|| format!("could not create {}", path.display()))?;

		Ok(Scratch { path })
	}
}

impl Drop for Scratch {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.path);
	}
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

	/// A timeline longer than fables will make, built without recording a word.
	fn long_video() -> (Story, Timeline) {
		let pages: String = (0..31).map(|_| "# park\n@pause 60\n").collect();
		let story = crate::story::parse(&format!(
			"+++\ntitle = \"Long\"\nauthor = \"A\"\n+++\n{pages}"
		))
		.unwrap();
		let cast = Cast::default();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let timeline = timeline::build(&story, &cast, &rehearsals, |_| {
			Ok(voice::Speech {
				text: String::new(),
				samples: Vec::new(),
				envelope: Vec::new(),
				track: lipsync::Track::default(),
			})
		})
		.unwrap();

		(story, timeline)
	}

	/// The filters fables may use when it cuts video: all of them part of libavfilter itself,
	/// with no external library, so every ffmpeg build has them. `drawtext` (freetype) and the
	/// like are not here on purpose: Homebrew's ffmpeg, for one, ships without them.
	const ALWAYS_BUILT: &[&str] = &[
		"zoompan", "setsar", "format", "overlay", "concat", "color", "loudnorm",
	];

	fn assert_always_built(args: &[std::ffi::OsString], what: &str) {
		let used = ffmpeg::filters_in(args);

		assert!(!used.is_empty(), "{what} names its filters");

		for filter in &used {
			assert!(
				ALWAYS_BUILT.contains(&filter.as_str()),
				"{what} uses `{filter}`, which not every ffmpeg has: {used:?}"
			);
		}

		assert_eq!(
			ffmpeg::encoders_in(args),
			["libx264", "aac"],
			"{what} encodes with libx264 and aac only"
		);
	}

	#[test]
	fn the_animatic_uses_only_filters_every_ffmpeg_has() {
		assert!(
			ANIMATIC_FILTERS
				.iter()
				.all(|filter| ALWAYS_BUILT.contains(filter))
		);

		// A slow push, so the graph carries a moving crop as well as still ones.
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid x=0.3\nand \
			 then they sat there quietly for a very long while indeed until the sun went down.\n# \
			 park\nThe end.\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let timeline = timeline::build(&story, &cast, &rehearsals, |line| {
			Ok(voice::silent(line, 4.0))
		})
		.unwrap();
		let scratch = tempfile::tempdir().unwrap();

		for format in [Format::Landscape, Format::Portrait] {
			let args = animatic_args(
				&story,
				&cast,
				&timeline,
				format,
				&scratch.path().join("soundtrack.wav"),
				scratch.path(),
				&scratch.path().join("animatic.mp4"),
			)
			.unwrap();
			let used = ffmpeg::filters_in(&args);

			assert!(used.iter().any(|filter| filter == "zoompan"));
			assert!(
				!args
					.iter()
					.any(|arg| arg.to_string_lossy().contains("drawtext")),
				"no drawtext anywhere"
			);
			assert!(
				ANIMATIC_FILTERS
					.iter()
					.all(|filter| used.iter().any(|used| used == filter)),
				"ANIMATIC_FILTERS is the list the graph uses: {used:?}"
			);
			assert_always_built(&args, "the animatic");
		}
	}

	#[test]
	fn the_video_encode_uses_only_filters_every_ffmpeg_has() {
		let soundtrack = Path::new("soundtrack.wav");
		let clip = Clip {
			width: 1920,
			height: 1080,
			fps: 30,
			frames: 30,
			audio: Some(soundtrack),
			label: "16x9",
		};

		assert_always_built(&clip.args(Path::new("out.mp4")), "the video");
	}

	#[test]
	fn a_video_longer_than_half_an_hour_is_refused_before_it_allocates() {
		let (story, timeline) = long_video();
		assert!(timeline.duration > MAX_SECONDS, "{}", timeline.duration);

		let dir = tempfile::tempdir().unwrap();
		let video = dir.path().join("video");
		let error = format!(
			"{:#}",
			make(&story, &timeline, &[], Options { preview: true }, &video).unwrap_err()
		);
		assert!(error.contains("30 minutes"), "{error}");
		assert!(!video.exists(), "nothing is made for a refused video");

		let soundtrack = dir.path().join("soundtrack.wav");
		let error = format!("{:#}", audio::write(&timeline, &soundtrack).unwrap_err());
		assert!(error.contains("30 minutes"), "{error}");
		assert!(!soundtrack.exists(), "no soundtrack is allocated");
	}
}
