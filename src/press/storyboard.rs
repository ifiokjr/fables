//! The storyboard: the video's camera plan drawn as a comic strip and written as a shot list,
//! one panel per shot, so a writer can see and read the cartoon without watching it.

use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::PixmapPaint;
use resvg::tiny_skia::Transform;

use crate::cast::Cast;
use crate::raster;
use crate::raster::BODY_FAMILY;
use crate::raster::DISPLAY_FAMILY;
use crate::stage::Travel;
use crate::story::Direction;
use crate::story::Story;
use crate::story::Transition;
use crate::story::plain_text;
use crate::text::display_width;
use crate::video::director;
use crate::video::director::View;
use crate::video::frames::Format;
use crate::video::frames::Renderer;
use crate::video::timeline::Framing;
use crate::video::timeline::Origin;
use crate::video::timeline::Setup;
use crate::video::timeline::Shot;
#[cfg(test)]
use crate::video::timeline::ShotKind;
use crate::video::timeline::Timeline;

/// Panel size: a video frame at about a third scale.
const LANDSCAPE: (u32, u32) = (640, 360);
const PORTRAIT: (u32, u32) = (360, 640);
/// Panels per board page.
const COLUMNS: u32 = 4;
const ROWS: u32 = 6;
const GAP: u32 = 20;
/// Height of the label strip under each panel, wide enough for a wrapped caption.
const LABEL: u32 = 120;
/// Where a spoken line is cut off in a label.
const LINE_CHARS: usize = 74;

/// How a shot is entered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
	/// The page's transition, on a page's first shot.
	Page(Transition),
	/// A cut from the shot before it.
	Cut,
	/// An eased arrival from the shot before it.
	Ease,
	/// A named move into the shot.
	Move(Travel),
}

impl Entry {
	/// How the shot list says it: `wipe in`, `cut in`, `ease in`, `push in`.
	pub fn text(self) -> String {
		match self {
			Entry::Page(transition) => format!("{} in", transition.name()),
			Entry::Cut => "cut in".to_owned(),
			Entry::Ease => "ease in".to_owned(),
			Entry::Move(travel) => format!("{} in", travel.name()),
		}
	}
}

/// One shot as the board and the shot list see it: a setup the camera holds, or a setup that
/// begins with a move, with its duration, source and words. A slow push is part of the shot it
/// pushes in on, not a panel of its own.
#[derive(Clone, Debug)]
pub struct Row {
	/// 1-based panel number, in film order.
	pub number: usize,
	/// The script page (0-based), or `None` for a card.
	pub page: Option<usize>,
	/// The script line the shot starts at; zero for a card.
	pub line: usize,
	/// The shot's start and hold, in seconds from the start of the video.
	pub start: f32,
	pub hold: f32,
	/// The time the panel's frame is drawn at: the start of a move, or the arrival of an ease.
	pub frame_at: f32,
	/// The shot size, without its subjects (`wide`, `auto`, `zoom 1.5x`).
	pub size: String,
	/// Who it frames, by name; `everyone` for a shot that names nobody.
	pub subjects: String,
	/// The move this shot makes, if any.
	pub movement: Option<Travel>,
	/// When the move begins and ends, in seconds from the start of the video.
	pub move_start: Option<f32>,
	pub move_end: Option<f32>,
	/// How the shot is entered.
	pub entry: Entry,
	/// Whether the script wrote the shot or the director chose it.
	pub origin: Origin,
	/// A hold the script's own cut left under the director's minimum.
	pub short: bool,
	/// The words spoken over the shot, plain text.
	pub spoken: Vec<String>,
	/// The first and last setups in the timeline this shot covers. A move that continues the
	/// shot before it is the second setup, and belongs to the same row.
	pub first_setup: usize,
	pub last_setup: usize,
}

impl Row {
	/// The caption the board and the shot list share: page, framing, hold and entry.
	pub fn label(&self) -> String {
		let mut label = format!(
			"{} · {} · {:.1}s · {}",
			self.page_text(),
			self.framing(),
			self.hold,
			self.entry.text()
		);

		if let Some(travel) = self.movement {
			let _ = write!(label, " · {}", travel.name());
		}

		if self.short {
			label.push_str(" · short");
		}

		label
	}

	/// `page 2`, `title` or `end`.
	pub fn page_text(&self) -> String {
		match self.page {
			Some(page) => format!("page {}", page + 1),
			None if self.origin == Origin::Card && self.start < 0.01 => "title".to_owned(),
			None => "end".to_owned(),
		}
	}

	/// The page number alone, for the shot list's table.
	fn page_cell(&self) -> String {
		match self.page {
			Some(page) => (page + 1).to_string(),
			None if self.page_text() == "title" => "title".to_owned(),
			None => "end".to_owned(),
		}
	}

	/// The shot size and its subjects, as one line.
	pub fn framing(&self) -> String {
		if self.subjects.is_empty() || self.subjects == "everyone" {
			self.size.clone()
		} else {
			format!("{} on {}", self.size, self.subjects)
		}
	}

	/// `line 17`, or `—` for a card.
	fn line_label(&self) -> String {
		if self.line == 0 {
			"—".to_owned()
		} else {
			format!("line {}", self.line)
		}
	}

	/// The line number alone, or `—`.
	fn line_cell(&self) -> String {
		if self.line == 0 {
			"—".to_owned()
		} else {
			self.line.to_string()
		}
	}
}

/// One board panel: the frame drawn at the shot's start framing, its caption and its spoken
/// line. A shot that moves also carries the framing it moves to, for the board to draw over it.
pub struct Panel {
	pub row: Row,
	pub frame: Pixmap,
	/// Where the shot ends, when it moves.
	pub end: Option<View>,
	/// The view the frame is drawn with, which the end view is drawn over.
	pub view: View,
	/// The caption, kept for callers that only read text.
	pub label: String,
	/// The first words spoken over the shot.
	pub line: String,
}

/// The plan, one row per shot, in film order.
pub fn plan(_story: &Story, timeline: &Timeline, cast: &Cast, portrait: bool) -> Vec<Row> {
	let mut rows: Vec<Row> = Vec::new();
	let mut last_shot = None;

	for (shot_index, shot) in timeline.shots.iter().enumerate() {
		let setups = shot.setups_for(portrait);

		for (index, setup) in setups.iter().enumerate() {
			let end = setups.get(index + 1).map_or(shot.end, |next| next.time);
			let continues = index > 0
				&& setup.travel.is_move()
				&& setup.framing == setups[index - 1].framing
				&& last_shot == Some(shot_index);

			if continues {
				if let Some(row) = rows.last_mut() {
					row.hold = end - row.start;
					row.movement = Some(setup.travel);
					row.move_start = Some(setup.time);
					row.move_end = Some(end);
					row.last_setup = index;
					row.spoken = spoken(timeline, shot, row.start, end);
				}

				continue;
			}

			let entry = if index == 0 {
				Entry::Page(shot.transition)
			} else {
				match setup.travel {
					Travel::Cut => Entry::Cut,
					Travel::Glide => Entry::Ease,
					move_ => Entry::Move(move_),
				}
			};
			let (size, subjects) = framing_text(&setup.framing, cast);
			let frame_at = if setup.travel.is_move() && index > 0 {
				// A move that changes the framing starts on the view it leaves.
				setup.time
			} else if setup.travel == Travel::Glide && index > 0 {
				(setup.time + glide_length(setup, end)).min(end - 1e-3)
			} else {
				setup.time
			};

			rows.push(Row {
				number: 0,
				page: shot.kind.page(),
				line: setup.line,
				start: setup.time,
				hold: end - setup.time,
				frame_at,
				size,
				subjects,
				movement: setup.travel.is_move().then_some(setup.travel),
				move_start: setup.travel.is_move().then_some(setup.time),
				move_end: setup.travel.is_move().then(|| {
					if setup.over > 0.0 {
						(setup.time + setup.over).min(end)
					} else {
						end
					}
				}),
				entry,
				origin: setup.origin,
				short: end - setup.time + 0.01 < director::MIN_SHOT,
				spoken: spoken(timeline, shot, setup.time, end),
				first_setup: index,
				last_setup: index,
			});
			last_shot = Some(shot_index);
		}
	}

	for (index, row) in rows.iter_mut().enumerate() {
		row.number = index + 1;
	}

	rows
}

/// How long a glide takes, from the direction or the director's default.
fn glide_length(setup: &Setup, end: f32) -> f32 {
	if setup.over > 0.0 {
		setup.over
	} else {
		director::CAMERA_EASE.min((end - setup.time).max(0.01))
	}
}

/// The words spoken over a shot: every line that overlaps it, plain text. A card's speech is
/// read from the timeline, because the title's line carries no caption.
fn spoken(timeline: &Timeline, shot: &Shot, start: f32, end: f32) -> Vec<String> {
	shot.lines
		.iter()
		.filter(|line| {
			line.start < end - 1e-3
				&& line.start + line.duration > start + 1e-3
				&& !(line.text.is_empty() && timeline.speeches[line.speech].text.is_empty())
		})
		.map(|line| {
			let text = if line.text.is_empty() {
				timeline.speeches[line.speech].text.clone()
			} else {
				plain_text(&line.text)
			};

			text.trim().to_owned()
		})
		.filter(|text| !text.is_empty())
		.collect()
}

/// A framing's size and subjects, with cast names in place of ids.
fn framing_text(framing: &Framing, cast: &Cast) -> (String, String) {
	match framing {
		Framing::Auto => ("auto".to_owned(), String::new()),
		Framing::Zoom { zoom, .. } => (format!("zoom {zoom:.1}x"), String::new()),
		Framing::Shot { size, on } => {
			let size = size.name().to_owned();

			if on.is_empty() {
				return (size, "everyone".to_owned());
			}

			let names: Vec<String> = on
				.iter()
				.map(|id| {
					cast.member(id)
						.map_or_else(|| id.clone(), |member| member.name.clone())
				})
				.collect();

			(size, names.join(", "))
		}
	}
}

/// Every panel of the board, drawn at each shot's start framing. A moving shot's end framing
/// comes back with it, so the board can draw it as an outline over the start.
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
	let rows = plan(story, timeline, cast, portrait);

	rows.into_iter()
		.map(|row| {
			let view = renderer.view_at(row.frame_at);
			let end = row
				.movement
				.map(|_| renderer.view_at((row.start + row.hold).max(row.frame_at) - 1e-3));
			let label = row.label();
			let line = truncate(row.spoken.first().map_or("", String::as_str), LINE_CHARS);
			let at = if row.page_text() == "title" {
				row.frame_at + 0.9
			} else {
				row.frame_at
			};

			Ok(Panel {
				frame: renderer.render_view(at, 0, view)?,
				row,
				end,
				view,
				label,
				line,
			})
		})
		.collect()
}

/// Writes one board page per 24 shots into `dir`, as `storyboard.png`, `storyboard-2.png`, …
/// Returns the paths, in order.
pub fn write(
	story: &Story,
	timeline: &Timeline,
	cast: &Cast,
	portrait: bool,
	dir: &Path,
) -> Result<Vec<PathBuf>> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;

	let stem = if portrait {
		"storyboard-portrait"
	} else {
		"storyboard"
	};

	boards(story, timeline, cast, portrait)?
		.iter()
		.enumerate()
		.map(|(page, board)| {
			let name = if page == 0 {
				format!("{stem}.png")
			} else {
				format!("{stem}-{}.png", page + 1)
			};
			let path = dir.join(name);
			std::fs::write(&path, raster::png(board)?)
				.with_context(|| format!("could not write {}", path.display()))?;
			Ok(path)
		})
		.collect()
}

/// Writes the shot list beside the board, and returns its path.
pub fn write_list(text: &str, dir: &Path, portrait: bool) -> Result<PathBuf> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
	let path = list_path(dir, portrait);
	std::fs::write(&path, text).with_context(|| format!("could not write {}", path.display()))?;

	Ok(path)
}

/// Where a shape's shot list sits, beside its board.
pub fn list_path(dir: &Path, portrait: bool) -> PathBuf {
	dir.join(if portrait {
		"storyboard-portrait.md"
	} else {
		"storyboard.md"
	})
}

/// The opening and closing camera views of every row, in stage units. The board draws the
/// opening one and outlines the closing one; an animatic interpolates between them.
pub fn views(
	story: &Story,
	timeline: &Timeline,
	rows: &[Row],
	portrait: bool,
) -> Vec<(View, Option<View>)> {
	let (width, height) = if portrait { PORTRAIT } else { LANDSCAPE };
	let format = if portrait {
		Format::Portrait
	} else {
		Format::Landscape
	};
	let renderer = Renderer::new(timeline, story, format, width, height);

	rows.iter()
		.map(|row| {
			let start = renderer.view_at(row.frame_at);
			let end = row
				.movement
				.map(|_| renderer.view_at((row.start + row.hold).max(row.frame_at) - 1e-3));

			(start, end)
		})
		.collect()
}

/// The first and last frame of every row in an animatic cut at `fps`, in film order. A row's
/// end is the next row's start, so the cut's boundaries are the timeline's.
pub fn frame_bounds(rows: &[Row], fps: u32) -> Vec<(usize, usize)> {
	let fps = f32::from(u16::try_from(fps).unwrap_or(u16::MAX));

	rows.iter()
		.map(|row| {
			let start = (row.start * fps).round() as usize;
			let end = ((row.start + row.hold) * fps).round() as usize;

			(start, end.max(start + 1))
		})
		.collect()
}

/// The board pages: every shot drawn at its start framing and captioned, tiled 24 to a page.
pub fn boards(
	story: &Story,
	timeline: &Timeline,
	cast: &Cast,
	portrait: bool,
) -> Result<Vec<Pixmap>> {
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

	panels
		.chunks(ROWS as usize * COLUMNS as usize)
		.map(|chunk| {
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

				if let Some(end) = panel.end {
					let overlay = overlay(panel.view, end, panel.row.movement, width, height)?;
					board.draw_pixmap(
						x,
						y,
						overlay.as_ref(),
						&PixmapPaint::default(),
						Transform::default(),
						None,
					);
				}

				let stamp = label_stamp(&panel.row, width)?;
				board.draw_pixmap(
					x,
					y + height as i32,
					stamp.as_ref(),
					&PixmapPaint::default(),
					Transform::default(),
					None,
				);
			}

			Ok(board)
		})
		.collect()
}

/// The shot list as Markdown: one row per shot, then the film's length, its shortest and
/// longest shots, and any shot the board marks `short`.
pub fn shot_list(story: &Story, timeline: &Timeline, rows: &[Row], portrait: bool) -> String {
	let shape = if portrait { "portrait" } else { "landscape" };
	let (ratio, other) = if portrait {
		("9:16", "16:9")
	} else {
		("16:9", "9:16")
	};
	let mut out = String::new();
	let _ = write!(
		out,
		"# {} — shot list ({shape})\n\nOne row per shot the camera makes, in film order. `line` \
		 is the story.md line the shot starts at.\n\n",
		story.meta.title
	);
	out.push_str("| # | page | line | at | hold | shot | subjects | move | in | by | spoken |\n");
	out.push_str("|---|------|------|----|------|------|----------|------|----|----|--------|\n");

	for row in rows {
		let _ = writeln!(
			out,
			"| {} | {} | {} | {:.1}s | {:.1}s | {} | {} | {} | {} | {} | {} |",
			row.number,
			row.page_cell(),
			row.line_cell(),
			row.start,
			row.hold,
			row.size,
			cell(&row.subjects),
			row.movement
				.map_or("—".to_owned(), |travel| travel.name().to_owned()),
			row.entry.text(),
			row.origin.name(),
			cell(&row.spoken.join(" / ")),
		);
	}

	let shortest = rows.iter().min_by(|a, b| a.hold.total_cmp(&b.hold));
	let longest = rows.iter().max_by(|a, b| a.hold.total_cmp(&b.hold));
	let short: Vec<&Row> = rows.iter().filter(|row| row.short).collect();
	let length = stamp(timeline.duration);
	let _ = write!(
		out,
		"\n**Length.** {ratio} {length} · {other} {length} ({:.1}s).\n",
		timeline.duration
	);

	if let (Some(shortest), Some(longest)) = (shortest, longest) {
		let _ = write!(
			out,
			"\n**Shots.** {} · shortest {:.1}s (#{}, {}) · longest {:.1}s (#{}, {}).\n",
			rows.len(),
			shortest.hold,
			shortest.number,
			shortest.page_text(),
			longest.hold,
			longest.number,
			longest.page_text()
		);
	}

	if short.is_empty() {
		out.push_str("\n**Short.** None.\n");
	} else {
		let marked: Vec<String> = short
			.iter()
			.map(|row| format!("#{} ({:.1}s, {})", row.number, row.hold, row.page_text()))
			.collect();
		let _ = writeln!(out, "\n**Short.** {}", marked.join(", "));
	}

	out
}

/// What pinning wrote into a script.
pub struct Pinned {
	/// The script with every pinned direction written in.
	pub script: String,
	/// The 1-based pages that were pinned, in script order.
	pub pages: Vec<usize>,
	/// How many `@camera` directions were written.
	pub count: usize,
	/// Anything the writer should know: pages left alone, and where portrait differs.
	pub notes: Vec<String>,
}

/// Writes the director's shots into the script as `@camera` directions, each before the line
/// or direction that starts it, so the writer can edit, delete or reorder them. A pinned
/// direction reproduces the director's choice: its framing, move, landing time and shake.
/// Pages that already direct their own camera are left exactly as written.
pub fn pin(
	source: &str,
	story: &Story,
	timeline: &Timeline,
	pages: Option<&[usize]>,
) -> Result<Pinned> {
	if story.meta.video.direction != Direction::Auto {
		anyhow::bail!(
			"pinning needs `[video] direction = \"auto\"`; the locked camera has no script 			 \
			 vocabulary"
		);
	}

	let newline = if source.contains("\r\n") {
		"\r\n"
	} else {
		"\n"
	};
	let mut insertions: Vec<(usize, String)> = Vec::new();
	let mut pinned_pages = Vec::new();
	let mut notes = Vec::new();

	for shot in &timeline.shots {
		let Some(page) = shot.kind.page() else {
			continue;
		};
		let number = page + 1;

		if pages.is_some_and(|pages| !pages.contains(&number)) {
			continue;
		}

		if story.pages[page].directed_by_camera() {
			notes.push(format!(
				"page {number} already directs its own camera; left as written"
			));
			continue;
		}

		pinned_pages.push(number);

		if plans_differ(&shot.setups, &shot.portrait_setups) {
			notes.push(format!(
				"page {number} plans different shots for portrait; pinned the landscape plan"
			));
		}

		// The page's opening camera goes straight after its heading, so it stays inside the
		// page; every later one goes before the line or direction that starts it. A move that
		// starts inside the opening hold (a slow push on a long establishing shot) goes after
		// the page's opening directions, before its first line of narration.
		let heading = story.pages[page].line;
		let first_line = story.pages[page]
			.beats
			.iter()
			.find_map(|beat| {
				match beat {
					crate::story::Beat::Say { line, .. } => Some(*line),
					_ => None,
				}
			})
			.unwrap_or_else(|| {
				story
					.pages
					.get(page + 1)
					.map_or_else(|| source.lines().count() + 1, |next| next.line)
			});

		for (index, setup) in shot.setups.iter().enumerate() {
			let anchor = if index == 0 {
				heading + 1
			} else if setup.line <= heading {
				first_line
			} else if setup.line > 0 {
				setup.line
			} else {
				notes.push(format!(
					"page {number}: a camera setup has no script line; left it out"
				));
				continue;
			};

			insertions.push((
				anchor,
				format!("{}{newline}", camera_direction(shot, setup, index)?),
			));
		}
	}

	if pinned_pages.is_empty() && notes.is_empty() {
		anyhow::bail!("no page has a shot to pin");
	}

	let lines: Vec<&str> = source.split_inclusive('\n').collect();
	let count = insertions.len();
	let mut script = String::with_capacity(source.len() + count * 56);

	for (index, line) in lines.iter().enumerate() {
		for (_, text) in insertions.iter().filter(|(at, _)| *at == index + 1) {
			script.push_str(text);
		}

		script.push_str(line);
	}

	let appended: Vec<&String> = insertions
		.iter()
		.filter(|(at, _)| *at > lines.len())
		.map(|(_, text)| text)
		.collect();

	if !appended.is_empty() && !script.is_empty() && !script.ends_with('\n') {
		script.push_str(newline);
	}

	for (index, text) in appended.iter().enumerate() {
		if !source.ends_with('\n') && index + 1 == appended.len() {
			script.push_str(text.trim_end_matches('\n'));
		} else {
			script.push_str(text);
		}
	}

	Ok(Pinned {
		script,
		pages: pinned_pages,
		count,
		notes,
	})
}

/// Removes every pinned direction and gives the script back exactly as it was before pinning.
/// The marker is the trailing `// pinned` pinning writes, so hand-written comments stay.
pub fn unpin(source: &str) -> (String, usize) {
	let mut out = String::with_capacity(source.len());
	let mut removed = 0;

	for piece in source.split_inclusive('\n') {
		let text = piece.trim_end_matches(['\n', '\r']).trim_end();

		if text.starts_with("@camera") && text.ends_with("// pinned") {
			removed += 1;

			// A pinned line that ended the file without a newline had one put in front of it;
			// take that back so the file matches its original bytes.
			if !piece.ends_with('\n') {
				out.pop();

				if out.ends_with('\r') {
					out.pop();
				}
			}

			continue;
		}

		out.push_str(piece);
	}

	(out, removed)
}

/// The `@camera` direction that reproduces one setup exactly.
fn camera_direction(shot: &Shot, setup: &Setup, index: usize) -> Result<String> {
	let mut tokens = Vec::new();

	match &setup.framing {
		Framing::Shot { size, on } => {
			tokens.push(format!("shot={}", size.name()));

			if !on.is_empty() {
				tokens.push(format!("on={}", on.join(",")));
			}
		}
		Framing::Zoom { zoom, look } => {
			// A float's own display round-trips exactly; the pin must not round a look point.
			tokens.push(format!("zoom={zoom}"));
			tokens.push(format!("look={},{}", look.0, look.1));
		}
		Framing::Auto => {
			anyhow::bail!("a shot with no shot size or zoom has no `@camera` word")
		}
	}

	match setup.travel {
		Travel::Cut => tokens.push("cut".to_owned()),
		Travel::Glide => tokens.push("ease".to_owned()),
		travel => {
			tokens.push(format!("move={}", travel.name()));
			tokens.push(format!("over={}", number(setup.over)));
		}
	}

	if setup.shake > 0.0 {
		tokens.push(format!("shake={}", number(setup.shake)));
	}

	if index == 0 {
		// The heading may carry a shake the director's opening shot does not use.
		if setup.shake <= 0.0 {
			tokens.push("shake=0".to_owned());
		}
	}

	let natural = natural_time(shot, setup, index);

	if (natural - setup.time).abs() > 5e-4 {
		tokens.push(format!("after={}", number(setup.time - shot.start)));
	}

	Ok(format!("@camera {} // pinned", tokens.join(" ")))
}

/// When the setup would land if the direction were written at its own line, without `after=`.
fn natural_time(shot: &Shot, setup: &Setup, index: usize) -> f32 {
	if index == 0 {
		return shot.start;
	}

	let beat = beat_time(shot, setup.line).unwrap_or(setup.time);

	match setup.travel {
		Travel::Cut => director::place_cut(beat, shot, shot.start).unwrap_or(beat),
		Travel::Glide => (beat - director::CAMERA_LEAD).max(shot.start),
		_ => beat,
	}
}

/// The time the line or direction at `line` is reached.
fn beat_time(shot: &Shot, line: usize) -> Option<f32> {
	shot.keys
		.iter()
		.find(|key| key.line == line)
		.map(|key| key.time)
		.or_else(|| {
			shot.lines
				.iter()
				.find(|spoken| spoken.line == line)
				.map(|spoken| spoken.start)
		})
}

fn plans_differ(landscape: &[Setup], portrait: &[Setup]) -> bool {
	landscape.len() != portrait.len()
		|| landscape.iter().zip(portrait).any(|(a, b)| {
			a.framing != b.framing || a.travel != b.travel || (a.time - b.time).abs() > 0.01
		})
}

/// A number as the script writes it: no exponent, no trailing zeros.
fn number(value: f32) -> String {
	let text = format!("{value:.3}");
	let text = text.trim_end_matches('0').trim_end_matches('.');

	if text.is_empty() || text == "-0" {
		"0".to_owned()
	} else {
		text.to_owned()
	}
}

/// A table cell: pipes escaped, one line.
fn cell(text: &str) -> String {
	let trimmed = text.trim();

	if trimmed.is_empty() {
		return "—".to_owned();
	}

	trimmed.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// A clock reading for a film length, `1:32`.
fn stamp(seconds: f32) -> String {
	let whole = seconds.max(0.0) as u32;

	format!("{}:{:02}", whole / 60, whole % 60)
}

/// The overlay a moving panel carries: the framing it moves to, as a thin outline, with an
/// arrow and the move's name.
fn overlay(
	start: View,
	end: View,
	movement: Option<Travel>,
	width: u32,
	height: u32,
) -> Result<Pixmap> {
	let (w, h) = (width as f32, height as f32);
	let aspect = w / h;
	let start_w = start.width(aspect);
	let scale = w / start_w;
	let left = start.centre.x - start_w / 2.0;
	let top = start.centre.y - start.height / 2.0;
	let x = (end.centre.x - end.width(aspect) / 2.0 - left) * scale;
	let y = (end.centre.y - end.height / 2.0 - top) * scale;
	let end_w = end.width(aspect) * scale;
	let end_h = end.height * scale;

	// Keep the outline on the panel, even when a pull widens past it.
	let clip_x = x.max(0.0);
	let clip_y = y.max(0.0);
	let clip_w = (x + end_w).min(w) - clip_x;
	let clip_h = (y + end_h).min(h) - clip_y;

	if clip_w < 4.0 || clip_h < 4.0 {
		return raster::render(&format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"/>"#
		));
	}

	let start_centre = (w / 2.0, h / 2.0);
	let end_centre = (clip_x + clip_w / 2.0, clip_y + clip_h / 2.0);
	let (dx, dy) = (end_centre.0 - start_centre.0, end_centre.1 - start_centre.1);
	let length = dx.hypot(dy);
	let (ux, uy) = if length < 8.0 {
		(0.707, 0.707)
	} else {
		(dx / length, dy / length)
	};
	let tip = if length < 8.0 {
		(end_centre.0 - 40.0 * ux, end_centre.1 - 40.0 * uy)
	} else {
		end_centre
	};
	let tail = (tip.0 - 54.0 * ux, tip.1 - 54.0 * uy);
	let head = 12.0;
	let wing = 7.0;
	let arrow = format!(
		"M {:.1} {:.1} L {:.1} {:.1} M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z",
		tail.0,
		tail.1,
		tip.0,
		tip.1,
		tip.0 - head * ux + wing * -uy,
		tip.1 - head * uy + wing * ux,
		tip.0,
		tip.1,
		tip.0 - head * ux - wing * -uy,
		tip.1 - head * uy - wing * ux,
	);
	let name = movement.map_or("move", Travel::name);

	raster::render(&format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">
			<rect x="{clip_x:.1}" y="{clip_y:.1}" width="{clip_w:.1}" height="{clip_h:.1}" fill="none" stroke="#ffc93c" stroke-width="3" stroke-dasharray="8 6"/>
			<path d="{arrow}" fill="none" stroke="#ffc93c" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
			<text x="{:.1}" y="{:.1}" font-family="{DISPLAY_FAMILY}" font-size="24" fill="#ffc93c" paint-order="stroke" stroke="#2a2238" stroke-width="4">{name}</text>
		</svg>"##,
		(clip_x + 8.0).min(w - 80.0),
		(clip_y + 28.0).min(h - 10.0),
	))
}

/// The strip under a panel: the panel and line number, its page and framing (wrapped to two
/// lines), its hold, entry, move and source, then the words spoken over it.
fn label_stamp(row: &Row, width: u32) -> Result<Pixmap> {
	let caption = format!("#{} · {} · {}", row.number, row.page_text(), row.framing());
	let spoken = row
		.spoken
		.first()
		.map_or(String::new(), |line| truncate(line, LINE_CHARS));
	let mut label = format!(
		"{} · {:.1}s · {}",
		row.line_label(),
		row.hold,
		row.entry.text()
	);

	if let Some(travel) = row.movement {
		let _ = write!(label, " · {}", travel.name());
	}

	if row.short {
		label.push_str(" · short");
	}

	let lines = wrap(&caption, width as f32 - 12.0, 25.0, 2);
	let (first, second) = (
		lines.first().cloned().unwrap_or_default(),
		lines.get(1).cloned(),
	);
	let mut stamp = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{LABEL}">
			<text x="6" y="30" font-family="{DISPLAY_FAMILY}" font-size="25" fill="#ffc93c">{}</text>"##,
		escape_text(&first)
	);

	if let Some(second) = &second {
		let _ = write!(
			stamp,
			r##"<text x="6" y="56" font-family="{DISPLAY_FAMILY}" font-size="25" fill="#ffc93c">{}</text>"##,
			escape_text(second)
		);
	}

	let detail_y = if second.is_some() { 82 } else { 58 };
	let _ = write!(
		stamp,
		r##"<text x="6" y="{detail_y}" font-family="{BODY_FAMILY}" font-size="20" fill="#f4ede1">{}</text>
			<text x="6" y="{spoken_y}" font-family="{BODY_FAMILY}" font-size="19" fill="#cfc3b4">{}</text>
		</svg>"##,
		escape_text(&label),
		escape_text(&spoken),
		spoken_y = LABEL as i32 - 14,
	);

	raster::render(&stamp)
}

/// Wraps a caption into at most `limit` lines that fit `width`, cutting the last with `…`.
fn wrap(text: &str, width: f32, size: f32, limit: usize) -> Vec<String> {
	let mut lines: Vec<String> = Vec::new();
	let mut current = String::new();

	for word in text.split_whitespace() {
		let candidate = if current.is_empty() {
			word.to_owned()
		} else {
			format!("{current} {word}")
		};

		if display_width(&candidate, size) <= width {
			current = candidate;
		} else {
			if !current.is_empty() {
				lines.push(std::mem::take(&mut current));
			}

			word.clone_into(&mut current);
		}
	}

	if !current.is_empty() {
		lines.push(current);
	}

	if lines.len() > limit {
		lines.truncate(limit);
	}

	for line in &mut lines {
		if display_width(line, size) > width {
			while display_width(line, size) > width - size && !line.is_empty() {
				line.pop();
			}

			line.push('…');
		}
	}

	lines
}

/// The words over a shot, cut for the panel strip.
fn truncate(text: &str, limit: usize) -> String {
	if text.chars().count() <= limit {
		return text.to_owned();
	}

	format!("{}…", text.chars().take(limit - 1).collect::<String>())
}

/// Escapes text for an SVG `<text>` element.
fn escape_text(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cast::Cast;
	use crate::stage::rehearse;
	use crate::video::lipsync::Track;
	use crate::video::timeline;
	use crate::video::voice::SAMPLE_RATE;
	use crate::video::voice::Speech;
	use crate::video::voice::Utterance;

	fn timeline_of(story: &Story, cast: &Cast) -> Timeline {
		let rehearsals = rehearse(story, cast).unwrap();

		timeline::build(story, cast, &rehearsals, |line: &Utterance<'_>| {
			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; SAMPLE_RATE as usize * line.text.len() / 5],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap()
	}

	#[test]
	fn the_shot_list_has_a_row_for_every_shot() {
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid x=0.3 \
			 talk\nHello there today my friend.\n@kid x=0.6 talk\nWhy hello to you too.\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let timeline = timeline_of(&story, &cast);
		let rows = plan(&story, &timeline, &cast, false);

		assert!(
			rows.len() >= 3,
			"the title, the page and the end: {}",
			rows.len()
		);
		assert_eq!(rows[0].page_text(), "title");
		assert_eq!(rows[0].origin, Origin::Card);
		assert!(rows.iter().any(|row| row.page == Some(0)));

		let text = shot_list(&story, &timeline, &rows, false);
		assert!(text.contains(
			"| # | page | line | at | hold | shot | subjects | move | in | by | spoken |"
		));
		assert!(text.contains("**Length.** 16:9"));
		assert!(text.contains("**Shots.**"));
		assert!(text.contains("Hello there today my friend."), "{text}");
	}

	#[test]
	fn a_slow_push_is_one_shot_not_two_panels() {
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid x=0.3\nand \
			 then they sat there quietly for a very long while indeed until the sun went down.\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		// Two words a second: long enough for the director's slow push.
		let rehearsals = rehearse(&story, &cast).unwrap();
		let timeline = timeline::build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; SAMPLE_RATE as usize * 4 * line.text.split_whitespace().count()],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap();

		let page = timeline
			.shots
			.iter()
			.find(|shot| shot.kind == ShotKind::Page(0))
			.unwrap();
		assert!(page.setups.len() >= 2, "the hold pushes in");
		assert_eq!(page.setups[1].travel, Travel::Push);

		let rows = plan(&story, &timeline, &cast, false);
		let page_rows: Vec<&Row> = rows.iter().filter(|row| row.page == Some(0)).collect();

		assert_eq!(
			page_rows.len(),
			page.setups.len() - 1,
			"the push is part of the hold before it: {:?}",
			page_rows.iter().map(|row| row.label()).collect::<Vec<_>>()
		);
		assert_eq!(page_rows.last().unwrap().movement, Some(Travel::Push));
	}

	#[test]
	fn only_a_page_s_first_shot_carries_its_transition() {
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park \
			 transition=iris\n@kid x=0.3 talk\nHello there today my friend.\n@mum x=0.7 talk\nWhy \
			 hello to you too.\n",
		)
		.unwrap();
		let cast =
			Cast::parse("[kid]\nname = \"Kid\"\n[mum]\nname = \"Mum\"\nkind = \"grown-up\"\n")
				.unwrap();
		let timeline = timeline_of(&story, &cast);
		let rows = plan(&story, &timeline, &cast, false);
		let page: Vec<&Row> = rows.iter().filter(|row| row.page == Some(0)).collect();

		assert_eq!(page[0].entry, Entry::Page(Transition::Iris));
		assert!(page[0].label().contains("iris in"));
		assert!(
			page[1..]
				.iter()
				.all(|row| row.entry != Entry::Page(Transition::Iris)),
			"{:?}",
			page.iter().map(|row| row.label()).collect::<Vec<_>>()
		);
	}

	#[test]
	fn labels_state_page_framing_hold_and_entry() {
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid x=0.3\nOne two \
			 three.\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let timeline = timeline_of(&story, &cast);
		let rows = plan(&story, &timeline, &cast, false);
		let page = rows.iter().find(|row| row.page == Some(0)).unwrap();

		assert!(page.label().starts_with("page 1 · "), "{}", page.label());
		assert!(page.label().contains(" · "), "{}", page.label());
		assert_eq!(
			page.label().split(" · ").nth(1),
			Some(page.framing().as_str())
		);
		assert!(page.label().contains(" in"), "{}", page.label());
	}

	#[test]
	fn long_lines_are_cut_for_the_strip() {
		let long = "a".repeat(200);
		assert_eq!(truncate(&long, 20).chars().count(), 20);
		assert!(truncate(&long, 20).ends_with('…'));
	}

	#[test]
	fn captions_wrap_to_two_lines() {
		let short = wrap("page 2 · wide on Mo", 300.0, 25.0, 2);
		assert_eq!(short.len(), 1);

		let long = wrap(
			"page 12 · medium on Mo Mumble, Hana of Kiteharbour and the dog from next door",
			260.0,
			25.0,
			2,
		);
		assert!(long.len() <= 2, "{long:?}");
		assert!(
			long.iter().all(|line| display_width(line, 25.0) <= 260.0),
			"{long:?}"
		);
	}

	#[test]
	fn a_hold_under_the_minimum_is_marked_short() {
		let story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid \
			 x=0.3\nHi.\n@camera shot=close on=kid\nBye.\n@camera shot=wide\n!\n",
		)
		.unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let timeline = timeline_of(&story, &cast);
		let rows = plan(&story, &timeline, &cast, false);

		let short = rows
			.iter()
			.find(|row| row.short)
			.expect("the written cut leaves one short hold");
		assert!(short.hold < director::MIN_SHOT, "{}", short.label());
		assert!(short.label().ends_with(" · short"), "{}", short.label());
	}
}
