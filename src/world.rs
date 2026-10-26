//! Worlds: a series' cast, puppets, stories and defaults, kept together in one folder.
//!
//! ```text
//! worlds/kerfuffle/
//!   world.toml      the world's name, its drawing style and series defaults (the narrator,
//!                   the book's matter)
//!   cast.toml       who lives there and what they look like
//!   puppets/        their authored puppets' part files
//!   recipe.md       the template `fables new` starts a book from
//!   stories/        one folder per book, each with a story.md
//! ```
//!
//! A command finds its world from `--world`, from a script's `world = "…"` front matter, or by
//! walking up from the script (or the current folder) to the nearest `world.toml`. From a
//! folder that holds `worlds/`, a single world there is used without asking.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use crate::art::pack;
use crate::art::pack::StylePack;
use crate::cast::Cast;
use crate::story::Story;
use crate::video::voice::Voice;

/// The file that marks a folder as a world.
pub const FILE: &str = "world.toml";

/// A loaded world.
#[derive(Clone, Debug)]
pub struct World {
	/// The folder the world lives in.
	pub dir: PathBuf,
	pub name: String,
	/// The style pack its puppets are drawn in (see [`StylePack`]).
	pub style: String,
	/// A few sentences on the world: where it is, its rule, its tone.
	pub about: Option<String>,
	/// Who reads the books aloud, unless a script names its own `[voice]`.
	pub narrator: Option<Voice>,
	pub matter: Matter,
}

/// The words a world writes into every book's front and back matter (`[book]` in world.toml).
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Matter {
	/// A note under the dedication on the copyright page, such as a game to play on every page.
	pub note: Option<String>,
	/// The heading over the cast parade at the back of the book, one paragraph per entry.
	pub meet: Vec<String>,
	/// The prompt over the drawing page, one paragraph per entry.
	pub draw: Vec<String>,
}

impl Default for Matter {
	fn default() -> Matter {
		Matter {
			note: None,
			meet: vec!["**Meet the folk**".into()],
			draw: vec!["**Draw your own character!**".into()],
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
	name: String,
	#[serde(default = "default_style")]
	style: String,
	#[serde(default)]
	about: Option<String>,
	#[serde(default)]
	narrator: Option<Voice>,
	#[serde(default)]
	book: Matter,
}

fn default_style() -> String {
	pack::DEFAULT.to_owned()
}

impl World {
	/// Loads the world in `dir` (or the world whose `world.toml` is `dir`).
	pub fn load(dir: &Path) -> Result<World> {
		let dir = if dir.ends_with(FILE) {
			dir.parent().unwrap_or(Path::new(""))
		} else {
			dir
		};
		let path = dir.join(FILE);
		let source = std::fs::read_to_string(&path)
			.with_context(|| format!("could not read {}", path.display()))?;
		let entry: Entry = toml::from_str(&source)
			.with_context(|| format!("{} is not a valid world file", path.display()))?;

		if let Some(voice) = &entry.narrator {
			voice
				.validate()
				.with_context(|| format!("the narrator in {}", path.display()))?;
		}

		StylePack::named(&entry.style)
			.with_context(|| format!("the style in {}", path.display()))?;

		Ok(World {
			dir: dir.to_path_buf(),
			name: entry.name,
			style: entry.style,
			about: entry.about,
			narrator: entry.narrator,
			matter: entry.book,
		})
	}

	/// The world for something at `start` (a script or a folder): the nearest `world.toml` at or
	/// above it, or else the only world in a `worlds/` folder beside it.
	pub fn find(start: &Path) -> Result<World> {
		let start = if start.as_os_str().is_empty() {
			Path::new(".")
		} else {
			start
		};
		let start = start
			.canonicalize()
			.with_context(|| format!("{} does not exist", start.display()))?;

		if let Some(dir) = start.ancestors().find(|dir| dir.join(FILE).is_file()) {
			return Self::load(dir);
		}

		let dir = if start.is_dir() {
			start.as_path()
		} else {
			start.parent().unwrap_or(&start)
		};
		let worlds = dir.join("worlds");
		let mut found: Vec<PathBuf> = std::fs::read_dir(&worlds)
			.into_iter()
			.flatten()
			.filter_map(|entry| Some(entry.ok()?.path()))
			.filter(|path| path.join(FILE).is_file())
			.collect();
		found.sort();

		match found.as_slice() {
			[only] => Self::load(only),
			[] => {
				bail!(
					"no world found: add a {FILE} to the world's folder, or pass --world <folder> \
					 (looked above {})",
					start.display()
				)
			}
			many => {
				let names: Vec<String> =
					many.iter().map(|path| path.display().to_string()).collect();
				bail!(
					"{} holds several worlds ({}); choose one with --world",
					worlds.display(),
					names.join(", ")
				)
			}
		}
	}

	/// The world a script belongs to: `explicit` if given (`--world`), else the script's
	/// `world = "…"` front matter (relative to the script), else the nearest one above it.
	pub fn for_story(story: &Story, explicit: Option<&Path>) -> Result<World> {
		match (explicit, story.meta.world.as_deref()) {
			(Some(dir), _) => Self::load(dir),
			(None, Some(dir)) => Self::load(&story.dir.join(dir)),
			(None, None) => Self::find(&story.dir),
		}
	}

	pub fn cast_file(&self) -> PathBuf {
		self.dir.join("cast.toml")
	}

	/// The style pack the world's puppets are drawn in.
	pub fn style(&self) -> Result<Arc<StylePack>> {
		StylePack::named(&self.style)
	}

	/// The world's cast, drawn in its style.
	pub fn cast(&self) -> Result<Cast> {
		Cast::load(&self.cast_file(), &self.style()?)
	}

	/// Where the world's books live.
	pub fn stories(&self) -> PathBuf {
		self.dir.join("stories")
	}

	/// The template new books start from, if the world has one.
	pub fn recipe(&self) -> Option<PathBuf> {
		Some(self.dir.join("recipe.md")).filter(|path| path.is_file())
	}

	/// Gives a script the world's book matter, and its narrator unless the script names its own.
	pub fn furnish(&self, story: &mut Story) {
		story.matter.clone_from(&self.matter);

		if story.meta.voice.is_none() {
			story.meta.voice.clone_from(&self.narrator);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn world(dir: &Path, name: &str, extra: &str) -> PathBuf {
		let folder = dir.join(name);
		std::fs::create_dir_all(&folder).unwrap();
		std::fs::write(folder.join(FILE), format!("name = \"{name}\"\n{extra}")).unwrap();
		folder
	}

	#[test]
	fn the_nearest_world_above_is_found() {
		let dir = tempfile::tempdir().unwrap();
		let home = world(dir.path(), "home", "[narrator]\nname = \"Daniel\"\n");
		let book = home.join("stories/book");
		std::fs::create_dir_all(&book).unwrap();

		let found = World::find(&book).unwrap();
		assert_eq!(found.name, "home");
		assert_eq!(found.narrator.unwrap().name, "Daniel");
	}

	#[test]
	fn a_single_world_beside_is_found_and_several_must_be_chosen() {
		let dir = tempfile::tempdir().unwrap();
		world(&dir.path().join("worlds"), "one", "");
		assert_eq!(World::find(dir.path()).unwrap().name, "one");

		world(&dir.path().join("worlds"), "two", "");
		let error = format!("{:#}", World::find(dir.path()).unwrap_err());
		assert!(error.contains("several worlds"), "{error}");
	}

	#[test]
	fn a_world_draws_in_the_default_style_unless_it_names_one() {
		let dir = tempfile::tempdir().unwrap();
		let plain = World::load(&world(dir.path(), "plain", "")).unwrap();
		assert_eq!(plain.style, pack::DEFAULT);

		let odd = world(dir.path(), "odd", "style = \"vaporwave\"\n");
		let error = format!("{:#}", World::load(&odd).unwrap_err());
		assert!(error.contains("unknown drawing style"), "{error}");
	}

	#[test]
	fn no_world_is_an_error_that_says_what_to_do() {
		let dir = tempfile::tempdir().unwrap();
		let error = format!("{:#}", World::find(dir.path()).unwrap_err());
		assert!(error.contains("--world"), "{error}");
	}

	#[test]
	fn a_script_keeps_its_own_narrator() {
		let dir = tempfile::tempdir().unwrap();
		let home = World::load(&world(
			dir.path(),
			"home",
			"[narrator]\nname = \"Daniel\"\n",
		))
		.unwrap();
		let mut own = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n[voice]\nname = \"Moira\"\n+++\n# plain\nHi.\n",
		)
		.unwrap();
		let mut plain =
			crate::story::parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# plain\nHi.\n").unwrap();
		home.furnish(&mut own);
		home.furnish(&mut plain);
		assert_eq!(own.meta.voice.unwrap().name, "Moira");
		assert_eq!(plain.meta.voice.unwrap().name, "Daniel");
	}
}
