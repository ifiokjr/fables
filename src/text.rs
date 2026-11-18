//! Page text layout.
//!
//! SVG has no line wrapping, and the ePub's live text must break exactly where the printed page
//! does, so lines are laid out once here using the embedded fonts' real advance widths. The
//! result is drawn as SVG `<text>` for print and positioned HTML for the ePub.

use std::fmt::Write as _;

use crate::art::Color;
use crate::art::escape;
use crate::art::palette;
use crate::raster;
use crate::story::RunStyle;
use crate::story::runs;

/// A rectangle in stage units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
	pub x: f32,
	pub y: f32,
	pub w: f32,
	pub h: f32,
}

/// Which edge of the area the block hugs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
	Top,
	Bottom,
	Middle,
}

/// A styled piece of a line, positioned on the stage.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
	pub text: String,
	pub style: RunStyle,
	/// Whether a word break (space or line break) comes before this piece.
	pub space_before: bool,
	pub x: f32,
	pub baseline: f32,
	pub size: f32,
	pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
	pub pieces: Vec<Piece>,
	/// The bounding box the lines actually occupy.
	pub bounds: Area,
	pub base_size: f32,
}

struct Metrics {
	face: ttf_parser::Face<'static>,
	per_em: f32,
}

impl Metrics {
	fn new(data: &'static [u8]) -> Metrics {
		let face = ttf_parser::Face::parse(data, 0)
			.unwrap_or_else(|error| panic!("embedded font is corrupt: {error}"));
		let per_em = f32::from(face.units_per_em());

		Metrics { face, per_em }
	}

	fn width(&self, text: &str, size: f32) -> f32 {
		let units: u32 = text
			.chars()
			.map(|ch| {
				self.face
					.glyph_index(ch)
					.and_then(|glyph| self.face.glyph_hor_advance(glyph))
					.map_or(u32::from(self.face.units_per_em()) / 2, u32::from)
			})
			.sum();

		units as f32 / self.per_em * size
	}
}

fn metrics(style: RunStyle) -> &'static Metrics {
	use std::sync::OnceLock;

	static PLAIN: OnceLock<Metrics> = OnceLock::new();
	static BOLD: OnceLock<Metrics> = OnceLock::new();
	static SHOUT: OnceLock<Metrics> = OnceLock::new();

	match style {
		RunStyle::Plain => PLAIN.get_or_init(|| Metrics::new(raster::BODY_FONT)),
		RunStyle::Bold => BOLD.get_or_init(|| Metrics::new(raster::BODY_BOLD_FONT)),
		RunStyle::Shout => SHOUT.get_or_init(|| Metrics::new(raster::DISPLAY_FONT)),
	}
}

/// How much bigger shouted words are than the body text.
pub const SHOUT_SCALE: f32 = 1.3;
const LINE_HEIGHT: f32 = 1.3;
const PARAGRAPH_GAP: f32 = 0.45;

fn size_of(style: RunStyle, base: f32) -> f32 {
	if style == RunStyle::Shout {
		base * SHOUT_SCALE
	} else {
		base
	}
}

/// A word made of one or more styled segments (`"**NOPE**,"` is one word, three segments).
type Word = Vec<(String, RunStyle)>;

fn words(paragraph: &str, plain: RunStyle) -> Vec<Word> {
	let mut out: Vec<Word> = vec![Vec::new()];

	for run in runs(paragraph) {
		for (i, part) in run.text.split(' ').enumerate() {
			if i > 0 && !out.last().is_some_and(Vec::is_empty) {
				out.push(Vec::new());
			}

			if !part.is_empty() {
				let style = if run.style == RunStyle::Plain {
					plain
				} else {
					run.style
				};
				out.last_mut()
					.unwrap_or_else(|| unreachable!())
					.push((part.to_owned(), style));
			}
		}
	}

	out.retain(|word| !word.is_empty());
	out
}

fn word_width(word: &Word, base: f32) -> f32 {
	word.iter()
		.map(|(text, style)| metrics(*style).width(text, size_of(*style, base)))
		.sum()
}

fn word_height(word: &Word, base: f32) -> f32 {
	word.iter()
		.map(|(_, style)| size_of(*style, base))
		.fold(base, f32::max)
}

/// Lays paragraphs out inside `area`, shrinking the type (down to 70%) until it fits.
pub fn layout(paragraphs: &[&str], area: Area, base_size: f32, anchor: Anchor) -> Block {
	layout_styled(paragraphs, area, base_size, anchor, RunStyle::Plain)
}

/// Like [`layout`], but unmarked text is set in `plain` (captions use bold throughout).
pub fn layout_styled(
	paragraphs: &[&str],
	area: Area,
	base_size: f32,
	anchor: Anchor,
	plain: RunStyle,
) -> Block {
	let smallest = base_size * 0.7;
	let mut size = base_size;

	loop {
		let block = layout_at(paragraphs, area, size, anchor, plain);

		if block.bounds.h <= area.h || size <= smallest {
			return block;
		}

		size = (size * 0.95).max(smallest);
	}
}

fn layout_at(paragraphs: &[&str], area: Area, base: f32, anchor: Anchor, plain: RunStyle) -> Block {
	let space = metrics(RunStyle::Plain).width(" ", base);
	let mut lines: Vec<(Vec<Word>, f32, f32, bool)> = Vec::new();

	for (p, paragraph) in paragraphs.iter().enumerate() {
		let mut line: Vec<Word> = Vec::new();
		let mut width = 0.0;

		for word in words(paragraph, plain) {
			let w = word_width(&word, base);
			let extra = if line.is_empty() { w } else { space + w };

			if !line.is_empty() && width + extra > area.w {
				let height = line
					.iter()
					.map(|word| word_height(word, base))
					.fold(base, f32::max);
				lines.push((std::mem::take(&mut line), width, height, false));
				width = w;
			} else {
				width += extra;
			}

			line.push(word);
		}

		if !line.is_empty() {
			let height = line
				.iter()
				.map(|word| word_height(word, base))
				.fold(base, f32::max);
			lines.push((line, width, height, p + 1 < paragraphs.len()));
		}
	}

	let total: f32 = lines
		.iter()
		.map(|(_, _, height, gap)| {
			height * LINE_HEIGHT + if *gap { base * PARAGRAPH_GAP } else { 0.0 }
		})
		.sum();
	let top = match anchor {
		Anchor::Top => area.y,
		Anchor::Bottom => area.y + area.h - total,
		Anchor::Middle => area.y + (area.h - total) / 2.0,
	};
	let mut pieces = Vec::new();
	let mut cursor_y = top;
	let mut widest: f32 = 0.0;

	for (words, width, height, gap) in &lines {
		let baseline = cursor_y + height * 0.92;
		let mut x = area.x + (area.w - width) / 2.0;
		widest = widest.max(*width);

		for (i, word) in words.iter().enumerate() {
			if i > 0 {
				x += space;
			}

			for (j, (text, style)) in word.iter().enumerate() {
				let size = size_of(*style, base);
				let w = metrics(*style).width(text, size);
				let space_before = j == 0 && !pieces.is_empty();

				// Consecutive same-style words on a line become one run: fewer elements, and
				// the PDF and ePub keep whole phrases selectable.
				if let Some(last) = pieces.last_mut().filter(|last: &&mut Piece| {
					last.style == *style && (last.baseline - baseline).abs() < 0.01
				}) {
					if space_before {
						last.text.push(' ');
					}

					last.text.push_str(text);
					last.width = x + w - last.x;
				} else {
					pieces.push(Piece {
						text: text.clone(),
						style: *style,
						space_before,
						x,
						baseline,
						size,
						width: w,
					});
				}

				x += w;
			}
		}

		cursor_y += height * LINE_HEIGHT + if *gap { base * PARAGRAPH_GAP } else { 0.0 };
	}

	Block {
		pieces,
		bounds: Area {
			x: area.x + (area.w - widest) / 2.0,
			y: top,
			w: widest,
			h: total,
		},
		base_size: base,
	}
}

/// The colours a block is printed in.
#[derive(Clone, Copy, Debug)]
pub struct Ink {
	pub body: Color,
	pub shout: Color,
}

impl Default for Ink {
	fn default() -> Self {
		Self {
			body: palette::INK,
			shout: palette::TOMATO,
		}
	}
}

/// The advance width of `text` set in `style`'s face at `size`, for callers that lay labels
/// out over rendered frames rather than through [`layout`].
pub fn width(text: &str, style: RunStyle, size: f32) -> f32 {
	metrics(style).width(text, size)
}

/// The advance width of `text` set in the display face at `size`, for callers that lay
/// labels out over rendered frames (the rig videos' titles) rather than through [`layout`].
pub fn display_width(text: &str, size: f32) -> f32 {
	width(text, RunStyle::Shout, size)
}

/// Font family, weight and colour for a style.
pub fn font(style: RunStyle, ink: Ink) -> (&'static str, u16, Color) {
	match style {
		RunStyle::Plain => (raster::BODY_FAMILY, 400, ink.body),
		RunStyle::Bold => (raster::BODY_FAMILY, 700, ink.body),
		RunStyle::Shout => (raster::DISPLAY_FAMILY, 400, ink.shout),
	}
}

/// Renders a block as SVG `<text>` elements.
pub fn svg(block: &Block, ink: Ink) -> String {
	let mut out = String::new();

	for piece in &block.pieces {
		let (family, weight, color) = font(piece.style, ink);
		let _ = write!(
			out,
			r#"<text x="{:.1}" y="{:.1}" font-family="{family}" font-weight="{weight}" font-size="{:.1}" fill="{color}" xml:space="preserve">{}</text>"#,
			piece.x,
			piece.baseline,
			piece.size,
			escape(&piece.text)
		);
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	const AREA: Area = Area {
		x: 200.0,
		y: 150.0,
		w: 2000.0,
		h: 700.0,
	};

	#[test]
	fn short_text_fits_on_one_centred_line() {
		let block = layout(&["Hello there."], AREA, 100.0, Anchor::Top);
		assert_eq!(block.pieces.len(), 1);
		let piece = &block.pieces[0];
		let centre = piece.x + piece.width / 2.0;
		assert!((centre - 1200.0).abs() < 1.0, "centre {centre}");
		assert!(
			piece.width > 300.0 && piece.width < 700.0,
			"width {}",
			piece.width
		);
	}

	#[test]
	fn long_text_wraps_inside_the_area() {
		let text = "Stanley did not brush on Monday. He did not brush on Tuesday. He did not \
		            brush on Wednesday, Thursday or Fun-day.";
		let block = layout(&[text], AREA, 100.0, Anchor::Top);
		let lines: std::collections::BTreeSet<i32> = block
			.pieces
			.iter()
			.map(|piece| piece.baseline as i32)
			.collect();
		assert!(lines.len() >= 3, "{} lines", lines.len());

		for piece in &block.pieces {
			assert!(
				piece.x >= AREA.x - 0.5 && piece.x + piece.width <= AREA.x + AREA.w + 0.5,
				"{piece:?}"
			);
		}
	}

	#[test]
	fn shouted_words_are_bigger_and_stay_attached_to_punctuation() {
		let block = layout(&["\"**NOPE**,\" said Stanley."], AREA, 100.0, Anchor::Top);
		let nope = block
			.pieces
			.iter()
			.find(|piece| piece.text == "NOPE")
			.unwrap();
		let comma = block
			.pieces
			.iter()
			.find(|piece| piece.text.starts_with(",\""))
			.unwrap();
		assert_eq!(comma.text, ",\" said Stanley.");
		assert!((nope.size - 130.0).abs() < 0.01);
		assert!(
			(comma.x - (nope.x + nope.width)).abs() < 0.01,
			"comma must follow NOPE with no space"
		);
	}

	#[test]
	fn oversized_text_shrinks_to_fit() {
		let text = "word ".repeat(200);
		let block = layout(&[text.as_str()], AREA, 100.0, Anchor::Top);
		assert!(block.base_size < 100.0);
		assert!(block.base_size >= 69.9);
	}

	#[test]
	fn bottom_anchor_ends_at_the_area_bottom() {
		let block = layout(&["One line."], AREA, 100.0, Anchor::Bottom);
		assert!((block.bounds.y + block.bounds.h - (AREA.y + AREA.h)).abs() < 0.5);
	}
}
