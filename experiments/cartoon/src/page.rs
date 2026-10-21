//! The park on its own, and one composed book page with the series' paper text card.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use fables::art::Color;
use fables::art::Path as Curve;
use fables::art::Svg;
use fables::art::figure::Mood;
use fables::art::figure::Pose;
use fables::art::pt;
use fables::art::seed_of;
use fables::text;
use fables::text::Anchor;
use fables::text::Area;
use fables::text::Ink;

use crate::park;
use crate::sheets::save;
use crate::stinky;
use crate::stinky::THREE_QUARTER;

/// The book's safe margin and body size, matching `fables::book`.
const SAFE: f32 = park::SIZE * 0.375 / 8.5;
const BODY_SIZE: f32 = 116.0;
const PAPER: Color = Color(0xFF_F4_E2);
const INK: Color = Color(0x2B_1D_2E);

/// The setting with nobody in it (Wormsley excepted; he is always there).
pub fn park_plate(out: &Path) -> Result<PathBuf> {
	let mut c = Svg::new();
	park::draw(&mut c);
	park::wormsley(&mut c, pt(2010.0, 1730.0), 1.0);
	park::draw_front(&mut c);
	let svg = c.finish((0.0, 0.0, park::SIZE, park::SIZE), 1600, 1600);
	save(&svg, &out.join("park.png"))
}

/// "This is Stanley." as a finished page: setting, character and the text card on top.
pub fn book_page(out: &Path) -> Result<PathBuf> {
	let mut c = Svg::new();
	park::draw(&mut c);
	park::wormsley(&mut c, pt(2010.0, 1730.0), 1.0);

	let pose = stinky::posed(Mood::Happy, Pose::Wave, THREE_QUARTER);
	c.begin("translate(1180 2140) scale(2.3)");
	stinky::draw(&mut c, &pose);
	c.end();
	park::draw_front(&mut c);

	let paragraphs = [
		"This is Stanley.",
		"Stanley liked bouncing on his bed, eating jam straight from the jar, and saying one word \
		 more than any other.",
	];
	let block = text::layout(
		&paragraphs,
		Area {
			x: SAFE + 120.0,
			y: SAFE + 70.0,
			w: park::SIZE - 2.0 * (SAFE + 120.0),
			h: 760.0,
		},
		BODY_SIZE,
		Anchor::Top,
	);
	let b = block.bounds;
	let (pad_x, pad_y) = (80.0, 60.0);
	let card = Curve::wobbly_poly(
		&[
			pt(b.x - pad_x, b.y - pad_y),
			pt(b.x + b.w + pad_x, b.y - pad_y),
			pt(b.x + b.w + pad_x, b.y + b.h + pad_y * 0.8),
			pt(b.x - pad_x, b.y + b.h + pad_y * 0.8),
		],
		46.0,
		5.0,
		seed_of("this-is-stanley"),
	);
	// The card lifts off the page with a soft shadow rather than a heavy outline.
	c.begin("translate(10 14)");
	c.fill_alpha(&card, INK, 0.12);
	c.end();
	c.brushed(&card, PAPER, INK, 5.0);
	c.raw(&text::svg(&block, Ink::default()));

	let svg = c.finish((0.0, 0.0, park::SIZE, park::SIZE), 1800, 1800);
	save(&svg, &out.join("book-page.png"))
}

/// The current tv-style proof of the same page beside the experiment's page.
pub fn page_comparison(out: &Path, new_page: &Path) -> Result<PathBuf> {
	let old_page = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../out/stinky-mcstinkbreath/tv/proofs/06-story-03.png");
	anyhow::ensure!(
		old_page.exists(),
		"render the book's tv proofs first ({} is missing)",
		old_page.display()
	);
	let (side, gap, top) = (1200.0, 60.0, 130.0);
	let (width, height) = (side * 2.0 + gap * 3.0, top + side + gap);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), crate::sheets::PAPER);

	for (index, (title, image)) in [
		("Now (tv style)", &old_page),
		("Experiment", &new_page.to_path_buf()),
	]
	.into_iter()
	.enumerate()
	{
		let x = gap + index as f32 * (side + gap);
		crate::sheets::label(&mut c, x + side / 2.0, 90.0, 54.0, title);
		c.raw(&format!(
			r#"<image x="{x:.1}" y="{top:.1}" width="{side:.1}" height="{side:.1}" href="{}"/>"#,
			fables::art::escape(&image.canonicalize()?.display().to_string())
		));
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("compare-page.png"))
}
