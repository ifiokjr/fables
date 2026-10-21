//! The `fables` command line.

use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use fables::art::style::Style;
use fables::art::style::StyleName;
use fables::book;
use fables::book::Side;
use fables::cast::Cast;
use fables::press;
use fables::press::print::Spec;
use fables::raster;
use fables::stage;
use fables::story::Story;
use fables::video::frames::Format;

/// Turn a picture-book script into proofs, an ePub, KDP print files and narrated videos.
///
/// Every command that takes a story also takes a folder of books (e.g. `stories/`) and builds
/// each `story.md` inside it.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
	#[command(subcommand)]
	command: Command,
	/// Draw in these art styles instead of the script's own: riso, tv, bold, cut-paper, crayon,
	/// storybook, a comma-separated list (bold,tv), or all. Each style renders into its own folder.
	#[arg(long, global = true)]
	style: Option<String>,
}

#[derive(Subcommand)]
enum Command {
	/// Start a new book from the Kerfuffle recipe.
	New {
		/// The book (and its star's) name, e.g. "Bogey `McBogeyface`".
		title: String,
		/// The cast id of the star, from world/cast.toml.
		#[arg(long)]
		star: String,
		#[arg(long, default_value = "Ifiok Jr.")]
		author: String,
		/// Where books live.
		#[arg(long, default_value = "stories")]
		dir: PathBuf,
	},
	/// Check a script for mistakes without rendering anything.
	Check { story: PathBuf },
	/// Render every page as a PNG plus a contact sheet.
	Proofs {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Build the fixed-layout ePub and the store cover image.
	Ebook {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Build the KDP paperback interior and cover PDFs.
	Print {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Build everything for the book: proofs, ePub and print files.
	Book {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Draw the same pages in every art style, side by side, to choose a look.
	Styles {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Render reference sheets: the whole cast, one character's moods, and every setting.
	Sheets {
		/// The cast file to draw.
		#[arg(long, default_value = "world/cast.toml")]
		cast: PathBuf,
		/// Also draw every mood for this cast member.
		#[arg(long)]
		moods: Option<String>,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Check a character's rig: a sheet of stances and gaits with the skeleton, and a motion test video.
	Rig {
		/// The cast id to test, e.g. stinky.
		id: String,
		#[arg(long, default_value = "world/cast.toml")]
		cast: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Narrate and animate the story as MP4 videos.
	Video {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
		/// Which shapes to render.
		#[arg(long, value_enum, default_value = "both")]
		format: Shape,
		/// Half resolution at 15 fps, for checking timing quickly.
		#[arg(long)]
		preview: bool,
	},
	/// Build the book and both videos.
	All {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
}

/// Video shapes.
#[derive(Clone, Copy, clap::ValueEnum)]
enum Shape {
	/// 16:9 for `YouTube`.
	Landscape,
	/// 9:16 for Shorts, `TikTok` and Reels.
	Portrait,
	Both,
}

impl Shape {
	fn formats(self) -> Vec<Format> {
		match self {
			Shape::Landscape => vec![Format::Landscape],
			Shape::Portrait => vec![Format::Portrait],
			Shape::Both => vec![Format::Landscape, Format::Portrait],
		}
	}
}

fn main() {
	if let Err(error) = run(Cli::parse()) {
		eprintln!("error: {error:#}");
		std::process::exit(1);
	}
}

fn run(cli: Cli) -> Result<()> {
	let choices = style_choices(cli.style.as_deref())?;

	match cli.command {
		Command::New {
			title,
			star,
			author,
			dir,
		} => new_story(&title, &star, &author, &dir),
		Command::Sheets { cast, moods, out } => {
			for style in sheet_styles(&choices) {
				sheets(
					&cast,
					moods.as_deref(),
					&out.join("sheets").join(style.name.name()),
					style,
				)?;
			}

			Ok(())
		}
		Command::Rig { id, cast, out } => {
			for style in sheet_styles(&choices) {
				rig(&id, &cast, &out.join("rig").join(style.name.name()), style)?;
			}

			Ok(())
		}
		Command::Styles { story, out } => {
			each(&story, |path| {
				let (story, cast) = load(path, None)?;
				styles(&story, &cast, &out)
			})
		}
		Command::Check { story } => each_style(&story, &choices, check),
		Command::Proofs { story, out } => {
			each_style(&story, &choices, |story, cast| proofs(story, cast, &out))
		}
		Command::Ebook { story, out } => {
			each_style(&story, &choices, |story, cast| ebook(story, cast, &out))
		}
		Command::Print { story, out } => {
			each_style(&story, &choices, |story, cast| print(story, cast, &out))
		}
		Command::Book { story, out } => {
			each_style(&story, &choices, |story, cast| {
				proofs(story, cast, &out)?;
				ebook(story, cast, &out)?;
				print(story, cast, &out)
			})
		}
		Command::Video {
			story,
			out,
			format,
			preview,
		} => {
			each_style(&story, &choices, |story, cast| {
				video(story, cast, &out, &format.formats(), preview)
			})
		}
		Command::All { story, out } => {
			each_style(&story, &choices, |story, cast| {
				proofs(story, cast, &out)?;
				ebook(story, cast, &out)?;
				print(story, cast, &out)?;
				video(story, cast, &out, &Shape::Both.formats(), false)
			})
		}
	}
}

/// The styles asked for with `--style`: one name, a comma-separated list, or `all`. `None` in
/// the list means "the script's own style".
fn style_choices(argument: Option<&str>) -> Result<Vec<Option<String>>> {
	let Some(argument) = argument else {
		return Ok(vec![None]);
	};

	if argument == "all" {
		return Ok(StyleName::ALL
			.iter()
			.map(|(name, _)| Some((*name).to_owned()))
			.collect());
	}

	argument
		.split(',')
		.map(str::trim)
		.filter(|name| !name.is_empty())
		.map(|name| {
			if Style::parse(name).is_none() {
				let names: Vec<&str> = StyleName::ALL.iter().map(|(name, _)| *name).collect();
				anyhow::bail!(
					"unknown style `{name}`; use {}, a comma-separated list, or all",
					names.join(", ")
				);
			}

			Ok(Some(name.to_owned()))
		})
		.collect()
}

/// Styles for sheets and rig tests, which have no script to take a default from.
fn sheet_styles(choices: &[Option<String>]) -> Vec<Style> {
	choices
		.iter()
		.map(|choice| choice.as_deref().and_then(Style::parse).unwrap_or_default())
		.collect()
}

/// Runs `action` on every script (see [`each`]) once per chosen style.
fn each_style(
	path: &Path,
	choices: &[Option<String>],
	mut action: impl FnMut(&Story, &Cast) -> Result<()>,
) -> Result<()> {
	each(path, |script| {
		for choice in choices {
			let (story, cast) = load(script, choice.as_deref())?;
			println!("  style: {}", story.style().name.name());
			action(&story, &cast)?;
		}

		Ok(())
	})
}

/// Where a book's outputs go: one folder per style, so renders in different styles sit side
/// by side instead of overwriting each other.
fn book_dir(out: &Path, story: &Story) -> PathBuf {
	out.join(&story.slug).join(story.style().name.name())
}

/// Runs `action` on one script, or on every `story.md` in a folder of books (stopping at the
/// first failure so a broken script is never silently skipped).
fn each(path: &Path, mut action: impl FnMut(&Path) -> Result<()>) -> Result<()> {
	if path.is_file() {
		return action(path);
	}

	let mut scripts: Vec<PathBuf> = std::fs::read_dir(path)
		.with_context(|| {
			format!(
				"{} is neither a script nor a folder of books",
				path.display()
			)
		})?
		.filter_map(|entry| Some(entry.ok()?.path().join("story.md")))
		.filter(|script| script.is_file())
		.collect();
	scripts.sort();

	if scripts.is_empty() {
		anyhow::bail!(
			"no story.md files found in the folders under {}",
			path.display()
		);
	}

	for script in &scripts {
		println!("— {}", script.display());
		action(script).with_context(|| format!("while building {}", script.display()))?;
	}

	Ok(())
}

/// The Kerfuffle recipe, filled in with a title and a star.
const RECIPE: &str = include_str!("../templates/recipe.md");

fn new_story(title: &str, star: &str, author: &str, dir: &Path) -> Result<()> {
	let slug = fables::story::slugify(title);
	let folder = dir.join(&slug);
	let path = folder.join("story.md");

	if path.exists() {
		anyhow::bail!(
			"{} already exists; pick another title or edit it",
			path.display()
		);
	}

	let year = 1970
		+ std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_or(0, |elapsed| elapsed.as_secs() / 31_556_952);
	let script = RECIPE
		.replace("{{title}}", title)
		.replace("{{star}}", star)
		.replace("{{author}}", author)
		.replace("{{year}}", &year.to_string());

	std::fs::create_dir_all(&folder)
		.with_context(|| format!("could not create {}", folder.display()))?;
	std::fs::write(&path, script).with_context(|| format!("could not write {}", path.display()))?;
	println!("✓ wrote {} from the Kerfuffle recipe", path.display());

	if let Err(error) = load(&path, None).and_then(|(story, cast)| check(&story, &cast)) {
		println!("  next: {error:#}");
		println!(
			"  (add `[{star}]` to world/cast.toml, then run `fables proofs {}`)",
			path.display()
		);
	}

	Ok(())
}

fn load(path: &Path, style: Option<&str>) -> Result<(Story, Cast)> {
	let mut story = Story::load(path)?;

	if let Some(name) = style {
		if Style::parse(name).is_none() {
			anyhow::bail!(
				"unknown style `{name}`; use riso, tv, bold, cut-paper, crayon or storybook"
			);
		}

		story.meta.style = Some(name.to_owned());
	}

	let cast_path = Cast::locate(&story.dir, story.meta.cast.as_deref())?;
	let cast = Cast::load(&cast_path)?;

	Ok((story, cast))
}

fn check(story: &Story, cast: &Cast) -> Result<()> {
	let rehearsals = stage::rehearse(story, cast)?;
	let pages = book::assemble(story, cast)?;
	let words: usize = story
		.pages
		.iter()
		.flat_map(|page| page.book_text())
		.map(|text| text.split_whitespace().count())
		.sum();

	println!(
		"✓ {} — {} script pages, {} book pages, {words} words, {} cast members",
		story.meta.title,
		rehearsals.len(),
		pages.len(),
		cast.members.len()
	);

	Ok(())
}

fn proofs(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let mut pages = vec![book::front_cover(story, cast)?];
	pages.extend(book::assemble(story, cast)?);
	pages.push(book::back_cover(story, cast)?);
	let dir = book_dir(out, story).join("proofs");
	press::proofs::write(&pages, &dir)?;
	done(&dir, started);

	Ok(())
}

fn ebook(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let pages = book::assemble(story, cast)?;
	let front = book::front_cover(story, cast)?;
	let dir = book_dir(out, story).join("ebook");
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;

	// KDP wants a separate marketing cover of at least 1600px on the short side and under 5 MB.
	let mut store = raster::render(&book::page_svg(&front, Side::Screen, 2560, 2560, true))?;
	raster::finish(&mut store, &front.style, 0.05, 0);
	let store_path = dir.join("cover.jpg");
	std::fs::write(&store_path, raster::jpeg(&store, 88)?)
		.with_context(|| format!("could not write {}", store_path.display()))?;

	let size = press::epub::PAGE_PX;
	let mut inner = raster::render(&book::page_svg(&front, Side::Screen, size, size, true))?;
	raster::finish(&mut inner, &front.style, 0.035, 0);
	let epub_path = dir.join(format!("{}.epub", story.slug));
	press::epub::write(story, &pages, &raster::jpeg(&inner, 84)?, &epub_path)?;
	done(&epub_path, started);

	Ok(())
}

fn print(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let pages = book::assemble(story, cast)?;
	let spec = Spec::for_pages(pages.len());
	let dir = book_dir(out, story).join("print");
	let interior = dir.join(format!("{}-interior.pdf", story.slug));
	press::print::write_interior(story, &pages, &interior)?;
	done(&interior, started);

	let started = Instant::now();
	let cover = dir.join(format!("{}-cover.pdf", story.slug));
	let front = book::front_cover(story, cast)?;
	let back = book::back_cover(story, cast)?;
	press::print::write_cover(story, &front, &back, spec, &cover)?;
	done(&cover, started);

	let specs = dir.join("kdp-setup.txt");
	std::fs::write(&specs, press::print::specs_text(story, spec))
		.with_context(|| format!("could not write {}", specs.display()))?;
	println!("{}", press::print::specs_text(story, spec));

	Ok(())
}

fn sheets(cast_path: &Path, moods: Option<&str>, out: &Path, style: Style) -> Result<()> {
	let started = Instant::now();
	let cast = Cast::load(cast_path)?;
	std::fs::create_dir_all(out).with_context(|| format!("could not create {}", out.display()))?;

	let path = out.join("kerfuffle-cast.png");
	std::fs::write(
		&path,
		raster::png(&press::sheets::cast_sheet(&cast, style)?)?,
	)?;
	done(&path, started);

	let path = out.join("kerfuffle-places.png");
	std::fs::write(&path, raster::png(&press::sheets::places_sheet(style)?)?)?;
	done(&path, started);

	if let Some(id) = moods {
		let member = cast
			.member(id)
			.with_context(|| format!("`{id}` is not in {}", cast_path.display()))?;
		let sheet = press::sheets::mood_sheet(member, style)?
			.with_context(|| format!("`{id}` is not a person, so has no moods"))?;
		let path = out.join(format!("{id}-moods.png"));
		std::fs::write(&path, raster::png(&sheet)?)?;
		done(&path, started);
	}

	Ok(())
}

fn rig(id: &str, cast_path: &Path, out: &Path, style: Style) -> Result<()> {
	let started = Instant::now();
	let cast = Cast::load(cast_path)?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}", cast_path.display()))?;
	let fables::cast::Kind::Figure(look) = &member.kind else {
		anyhow::bail!("`{id}` is not a person, so has no rig");
	};
	let dir = out.to_path_buf();
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;

	let sheet = dir.join(format!("{id}-rig.png"));
	std::fs::write(
		&sheet,
		raster::png(&fables::video::motion::rig_sheet(look, style)?)?,
	)?;
	done(&sheet, started);

	let video = dir.join(format!("{id}-motion.mp4"));
	fables::video::motion::motion_test(look, &member.name, style, &video)?;
	done(&video, started);

	Ok(())
}

fn styles(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let pages = book::assemble(story, cast)?;
	let story_pages: Vec<&book::BookPage> = pages
		.iter()
		.filter(|page| page.label.starts_with("story-"))
		.collect();
	let mut picks = vec![book::front_cover(story, cast)?];

	for fraction in [0.15, 0.45, 0.8] {
		if let Some(page) =
			story_pages.get((fraction * story_pages.len().saturating_sub(1) as f32) as usize)
		{
			picks.push((*page).clone());
		}
	}

	let path = out.join(&story.slug).join("styles.png");
	press::board::write(&picks, &path)?;
	done(&path, started);

	Ok(())
}

fn video(story: &Story, cast: &Cast, out: &Path, formats: &[Format], preview: bool) -> Result<()> {
	let started = Instant::now();
	// Voices are shared by every style, so the take cache sits above the per-style folders.
	let timeline = fables::video::plan(
		story,
		cast,
		&out.join(&story.slug).join("cache").join("voice"),
	)?;
	let paths = fables::video::make(
		story,
		&timeline,
		formats,
		fables::video::Options { preview },
		&book_dir(out, story).join("video"),
	)?;

	for path in paths {
		done(&path, started);
	}

	Ok(())
}

fn done(path: &Path, started: Instant) {
	println!(
		"✓ wrote {} in {:.1}s",
		path.display(),
		started.elapsed().as_secs_f32()
	);
}
