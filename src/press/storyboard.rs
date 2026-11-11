//! The storyboard: the video's camera plan drawn as a comic strip, one labelled panel per
//! camera setup, so a writer can see the cartoon without watching it.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::PixmapPaint;
use resvg::tiny_skia::Transform;

use crate::art::escape;
use crate::cast::Cast;
use crate::raster;
use crate::raster::BODY_FAMILY;
use crate::raster::DISPLAY_FAMILY;
use crate::story::Story;
use crate::story::plain_text;
use crate::video::frames::Format;
use crate::video::frames::Renderer;
use crate::video::timeline::Framing;
use crate::video::timeline::Setup;
use crate::video::timeline::Shot;
use crate::video::timeline::ShotKind;
use crate::video::timeline::Timeline;

/// Panel size: a video frame at about a third scale.
const LANDSCAPE: (u32, u32) = (640, 360);
const PORTRAIT: (u32, u32) = (360, 640);
/// Panels per board page.
const COLUMNS: u32 = 4;
const ROWS: u32 = 6;
const GAP: u32 = 20;
/// Height of the label strip under each panel.
const LABEL: u32 = 92;
/// Where a spoken line is cut off in its label.
const LINE_CHARS: usize = 64;

/// One board panel: the frame drawn at the setup's midpoint, its caption and its spoken line.
pub struct Panel {
	pub frame: Pixmap,
	pub label: String,
	pub line: String,
}

/// Every panel of the board as (caption, spoken line), one per camera setup, drawn at the
/// setup's midpoint.
pub fn panels(
	story: &Story,
	timeline: &Timeline,
	cast: &Cast,
	portrait: bool,
) -> Result<Vec<Panel>> {
	let (width, height) = if portrait { PORTRAIT } else { LANDSCAPE };
	let format = if portrait {
		Format::Portrait
	} else {
		Format::Landscape
	};
	let renderer = Renderer::new(timeline, story, format, width, height);

	timeline
		.shots
		.iter()
		.flat_map(|shot| {
			let setups = shot.setups_for(portrait);
			setups.iter().enumerate().map(move |(index, setup)| {
				let end = setups.get(index + 1).map_or(shot.end, |next| next.time);
				let mid = f32::midpoint(setup.time, end);
				(
					mid,
					caption(shot, setup, end - setup.time, cast),
					spoken(shot, mid),
				)
			})
		})
		.map(|(mid, label, line)| {
			Ok(Panel {
				frame: renderer.render(mid, 0)?,
				label,
				line,
			})
		})
		.collect()
}

/// Writes one board page per 24 camera setups into `dir`, as `storyboard.png`,
/// `storyboard-2.png`, … Returns the paths, in order.
pub fn write(
	story: &Story,
	timeline: &Timeline,
	cast: &Cast,
	portrait: bool,
	dir: &Path,
) -> Result<Vec<PathBuf>> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;

	let (width, height) = if portrait { PORTRAIT } else { LANDSCAPE };
	let panels = panels(story, timeline, cast, portrait)?;

	anyhow::ensure!(
		!panels.is_empty(),
		"the storyboard has no camera setups to draw"
	);

	let cell_w = width + GAP;
	let cell_h = height + LABEL + GAP;
	let board_w = GAP + COLUMNS * cell_w;
	let board_h = GAP + ROWS * cell_h;
	let mut paths = Vec::new();

	for (page, chunk) in panels.chunks(ROWS as usize * COLUMNS as usize).enumerate() {
		let mut board = Pixmap::new(board_w, board_h).context("storyboard page too large")?;
		board.fill(resvg::tiny_skia::Color::from_rgba8(0x2a, 0x22, 0x38, 0xff));

		for (slot, panel) in chunk.iter().enumerate() {
			let column = slot as u32 % COLUMNS;
			let row = slot as u32 / COLUMNS;
			let x = (GAP + column * cell_w) as i32;
			let y = (GAP + row * cell_h) as i32;

			board.draw_pixmap(
				x,
				y,
				panel.frame.as_ref(),
				&PixmapPaint::default(),
				Transform::default(),
				None,
			);
			board.draw_pixmap(
				x,
				y + height as i32,
				label_stamp(&panel.label, &panel.line, width, LABEL)?.as_ref(),
				&PixmapPaint::default(),
				Transform::default(),
				None,
			);
		}

		let stem = if portrait {
			"storyboard-portrait"
		} else {
			"storyboard"
		};
		let name = if page == 0 {
			format!("{stem}.png")
		} else {
			format!("{stem}-{}.png", page + 1)
		};
		let path = dir.join(name);
		std::fs::write(&path, raster::png(&board)?)
			.with_context(|| format!("could not write {}", path.display()))?;
		paths.push(path);
	}

	Ok(paths)
}

/// The strip under a panel: the page, the shot, who it frames, how long it holds, and how the
/// page arrived — then the words spoken over it.
fn label_stamp(label: &str, line: &str, width: u32, height: u32) -> Result<Pixmap> {
	let stamp = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">
			<text x="6" y="38" font-family="{DISPLAY_FAMILY}" font-size="32" fill="#ffc93c">{}</text>
			<text x="6" y="74" font-family="{BODY_FAMILY}" font-size="27" fill="#f4ede1">{}</text>
		</svg>"##,
		escape(label),
		escape(line)
	);

	raster::render(&stamp)
}

/// The panel's caption: page, shot size and subjects, hold, and the transition in.
fn caption(shot: &Shot, setup: &Setup, hold: f32, cast: &Cast) -> String {
	let page = match shot.kind.page() {
		Some(page) => format!("page {}", page + 1),
		None if shot.kind == ShotKind::Title => "title".to_owned(),
		None => "end".to_owned(),
	};

	let framing = match &setup.framing {
		Framing::Auto => "auto".to_owned(),
		Framing::Zoom { zoom, .. } => format!("zoom {zoom:.1}x"),
		Framing::Shot { size, on } => {
			if on.is_empty() {
				size.name().to_owned()
			} else {
				let names: Vec<String> = on
					.iter()
					.map(|id| {
						cast.member(id)
							.map_or_else(|| id.clone(), |member| member.name.clone())
					})
					.collect();
				format!("{} on {}", size.name(), names.join(", "))
			}
		}
	};

	format!(
		"{page} · {framing} · {hold:.1}s · {} in",
		shot.transition.name()
	)
}

/// The words spoken at `time`, or nothing.
fn spoken(shot: &Shot, time: f32) -> String {
	shot.lines
		.iter()
		.find(|line| line.start <= time && time < line.start + line.duration)
		.map(|line| truncate(&plain_text(&line.text), LINE_CHARS))
		.unwrap_or_default()
}

fn truncate(text: &str, limit: usize) -> String {
	if text.chars().count() <= limit {
		return text.to_owned();
	}

	format!("{}…", text.chars().take(limit - 1).collect::<String>())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn labels_name_the_page_shot_and_transition() {
		use crate::cast::Cast;
		use crate::stage::rehearse;
		use crate::video::lipsync::Track;
		use crate::video::timeline;
		use crate::video::voice::SAMPLE_RATE;
		use crate::video::voice::Speech;
		use crate::video::voice::Utterance;

		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid x=0.3\nOne two \
			 three.\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let timeline = timeline::build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; SAMPLE_RATE as usize * line.text.len() / 5],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap();

		let captions: Vec<String> = timeline
			.shots
			.iter()
			.flat_map(|shot| {
				shot.setups_for(false)
					.iter()
					.enumerate()
					.map(|(index, setup)| {
						let end = shot
							.setups_for(false)
							.get(index + 1)
							.map_or(shot.end, |next| next.time);
						caption(shot, setup, end - setup.time, &cast)
					})
			})
			.collect();

		assert!(captions[0].starts_with("title · auto ·"), "{}", captions[0]);
		assert!(captions[1].starts_with("page 1 · "), "{}", captions[1]);
	}

	#[test]
	fn long_lines_are_cut_for_the_strip() {
		let long = "a".repeat(200);
		assert_eq!(truncate(&long, 20).chars().count(), 20);
		assert!(truncate(&long, 20).ends_with('…'));
	}
}
