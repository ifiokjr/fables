//! The cast file: who lives in a world and what they look like.
//!
//! Characters are data, not code. A new character for a new book is a new table in
//! a world's `cast.toml`, built from the puppet's parts (hair styles, patterns, extras), so
//! authors never need to touch Rust to grow the world.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

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
use crate::art::pack::StylePack;
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

	/// The `kind` a cast file gives this, e.g. `kid`, `grown-up` or `dog`.
	pub fn name(&self) -> &'static str {
		match self {
			Kind::Figure(look) => {
				match look.build {
					Build::Kid => "kid",
					Build::Grownup => "grown-up",
				}
			}
			Kind::Bird(_) => "bird",
			Kind::Dog(_) => "dog",
			Kind::Worm => "worm",
			Kind::Flower(..) => "flower",
			Kind::Tree(_) => "tree",
			Kind::Toothbrush(_) => "toothbrush",
		}
	}
}

/// A plain character drawn by a style's template, for showing the style itself.
pub fn template_look(style: &Arc<StylePack>, build: Build) -> Result<Look> {
	let kind = match build {
		Build::Kid => "kid",
		Build::Grownup => "grown-up",
	};
	let entry: Entry = toml::from_str(&format!(
		"name = \"Template\"\nkind = \"{kind}\"\nskin = \"#f6d7c3\"\nhair-colour = \
		 \"#3d3a5c\"\ntop = \"#eef1f8\"\ntrim = \"#d6455d\"\nbottom = \"#3a4a7a\"\nshoes = \
		 \"#2c2c3a\"\neyes = \"#4a7ad0\"\n"
	))?;
	let mut look = build_look("template", &entry, Path::new(""), style)?;
	let file = PathBuf::from(format!("styles/{}/template.svg", style.name));
	look.puppet = Some(Authored::from_parts(&file, style.template_figure(), style)?);
	Ok(look)
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
	/// The cast fields only the parametric puppet reads (`hair`, `head`, `body`, `nose`,
	/// `mouth`), named. They are ignored when the character is drawn from parts, and `check`
	/// says so once per member.
	pub parametric_fields: Vec<&'static str>,
}

/// A visitor from another world, invited by a script's front matter (`guests = ["world/id"]`).
/// They keep their own puppet, their own world's construction and their voice; see
/// [`World::cast_for`](crate::world::World::cast_for).
#[derive(Clone, Debug)]
pub struct Guest {
	/// The guest world's folder under the same root as the host.
	pub world: String,
	/// Their cast entry in that world, built with that world's style.
	pub member: Member,
}

impl Guest {
	/// How directions name them when their id clashes with a local cast member: `world.id`.
	pub fn qualified(&self) -> String {
		format!("{}.{}", self.world, self.member.id)
	}
}

#[derive(Clone, Debug, Default)]
pub struct Cast {
	pub members: BTreeMap<String, Member>,
	/// Visitors from other worlds, invited by the script's front matter. The
	/// "meet the folk" page and the world's sheets list only `members`.
	pub guests: Vec<Guest>,
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
	/// The character's drawing: an authored puppet's part file, relative to the cast file, or
	/// `"parametric"` for the older puppet. Without a line they are drawn from their drawing
	/// style's template, in their own colours.
	#[serde(default)]
	puppet: Option<String>,
}

/// The `puppet` value that asks for the older parametric puppet.
pub const PARAMETRIC: &str = "parametric";

fn default_kind() -> String {
	"kid".to_owned()
}

impl Entry {
	/// The cast fields only the parametric puppet reads, which an authored (or template-drawn)
	/// character ignores.
	fn parametric_fields(&self) -> Vec<&'static str> {
		let mut fields = Vec::new();

		for (set, name) in [
			(self.hair.is_some(), "hair"),
			(self.head.is_some(), "head"),
			(self.body.is_some(), "body"),
			(self.nose.is_some(), "nose"),
			(self.mouth.is_some(), "mouth"),
		] {
			if set {
				fields.push(name);
			}
		}

		fields
	}
}

impl Cast {
	/// Loads a cast file whose puppets are drawn in `style`. Puppet part files are found
	/// relative to it.
	pub fn load(path: &Path, style: &Arc<StylePack>) -> Result<Cast> {
		let source = std::fs::read_to_string(path)
			.with_context(|| format!("could not read cast file {}", path.display()))?;
		Self::parse_in(&source, path.parent().unwrap_or(Path::new("")), style)
			.with_context(|| format!("in {}", path.display()))
	}

	/// Parses a cast file in the default style whose puppet part files are relative to the
	/// current directory.
	pub fn parse(source: &str) -> Result<Cast> {
		Self::parse_in(source, Path::new(""), &StylePack::default_pack())
	}

	/// Parses a cast file whose puppets are drawn in `style` from part files relative to `dir`.
	pub fn parse_in(source: &str, dir: &Path, style: &Arc<StylePack>) -> Result<Cast> {
		let entries: BTreeMap<String, Entry> =
			toml::from_str(source).context("cast file is not valid TOML")?;
		let mut members = BTreeMap::new();

		for (id, entry) in entries {
			let kind = build_kind(&id, &entry, dir, style)
				.with_context(|| format!("cast member `{id}`"))?;
			let parametric_fields = entry.parametric_fields();

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
					parametric_fields,
				},
			);
		}

		Ok(Cast {
			members,
			guests: Vec::new(),
		})
	}

	/// Invites a guest from another world. Their id stays theirs (`@hana`); when a local cast
	/// member already has it, directions must name them `world.id` instead.
	pub fn add_guest(&mut self, world: &str, member: Member) {
		self.guests.push(Guest {
			world: world.to_owned(),
			member,
		});
	}

	/// Resolves an actor id like `stinky`, `bird.a` or `flower.3` to how it is drawn. A guest
	/// is resolved by its own id, or by `world.id` when its id clashes with a local cast
	/// member (the bare id always means the local one).
	pub fn resolve(&self, actor: &str) -> Result<Kind> {
		if let Some(member) = self.member(actor) {
			return Ok(member.kind.clone());
		}

		let base = actor.split('.').next().unwrap_or(actor);

		let kind = match base {
			"bird" => Kind::Bird(palette::CORNFLOWER),
			"dog" => Kind::Dog(palette::TOFFEE),
			"worm" => Kind::Worm,
			"flower" => Kind::Flower(FlowerKind::Daisy, palette::BUBBLEGUM),
			"tree" => Kind::Tree(palette::LEAF),
			"toothbrush" => Kind::Toothbrush(palette::TEAL),
			_ => {
				let known: Vec<&str> = self.members.keys().map(String::as_str).collect();
				let invited: Vec<String> = self
					.guests
					.iter()
					.map(|guest| format!("{}/{}", guest.world, guest.member.id))
					.collect();
				let guests = if invited.is_empty() {
					String::new()
				} else {
					format!("; guests are {}", invited.join(", "))
				};

				bail!(
					"unknown actor `{actor}`; cast members are {}, and generic actors are bird, \
					 dog, worm, flower, tree, toothbrush{guests}",
					known.join(", ")
				);
			}
		};

		Ok(kind)
	}

	/// The cast member an actor id names: one of this world's, or a guest. Copies
	/// (`@stinky.monday`) name their base member; a guest is named `world.id` when its id
	/// clashes with a local one.
	pub fn member(&self, actor: &str) -> Option<&Member> {
		if let Some((world, rest)) = actor.split_once('.') {
			let base = rest.split('.').next().unwrap_or(rest);

			if let Some(guest) = self
				.guests
				.iter()
				.find(|guest| guest.world == world && guest.member.id == base)
			{
				return Some(&guest.member);
			}
		}

		let base = actor.split('.').next().unwrap_or(actor);

		if let Some(member) = self.members.get(base) {
			return Some(member);
		}

		self.guests
			.iter()
			.find(|guest| guest.member.id == base)
			.map(|guest| &guest.member)
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

fn build_kind(id: &str, entry: &Entry, dir: &Path, style: &Arc<StylePack>) -> Result<Kind> {
	let tint = |fallback| color("colour", entry.colour.as_ref(), fallback);

	match entry.kind.as_str() {
		"kid" | "grown-up" => Ok(Kind::Figure(build_look(id, entry, dir, style)?)),
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

fn build_look(id: &str, entry: &Entry, dir: &Path, style: &Arc<StylePack>) -> Result<Look> {
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

	let puppet = match entry.puppet.as_deref() {
		Some(PARAMETRIC) => None,
		Some(file) => Some(Authored::load(&dir.join(file), style)?),
		None => Some(Authored::from_template(style)?),
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
	fn guests_resolve_by_id_and_by_world_on_a_clash() {
		let mut cast = Cast::parse(CAST).unwrap();
		let visitor = |id: &str| {
			Member {
				id: id.into(),
				name: "Visitor".into(),
				born: None,
				habit: None,
				about: None,
				voice: None,
				kind: Kind::Figure(
					build_look(
						"visitor",
						&toml::from_str("name = \"V\"\nskin = \"teal\"\n").unwrap(),
						Path::new(""),
						&StylePack::default_pack(),
					)
					.unwrap(),
				),
				parametric_fields: Vec::new(),
			}
		};

		// No clash: the bare id finds the guest.
		cast.add_guest("kiteharbour", visitor("hana"));
		assert!(cast.member("hana").is_some_and(|m| m.name == "Visitor"));
		assert!(cast.resolve("hana").unwrap().is_figure());
		assert!(cast.member("hana.copy").is_some_and(|m| m.id == "hana"));

		// Qualified names always find the guest.
		assert!(
			cast.member("kiteharbour.hana")
				.is_some_and(|m| m.id == "hana"),
			"world.id names the guest"
		);

		// A clash: the bare id means the local one, world.id the guest.
		cast.add_guest("kerfuffle", visitor("stinky"));
		assert!(
			cast.member("stinky")
				.is_some_and(|m| m.name == "Stinky McStinkbreath")
		);
		assert!(
			cast.member("kerfuffle.stinky")
				.is_some_and(|m| m.name == "Visitor"),
			"world.id still finds the guest"
		);

		let message = format!("{:#}", cast.resolve("granny").unwrap_err());
		assert!(
			message.contains("guests are kiteharbour/hana, kerfuffle/stinky"),
			"{message}"
		);
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
			 \"worlds/kerfuffle/puppets/stinky.svg\"\n[kid]\nname = \"Kid\"\npuppet = \
			 \"worlds/kerfuffle/puppets/stinky.svg\"\n",
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
		let world = Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/kerfuffle/puppets");

		std::fs::copy(
			world.join("stinky.svg"),
			dir.path().join("puppets/stinky.svg"),
		)
		.unwrap();

		std::fs::write(
			&path,
			"[kid]\nname = \"Kid\"\npuppet = \"puppets/stinky.svg\"\n",
		)
		.unwrap();
		let cast = Cast::load(&path, &StylePack::default_pack()).unwrap();
		let Kind::Figure(look) = cast.resolve("kid").unwrap() else {
			panic!("kid is a person");
		};
		let puppet = look.puppet.expect("the kid has a puppet");
		assert_eq!(puppet.file, dir.path().join("puppets/stinky.svg"));
		assert!(!puppet.template, "a part file is not the template");
		assert!(puppet.parts.get("head.front").is_some());
		assert!(
			puppet.parts.get("hand-fist").is_some(),
			"kit parts come along"
		);
	}

	#[test]
	fn a_character_without_a_puppet_is_drawn_from_the_style_template() {
		let style = StylePack::named("chibi").unwrap();
		let cast = Cast::parse_in(
			"[pip]\nname = \"Pip\"\nskin = \"teal\"\nhair = \"bun\"\n",
			Path::new(""),
			&style,
		)
		.unwrap();
		let Kind::Figure(look) = cast.resolve("pip").unwrap() else {
			panic!("pip is a person");
		};
		let puppet = look
			.puppet
			.expect("a template-drawn character is drawn from parts");
		assert!(puppet.template, "no file of their own yet");
		assert_eq!(puppet.style.name, "chibi");
		assert_eq!(
			puppet.parts.get("head.front"),
			style.template.get("head.front")
		);
		assert!(
			puppet.parts.get("hand-fist").is_some(),
			"the style's kit comes along"
		);
		assert_eq!(
			cast.member("pip").unwrap().parametric_fields,
			vec!["hair"],
			"the fields only the parametric puppet reads are named"
		);

		let cartoon = Cast::parse("[stinky]\nname = \"Stinky\"\n").unwrap();
		let Kind::Figure(look) = cartoon.resolve("stinky").unwrap() else {
			panic!("stinky is a person");
		};
		assert!(
			look.puppet.is_some_and(|puppet| puppet.template),
			"the default cast is drawn from the cartoon template"
		);
	}

	#[test]
	fn the_parametric_puppet_is_asked_for_by_name() {
		let cast = Cast::parse("[old]\nname = \"Old\"\npuppet = \"parametric\"\n").unwrap();
		let Kind::Figure(look) = cast.resolve("old").unwrap() else {
			panic!("old is a person");
		};
		assert!(
			look.puppet.is_none(),
			"`puppet = \"parametric\"` still draws the old puppet"
		);
	}
}
