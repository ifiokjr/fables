//! The cast file: who lives in Kerfuffle and what they look like.
//!
//! Characters are data, not code. A new character for a new book is a new table in
//! `world/cast.toml` built from the puppet's parts (hair styles, patterns, extras), so authors
//! never need to touch Rust to grow the world.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use crate::art::Color;
use crate::art::critters::FlowerKind;
use crate::art::figure::BodyShape;
use crate::art::figure::Build;
use crate::art::figure::Extra;
use crate::art::figure::HairStyle;
use crate::art::figure::HeadShape;
use crate::art::figure::Legwear;
use crate::art::figure::Look;
use crate::art::figure::Nose;
use crate::art::figure::Pattern;
use crate::art::palette;
use crate::art::puppet::Authored;
use crate::art::seed_of;
use crate::video::voice::Voice;

/// How an actor is drawn.
#[derive(Clone, Debug)]
pub enum Kind {
	Figure(Look),
	Bird(Color),
	Dog(Color),
	Worm,
	Flower(FlowerKind, Color),
	Tree(Color),
	Toothbrush(Color),
}

impl Kind {
	pub fn is_figure(&self) -> bool {
		matches!(self, Kind::Figure(_))
	}
}

/// A character from the cast file.
#[derive(Clone, Debug)]
pub struct Member {
	pub id: String,
	pub name: String,
	pub born: Option<String>,
	pub habit: Option<String>,
	pub about: Option<String>,
	/// The character's own voice for dialogue, consistent across every book.
	pub voice: Option<Voice>,
	pub kind: Kind,
}

#[derive(Clone, Debug, Default)]
pub struct Cast {
	pub members: BTreeMap<String, Member>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Entry {
	name: String,
	#[serde(default)]
	born: Option<String>,
	#[serde(default)]
	habit: Option<String>,
	#[serde(default)]
	about: Option<String>,
	#[serde(default = "default_kind")]
	kind: String,
	#[serde(default)]
	skin: Option<String>,
	#[serde(default)]
	hair: Option<String>,
	#[serde(default)]
	hair_colour: Option<String>,
	#[serde(default)]
	head: Option<String>,
	#[serde(default)]
	body: Option<String>,
	#[serde(default)]
	nose: Option<String>,
	#[serde(default)]
	eyes: Option<String>,
	#[serde(default)]
	top: Option<String>,
	#[serde(default)]
	pattern: Option<String>,
	#[serde(default)]
	trim: Option<String>,
	#[serde(default)]
	legwear: Option<String>,
	#[serde(default)]
	bottom: Option<String>,
	#[serde(default)]
	shoes: Option<String>,
	#[serde(default)]
	extras: Vec<String>,
	#[serde(default)]
	mouth: Option<f32>,
	#[serde(default)]
	colour: Option<String>,
	#[serde(default)]
	flower: Option<String>,
	#[serde(default)]
	voice: Option<Voice>,
	/// An authored puppet's part file, relative to the cast file.
	#[serde(default)]
	puppet: Option<PathBuf>,
}

fn default_kind() -> String {
	"kid".to_owned()
}

impl Cast {
	/// Loads a cast file. Puppet part files are found relative to it.
	pub fn load(path: &Path) -> Result<Cast> {
		let source = std::fs::read_to_string(path)
			.with_context(|| format!("could not read cast file {}", path.display()))?;
		Self::parse_in(&source, path.parent().unwrap_or(Path::new("")))
			.with_context(|| format!("in {}", path.display()))
	}

	/// Parses a cast file whose puppet part files are relative to the current directory.
	pub fn parse(source: &str) -> Result<Cast> {
		Self::parse_in(source, Path::new(""))
	}

	/// Parses a cast file whose puppet part files are relative to `dir`.
	pub fn parse_in(source: &str, dir: &Path) -> Result<Cast> {
		let entries: BTreeMap<String, Entry> =
			toml::from_str(source).context("cast file is not valid TOML")?;
		let mut members = BTreeMap::new();

		for (id, entry) in entries {
			let kind =
				build_kind(&id, &entry, dir).with_context(|| format!("cast member `{id}`"))?;

			if let Some(voice) = &entry.voice {
				voice
					.validate()
					.with_context(|| format!("cast member `{id}` voice"))?;
			}

			members.insert(
				id.clone(),
				Member {
					id,
					name: entry.name,
					born: entry.born,
					habit: entry.habit,
					about: entry.about,
					voice: entry.voice,
					kind,
				},
			);
		}

		Ok(Cast { members })
	}

	/// Finds the cast file for a story: an explicit path, or the nearest `world/cast.toml` above it.
	pub fn locate(story_dir: &Path, explicit: Option<&str>) -> Result<PathBuf> {
		if let Some(path) = explicit {
			return Ok(story_dir.join(path));
		}

		let start = if story_dir.as_os_str().is_empty() {
			Path::new(".")
		} else {
			story_dir
		};
		let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

		for dir in start.ancestors() {
			let candidate = dir.join("world").join("cast.toml");

			if candidate.is_file() {
				return Ok(candidate);
			}
		}

		bail!(
			"no cast file found: add `cast = \"path/to/cast.toml\"` to the front matter or create \
			 world/cast.toml above {}",
			start.display()
		)
	}

	/// Resolves an actor id like `stinky`, `bird.a` or `flower.3` to how it is drawn.
	pub fn resolve(&self, actor: &str) -> Result<Kind> {
		let base = actor.split('.').next().unwrap_or(actor);

		if let Some(member) = self.members.get(base) {
			return Ok(member.kind.clone());
		}

		let kind = match base {
			"bird" => Kind::Bird(palette::CORNFLOWER),
			"dog" => Kind::Dog(palette::TOFFEE),
			"worm" => Kind::Worm,
			"flower" => Kind::Flower(FlowerKind::Daisy, palette::BUBBLEGUM),
			"tree" => Kind::Tree(palette::LEAF),
			"toothbrush" => Kind::Toothbrush(palette::TEAL),
			_ => {
				let known: Vec<&str> = self.members.keys().map(String::as_str).collect();
				bail!(
					"unknown actor `{actor}`; cast members are {}, and generic actors are bird, \
					 dog, worm, flower, tree, toothbrush",
					known.join(", ")
				);
			}
		};

		Ok(kind)
	}

	pub fn member(&self, actor: &str) -> Option<&Member> {
		self.members.get(actor.split('.').next().unwrap_or(actor))
	}
}

fn color(field: &str, value: Option<&String>, fallback: Color) -> Result<Color> {
	match value {
		Some(name) => {
			Color::parse(name).with_context(|| {
				let names: Vec<&str> = palette::NAMED.iter().map(|(name, _)| *name).collect();
				format!(
					"`{field}` colour `{name}` is not a palette name ({}) or #rrggbb",
					names.join(", ")
				)
			})
		}
		None => Ok(fallback),
	}
}

fn build_kind(id: &str, entry: &Entry, dir: &Path) -> Result<Kind> {
	let tint = |fallback| color("colour", entry.colour.as_ref(), fallback);

	match entry.kind.as_str() {
		"kid" | "grown-up" => Ok(Kind::Figure(build_look(id, entry, dir)?)),
		"bird" => Ok(Kind::Bird(tint(palette::CORNFLOWER)?)),
		"dog" => Ok(Kind::Dog(tint(palette::TOFFEE)?)),
		"worm" => Ok(Kind::Worm),
		"tree" => Ok(Kind::Tree(tint(palette::LEAF)?)),
		"toothbrush" => Ok(Kind::Toothbrush(tint(palette::TEAL)?)),
		"flower" => {
			let flower = match entry.flower.as_deref() {
				None | Some("daisy") => FlowerKind::Daisy,
				Some("tulip") => FlowerKind::Tulip,
				Some("sunflower") => FlowerKind::Sunflower,
				Some(other) => bail!("unknown flower `{other}`; use daisy, tulip or sunflower"),
			};

			Ok(Kind::Flower(flower, tint(palette::BUBBLEGUM)?))
		}
		other => {
			bail!(
				"unknown kind `{other}`; use kid, grown-up, bird, dog, worm, flower, tree or \
				 toothbrush"
			)
		}
	}
}

fn build_look(id: &str, entry: &Entry, dir: &Path) -> Result<Look> {
	let build = if entry.kind == "grown-up" {
		Build::Grownup
	} else {
		Build::Kid
	};
	let head = match entry.head.as_deref().unwrap_or("round") {
		"round" => HeadShape::Round,
		"egg" => HeadShape::Egg,
		"wide" => HeadShape::Wide,
		other => bail!("unknown head `{other}`; use round, egg or wide"),
	};
	let body = match entry.body.as_deref().unwrap_or("bell") {
		"bell" => BodyShape::Bell,
		"bean" => BodyShape::Bean,
		"pear" => BodyShape::Pear,
		"round" => BodyShape::Round,
		other => bail!("unknown body `{other}`; use bell, bean, pear or round"),
	};
	let nose = match entry.nose.as_deref().unwrap_or("button") {
		"button" => Nose::Button,
		"round" => Nose::Round,
		"long" => Nose::Long,
		"none" => Nose::None,
		other => bail!("unknown nose `{other}`; use button, round, long or none"),
	};
	let hair = match entry.hair.as_deref().unwrap_or("tuft") {
		"bald" => HairStyle::Bald,
		"tuft" => HairStyle::Tuft,
		"bun" => HairStyle::Bun,
		"curls" => HairStyle::Curls,
		"pigtails" => HairStyle::Pigtails,
		"bob" => HairStyle::Bob,
		"spikes" => HairStyle::Spikes,
		"puff" => HairStyle::Puff,
		"fringe" => HairStyle::Fringe,
		other => {
			bail!(
				"unknown hair `{other}`; use bald, tuft, bun, curls, pigtails, bob, spikes, puff \
				 or fringe"
			)
		}
	};
	let pattern = match entry.pattern.as_deref().unwrap_or("plain") {
		"plain" => Pattern::Plain,
		"stripes" => Pattern::Stripes,
		"dots" => Pattern::Dots,
		"star" => Pattern::Star,
		other => bail!("unknown pattern `{other}`; use plain, stripes, dots or star"),
	};
	let legwear = match entry.legwear.as_deref().unwrap_or("trousers") {
		"shorts" => Legwear::Shorts,
		"trousers" => Legwear::Trousers,
		"skirt" => Legwear::Skirt,
		other => bail!("unknown legwear `{other}`; use shorts, trousers or skirt"),
	};
	let extras = entry
		.extras
		.iter()
		.map(|extra| {
			Ok(match extra.as_str() {
				"freckles" => Extra::Freckles,
				"glasses" => Extra::Glasses,
				"moustache" => Extra::Moustache,
				"apron" => Extra::Apron,
				"post-cap" => Extra::PostCap,
				"satchel" => Extra::Satchel,
				"bow" => Extra::Bow,
				"muddy-hands" => Extra::MuddyHands,
				"bogey" => Extra::Bogey,
				"lashes" => Extra::Lashes,
				other => bail!("unknown extra `{other}`"),
			})
		})
		.collect::<Result<Vec<_>>>()?;

	let puppet = match &entry.puppet {
		Some(file) => Some(Authored::load(&dir.join(file))?),
		None => None,
	};

	Ok(Look {
		build,
		head,
		body,
		nose,
		eyes: color("eyes", entry.eyes.as_ref(), palette::COCOA)?,
		skin: color("skin", entry.skin.as_ref(), palette::TANGERINE)?,
		hair,
		hair_color: color("hair-colour", entry.hair_colour.as_ref(), palette::PLUM)?,
		top: color("top", entry.top.as_ref(), palette::TEAL)?,
		pattern,
		trim: color("trim", entry.trim.as_ref(), palette::CREAM)?,
		legwear,
		bottom: color("bottom", entry.bottom.as_ref(), palette::NIGHT)?,
		shoes: color("shoes", entry.shoes.as_ref(), palette::INK)?,
		extras,
		mouth_size: entry.mouth.unwrap_or(1.0),
		seed: seed_of(id),
		puppet,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	const CAST: &str = r##"
[stinky]
name = "Stinky McStinkbreath"
born = "Stanley"
skin = "tangerine"
hair = "tuft"
pattern = "stripes"
legwear = "shorts"
extras = ["freckles"]
mouth = 1.4

[biscuit]
name = "Biscuit"
kind = "dog"
colour = "#c9865a"
"##;

	#[test]
	fn resolves_members_instances_and_generics() {
		let cast = Cast::parse(CAST).unwrap();
		assert!(cast.resolve("stinky").unwrap().is_figure());
		assert!(cast.resolve("stinky.monday").unwrap().is_figure());
		assert!(matches!(cast.resolve("biscuit").unwrap(), Kind::Dog(c) if c == palette::TOFFEE));
		assert!(matches!(cast.resolve("bird.a").unwrap(), Kind::Bird(_)));
		assert_eq!(
			cast.member("stinky.monday").unwrap().born.as_deref(),
			Some("Stanley")
		);
	}

	#[test]
	fn unknown_actor_lists_alternatives() {
		let cast = Cast::parse(CAST).unwrap();
		let message = format!("{:#}", cast.resolve("granny").unwrap_err());
		assert!(message.contains("biscuit, stinky"), "{message}");
	}

	#[test]
	fn bad_colour_names_are_explained() {
		let message = format!(
			"{:#}",
			Cast::parse("[x]\nname = \"X\"\nskin = \"chartreuse\"\n").unwrap_err()
		);
		assert!(message.contains("chartreuse"), "{message}");
		assert!(message.contains("tangerine"), "{message}");
	}

	#[test]
	fn authored_grown_ups_stand_taller_than_kids() {
		let cast = Cast::parse(
			"[mum]\nname = \"Mum\"\nkind = \"grown-up\"\npuppet = \
			 \"world/puppets/stinky.svg\"\n[kid]\nname = \"Kid\"\npuppet = \
			 \"world/puppets/stinky.svg\"\n",
		)
		.unwrap();
		let height = |id: &str| {
			let Ok(Kind::Figure(look)) = cast.resolve(id) else {
				panic!("{id} is a person");
			};
			look.frame().height()
		};
		assert!(height("mum") > height("kid") * 1.4);
	}

	#[test]
	fn puppet_files_are_found_beside_the_cast_file() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("cast.toml");
		std::fs::create_dir(dir.path().join("puppets")).unwrap();
		let world = Path::new(env!("CARGO_MANIFEST_DIR")).join("world/puppets");

		for file in ["kit.svg", "stinky.svg"] {
			std::fs::copy(world.join(file), dir.path().join("puppets").join(file)).unwrap();
		}

		std::fs::write(
			&path,
			"[kid]\nname = \"Kid\"\npuppet = \"puppets/stinky.svg\"\n",
		)
		.unwrap();
		let cast = Cast::load(&path).unwrap();
		let Kind::Figure(look) = cast.resolve("kid").unwrap() else {
			panic!("kid is a person");
		};
		let puppet = look.puppet.expect("the kid has a puppet");
		assert_eq!(puppet.file, dir.path().join("puppets/stinky.svg"));
		assert!(puppet.parts.get("head.front").is_some());
		assert!(
			puppet.parts.get("hand-fist").is_some(),
			"kit parts come along"
		);
	}
}
