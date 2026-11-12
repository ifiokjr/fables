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

use crate::art::light::Light;
use crate::art::light::Preset;
use crate::art::pack;
use crate::art::pack::StylePack;
use crate::art::settings::Library;
use crate::cast::Cast;
use crate::cast::Kind;
use crate::story::Drawing;
use crate::story::GuestStyle;
use crate::story::Story;
use crate::video::voice::Voice;

/// The file that marks a folder as a world.
pub const FILE: &str = "world.toml";

/// A world's folder name (the last component of its path).
fn folder_of(dir: &Path) -> &str {
	dir.file_name().and_then(|name| name.to_str()).unwrap_or("")
}

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
	/// The light the world's books start in, unless a book names its own (`world.toml`:
	/// `light = "golden"`).
	pub light: Option<Light>,
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
	light: Option<String>,
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

		let light = entry
			.light
			.as_deref()
			.map(|name| {
				Light::parse(name).ok_or_else(|| {
					anyhow::anyhow!(
						"unknown light `{name}` in {}; try {}",
						path.display(),
						Preset::names()
					)
				})
			})
			.transpose()?;

		Ok(World {
			dir: dir.to_path_buf(),
			name: entry.name,
			style: entry.style,
			about: entry.about,
			light,
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

	/// The settings the world's books can use: the built-in ones, and its own `settings/`.
	pub fn settings(&self) -> Result<Arc<Library>> {
		Library::for_world(&self.dir)
	}

	/// The world's cast, drawn in its style.
	pub fn cast(&self) -> Result<Cast> {
		Cast::load(&self.cast_file(), &self.style()?)
	}

	/// The other worlds under the same root as this one (the ones a script's guests can come
	/// from).
	pub fn neighbours(&self) -> Result<Vec<World>> {
		let Some(root) = self.dir.parent() else {
			return Ok(Vec::new());
		};

		let mut found: Vec<PathBuf> = std::fs::read_dir(root)
			.into_iter()
			.flatten()
			.filter_map(|entry| Some(entry.ok()?.path()))
			.filter(|path| path.join(FILE).is_file() && path != &self.dir)
			.collect();
		found.sort();

		found.iter().map(|path| World::load(path)).collect()
	}

	/// The cast a story rehearses with: this world's folk plus the guests its front matter
	/// invites from the worlds beside it (`guests = ["kiteharbour/hana"]`). A guest is loaded
	/// with their own world's style, so their construction, line and shading and their voice
	/// travel with them; `guest-style = "host"` inks them with this world's `[line]` and
	/// `[shade]` instead.
	pub fn cast_for(&self, story: &Story) -> Result<Cast> {
		let mut cast = self.cast()?;

		if story.meta.guests.is_empty() {
			return Ok(cast);
		}

		let neighbours = self.neighbours()?;
		let folders: Vec<&str> = neighbours
			.iter()
			.map(|world| folder_of(&world.dir))
			.collect();
		let host_style = self.style()?;

		for invited in &story.meta.guests {
			let Some((folder, id)) = invited.split_once('/') else {
				bail!("guest `{invited}` should be `world/character`, e.g. `kiteharbour/hana`");
			};

			let Some(index) = folders.iter().position(|name| *name == folder) else {
				let beside = if folders.is_empty() {
					format!(
						"there are no other worlds under {}",
						self.dir.parent().unwrap_or(Path::new("")).display()
					)
				} else {
					format!("the worlds beside {} are {}", self.name, folders.join(", "))
				};

				bail!("there is no world `{folder}` to invite a guest from; {beside}");
			};

			let guest_world = &neighbours[index];
			let guest_cast = guest_world
				.cast()
				.with_context(|| format!("inviting a guest from {}", guest_world.name))?;

			let Some(member) = guest_cast.member(id) else {
				let known: Vec<&str> = guest_cast.members.keys().map(String::as_str).collect();

				bail!(
					"{} has no `{id}` to invite; its cast is: {}",
					guest_world.name,
					known.join(", ")
				);
			};

			let mut member = member.clone();

			// The host's finish inks the guest's own construction, when the script asks for it.
			if story.meta.guest_style == GuestStyle::Host
				&& let Kind::Figure(look) = &mut member.kind
				&& let Some(puppet) = &mut look.puppet
			{
				puppet.ink_style = Some(Arc::clone(&host_style));
			}

			cast.add_guest(folder, member);
		}

		Ok(cast)
	}

	/// Where the world's books live.
	pub fn stories(&self) -> PathBuf {
		self.dir.join("stories")
	}

	/// The template new books start from, if the world has one.
	pub fn recipe(&self) -> Option<PathBuf> {
		Some(self.dir.join("recipe.md")).filter(|path| path.is_file())
	}

	/// Gives a script the world's book matter and the way its style prints and moves, and its
	/// narrator unless the script names its own.
	pub fn furnish(&self, story: &mut Story) -> Result<()> {
		let style = self.style()?;
		story.matter.clone_from(&self.matter);
		story.settings = self.settings()?;
		story.drawing = Drawing {
			fill: style.shade.fill,
			tone: style.shade.tone,
			palette: style.shade.palette,
			motion: style.motion,
		};

		// The world's default light applies unless the book named its own.
		if let Some(light) = &self.light
			&& story.light.is_day()
		{
			story.light = *light;
		}

		if story.meta.voice.is_none() {
			story.meta.voice.clone_from(&self.narrator);
		}

		Ok(())
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
		home.furnish(&mut own).unwrap();
		home.furnish(&mut plain).unwrap();
		assert_eq!(own.meta.voice.unwrap().name, "Moira");
		assert_eq!(plain.meta.voice.unwrap().name, "Daniel");
	}

	#[test]
	fn a_world_lights_its_books_unless_the_book_names_a_light() {
		let dir = tempfile::tempdir().unwrap();
		let golden = World::load(&world(dir.path(), "golden", "light = \"golden\"\n")).unwrap();
		let story =
			&mut crate::story::parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# plain\nHi.\n")
				.unwrap();
		golden.furnish(story).unwrap();
		assert_eq!(story.light, Light::of(Preset::Golden));

		// The book's own front matter wins over the world's default.
		let dawn = &mut crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nlight = \"dawn\"\n+++\n# plain\nHi.\n",
		)
		.unwrap();
		golden.furnish(dawn).unwrap();
		assert_eq!(dawn.light, Light::of(Preset::Dawn));

		let error = format!(
			"{:#}",
			World::load(&world(dir.path(), "odd", "light = \"sunset\"\n")).unwrap_err()
		);
		assert!(
			error.contains("sunset") && error.contains("candle"),
			"{error}"
		);
	}

	/// Two worlds beside each other: `home` (cartoon) with a local `mo` and `tia`, and `away`
	/// (anime) with a local `mo` of its own and a `hana`.
	fn two_worlds(dir: &Path) -> World {
		let worlds = dir.join("worlds");
		let home = world(&worlds, "home", "");
		std::fs::write(
			home.join("cast.toml"),
			"[mo]\nname = \"Local Mo\"\n[tia]\nname = \"Local Tia\"\n",
		)
		.unwrap();

		let away = world(&worlds, "away", "style = \"anime\"\n");
		std::fs::write(
			away.join("cast.toml"),
			"[mo]\nname = \"Away Mo\"\n[hana]\nname = \"Away Hana\"\n",
		)
		.unwrap();

		World::load(&home).unwrap()
	}

	fn story_with_guests(front: &str) -> Story {
		crate::story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n{front}+++\n# plain\nHi.\n"
		))
		.unwrap()
	}

	#[test]
	fn a_guest_is_found_keeps_their_id_and_shares_the_stage_with_a_clashing_local() {
		let dir = tempfile::tempdir().unwrap();
		let home = two_worlds(dir.path());

		let cast = home
			.cast_for(&story_with_guests("guests = [\"away/hana\"]\n"))
			.unwrap();
		assert_eq!(cast.guests.len(), 1);
		assert!(
			cast.member("hana").is_some_and(|m| m.name == "Away Hana"),
			"no clash: the bare id finds the guest"
		);
		assert_eq!(
			cast.member("mo").map(|m| m.name.as_str()),
			Some("Local Mo"),
			"the host's cast is untouched"
		);

		let clash = home
			.cast_for(&story_with_guests("guests = [\"away/mo\"]\n"))
			.unwrap();
		assert!(
			clash.member("mo").is_some_and(|m| m.name == "Local Mo"),
			"on a clash the bare id means the local one"
		);
		assert!(
			clash.member("away.mo").is_some_and(|m| m.name == "Away Mo"),
			"world.id finds the guest"
		);
	}

	#[test]
	fn a_missing_world_or_character_is_an_error_naming_what_exists() {
		let dir = tempfile::tempdir().unwrap();
		let home = two_worlds(dir.path());

		let error = format!(
			"{:#}",
			home.cast_for(&story_with_guests("guests = [\"nowhere/pip\"]\n"))
				.unwrap_err()
		);
		assert!(
			error.contains("no world `nowhere`") && error.contains("away"),
			"{error}"
		);

		let error = format!(
			"{:#}",
			home.cast_for(&story_with_guests("guests = [\"away/pip\"]\n"))
				.unwrap_err()
		);
		assert!(
			error.contains("has no `pip`") && error.contains("hana, mo"),
			"{error}"
		);

		let error = format!(
			"{:#}",
			home.cast_for(&story_with_guests("guests = [\"away.hana\"]\n"))
				.unwrap_err()
		);
		assert!(
			error.contains("world/character") && error.contains("away.hana"),
			"{error}"
		);
	}

	#[test]
	fn a_guest_carries_their_own_worlds_style_into_a_host_book() {
		let root = Path::new(env!("CARGO_MANIFEST_DIR"));
		let host = World::load(&root.join("worlds/mumbleton")).unwrap();
		let cast = host
			.cast_for(&story_with_guests("guests = [\"kiteharbour/hana\"]\n"))
			.unwrap();

		let Kind::Figure(look) = cast.resolve("hana").unwrap() else {
			panic!("hana is a person");
		};
		let puppet = look.puppet.as_ref().expect("hana has a puppet");
		assert_eq!(puppet.style.name, "anime", "her construction is her own");
		assert!(
			puppet.ink_style.is_none(),
			"by default she is inked with her own style too"
		);
		assert_eq!(puppet.inker().name, "anime");

		let Kind::Figure(look) = cast.resolve("mo").unwrap() else {
			panic!("mo is a person");
		};
		assert_eq!(
			look.puppet.as_ref().expect("mo has a puppet").style.name,
			"cartoon"
		);
	}

	#[test]
	fn guest_style_host_inks_the_guest_with_the_host_worlds_dials() {
		let root = Path::new(env!("CARGO_MANIFEST_DIR"));
		let host = World::load(&root.join("worlds/mumbleton")).unwrap();
		let cast = host
			.cast_for(&story_with_guests(
				"guests = [\"kiteharbour/hana\"]\nguest-style = \"host\"\n",
			))
			.unwrap();

		let Kind::Figure(look) = cast.resolve("hana").unwrap() else {
			panic!("hana is a person");
		};
		let puppet = look.puppet.as_ref().expect("hana has a puppet");
		assert_eq!(
			puppet.style.name, "anime",
			"her construction is still her own"
		);
		assert_eq!(
			puppet.inker().name,
			"cartoon",
			"but she is inked with the host's line and shade"
		);
	}
}
