//! Print-ready PDFs for a KDP paperback: the interior and the full-wrap cover.
//!
//! KDP's rules for a full-bleed 8.5" × 8.5" premium-colour paperback:
//! - Interior pages are 8.625" × 8.75": 0.125" bleed on the top, bottom and outside edge only.
//! - Text stays 0.375" inside the trim (handled by the book layout).
//! - The cover is back + spine + front with 0.125" bleed all round; the spine is 0.002347" per
//!   page on premium colour paper, and spine text is only allowed from 79 pages.
//! - Art should be at least 300 DPI and the PDF flattened with fonts embedded.
//!
//! Art is rasterised at 300 DPI (flattening every transparency KDP might trip over) and the
//! lettering stays vector, with its fonts subset and embedded.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use base64::Engine as _;
use pdf_writer::Finish;
use pdf_writer::Name;
use pdf_writer::Pdf;
use pdf_writer::Rect;
use pdf_writer::Ref;
use pdf_writer::TextStr;
use rayon::prelude::*;

use crate::art::STAGE;
use crate::art::Svg;
use crate::art::palette;
use crate::book;
use crate::book::BLEED;
use crate::book::BookPage;
use crate::book::Side;
use crate::raster;
use crate::story::Story;

pub const TRIM_IN: f32 = 8.5;
pub const BLEED_IN: f32 = 0.125;
/// Premium colour paper thickness per page (KDP).
pub const SPINE_PER_PAGE_IN: f32 = 0.002_347;
pub const DPI: f32 = 300.0;
const GRAIN: f32 = 0.06;
const QUALITY: u8 = 92;

const UNITS_PER_IN: f32 = STAGE / TRIM_IN;

/// Measurements KDP asks for when setting up the paperback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
	pub pages: usize,
	pub spine_in: f32,
	pub cover_width_in: f32,
	pub cover_height_in: f32,
}

impl Spec {
	pub fn for_pages(pages: usize) -> Spec {
		let spine_in = pages as f32 * SPINE_PER_PAGE_IN;

		Spec {
			pages,
			spine_in,
			cover_width_in: BLEED_IN * 2.0 + TRIM_IN * 2.0 + spine_in,
			cover_height_in: TRIM_IN + BLEED_IN * 2.0,
		}
	}
}

/// Writes the interior PDF.
pub fn write_interior(story: &Story, pages: &[BookPage], path: &Path) -> Result<()> {
	let width_in = TRIM_IN + BLEED_IN;
	let height_in = TRIM_IN + BLEED_IN * 2.0;
	let svgs: Vec<String> = pages
		.par_iter()
		.enumerate()
		.map(|(index, page)| {
			// Page 1 is a right-hand page, so odd page numbers bleed off the right edge.
			let side = if index % 2 == 0 {
				Side::Recto
			} else {
				Side::Verso
			};
			let view = book::view_box(side);
			let art = art_jpeg(page, side, width_in, height_in)?;
			let mut text = Svg::new();
			book::draw_text(&mut text, page);

			Ok(print_svg(
				&art,
				view,
				width_in,
				height_in,
				&text_markup(text, view),
			))
		})
		.collect::<Result<_>>()?;

	write_pdf(story, &svgs, width_in, height_in, path)
}

fn art_jpeg(page: &BookPage, side: Side, width_in: f32, height_in: f32) -> Result<Vec<u8>> {
	let px_w = (width_in * DPI).round() as u32;
	let px_h = (height_in * DPI).round() as u32;
	let mut pixmap = raster::render(&book::page_svg(page, side, px_w, px_h, false))?;
	raster::finish(&mut pixmap, &page.style, GRAIN, 0);

	raster::jpeg(&pixmap, QUALITY)
}

/// Text layers are drawn on their own document so only the lettering markup is reused.
fn text_markup(svg: Svg, view: (f32, f32, f32, f32)) -> String {
	let full = svg.finish(view, 1, 1);
	let start = full
		.find("</defs>")
		.map_or(0, |index| index + "</defs>".len());
	let end = full.rfind("</svg>").unwrap_or(full.len());

	full[start..end].to_owned()
}

/// An SVG page: the art as one embedded JPEG, the lettering as vector text on top.
fn print_svg(
	jpeg: &[u8],
	view: (f32, f32, f32, f32),
	width_in: f32,
	height_in: f32,
	text: &str,
) -> String {
	let (x, y, w, h) = view;
	let data = base64::engine::general_purpose::STANDARD.encode(jpeg);

	format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{:.3}" height="{:.3}" viewBox="{x:.3} {y:.3} {w:.3} {h:.3}"><image x="{x:.3}" y="{y:.3}" width="{w:.3}" height="{h:.3}" preserveAspectRatio="none" xlink:href="data:image/jpeg;base64,{data}"/>{text}</svg>"#,
		width_in * 72.0,
		height_in * 72.0,
	)
}

/// Stitches single-page SVGs into one PDF, one page each.
fn write_pdf(
	story: &Story,
	svgs: &[String],
	width_in: f32,
	height_in: f32,
	path: &Path,
) -> Result<()> {
	let chunks: Vec<(pdf_writer::Chunk, Ref)> = svgs
		.par_iter()
		.map(|svg| {
			let tree = svg2pdf::usvg::Tree::from_str(svg, &raster::options())
				.context("print page failed to parse")?;
			svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
				.map_err(|error| anyhow!("PDF conversion failed: {error}"))
		})
		.collect::<Result<_>>()?;

	let mut alloc = Ref::new(1);
	let catalog = alloc.bump();
	let tree = alloc.bump();
	let info = alloc.bump();
	let mut pdf = Pdf::new();
	let mut page_ids = Vec::with_capacity(chunks.len());
	let width_pt = width_in * 72.0;
	let height_pt = height_in * 72.0;
	let name = Name(b"Art");

	for (chunk, root) in chunks {
		let mut map = HashMap::new();
		let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
		let xobject = map[&root];
		let page_id = alloc.bump();
		let content_id = alloc.bump();
		let mut page = pdf.page(page_id);
		page.media_box(Rect::new(0.0, 0.0, width_pt, height_pt));
		page.trim_box(trim_box(width_pt, height_pt, page_ids.len(), width_in));
		page.parent(tree);
		page.contents(content_id);
		page.resources().x_objects().pair(name, xobject);
		page.finish();

		let mut content = pdf_writer::Content::new();
		content.transform([width_pt, 0.0, 0.0, height_pt, 0.0, 0.0]);
		content.x_object(name);
		pdf.stream(content_id, &content.finish());
		pdf.extend(&chunk);
		page_ids.push(page_id);
	}

	pdf.catalog(catalog).pages(tree);
	pdf.pages(tree)
		.kids(page_ids.iter().copied())
		.count(page_ids.len() as i32);
	let mut document_info = pdf.document_info(info);
	document_info.title(TextStr(&story.meta.title));
	document_info.author(TextStr(&story.meta.author));
	document_info.creator(TextStr("fables"));
	document_info.finish();

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.with_context(|| format!("could not create {}", parent.display()))?;
	}

	std::fs::write(path, pdf.finish())
		.with_context(|| format!("could not write {}", path.display()))
}

/// The trim box tells prepress where the page will be cut. Interior pages have bleed on the
/// outer edge only; the cover has it all round.
fn trim_box(width_pt: f32, height_pt: f32, index: usize, width_in: f32) -> Rect {
	let bleed = BLEED_IN * 72.0;

	if width_in > TRIM_IN * 2.0 {
		return Rect::new(bleed, bleed, width_pt - bleed, height_pt - bleed);
	}

	if index.is_multiple_of(2) {
		Rect::new(0.0, bleed, width_pt - bleed, height_pt - bleed)
	} else {
		Rect::new(bleed, bleed, width_pt, height_pt - bleed)
	}
}

/// Writes the full-wrap cover PDF: back on the left, spine, front on the right.
pub fn write_cover(
	story: &Story,
	front: &BookPage,
	back: &BookPage,
	spec: Spec,
	path: &Path,
) -> Result<()> {
	let spine = spec.spine_in * UNITS_PER_IN;
	let width = STAGE * 2.0 + spine + BLEED * 2.0;
	let height = STAGE + BLEED * 2.0;
	let view = (0.0, 0.0, width, height);
	let back_x = BLEED;
	let front_x = BLEED + STAGE + spine;

	let mut art = Svg::new();
	art.fill(
		&crate::art::Path::rect(0.0, 0.0, width, height),
		palette::PAPER,
	);
	panel(&mut art, back, back_x, true);
	panel(&mut art, front, front_x, false);
	spine_band(&mut art, back_x + STAGE, spine, height, front);

	let px_w = (spec.cover_width_in * DPI).round() as u32;
	let px_h = (spec.cover_height_in * DPI).round() as u32;
	let mut pixmap = raster::render(&art.finish(view, px_w, px_h))?;
	raster::finish(&mut pixmap, &front.style, GRAIN, 0);
	let jpeg = raster::jpeg(&pixmap, QUALITY)?;

	let mut text = Svg::new();

	for (page, x) in [(back, back_x), (front, front_x)] {
		text.begin(&format!("translate({x:.2} {BLEED:.2})"));
		book::draw_text(&mut text, page);
		text.end();
	}

	let svg = print_svg(
		&jpeg,
		view,
		spec.cover_width_in,
		spec.cover_height_in,
		&text_markup(text, view),
	);

	write_pdf(
		story,
		&[svg],
		spec.cover_width_in,
		spec.cover_height_in,
		path,
	)
}

/// Draws one cover panel's art with its bleed. Outer edges extend into the bleed; the edge
/// against the spine is clipped so the panels meet cleanly.
fn panel(svg: &mut Svg, page: &BookPage, x: f32, is_back: bool) {
	let clip = svg.next_id("panel");
	let (left, right) = if is_back {
		(-BLEED, STAGE)
	} else {
		(0.0, STAGE + BLEED)
	};
	svg.define(&format!(
		r#"<clipPath id="{clip}"><rect x="{left:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath>"#,
		-BLEED,
		right - left,
		STAGE + BLEED * 2.0
	));
	svg.begin(&format!("translate({x:.2} {BLEED:.2})"));
	svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
	book::draw_art(svg, page);
	svg.raw("</g>");
	svg.end();
}

/// The spine: a solid band in the cover tint so a slightly off-centre trim still looks right.
fn spine_band(svg: &mut Svg, x: f32, width: f32, height: f32, front: &BookPage) {
	if width <= 0.0 {
		return;
	}

	svg.fill(
		&crate::art::Path::rect(x, 0.0, width, height),
		front.scene.weather.tint,
	);
}

/// A plain-text summary of the numbers to type into KDP.
pub fn specs_text(story: &Story, spec: Spec) -> String {
	let mut out = String::new();
	let _ = writeln!(out, "# {} — KDP paperback setup\n", story.meta.title);
	let _ = writeln!(out, "Trim size:           8.5\" x 8.5\" (21.59 x 21.59 cm)");
	let _ = writeln!(
		out,
		"Bleed:               yes (interior PDF is 8.625\" x 8.75\")"
	);
	let _ = writeln!(
		out,
		"Interior:            premium colour, white paper (the only colour option under 72 pages)"
	);
	let _ = writeln!(out, "Page count:          {}", spec.pages);
	let _ = writeln!(
		out,
		"Spine width:         {:.4}\" ({} pages x {SPINE_PER_PAGE_IN}\")",
		spec.spine_in, spec.pages
	);
	let _ = writeln!(
		out,
		"Cover PDF size:      {:.4}\" x {:.4}\"",
		spec.cover_width_in, spec.cover_height_in
	);
	let _ = writeln!(
		out,
		"Spine text:          none (KDP allows it from 79 pages)"
	);
	let _ = writeln!(
		out,
		"Barcode:             leave KDP's automatic barcode on; the back cover keeps the \
		 lower-right 2\" x 1.2\" clear"
	);
	let _ = writeln!(
		out,
		"Colour:              RGB, 300 DPI art; KDP converts to CMYK, so check bright pinks and \
		 greens on a printed proof"
	);
	let _ = writeln!(
		out,
		"AI disclosure:       answer KDP's AI-generated content questions honestly for both text \
		 and images"
	);
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn spine_and_cover_follow_kdp_formulas() {
		let spec = Spec::for_pages(32);
		assert!((spec.spine_in - 0.075_104).abs() < 1e-5);
		assert!((spec.cover_width_in - (0.25 + 17.0 + 0.075_104)).abs() < 1e-4);
		assert!((spec.cover_height_in - 8.75).abs() < 1e-6);
	}

	#[test]
	fn interior_trim_box_sits_on_the_inside_edge() {
		let width_pt = (TRIM_IN + BLEED_IN) * 72.0;
		let height_pt = (TRIM_IN + BLEED_IN * 2.0) * 72.0;
		let recto = trim_box(width_pt, height_pt, 0, TRIM_IN + BLEED_IN);
		let verso = trim_box(width_pt, height_pt, 1, TRIM_IN + BLEED_IN);
		assert!(
			recto.x1.abs() < f32::EPSILON,
			"right-hand pages are bound on the left"
		);
		assert!(
			(verso.x2 - width_pt).abs() < f32::EPSILON,
			"left-hand pages are bound on the right"
		);
		assert!((recto.x2 - recto.x1 - TRIM_IN * 72.0).abs() < 0.01);
	}
}
