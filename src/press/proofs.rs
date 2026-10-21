//! Quick-look PNGs of every page plus a contact sheet, for reviewing a draft.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use rayon::prelude::*;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::PixmapPaint;
use resvg::tiny_skia::Transform;

use crate::book::BookPage;
use crate::book::Side;
use crate::book::page_svg;
use crate::raster;

/// Pixel size of each proof page.
const PROOF: u32 = 1200;
/// Thumbnails per row on the contact sheet.
const COLUMNS: u32 = 6;
const THUMB: u32 = 360;

/// Writes `page-NN-label.png` for every page and `contact-sheet.png` into `dir`.
pub fn write(pages: &[BookPage], dir: &Path) -> Result<()> {
	std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;

	let rendered: Vec<Pixmap> = pages
		.par_iter()
		.enumerate()
		.map(|(index, page)| {
			let mut pixmap = raster::render(&page_svg(page, Side::Screen, PROOF, PROOF, true))?;
			raster::finish(&mut pixmap, &page.style, 0.07, 0);
			let file = dir.join(format!("{:02}-{}.png", index + 1, page.label));
			std::fs::write(&file, raster::png(&pixmap)?)
				.with_context(|| format!("could not write {}", file.display()))?;

			Ok(pixmap)
		})
		.collect::<Result<_>>()?;

	let rows = (rendered.len() as u32).div_ceil(COLUMNS);
	let gap = 24;
	let mut sheet = Pixmap::new(COLUMNS * (THUMB + gap) + gap, rows * (THUMB + gap) + gap)
		.context("contact sheet too large")?;
	sheet.fill(resvg::tiny_skia::Color::from_rgba8(0x2a, 0x22, 0x38, 0xff));
	let scale = THUMB as f32 / PROOF as f32;

	for (index, pixmap) in rendered.iter().enumerate() {
		let column = index as u32 % COLUMNS;
		let row = index as u32 / COLUMNS;
		let x = (gap + column * (THUMB + gap)) as f32;
		let y = (gap + row * (THUMB + gap)) as f32;
		sheet.draw_pixmap(
			0,
			0,
			pixmap.as_ref(),
			&PixmapPaint {
				quality: resvg::tiny_skia::FilterQuality::Bicubic,
				..PixmapPaint::default()
			},
			Transform::from_row(scale, 0.0, 0.0, scale, x, y),
			None,
		);
	}

	let file = dir.join("contact-sheet.png");
	std::fs::write(&file, raster::png(&sheet)?)
		.with_context(|| format!("could not write {}", file.display()))?;

	Ok(())
}
