//! The `fables` command line.

use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use fables::art::pack::StylePack;
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
use fables::studio::BuildName;
use fables::studio::CharacterBrief;
use fables::studio::Colours;
use fables::studio::PartData;
use fables::studio::SheetKind;
use fables::studio::Studio;
use fables::studio::WorldBrief;
use fables::video::frames::Format;
use fables::world::World;

/// Writes a line of the command's output, returning its error instead of panicking the way
/// `println!` does.
///
/// The reader can go away before the command has finished: `fables style list | head -1` closes
/// the pipe after its one line. [`main`] treats that broken pipe as the end of the job.
fn say(line: std::fmt::Arguments<'_>) -> Result<()> {
	writeln!(std::io::stdout().lock(), "{line}")?;
	Ok(())
}

/// Writes a line of output through [`say`].
macro_rules! say {
	($($arg:tt)*) => {
		say(format_args!($($arg)*))
	};
}

/// Turn a picture-book script into proofs, an ePub, KDP print files and narrated videos.
///
/// Every command that takes a story also takes a folder of books (e.g. a world's `stories/`), or a
/// world folder, and builds each `story.md` inside it.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
	#[command(subcommand)]
	command: Command,
	/// Draw in these page skins instead of the script's own: riso, tv, bold, cut-paper, crayon,
	/// storybook, clear, a comma-separated list (bold,tv), or all. Each skin renders into its
	/// own folder. For the drawing styles figures are built with (chibi, clear-line…), see
	/// `fables style list`.
	#[arg(long, global = true)]
	style: Option<String>,
	/// The world to work in: a folder with a world.toml. Defaults to the nearest world above the
	/// script (or the current folder), or the only one in a `worlds/` folder here.
	#[arg(long, global = true)]
	world: Option<PathBuf>,
	/// The folder whose `worlds/` the studio works in (default: the current folder).
	#[arg(long, global = true, default_value = ".")]
	root: PathBuf,
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
		/// The author on the title page. Defaults to the world's `author` in world.toml.
		#[arg(long)]
		author: Option<String>,
		/// Where books live (default: the world's stories/ folder).
		#[arg(long)]
		dir: Option<PathBuf>,
	},
	/// Start a world, and add characters to one.
	World {
		#[command(subcommand)]
		action: WorldAction,
	},
	/// Add a character to a world's cast.
	Character {
		#[command(subcommand)]
		action: CharacterAction,
	},
	/// Start a world's own setting, blank or from one it already uses.
	Setting {
		#[command(subcommand)]
		action: SettingAction,
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
	/// Draw the same pages in every page skin, side by side, to choose a look.
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
		/// Outline, on every setting of the places sheet, where a typical text card and the
		/// actors sit.
		#[arg(long)]
		zones: bool,
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
	Mcp,
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
enum WorldAction {
	/// Start a world: a world.toml, an empty cast with a commented example, and stories/.
	///
	/// Example: `fables world new "Teacup Lane" --style chibi --author "Ifiok Jr."`
	New {
		/// The world's name, e.g. "Teacup Lane". Its folder is the name as a slug.
		name: String,
		/// The drawing style its folk are drawn in, from `fables style list`.
		#[arg(long)]
		style: String,
		/// A sentence or two on the world: where it is, its rule, its tone.
		#[arg(long)]
		about: Option<String>,
		/// The author credited in its books, unless a book names its own.
		#[arg(long)]
		author: Option<String>,
	},
}

#[derive(Subcommand)]
enum CharacterAction {
	/// Add a character to the world's cast, in their own colours.
	///
	/// Example: `fables character new pip --name "Pip Pocket" --top teal --draw`
	New {
		/// The cast id scripts use after `@`: lowercase letters, digits and hyphens.
		id: String,
		/// Their full name, e.g. "Pip Pocket".
		#[arg(long)]
		name: String,
		/// kid or grown-up.
		#[arg(long, default_value = "kid", value_parser = ["kid", "grown-up"])]
		kind: String,
		/// Who they are, in a sentence.
		#[arg(long)]
		about: Option<String>,
		#[arg(long)]
		skin: Option<String>,
		#[arg(long = "hair-colour")]
		hair_colour: Option<String>,
		#[arg(long)]
		top: Option<String>,
		#[arg(long)]
		trim: Option<String>,
		#[arg(long)]
		bottom: Option<String>,
		#[arg(long)]
		shoes: Option<String>,
		#[arg(long)]
		eyes: Option<String>,
		/// The pattern on the top, in the trim colour: plain, stripes, dots or star.
		#[arg(long)]
		pattern: Option<String>,
		/// Also write their part file from the style's template, so it can be redrawn.
		#[arg(long)]
		draw: bool,
	},
}

#[derive(Subcommand)]
enum SettingAction {
	/// Start a world's own setting, blank or from a setting it already uses.
	///
	/// Example: `fables setting new harbour --from park --about "The harbour wall"`
	New {
		/// The setting's name, as a page heading uses it (`# harbour`).
		name: String,
		/// A setting to start from, to repaint it; leave out for a sky over the ground.
		#[arg(long)]
		from: Option<String>,
		/// A line on what the place is (default: the name in words).
		#[arg(long)]
		about: Option<String>,
	},
}

#[derive(Subcommand)]
enum PuppetAction {
	/// Check part files against the parts a figure is drawn with: missing parts, views without
	/// their pair, parts that swap instead of blending, names no figure uses, and parts that
	/// come apart when the figure is posed on its own body (a head clear of the torso, an arm or
	/// leg starting outside it).
	Check {
		/// The cast id to check; leave it out to check every person drawn from parts, their own
		/// part file or their style's template for their body.
		id: Option<String>,
	},
	/// Start a cast member's part file to redraw part by part, from the style's template or
	/// another puppet's drawings, and point their cast entry at it.
	///
	/// Every style has two bodies, a kid and a grown-up, and a template drawn for each: a cast
	/// member with `kind = "grown-up"` starts from the grown-up's torso, sleeves and legwear, cut
	/// for the taller body; anyone else starts from the kid's.
	///
	/// Example: `fables puppet template pip`
	Template {
		/// The cast id the file is for, e.g. bogey. Its `kind` in the cast file (`kid` or
		/// `grown-up`) picks the body the template is drawn for.
		id: String,
		/// The cast member whose drawings to start from (default: the style's template).
		#[arg(long)]
		from: Option<String>,
	},
	/// Print a character's parts as path data: their own (or their style's template) and the
	/// kit's, marked.
	///
	/// Example: `fables puppet parts pip head.front ear`
	Parts {
		/// The cast id, e.g. stinky.
		id: String,
		/// Parts to read; leave empty for all their own parts.
		parts: Vec<String>,
	},
	/// Write or replace a character's parts, through the same check the MCP server uses: a
	/// change that would break the figure is refused with the findings.
	///
	/// Example: `fables puppet set pip ear --d "M0 -20 C-16 -32 -36 -20 -34 2 Z"`
	Set {
		/// The cast id, e.g. stinky.
		id: String,
		/// The part to write, e.g. hair.front, with `--d`.
		part: Option<String>,
		/// SVG path data for the part.
		#[arg(long, value_name = "PATH")]
		d: Option<String>,
		/// An SVG file of several parts, each path's id its name.
		#[arg(long, value_name = "FILE")]
		file: Option<PathBuf>,
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
	/// List the drawing styles, with the bodies (kid, grown-up) and page skin of each.
	List,
	/// Check a style: each body's template over its kit must draw a whole figure whose parts
	/// join up when posed, and its bodies must stand up.
	Check { name: String },
	/// Describe a style: its finish, bodies and the template each is drawn from, every part a
	/// figure is drawn with, its coordinate space and the drawing rules. Read this before
	/// drawing a part.
	///
	/// Example: `fables style describe chibi`
	Describe { name: String },
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
	match run(Cli::parse()) {
		Ok(()) => {}
		Err(error) if broken_pipe(&error) => {}
		Err(error) => {
			eprintln!("error: {error:#}");
			std::process::exit(1);
		}
	}
}

/// Whether the command failed only because its reader went away (`| head -1`), which is not a
/// failure to report.
fn broken_pipe(error: &anyhow::Error) -> bool {
	error.chain().any(|cause| {
		cause
			.downcast_ref::<std::io::Error>()
			.is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
	})
}

fn run(cli: Cli) -> Result<()> {
	let Cli {
		command,
		style,
		world: flag,
		root,
	} = cli;
	let flag = flag.as_deref();
	// The world for commands that don't start from a script.
	let world = || {
		match flag {
			Some(dir) => World::load(dir),
			None => World::find(&root),
		}
	};

	match command {
		Command::New {
			title,
			star,
			author,
			dir,
		} => new_story(&world()?, &title, &star, author.as_deref(), dir.as_deref()),
		Command::World { action } => {
			match action {
				WorldAction::New {
					name,
					style,
					about,
					author,
				} => world_new(&root, &name, &style, about.as_deref(), author.as_deref()),
			}
		}
		Command::Character { action } => {
			match action {
				CharacterAction::New {
					id,
					name,
					kind,
					about,
					skin,
					hair_colour,
					top,
					trim,
					bottom,
					shoes,
					eyes,
					pattern,
					draw,
				} => {
					let build = if kind == "grown-up" {
						BuildName::GrownUp
					} else {
						BuildName::Kid
					};
					let brief = CharacterBrief {
						id,
						name,
						build,
						born: None,
						about,
						habit: None,
						colours: Colours {
							skin: skin.unwrap_or_else(|| "tangerine".into()),
							hair: hair_colour.unwrap_or_else(|| "plum".into()),
							top: top.unwrap_or_else(|| "teal".into()),
							trim: trim.unwrap_or_else(|| "cream".into()),
							bottom: bottom.unwrap_or_else(|| "night".into()),
							shoes: shoes.unwrap_or_else(|| "ink".into()),
							eyes: eyes.unwrap_or_else(|| "cocoa".into()),
						},
						pattern,
						extras: Vec::new(),
						draw,
					};

					character_new(&world()?, &brief)
				}
			}
		}
		Command::Setting { action } => {
			match action {
				SettingAction::New { name, from, about } => {
					setting_new(&world()?, &name, from.as_deref(), about.as_deref())
				}
			}
		}
		Command::Sheets { moods, zones, out } => {
			let choices = style_choices(style.as_deref())?;
			let world = world()?;
			let zones = if zones {
				Some(press::sheets::Zones::measure(&world.style()?)?)
			} else {
				None
			};

			for style in sheet_styles(&choices) {
				sheets(
					&world,
					moods.as_deref(),
					zones.as_ref(),
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
				PuppetAction::Parts { id, parts: names } => puppet_parts(&world()?, &id, &names),
				PuppetAction::Set { id, part, d, file } => {
					puppet_set(
						&world()?,
						&id,
						part.as_deref(),
						d.as_deref(),
						file.as_deref(),
					)
				}
				PuppetAction::Sheet { id, out } => puppet_sheet(&world()?, &id, &out),
			}
		}
		Command::Mcp => fables::mcp::serve(&root),
		Command::Voice { action } => {
			match action {
				VoiceAction::Providers => {
					for provider in Studio::voice_providers() {
						let state = if provider.ready { "ready" } else { "not ready" };
						say!("{:<11} {state:<10} {}", provider.provider, provider.needs)?;
					}

					Ok(())
				}
				VoiceAction::List => {
					for (name, language) in Studio::say_voices()? {
						say!("{name:<28} {language}")?;
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
						let bodies: Vec<&str> = pack.bodies().map(|(body, _)| body).collect();
						say!(
							"{name} (skin: {}; bodies: {}): {}",
							pack.skin,
							bodies.join(", "),
							pack.description
						)?;
					}

					Ok(())
				}
				StyleAction::Check { name } => style_check(&name),
				StyleAction::Describe { name } => style_describe(&name),
				StyleAction::Sheet { name, out } => style_sheet(&name, &out),
			}
		}
		Command::Rig { id, motion, out } => {
			let choices = style_choices(style.as_deref())?;
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
		Command::Check { story } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, check)
		}
		Command::Review { story, out } => {
			each(&story, |path| {
				let (story, cast, _) = load(path, None, flag)?;
				review(&story, &cast, &out)
			})
		}
		Command::Proofs { story, out } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
				proofs(story, cast, &out)
			})
		}
		Command::Ebook { story, out } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
				ebook(story, cast, &out)
			})
		}
		Command::Print { story, out } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
				print(story, cast, &out)
			})
		}
		Command::Book { story, out } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
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
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
				video(story, cast, &out, &format.formats(), preview)
			})
		}
		Command::Storyboard { story, out, format } => {
			anyhow::ensure!(
				format != Shape::Both,
				"storyboard draws one shape; use --format landscape or portrait"
			);
			let choices = style_choices(style.as_deref())?;

			each_style(&story, &choices, flag, |story, cast, _| {
				storyboard(story, cast, &out, format == Shape::Portrait)
			})
		}
		Command::All { story, out } => {
			let choices = style_choices(style.as_deref())?;
			each_style(&story, &choices, flag, |story, cast, _| {
				proofs(story, cast, &out)?;
				ebook(story, cast, &out)?;
				print(story, cast, &out)?;
				video(story, cast, &out, &Shape::Both.formats(), false)
			})
		}
	}
}

/// The page skins asked for with `--style`: one name, a comma-separated list, or `all`. `None`
/// in the list means "the skin the script or its drawing style names".
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
					"unknown page skin `{name}`; use {}, a comma-separated list, or all (for \
					 drawing styles such as chibi, run `fables style list`)",
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

/// Runs `action` on every script (see [`each`]) once per chosen page skin. The third argument
/// says whether this is the first skin, so a note is printed once rather than once per skin.
fn each_style(
	path: &Path,
	choices: &[Option<String>],
	world: Option<&Path>,
	mut action: impl FnMut(&Story, &Cast, bool) -> Result<()>,
) -> Result<()> {
	each(path, |script| {
		let mut resolved = None;

		for (index, choice) in choices.iter().enumerate() {
			let (story, cast, dir) = load(script, choice.as_deref(), world)?;

			// Say which world a script resolved, so a redirected build is visible.
			if resolved.as_ref() != Some(&dir) {
				say!("  world: {}", dir.display())?;
				resolved = Some(dir);
			}

			say!(
				"  drawing: {} · skin: {}",
				story.drawing_style,
				story.style().name.name()
			)?;
			action(&story, &cast, index == 0)?;
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
		say!("— {}", script.display())?;
		action(script).with_context(|| format!("while building {}", script.display()))?;
	}

	Ok(())
}

/// The age band every new book is read aloud to until its author says otherwise.
const NEW_BOOK_AGE: &str = "3-5";

/// Gives a new book the default `age` in its front matter. Every new book's text passes here,
/// whether it came from the world's `recipe.md` or from the built-in flow that starts a book
/// in a world without one, so both paths get the band; a recipe that names its own `age`
/// keeps it.
fn with_default_age(script: String) -> String {
	let front = script.split("\n+++").next().unwrap_or(&script);
	let names_age = front.lines().skip(1).any(|line| {
		line.trim()
			.strip_prefix("age")
			.is_some_and(|rest| rest.trim_start().starts_with('='))
	});

	if names_age {
		return script;
	}

	match script.find("\n+++") {
		Some(end) => {
			format!(
				"{}\nage = \"{NEW_BOOK_AGE}\"{}",
				&script[..end],
				&script[end..]
			)
		}
		None => script,
	}
}

/// Starts a book from the world's recipe, filled in with a title, a star and an author.
fn new_story(
	world: &World,
	title: &str,
	star: &str,
	author: Option<&str>,
	dir: Option<&Path>,
) -> Result<()> {
	let author = author.or(world.author.as_deref()).with_context(|| {
		"no author for the title page: pass --author, or set author in the world's world.toml"
	})?;
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
	let script = with_default_age(
		recipe
			.replace("{{title}}", title)
			.replace("{{star}}", star)
			.replace("{{author}}", author)
			.replace("{{year}}", &year.to_string()),
	);

	std::fs::create_dir_all(&folder)
		.with_context(|| format!("could not create {}", folder.display()))?;
	std::fs::write(&path, script).with_context(|| format!("could not write {}", path.display()))?;
	say!("✓ wrote {} from the {} recipe", path.display(), world.name)?;

	if let Err(error) =
		load(&path, None, Some(&world.dir)).and_then(|(story, cast, _)| check(&story, &cast, true))
	{
		say!("  next: {error:#}")?;
		say!(
			"  (add `[{star}]` to {}, then run `fables proofs {}`)",
			world.cast_file().display(),
			path.display()
		)?;
	}

	Ok(())
}

/// Starts a world under `<root>/worlds/`, and says what to make next.
fn world_new(
	root: &Path,
	name: &str,
	style: &str,
	about: Option<&str>,
	author: Option<&str>,
) -> Result<()> {
	let studio = Studio::new(root);
	let world = studio.create_world(&WorldBrief {
		name: name.to_owned(),
		about: about.map(str::to_owned),
		style: Some(style.to_owned()),
		author: author.map(str::to_owned),
	})?;
	let dir = root.join("worlds").join(&world.folder);
	let skin = StylePack::named(&world.style)?.skin.clone();
	say!(
		"✓ started {} in {} (drawing style {}, page skin {skin})",
		world.name,
		dir.display(),
		world.style
	)?;
	say!(
		"  next: fables character new pip --name \"Pip Pocket\" --world {}",
		dir.display()
	)?;
	Ok(())
}

/// Adds a character to a world's cast, drawn from its style's template.
fn character_new(world: &World, brief: &CharacterBrief) -> Result<()> {
	let report = fables::studio::create_character(world, brief)?;
	let how = match report.drawn {
		Some("puppet") => {
			format!(
				"own part file {}",
				report.puppet.as_deref().unwrap_or_default()
			)
		}
		Some("template") => {
			format!(
				"drawn from {}",
				world.style()?.template_origin(brief.build.build())
			)
		}
		_ => "drawn by the parametric puppet".to_owned(),
	};
	say!(
		"✓ added {} ({}) to {} — {how}",
		report.name,
		report.id,
		world.cast_file().display()
	)?;
	let next = if brief.draw {
		format!("fables puppet parts {}", brief.id)
	} else {
		format!("fables puppet sheet {}", brief.id)
	};
	say!("  next: {next}")?;
	Ok(())
}

/// Starts a world's own setting.
fn setting_new(world: &World, name: &str, from: Option<&str>, about: Option<&str>) -> Result<()> {
	let about = about.map_or_else(|| name.replace('-', " "), str::to_owned);
	let setting = fables::studio::create_setting(world, name, &about, from)?;
	say!(
		"✓ created `{}` in {}",
		setting.name,
		world
			.dir
			.join("settings")
			.join(format!("{name}.svg"))
			.display()
	)?;
	say!("  next: fables sheets   (see every setting drawn)")?;
	Ok(())
}

/// Prints what a drawing style offers a character designer.
fn style_describe(name: &str) -> Result<()> {
	let report = Studio::describe_style(name)?;
	say!("{}: {}", report.name, report.description)?;
	say!("page skin: {}", report.skin)?;
	let finish = &report.finish;
	say!(
		"finish: line {}, detail {}, light ({}, {}), cel shadow {}, far {}",
		finish.line,
		finish.detail,
		finish.light.x,
		finish.light.y,
		finish.cel_shadow,
		finish.far
	)?;
	say!(
		"face: turn {}, radius {}, slide {} (head units)",
		report.face.turn,
		report.face.radius,
		report.face.slide
	)?;
	say!("bodies (figure units, soles at y = 0, up is negative):")?;

	for body in &report.bodies {
		say!(
			"  {:<9} neck y={}, shoulder y={}, pelvis y={}; drawn from {} ({} parts)",
			body.name,
			body.neck_y,
			body.shoulder_y,
			body.pelvis_y,
			body.template,
			body.template_parts
		)?;
	}

	say!("  (`kind = \"grown-up\"` in the cast file puts a person on the grown-up body)")?;

	say!("parts (name, views, need, home, space):")?;

	for part in &report.parts {
		say!(
			"  {:<22} {:<8} {:<8} {:<9} {}",
			part.name,
			part.views,
			part.need,
			part.home,
			part.space
		)?;
	}

	say!("rules:")?;

	for rule in &report.rules {
		say!("  - {rule}")?;
	}

	say!("  next: fables character new pip --name \"Pip Pocket\"   (start one from the template)")?;
	Ok(())
}

/// Loads a script and its world's cast, furnished with the world's book matter and narrator.
/// Returns the resolved world folder too, so a build says which world it read.
fn load(path: &Path, style: Option<&str>, world: Option<&Path>) -> Result<(Story, Cast, PathBuf)> {
	let mut story = Story::load(path)?;

	if let Some(name) = style {
		if Style::parse(name).is_none() {
			anyhow::bail!(
				"unknown page skin `{name}`; use riso, tv, bold, cut-paper, crayon, storybook or \
				 clear (for drawing styles, run `fables style list`)"
			);
		}

		story.meta.style = Some(name.to_owned());
	}

	let world = World::for_story(&story, world)?;
	world.furnish(&mut story)?;
	let cast = world.cast_for(&story)?;

	Ok((story, cast, world.dir.clone()))
}

fn check(story: &Story, cast: &Cast, notes: bool) -> Result<()> {
	let report = publish::check(story, cast)?;
	say!(
		"✓ {} — {} script pages, {} book pages, {} words, {} cast members",
		report.title,
		report.script_pages,
		report.book_pages,
		report.words,
		report.cast_members
	)?;

	if notes {
		for note in &report.notes {
			say!("  note: {note}")?;
		}
	}

	if story.age.is_none() {
		println!(
			"· no `age` in the front matter, so the narrator keeps the older pace; add `age = \
			 \"3-5\"` (or `under-3`, `5-7`) to pace the read-aloud."
		);
	}

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

		say!(
			"{severity} line {}: {} {}",
			finding.line,
			finding.observation,
			finding.suggestion
		)?;
	}

	if findings.is_empty() {
		say!("Nothing a director would change.")?;
	}

	say!(
		"— {} ({} pages, {} words): {fixes} fix, {considers} consider.",
		story.meta.title,
		story.pages.len(),
		fables::review::story_words(story)
	)?;

	Ok(())
}

fn proofs(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	done(&publish::proofs(story, cast, out)?, started)?;
	Ok(())
}

fn ebook(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	done(&publish::ebook(story, cast, out)?.epub, started)?;
	Ok(())
}

fn print(story: &Story, cast: &Cast, out: &Path) -> Result<()> {
	let started = Instant::now();
	let print = publish::print(story, cast, out)?;
	done(&print.interior, started)?;
	done(&print.cover, started)?;
	say!("{}", print.specs)?;
	Ok(())
}

fn sheets(
	world: &World,
	moods: Option<&str>,
	zones: Option<&press::sheets::Zones>,
	out: &Path,
	style: Style,
) -> Result<()> {
	let started = Instant::now();
	let cast = world.cast()?;
	let slug = fables::story::slugify(&world.name);
	std::fs::create_dir_all(out).with_context(|| format!("could not create {}", out.display()))?;

	let path = out.join(format!("{slug}-cast.png"));
	std::fs::write(
		&path,
		raster::png(&press::sheets::cast_sheet(&cast, &world.name, style)?)?,
	)?;
	done(&path, started)?;

	let path = out.join(format!("{slug}-places.png"));
	std::fs::write(
		&path,
		raster::png(&press::sheets::places_sheet(
			&*world.settings()?,
			style,
			zones,
		)?)?,
	)?;
	done(&path, started)?;

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
	done(&path, started)?;

	if let Some(id) = moods {
		let member = cast
			.member(id)
			.with_context(|| format!("`{id}` is not in {}", world.cast_file().display()))?;
		let sheet = press::sheets::mood_sheet(member, style)?
			.with_context(|| format!("`{id}` is not a person, so has no moods"))?;
		let path = out.join(format!("{id}-moods.png"));
		std::fs::write(&path, raster::png(&sheet)?)?;
		done(&path, started)?;
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
	done(&sheet, started)?;

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
	done(&video, started)?;

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
	done(&acting, started)?;

	Ok(())
}

fn puppet_check(world: &World, id: Option<&str>) -> Result<()> {
	let cast = world.cast()?;
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
		let member = cast
			.member(id)
			.with_context(|| format!("`{id}` is not in {}", world.cast_file().display()))?;
		let fables::cast::Kind::Figure(look) = &member.kind else {
			anyhow::bail!("`{id}` is not a person, so has no puppet");
		};
		let Some(puppet) = &look.puppet else {
			anyhow::bail!(
				"`{id}` is drawn by the parametric puppet, so has no parts to check; start a \
				 drawing with `fables puppet template {id}`"
			);
		};
		// The parts, and whether they join up on the character's own body, posed.
		let mut findings = puppet.parts.check();
		findings.extend(fables::art::puppet::joins(puppet, look.build));
		let count = puppet.parts.iter().count();
		let source = if puppet.template {
			format!(
				"{} (no part file of their own)",
				puppet.style.template_origin(look.build)
			)
		} else {
			puppet.file.display().to_string()
		};
		say!("{id}: {count} parts from {source}")?;

		for finding in &findings {
			say!("  {finding}")?;
		}

		errors += findings
			.iter()
			.filter(|finding| finding.level == Level::Error)
			.count();
	}

	anyhow::ensure!(errors == 0, "{errors} part errors");
	Ok(())
}

/// A character's parts as path data, each marked with where it came from.
fn puppet_parts(world: &World, id: &str, names: &[String]) -> Result<()> {
	let parts = fables::studio::character_parts(world, id, names)?;

	for part in &parts {
		say!("{:<8} {:<24} {}", part.from, part.name, part.d)?;
	}

	say!("— {} parts", parts.len())?;
	say!(
		"  next: fables puppet set {id} <part> --d \"M…\"   (redraw one, checked before it is \
		 written)"
	)?;
	Ok(())
}

/// Writes parts through the studio's checked operation: a change that would break the figure is
/// refused with the findings.
fn puppet_set(
	world: &World,
	id: &str,
	part: Option<&str>,
	d: Option<&str>,
	file: Option<&Path>,
) -> Result<()> {
	let parts = match (part, d, file) {
		(Some(part), Some(d), None) => {
			vec![PartData {
				name: part.to_owned(),
				d: d.to_owned(),
			}]
		}
		(Some(part), None, None) => {
			anyhow::bail!("`{part}` needs `--d \"<path data>\"`, or use `--file <svg>`")
		}
		(None, Some(_), None) => {
			anyhow::bail!(
				"`--d` needs a part name, e.g. `fables puppet set {id} hair.front --d \"M…\"`"
			)
		}
		(None, None, Some(file)) => parts_from_file(file)?,
		(None, None, None) => {
			anyhow::bail!(
				"name a part with `--d`, or an SVG file of parts with `--file`; `fables puppet \
				 parts {id}` lists the names"
			)
		}
		_ => anyhow::bail!("use either `--d` or `--file`, not both"),
	};

	anyhow::ensure!(!parts.is_empty(), "no parts to write");
	let update = fables::studio::set_parts(world, id, &parts, &[])?;

	for finding in &update.findings {
		say!("  {finding}")?;
	}

	anyhow::ensure!(
		update.written,
		"the change would break `{id}`'s figure, so nothing was written"
	);
	say!(
		"✓ wrote {} part{} to {id}'s part file ({} own parts)",
		parts.len(),
		if parts.len() == 1 { "" } else { "s" },
		update.own_parts
	)?;
	say!("  next: fables puppet sheet {id}")?;
	Ok(())
}

/// Parts from an SVG file: a whole document, or a fragment of elements.
fn parts_from_file(file: &Path) -> Result<Vec<PartData>> {
	let source = std::fs::read_to_string(file)
		.with_context(|| format!("could not read {}", file.display()))?;
	let document = if source.trim_start().starts_with("<svg") {
		source
	} else {
		format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{source}</svg>")
	};
	let parts = Parts::parse(&document).with_context(|| format!("in {}", file.display()))?;

	Ok(parts
		.iter()
		.map(|(name, shape)| {
			PartData {
				name: name.to_owned(),
				d: shape.exact_data(),
			}
		})
		.collect())
}

/// Starts a part file and points the cast entry at it.
fn puppet_template(world: &World, id: &str, from: Option<&str>) -> Result<()> {
	let report = fables::studio::template_puppet(world, id, from)?;
	say!(
		"✓ wrote {} and set `puppet` in {}",
		report.puppet.as_deref().unwrap_or("the part file"),
		world.cast_file().display()
	)?;
	say!("  next: fables puppet sheet {id}")?;
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
		done(&sheet, started)?;
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
	say!("  {} ({:.1}s)", take.label, take.seconds)?;
	done(&path, started)?;
	Ok(())
}

fn style_check(name: &str) -> Result<()> {
	let pack = StylePack::named(name)?;
	let findings = pack.check();
	let bodies: Vec<&str> = pack.bodies().map(|(body, _)| body).collect();
	say!(
		"{name}: {} kit parts, {} kid and {} grown-up template parts, bodies: {}",
		pack.kit.iter().count(),
		pack.template_for(fables::art::figure::Build::Kid)
			.iter()
			.count(),
		pack.template_for(fables::art::figure::Build::Grownup)
			.iter()
			.count(),
		bodies.join(", ")
	)?;

	for finding in &findings {
		say!("  {finding}")?;
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
		.zip(Studio::style_sheets(name, 2600)?)
	{
		let sheet = dir.join(format!("{name}-{kind}.png"));
		std::fs::write(&sheet, image.png)
			.with_context(|| format!("could not write {}", sheet.display()))?;
		done(&sheet, started)?;
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
	done(&path, started)?;

	Ok(())
}

fn video(story: &Story, cast: &Cast, out: &Path, formats: &[Format], preview: bool) -> Result<()> {
	let started = Instant::now();

	for path in publish::video(story, cast, out, formats, preview)? {
		done(&path, started)?;
	}

	Ok(())
}

fn storyboard(story: &Story, cast: &Cast, out: &Path, portrait: bool) -> Result<()> {
	let started = Instant::now();

	for path in publish::storyboard(story, cast, out, portrait)? {
		done(&path, started)?;
	}

	Ok(())
}

fn done(path: &Path, started: Instant) -> Result<()> {
	say!(
		"✓ wrote {} in {:.1}s",
		path.display(),
		started.elapsed().as_secs_f32()
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A minimal world whose recipe `fables new` can start a book from.
	fn new_world(dir: &Path, extra: &str) -> World {
		let home = dir.join("worlds/home");
		std::fs::create_dir_all(home.join("stories")).unwrap();
		std::fs::write(
			home.join(fables::world::FILE),
			format!("name = \"Home\"\n{extra}"),
		)
		.unwrap();
		std::fs::write(
			home.join("recipe.md"),
			"+++\ntitle = \"{{title}}\"\nauthor = \"{{author}}\"\nstar = \"{{star}}\"\n+++\n# \
			 plain\nOnce upon a time.\n",
		)
		.unwrap();
		World::load(&home).unwrap()
	}

	/// Runs `new_story` and reads the book it wrote.
	fn new_book(world: &World, author: Option<&str>) -> Result<String> {
		let path = world.stories().join("a-new-tale/story.md");
		new_story(world, "A New Tale", "mo", author, None)?;
		Ok(std::fs::read_to_string(path)?)
	}

	#[test]
	fn the_author_flag_wins_over_the_world() {
		let dir = tempfile::tempdir().unwrap();
		let world = new_world(dir.path(), "author = \"World Author\"\n");
		let script = new_book(&world, Some("Flag Author")).unwrap();
		assert!(script.contains("author = \"Flag Author\""), "{script}");
	}

	#[test]
	fn the_worlds_author_is_the_default() {
		let dir = tempfile::tempdir().unwrap();
		let world = new_world(dir.path(), "author = \"World Author\"\n");
		let script = new_book(&world, None).unwrap();
		assert!(script.contains("author = \"World Author\""), "{script}");
	}

	#[test]
	fn no_author_is_an_error_that_says_what_to_do() {
		let dir = tempfile::tempdir().unwrap();
		let world = new_world(dir.path(), "");
		let error = format!("{:#}", new_book(&world, None).unwrap_err());
		assert!(
			error.contains("--author") && error.contains("world.toml"),
			"{error}"
		);
	}

	#[test]
	fn a_new_book_starts_at_the_default_age() {
		let dir = tempfile::tempdir().unwrap();
		let world = new_world(dir.path(), "author = \"World Author\"\n");
		let script = new_book(&world, None).unwrap();
		assert!(script.contains("age = \"3-5\""), "{script}");
		assert_eq!(script.matches("\nage").count(), 1, "{script}");
	}

	#[test]
	fn a_recipe_that_names_its_own_age_keeps_it() {
		let own = "+++\ntitle = \"T\"\nage = \"5-7\"\n+++\n\n# park\nHi.\n";
		assert_eq!(with_default_age(own.to_owned()), own);
		let tight = "+++\ntitle = \"T\"\nage=\"under-3\"\n+++\n";
		assert_eq!(with_default_age(tight.to_owned()), tight);
	}
}
