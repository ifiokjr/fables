//! Making a book's outputs from a loaded script and cast: the check, proofs, the ePub, KDP print
//! files and videos. The CLI and the MCP server both build books through here, into one folder
//! per book and art style: `<out>/<book>/<style>/`.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;

use crate::book;
use crate::book::Side;
use crate::cast::Cast;
use crate::press;
use crate::press::print::Spec;
use crate::raster;
use crate::stage;
use crate::story::Story;
use crate::video::frames::Format;

/// What a script adds up to.
#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
	pub title: String,
	pub script_pages: usize,
	pub book_pages: usize,
	pub words: usize,
	pub cast_members: usize,
}

/// The ePub and its store cover.
#[derive(Clone, Debug)]
pub struct Ebook {
	pub epub: PathBuf,
	pub cover: PathBuf,
}

/// The KDP print files and the numbers to type into KDP.
#[derive(Clone, Debug)]
pub struct Print {
	pub interior: PathBuf,
	pub cover: PathBuf,
	pub setup: PathBuf,
	pub specs: String,
}

/// Where a book's outputs go: one folder per style, so renders in different styles sit side
/// by side instead of overwriting each other.
pub fn book_dir(out: &Path, story: &Story) -> PathBuf {
	out.join(&story.slug).join(story.style().name.name())
}

/// Rehearses every page and lays out the book, so mistakes surface without rendering.
pub fn check(story: &Story, cast: &Cast) -> Result<CheckReport> {
	let rehearsals = stage::rehearse(story, cast)?;
	let pages = book::assemble(story, cast)?;
	let words = story
		.pages
		.iter()
		.flat_map(|page| page.book_text())
		.map(|text| text.split_whitespace().count())
		.sum();

	Ok(CheckReport {
		title: story.meta.title.clone(),
		script_pages: rehearsals.len(),
		book_pages: pages.len(),
		words,
		cast_members: cast.members.len(),
	})
}

/// Every page as a PNG, covers included, plus a contact sheet. Returns the folder.
pub fn proofs(story: &Story, cast: &Cast, out: &Path) -> Result<PathBuf> {
	let mut pages = vec![book::front_cover(story, cast)?];
	pages.extend(book::assemble(story, cast)?);
	pages.push(book::back_cover(story, cast)?);
	let dir = book_dir(out, story).join("proofs");
	press::proofs::write(&pages, &dir)?;
	Ok(dir)
}

/// The fixed-layout ePub, and a store cover KDP accepts.
pub fn ebook(story: &Story, cast: &Cast, out: &Path) -> Result<Ebook> {
	let pages = book::assemble(story, cast)?;
	let front = book::front_cover(story, cast)?;
	let dir = book_dir(out, story).join("ebook");
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;

	// KDP wants a separate marketing cover of at least 1600px on the short side and under 5 MB.
	let mut store = raster::render(&book::page_svg(&front, Side::Screen, 2560, 2560, true))?;
	raster::finish(&mut store, &front.style, 0.05, 0);
	let cover = dir.join("cover.jpg");
	std::fs::write(&cover, raster::jpeg(&store, 88)?)
		.with_context(|| format!("could not write {}", cover.display()))?;

	let size = press::epub::PAGE_PX;
	let mut inner = raster::render(&book::page_svg(&front, Side::Screen, size, size, true))?;
	raster::finish(&mut inner, &front.style, 0.035, 0);
	let epub = dir.join(format!("{}.epub", story.slug));
	press::epub::write(story, &pages, &raster::jpeg(&inner, 84)?, &epub)?;

	Ok(Ebook { epub, cover })
}

/// The KDP paperback: interior and full-wrap cover PDFs, and the setup numbers.
pub fn print(story: &Story, cast: &Cast, out: &Path) -> Result<Print> {
	let pages = book::assemble(story, cast)?;
	let spec = Spec::for_pages(pages.len());
	let dir = book_dir(out, story).join("print");
	let interior = dir.join(format!("{}-interior.pdf", story.slug));
	press::print::write_interior(story, &pages, &interior)?;

	let cover = dir.join(format!("{}-cover.pdf", story.slug));
	let front = book::front_cover(story, cast)?;
	let back = book::back_cover(story, cast)?;
	press::print::write_cover(story, &front, &back, spec, &cover)?;

	let specs = press::print::specs_text(story, spec);
	let setup = dir.join("kdp-setup.txt");
	std::fs::write(&setup, &specs)
		.with_context(|| format!("could not write {}", setup.display()))?;

	Ok(Print {
		interior,
		cover,
		setup,
		specs,
	})
}

/// The narrated, lip-synced videos in each format. Voices are shared by every art style, so
/// the take cache sits above the per-style folders.
pub fn video(
	story: &Story,
	cast: &Cast,
	out: &Path,
	formats: &[Format],
	preview: bool,
) -> Result<Vec<PathBuf>> {
	let timeline = crate::video::plan(
		story,
		cast,
		&out.join(&story.slug).join("cache").join("voice"),
	)?;

	crate::video::make(
		story,
		&timeline,
		formats,
		crate::video::Options { preview },
		&book_dir(out, story).join("video"),
	)
}
