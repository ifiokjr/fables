//! The `fables` command line.

use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use fables::art::pack::StylePack;
use fables::art::parts;
use fables::art::parts::Level;
use fables::art::parts::Parts;
use fables::art::settings::built_in_names;
use fables::art::style::Style;
use fables::art::style::StyleName;
use fables::book;
use fables::cast::Cast;
use fables::press;
use fables::publish;
use fables::raster;
use fables::story::Story;
use fables::studio::SheetKind;
use fables::video::frames::Format;
use fables::world::World;

/// Turn a picture-book script into proofs, an ePub, KDP print files and narrated videos.
///
/// Every command that takes a story also takes a folder of books (e.g. a world's `stories/`), or a
/// world folder, and builds each `story.md` inside it.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
	#[command(subcommand)]
	command: Command,
	/// Draw in these art styles instead of the script's own: riso, tv, bold, cut-paper, crayon,
	/// storybook, clear, a comma-separated list (bold,tv), or all. Each style renders into its
	/// own folder.
	#[arg(long, global = true)]
	style: Option<String>,
	/// The world to work in: a folder with a world.toml. Defaults to the nearest world above the
	/// script (or the current folder), or the only one in a `worlds/` folder here.
	#[arg(long, global = true)]
	world: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
	/// Start a new book from the world's recipe.
	New {
		/// The book (and its star's) name, e.g. "Bogey `McBogeyface`".
		title: String,
		/// The cast id of the star, from the world's cast.toml.
		#[arg(long)]
		star: String,
		#[arg(long, default_value = "Ifiok Jr.")]
		author: String,
		/// Where books live (default: the world's stories/ folder).
		#[arg(long)]
		dir: Option<PathBuf>,
	},
	/// Check a script for mistakes without rendering anything.
	Check { story: PathBuf },
	/// Critique a script the way a director would: findings with line numbers and suggestions,
	/// never a refusal.
	Review {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
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
		/// Move the way another drawing style moves, e.g. manga (default: the character's own).
		#[arg(long)]
		motion: Option<String>,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
	/// Work with authored puppets' part files (the world's `puppets/`).
	Puppet {
		#[command(subcommand)]
		action: PuppetAction,
	},
	/// Serve the studio to an agent over MCP (stdio): worlds, styles and characters, with
	/// model sheets returned as images.
	Mcp {
		/// The folder whose `worlds/` the studio works in.
		#[arg(long, default_value = ".")]
		root: PathBuf,
	},
	/// Hear voices: the providers, the offline voices, and a line in a character's voice.
	Voice {
		#[command(subcommand)]
		action: VoiceAction,
	},
	/// Work with drawing styles (`styles/`): the finish, bodies and kit puppets are drawn with.
	#[command(name = "style")]
	DrawingStyle {
		#[command(subcommand)]
		action: StyleAction,
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
	/// Draw the video's camera plan as a comic strip: one labelled panel per camera setup.
	Storyboard {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
		/// Which shape's camera plan to draw.
		#[arg(long, value_enum, default_value = "landscape")]
		format: Shape,
	},
	/// Build the book and both videos.
	All {
		story: PathBuf,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
}

#[derive(Subcommand)]
enum PuppetAction {
	/// Check part files against the parts a figure is drawn with: missing parts, views without
	/// their pair, parts that swap instead of blending, and names no figure uses.
	Check {
		/// The cast id to check; leave it out to check every member with a puppet.
		id: Option<String>,
	},
	/// Start a cast member's part file to redraw part by part, from the style's template or
	/// another puppet's drawings.
	Template {
		/// The cast id the file is for, e.g. bogey.
		id: String,
		/// The puppet whose drawings to start from (default: the style's template).
		#[arg(long)]
		from: Option<String>,
	},
	/// Render every part of a puppet: each turning part front-on, half-turned and three-quarters.
	Sheet {
		/// The cast id, e.g. stinky.
		id: String,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
}

#[derive(Subcommand)]
enum VoiceAction {
	/// The voice providers, and which can speak right now.
	Providers,
	/// The macOS voices the offline `say` provider can use.
	List,
	/// Speak a line in a cast member's voice (or `narrator`), and save it as a WAV.
	Audition {
		/// A cast id, or `narrator`.
		who: String,
		line: String,
		/// A mood to deliver it in, e.g. cross.
		#[arg(long)]
		mood: Option<String>,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
}

#[derive(Subcommand)]
enum StyleAction {
	/// List the drawing styles.
	List,
	/// Check a style: its template over its kit must draw a whole figure, and its bodies must
	/// stand up.
	Check { name: String },
	/// Render a style: its template character on a model sheet, and its kit and template parts.
	Sheet {
		name: String,
		#[arg(long, default_value = "out")]
		out: PathBuf,
	},
}

/// Video shapes.
#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
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
	let flag = cli.world.as_deref();
	// The world for commands that don't start from a script.
	let world = || {
		match flag {
			Some(dir) => World::load(dir),
			None => World::find(Path::new(".")),
		}
	};

	match cli.command {
		Command::New {
			title,
			star,
			author,
			dir,
		} => new_story(&world()?, &title, &star, &author, dir.as_deref()),
		Command::Sheets { moods, out } => {
			let world = world()?;

			for style in sheet_styles(&choices) {
				sheets(
					&world,
					moods.as_deref(),
					&out.join("sheets").join(style.name.name()),
					style,
				)?;
			}

			Ok(())
		}
		Command::Puppet { action } => {
			match action {
				PuppetAction::Check { id } => puppet_check(&world()?, id.as_deref()),
				PuppetAction::Template { id, from } => {
					puppet_template(&world()?, &id, from.as_deref())
				}
				PuppetAction::Sheet { id, out } => puppet_sheet(&world()?, &id, &out),
			}
		}
		Command::Mcp { root } => fables::mcp::serve(&root),
		Command::Voice { action } => {
			match action {
				VoiceAction::Providers => {
					for provider in fables::studio::Studio::voice_providers() {
						let state = if provider.ready { "ready" } else { "not ready" };
						println!("{:<11} {state:<10} {}", provider.provider, provider.needs);
					}

					Ok(())
				}
				VoiceAction::List => {
					for (name, language) in fables::studio::Studio::say_voices()? {
						println!("{name:<28} {language}");
					}

					Ok(())
				}
				VoiceAction::Audition {
					who,
					line,
					mood,
					out,
				} => audition(&world()?, &who, &line, mood.as_deref(), &out),
			}
		}
		Command::DrawingStyle { action } => {
			match action {
				StyleAction::List => {
					for name in StylePack::names() {
						let pack = StylePack::named(name)?;
						println!("{name}: {}", pack.description);
					}

					Ok(())
				}
				StyleAction::Check { name } => style_check(&name),
				StyleAction::Sheet { name, out } => style_sheet(&name, &out),
			}
		}
		Command::Rig { id, motion, out } => {
			let world = world()?;

			for style in sheet_styles(&choices) {
				rig(
					&world,
					&id,
					motion.as_deref(),
					&out.join("rig").join(style.name.name()),
					style,
				)?;
			}

			Ok(())
		}
		Command::Styles { story, out } => {
			each(&story, |path| {
				let (story, cast, _) = load(path, None, flag)?;
				styles(&story, &cast, &out)
			})
		}
		Command::Check { story } => each_style(&story, &choices, flag, check),
		Command::Review { story, out } => {
			each(&story, |path| {
				let (story, cast, _) = load(path, None, flag)?;
				review(&story, &cast, &out)
			})
		}
		Command::Proofs { story, out } => {
			each_style(&story, &choices, flag, |story, cast| {
				proofs(story, cast, &out)
			})
		}
		Command::Ebook { story, out } => {
			each_style(&story, &choices, flag, |story, cast| {
				ebook(story, cast, &out)
			})
		}
		Command::Print { story, out } => {
			each_style(&story, &choices, flag, |story, cast| {
				print(story, cast, &out)
			})
		}
		Command::Book { story, out } => {
			each_style(&story, &choices, flag, |story, cast| {
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
			each_style(&story, &choices, flag, |story, cast| {
				video(story, cast, &out, &format.formats(), preview)
			})
		}
		Command::Storyboard { story, out, format } => {
			anyhow::ensure!(
				format != Shape::Both,
				"storyboard draws one shape; use --format landscape or portrait"
			);

			each_style(&story, &choices, flag, |story, cast| {
				storyboard(story, cast, &out, format == Shape::Portrait)
			})
		}
		Command::All { story, out } => {
			each_style(&story, &choices, flag, |story, cast| {
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
	world: Option<&Path>,
	mut action: impl FnMut(&Story, &Cast) -> Result<()>,
) -> Result<()> {
	each(path, |script| {
		let mut resolved = None;

		for choice in choices {
			let (story, cast, dir) = load(script, choice.as_deref(), world)?;

			// Say which world a script resolved, so a redirected build is visible.
			if resolved.as_ref() != Some(&dir) {
				println!("  world: {}", dir.display());
				resolved = Some(dir);
			}

			println!("  style: {}", story.style().name.name());
			action(&story, &cast)?;
		}

		Ok(())
	})
}

/// Runs `action` on one script, on every `story.md` in a folder of books, or on every book in a
/// world (stopping at the first failure so a broken script is never silently skipped).
fn each(path: &Path, mut action: impl FnMut(&Path) -> Result<()>) -> Result<()> {
	if path.is_file() {
		return action(path);
	}

	let stories;
	let path = if path.join(fables::world::FILE).is_file() {
		stories = World::load(path)?.stories();
		stories.as_path()
	} else {
		path
	};

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

/// Starts a book from the world's recipe, filled in with a title, a star and an author.
fn new_story(
	world: &World,
	title: &str,
	star: &str,
	author: &str,
	dir: Option<&Path>,
) -> Result<()> {
	let recipe = world.recipe().with_context(|| {
		format!(
			"{} has no recipe.md to start a book from",
			world.dir.display()
		)
	})?;
	let recipe = std::fs::read_to_string(&recipe)
		.with_context(|| format!("could not read {}", recipe.display()))?;
	let slug = fables::story::slugify(title);
	anyhow::ensure!(
		!slug.is_empty(),
		"the title needs at least one letter or digit: it names the book's folder"
	);
	let folder = dir
		.map_or_else(|| world.stories(), Path::to_path_buf)
		.join(&slug);
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
	let script = recipe
		.replace("{{title}}", title)
		.replace("{{star}}", star)
		.replace("{{author}}", author)
		.replace("{{year}}", &year.to_string());

	std::fs::create_dir_all(&folder)
		.with_context(|| format!("could not create {}", folder.display()))?;
	std::fs::write(&path, script).with_context(|| format!("could not write {}", path.display()))?;
	println!("✓ wrote {} from the {} recipe", path.display(), world.name);

	if let Err(error) =
		load(&path, None, Some(&world.dir)).and_then(|(story, cast, _)| check(&story, &cast))
	{
		println!("  next: {error:#}");
		println!(
			"  (add `[{star}]` to {}, then run `fables proofs {}`)",
			world.cast_file().display(),
			path.display()
		);
	}

	Ok(())
}

/// Loads a script and its world's cast, furnished with the world's book matter and narrator.
/// Returns the resolved world folder too, so a build says which world it read.
fn load(path: &Path, style: Option<&str>, world: Option<&Path>) -> Result<(Story, Cast, PathBuf)> {
	let mut story = Story::load(path)?;

	if let Some(name) = style {
		if Style::parse(name).is_none() {
			anyhow::bail!(
				"unknown style `{name}`; use riso, tv, bold, cut-paper, crayon, storybook or clear"
			);
		}

		story.meta.style = Some(name.to_owned());
	}

	let world = World::for_story(&story, world)?;
	world.furnish(&mut story)?;
	let cast = world.cast_for(&story)?;

	Ok((story, cast, world.dir.clone()))
}

fn check(story: &Story, cast: &Cast) -> Result<()> {
	let report = publish::check(story, cast)?;
	println!(
		"✓ {} — {} script pages, {} book pages, {} words, {} cast members",
		report.title, report.script_pages, report.book_pages, report.words, report.cast_members
	);

	Ok(())
}

/// Prints a review's findings, fixes first, and the counts.
fn review(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let findings = publish::review(story, cast, out)?;
	let fixes = findings
		.iter()
		.filter(|finding| finding.severity == fables::review::Severity::Fix)
		.count();
	let considers = findings.len() - fixes;

	for finding in &findings {
		let severity = match finding.severity {
			fables::review::Severity::Fix => "fix     ",
			fables::review::Severity::Consider => "consider",
		};

		println!(
			"{severity} line {}: {} {}",
			finding.line, finding.observation, finding.suggestion
		);
	}

	if findings.is_empty() {
		println!("Nothing a director would change.");
	}

	println!(
		"— {} ({} pages, {} words): {fixes} fix, {considers} consider.",
		story.meta.title,
		story.pages.len(),
		fables::review::story_words(story)
	);

	Ok(())
}

fn proofs(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	done(&publish::proofs(story, cast, out)?, started);
	Ok(())
}

fn ebook(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	done(&publish::ebook(story, cast, out)?.epub, started);
	Ok(())
}

fn print(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let print = publish::print(story, cast, out)?;
	done(&print.interior, started);
	done(&print.cover, started);
	println!("{}", print.specs);
	Ok(())
}

fn sheets(world: &World, moods: Option<&str>, out: &Path, style: Style) -> Result<()> {
	let started = Instant::now();
	let cast = world.cast()?;
	let slug = fables::story::slugify(&world.name);
	std::fs::create_dir_all(out).with_context(|| format!("could not create {}", out.display()))?;

	let path = out.join(format!("{slug}-cast.png"));
	std::fs::write(
		&path,
		raster::png(&press::sheets::cast_sheet(&cast, &world.name, style)?)?,
	)?;
	done(&path, started);

	let path = out.join(format!("{slug}-places.png"));
	std::fs::write(
		&path,
		raster::png(&press::sheets::places_sheet(&*world.settings()?, style)?)?,
	)?;
	done(&path, started);

	// One setting with one cast member, drawn under every light: the world's colour script.
	let settings = world.settings()?;
	let own = settings
		.names()
		.find(|name| !built_in_names().any(|built_in| built_in == *name))
		.unwrap_or("park");
	let setting = settings.find(own)?;
	let member = cast
		.members
		.values()
		.find(|member| member.kind.is_figure())
		.with_context(|| format!("{} has nobody to stand in the light", world.name))?;
	let path = out.join(format!("{slug}-lights.png"));
	std::fs::write(
		&path,
		raster::png(&press::sheets::lights_sheet(
			&world.name,
			&setting,
			member,
			style,
		)?)?,
	)?;
	done(&path, started);

	if let Some(id) = moods {
		let member = cast
			.member(id)
			.with_context(|| format!("`{id}` is not in {}", world.cast_file().display()))?;
		let sheet = press::sheets::mood_sheet(member, style)?
			.with_context(|| format!("`{id}` is not a person, so has no moods"))?;
		let path = out.join(format!("{id}-moods.png"));
		std::fs::write(&path, raster::png(&sheet)?)?;
		done(&path, started);
	}

	Ok(())
}

fn rig(world: &World, id: &str, motion: Option<&str>, out: &Path, style: Style) -> Result<()> {
	let started = Instant::now();
	let cast = world.cast()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}", world.cast_file().display()))?;
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

	let (motion, label) = if let Some(name) = motion {
		(StylePack::named(name)?.motion, format!("-{name}"))
	} else {
		let own = look
			.puppet
			.as_ref()
			.map_or_else(Default::default, |puppet| puppet.style.motion);
		(own, String::new())
	};
	let video = dir.join(format!("{id}-motion{label}.mp4"));
	fables::video::motion::motion_test(look, &member.name, (style, &motion), &video)?;
	done(&video, started);

	// A second motion clip: the acting rules, in order, on this character and a scene
	// partner (the next person in the cast, or a copy of the same one).
	let partner = cast
		.members
		.values()
		.find(|other| other.id != member.id && other.kind.is_figure());
	let (partner_look, partner_name) = match partner {
		Some(other) => {
			let fables::cast::Kind::Figure(look) = &other.kind else {
				unreachable!("a figure partner");
			};

			(look.clone(), other.name.clone())
		}
		None => (look.clone(), member.name.clone()),
	};
	let acting = dir.join(format!("{id}-acting{label}.mp4"));
	fables::video::motion::acting_test(
		[look, &partner_look],
		&format!("{} & {}", member.name, partner_name),
		(style, &motion),
		&acting,
	)?;
	done(&acting, started);

	Ok(())
}

/// A cast member's puppet part file, or an explanation of why they have none.
fn puppet_file(world: &World, cast: &Cast, id: &str) -> Result<PathBuf> {
	let cast_path = world.cast_file();
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}", cast_path.display()))?;

	match &member.kind {
		fables::cast::Kind::Figure(look) => {
			look.puppet
				.as_ref()
				.map(|puppet| puppet.file.clone())
				.with_context(|| {
					let file = cast_path
						.with_file_name("puppets")
						.join(format!("{id}.svg"));
					let start = if file.exists() {
						format!("{} exists, so", file.display())
					} else {
						format!("start one with `fables puppet template {id}`, then")
					};
					format!(
						"`{id}` has no puppet yet: {start} add `puppet = \"puppets/{id}.svg\"` to \
						 [{id}] in {}",
						cast_path.display()
					)
				})
		}
		_ => anyhow::bail!("`{id}` is not a person, so has no puppet"),
	}
}

fn puppet_check(world: &World, id: Option<&str>) -> Result<()> {
	let cast = world.cast()?;
	let style = world.style()?;
	let ids: Vec<&str> = match id {
		Some(id) => vec![id],
		None => {
			cast.members
				.values()
				.filter(
					|member| matches!(&member.kind, fables::cast::Kind::Figure(look) if look.puppet.is_some()),
				)
				.map(|member| member.id.as_str())
				.collect()
		}
	};
	let mut errors = 0;

	for id in ids {
		let path = puppet_file(world, &cast, id)?;
		let parts = Parts::load_puppet(&path, &style.kit)?;
		let findings = parts.check();
		let count = parts.iter().count();
		println!(
			"{id}: {count} parts from {} over the {} kit",
			path.display(),
			style.name
		);

		for finding in &findings {
			println!("  {finding}");
		}

		errors += findings
			.iter()
			.filter(|finding| finding.level == Level::Error)
			.count();
	}

	anyhow::ensure!(errors == 0, "{errors} part errors");
	Ok(())
}

/// Starts a part file from another puppet's own parts, or from the style's template.
fn puppet_template(world: &World, id: &str, from: Option<&str>) -> Result<()> {
	let cast = world.cast()?;
	let style = world.style()?;
	let member = cast
		.member(id)
		.with_context(|| format!("`{id}` is not in {}", world.cast_file().display()))?;
	let target = world.dir.join("puppets").join(format!("{id}.svg"));
	anyhow::ensure!(
		!target.exists(),
		"{} already exists; edit it, or move it aside to start again",
		target.display()
	);

	// Only the character's own parts: the kit is shared, and redrawing a kit part is a choice.
	let (parts, origin) = match from {
		Some(from) => {
			(
				Parts::load(&puppet_file(world, &cast, from)?)?,
				from.to_owned(),
			)
		}
		None => {
			(
				style.template.clone(),
				format!("the {} template", style.name),
			)
		}
	};
	let drawings: Vec<(&str, &fables::art::shape::Shape)> = parts.iter().collect();
	let title = format!(
		"{}: own parts (started from {origin}; redraw each one)",
		member.name
	);
	std::fs::create_dir_all(world.dir.join("puppets"))
		.with_context(|| format!("could not create {}", world.dir.join("puppets").display()))?;
	std::fs::write(&target, parts::write(&title, &drawings))
		.with_context(|| format!("could not write {}", target.display()))?;
	println!(
		"✓ wrote {}\n  Add `puppet = \"puppets/{id}.svg\"` to [{id}] in {}, redraw the parts, and \
		 run `fables puppet check {id}`.",
		target.display(),
		world.cast_file().display()
	);
	Ok(())
}

/// A puppet's model sheet (the one designs are signed off on) and its parts sheet.
fn puppet_sheet(world: &World, id: &str, out: &Path) -> Result<()> {
	let started = Instant::now();
	let dir = out.join("puppets");
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
	let kinds = [SheetKind::Model, SheetKind::Parts];
	let images = fables::studio::sheets_for(world, id, &kinds, 2600)?;

	for (kind, image) in ["model", "parts"].iter().zip(images) {
		let sheet = dir.join(format!("{id}-{kind}.png"));
		std::fs::write(&sheet, image.png)
			.with_context(|| format!("could not write {}", sheet.display()))?;
		done(&sheet, started);
	}

	Ok(())
}

fn audition(world: &World, who: &str, line: &str, mood: Option<&str>, out: &Path) -> Result<()> {
	let started = Instant::now();
	let take = fables::studio::audition(world, who, line, mood, &out.join("cache/voice"))?;
	let dir = out.join("voices");
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
	let path = dir.join(format!(
		"{who}-{}.wav",
		fables::story::slugify(&line.chars().take(40).collect::<String>())
	));
	std::fs::write(&path, take.wav)
		.with_context(|| format!("could not write {}", path.display()))?;
	println!("  {} ({:.1}s)", take.label, take.seconds);
	done(&path, started);
	Ok(())
}

fn style_check(name: &str) -> Result<()> {
	let pack = StylePack::named(name)?;
	let findings = pack.check();
	let bodies: Vec<&str> = pack.bodies().map(|(body, _)| body).collect();
	println!(
		"{name}: {} kit parts, {} template parts, bodies: {}",
		pack.kit.iter().count(),
		pack.template.iter().count(),
		bodies.join(", ")
	);

	for finding in &findings {
		println!("  {finding}");
	}

	let errors = findings
		.iter()
		.filter(|finding| finding.level == Level::Error)
		.count();
	anyhow::ensure!(errors == 0, "{errors} errors in the {name} style");
	Ok(())
}

fn style_sheet(name: &str, out: &Path) -> Result<()> {
	let started = Instant::now();
	let dir = out.join("styles");
	std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;

	for (kind, image) in ["model", "parts"]
		.iter()
		.zip(fables::studio::Studio::style_sheets(name, 2600)?)
	{
		let sheet = dir.join(format!("{name}-{kind}.png"));
		std::fs::write(&sheet, image.png)
			.with_context(|| format!("could not write {}", sheet.display()))?;
		done(&sheet, started);
	}

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

	for path in publish::video(story, cast, out, formats, preview)? {
		done(&path, started);
	}

	Ok(())
}

fn storyboard(story: &Story, cast: &Cast, out: &Path, portrait: bool) -> Result<()> {
	let started = Instant::now();

	for path in publish::storyboard(story, cast, out, portrait)? {
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
