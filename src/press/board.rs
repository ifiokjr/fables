//! The style board: the same pages drawn in every art style, side by side, for choosing a look.

use anyhow::Context;
use anyhow::Result;
use rayon::prelude::*;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::PixmapPaint;
use resvg::tiny_skia::Transform;

use crate::art::escape;
use crate::art::style::Style;
use crate::art::style::StyleName;
use crate::book::BookPage;
use crate::book::Side;
use crate::book::page_svg;
use crate::raster;

const TILE: u32 = 640;
const GAP: u32 = 28;
const LABEL: u32 = 96;

/// What each style is, in a few words, for the board's row labels.
fn caption(name: StyleName) -> &'static str {
	match name {
		StyleName::Riso => "riso — two-colour print, wobbly ink, dot screens",
		StyleName::Tv => "tv — clean preschool-TV shapes, no outlines, soft shading",
		StyleName::Bold => "bold — classic cartoon: heavy outlines, cel shadows, gloss",
		StyleName::CutPaper => "cut-paper — collage with torn edges and cast shadows",
		StyleName::Crayon => "crayon — scribbled colour that strays over the lines",
		StyleName::Storybook => "storybook — pencil line over watercolour washes",
		StyleName::Clear => "clear — one even line, flat colour, no shading",
		StyleName::Shadow => "shadow — shadow theatre: ink shadows on a lit screen",
		StyleName::Card => "card — paper theatre: white cut edges, hard shadows, card planes",
	}
}

/// Renders `pages` once per style into one board: a labelled row per style.
pub fn write(pages: &[BookPage], path: &std::path::Path) -> Result<()> {
	let styles: Vec<StyleName> = StyleName::ALL.iter().map(|(_, name)| *name).collect();
	let columns = pages.len() as u32;
	let rows = styles.len() as u32;
	let width = GAP + columns * (TILE + GAP);
	let height = GAP + rows * (LABEL + TILE + GAP);

	let tiles: Vec<Vec<Pixmap>> = styles
		.par_iter()
		.map(|name| {
			let style = Style::named(*name);
			pages
				.iter()
				.map(|page| {
					let mut page = page.clone();
					page.style = style;
					let mut pixmap =
						raster::render(&page_svg(&page, Side::Screen, TILE, TILE, true))?;
					raster::finish(&mut pixmap, &style, 0.06, 0);
					Ok(pixmap)
				})
				.collect::<Result<Vec<_>>>()
		})
		.collect::<Result<_>>()?;

	let mut board = Pixmap::new(width, height).context("style board too large")?;
	board.fill(resvg::tiny_skia::Color::from_rgba8(0x2a, 0x22, 0x38, 0xff));

	for (row, (name, row_tiles)) in styles.iter().zip(&tiles).enumerate() {
		let y = GAP + row as u32 * (LABEL + TILE + GAP);
		let label = raster::render(&format!(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{LABEL}"><text x="{GAP}" y="{}" font-family="{}" font-size="52" fill="#ffc93c">{}</text></svg>"##,
			LABEL - 26,
			raster::DISPLAY_FAMILY,
			escape(caption(*name))
		))?;
		board.draw_pixmap(
			0,
			y as i32,
			label.as_ref(),
			&PixmapPaint::default(),
			Transform::default(),
			None,
		);

		for (column, tile) in row_tiles.iter().enumerate() {
			let x = GAP + column as u32 * (TILE + GAP);
			board.draw_pixmap(
				x as i32,
				(y + LABEL) as i32,
				tile.as_ref(),
				&PixmapPaint::default(),
				Transform::default(),
				None,
			);
		}
	}

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.with_context(|| format!("could not create {}", parent.display()))?;
	}

	std::fs::write(path, raster::png(&board)?)
		.with_context(|| format!("could not write {}", path.display()))
}
