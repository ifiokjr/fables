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
use crate::cast::Kind;
use crate::press;
use crate::press::print::Spec;
use crate::raster;
use crate::stage;
use crate::story::Story;
use crate::story::plain_text;
use crate::video::frames::Format;
use crate::video::voice::Speech;

/// What a script adds up to.
#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
	pub title: String,
	/// The drawing style its world draws figures with, e.g. `chibi`.
	pub drawing: String,
	/// The page skin it prints in, e.g. `riso`.
	pub skin: String,
	pub script_pages: usize,
	pub book_pages: usize,
	pub words: usize,
	pub cast_members: usize,
	/// Things worth knowing that are not mistakes: cast fields a drawn character ignores, and
	/// the like. One line per member at most.
	pub notes: Vec<String>,
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
		drawing: story.drawing_style.clone(),
		skin: story.style().name.name().to_owned(),
		script_pages: rehearsals.len(),
		book_pages: pages.len(),
		words,
		cast_members: cast.members.len(),
		notes: cast_notes(cast),
	})
}

/// The cast fields a drawn character cannot use, once per member, naming them.
fn cast_notes(cast: &Cast) -> Vec<String> {
	cast.members
		.values()
		.filter_map(|member| {
			let Kind::Figure(look) = &member.kind else {
				return None;
			};
			let puppet = look.puppet.as_ref()?;

			if member.parametric_fields.is_empty() {
				return None;
			}

			let drawn = if puppet.template {
				format!("drawn from {}", puppet.style.template_origin(look.build))
			} else {
				"drawn from an authored puppet".to_owned()
			};
			let fields: Vec<String> = member
				.parametric_fields
				.iter()
				.map(|field| format!("`{field}`"))
				.collect();

			Some(format!(
				"{} is {drawn}, so {} do nothing; remove them, or set `puppet = \"{}\"` to draw \
				 the older parametric puppet.",
				member.id,
				join_names(&fields),
				crate::cast::PARAMETRIC
			))
		})
		.collect()
}

/// A list of names with commas and a final `and`.
fn join_names(names: &[String]) -> String {
	match names {
		[] => String::new(),
		[one] => one.clone(),
		[one, two] => format!("{one} and {two}"),
		_ => {
			let (last, rest) = names.split_last().expect("a list of three or more");
			format!("{} and {last}", rest.join(", "))
		}
	}
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

/// A storyboard's pictures and the shot list that reads them.
#[derive(Clone, Debug)]
pub struct Storyboard {
	pub images: Vec<PathBuf>,
	pub list: PathBuf,
	/// The shot list as Markdown, as printed to the terminal.
	pub text: String,
}

/// The camera plan drawn as a comic strip with its shot list beside it, in the video's framing.
/// Never records a voice — a line without a cached take is measured from its word count, so
/// the board and list render offline in a second or two.
pub fn storyboard(story: &Story, cast: &Cast, out: &Path, portrait: bool) -> Result<Storyboard> {
	let timeline = estimated_timeline(story, cast, out)?;
	let dir = book_dir(out, story).join("storyboard");
	let rows = press::storyboard::plan(story, &timeline, cast, portrait);
	let text = press::storyboard::shot_list(story, &timeline, &rows, portrait);
	let images = press::storyboard::write(story, &timeline, cast, portrait, &dir)?;
	let list = press::storyboard::write_list(&text, &dir, portrait)?;

	Ok(Storyboard { images, list, text })
}

/// The timeline with estimated durations: a cached take when there is one, else the line's
/// words read at about 2.6 a second, so boards, sampled frames and reviews run offline
/// without a microphone or ffmpeg.
pub fn estimated_timeline(
	story: &Story,
	cast: &Cast,
	out: &Path,
) -> Result<crate::video::timeline::Timeline> {
	let cache = out.join(&story.slug).join("cache").join("voice");
	let rehearsals = stage::rehearse(story, cast)?;
	crate::video::timeline::build(story, cast, &rehearsals, |line| {
		Ok(offline_take(line, &cache))
	})
}

/// Critiques a rehearsed story: what a director would change, with line numbers (see
/// [`review`](crate::review)). Advice, never a refusal; timings are estimated offline.
pub fn review(story: &Story, cast: &Cast, out: &Path) -> Result<Vec<crate::review::Finding>> {
	let rehearsals = stage::rehearse(story, cast)?;
	let timeline = estimated_timeline(story, cast, out)?;

	Ok(crate::review::review(story, &rehearsals, &timeline))
}

/// The length of a spoken line without a microphone: the cached take when there is one, else
/// the words read at about 2.6 words a second.
fn offline_take(line: &crate::video::voice::Utterance<'_>, cache: &Path) -> Speech {
	use crate::art::seed_of;
	use crate::video::lipsync::Track;
	use crate::video::voice::SAMPLE_RATE;

	// The same key `voice::speak` caches takes under, so a recorded take gives the real length.
	let key = crate::video::voice::cache_key(line.voice, line.delivery, line.text);
	let wav = cache.join(format!("{:016x}.wav", seed_of(&key)));
	let seconds = match crate::video::voice::read_wav(&wav) {
		Ok((samples, _rate)) => samples.len() as f32 / SAMPLE_RATE as f32,
		Err(_) => (plain_text(line.text).split_whitespace().count() as f32 / 2.6).max(0.3),
	};

	Speech {
		text: line.text.to_owned(),
		samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
		envelope: Vec::new(),
		track: Track::default(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::story;

	#[test]
	fn a_field_only_the_parametric_puppet_reads_is_noted_once() {
		let cast = Cast::parse("[pip]\nname = \"Pip\"\nhead = \"wide\"\nmouth = 1.2\n").unwrap();
		let story =
			story::parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# plain\n@pip\nHi.\n").unwrap();
		let report = check(&story, &cast).unwrap();
		assert_eq!(report.notes.len(), 1, "{:?}", report.notes);
		assert!(
			report.notes[0].contains("`head` and `mouth`"),
			"{:?}",
			report.notes
		);
		assert!(report.notes[0].contains("template"), "{:?}", report.notes);

		let parametric = Cast::parse(
			"[pip]\nname = \"Pip\"\nhead = \"wide\"\nmouth = 1.2\npuppet = \"parametric\"\n",
		)
		.unwrap();
		assert!(
			check(&story, &parametric).unwrap().notes.is_empty(),
			"the parametric puppet reads those fields"
		);
	}

	#[test]
	fn a_check_reports_the_drawing_style_and_page_skin() {
		let cast = Cast::parse("[pip]\nname = \"Pip\"\n").unwrap();
		let story =
			story::parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# plain\n@pip\nHi.\n").unwrap();
		let report = check(&story, &cast).unwrap();
		assert_eq!(report.drawing, crate::art::pack::DEFAULT);
		assert_eq!(report.skin, "riso");
	}
}
