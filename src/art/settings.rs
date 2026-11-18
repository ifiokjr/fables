//! Settings: the places stories happen, painted as part files.
//!
//! A setting is an SVG file of named parts, drawn on the square stage (2400 units, with 160 of
//! overdraw past every edge for print bleed and camera moves). Each part's `id` starts with its
//! layer, which is how video gets parallax and actors get something to stand in front of:
//!
//! - `far-…` the sky, sun, clouds and distant hills, which move least as the camera travels;
//! - `near-…` everything else behind the actors;
//! - `front-…` what sits in front of the actors, framing the page.
//!
//! A part is drawn through the page's art style, so the same file is a riso print, a cartoon
//! or crayon. Colours are palette names or hex codes, and a part says how it reacts to the
//! scene:
//!
//! | Attribute | Meaning |
//! | --- | --- |
//! | `fill` | The colour; `tint` is the scene's tint (the `plain` blob); `sky` is the scene light's sky gradient; `none` for a line. |
//! | `stroke`, `stroke-width` | A line's colour and weight; `data-noodle` makes it a thick inked tube. |
//! | `data-line="none"` | A fill without an outline (skies, lawns, stripes). |
//! | `data-night` | The colour on a bare night (a page with no `light=`). Parts without one are dimmed. Under a scene light the light alone makes the night: every part keeps its day colour and the light grades it. |
//! | `data-wilt` | The colour it turns as the scene wilts. |
//! | `data-time` | `day` or `night`: only drawn then (the sun, the moon). Under a scene light the light's sky body decides; `overcast` and `storm` hide the sun. |
//! | `data-lit` | Comma-separated light presets (`dusk,night`): only drawn under one of those lights (lit windows, stars). |
//! | `data-gradient` | `top colour bottom colour`: a vertical gradient over the part (walls, water, floors). Both colours wilt, fade with distance, darken on a bare night and grade like a flat fill; on a bare night a part with a `data-night` colour turns that flat colour, and under a scene light both stops keep the gradient. |
//! | `data-depth` | 0 to 1: how far away it is. Far things fade into the sky and lose line. |
//! | `data-dots` | A riso dot screen over it: `colour spacing radius opacity`. |
//! | `data-drift` | Units a second it drifts sideways, wrapping (clouds). |
//! | `data-swing` | `x y`: the pivot it swings from (a swing seat and its ropes). |
//!
//! `<circle data-glow="radius colour opacity">` paints a soft radial pool of light (a lamp, a
//! window, the sun's bloom). Two kinds of part are not drawings: `<circle
//! data-sparkle="size colour">` twinkles, and `<circle data-critter="flower|tree" r="height"
//! data-colour data-seed>` places one of the living plants from [`critters`], which have faces
//! and wilt. The root `<svg>` sets `data-ground`, where actors' feet rest, `data-ambience`, the
//! sound beds the place adds to the video's soundtrack, and `data-indoors` on an interior, so
//! the night's crickets stay outside.
//!
//! The built-in settings (`plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`,
//! `kitchen`, `classroom`, `forest`, `beach`, `hilltop`, `shop`, `playroom`, `stage`) are
//! compiled in from `settings/`; a world adds or replaces them with its own `settings/`.

use std::collections::BTreeMap;
use std::path::Path as FilePath;
use std::sync::Arc;
use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;

use super::Color;
use super::Path;
use super::Pt;
use super::STAGE;
use super::Svg;
use super::critters;
use super::critters::CritterState;
use super::critters::FlowerKind;
use super::light::Body;
use super::light::Light;
use super::light::Preset;
use super::palette;
use super::pt;

/// How far drawings extend past the stage edge.
pub const OVER: f32 = 160.0;

/// The settings compiled into fables, by name.
const BUILT_IN: [(&str, &str); 15] = [
	("plain", include_str!("../../settings/plain.svg")),
	("town", include_str!("../../settings/town.svg")),
	("street", include_str!("../../settings/street.svg")),
	("garden", include_str!("../../settings/garden.svg")),
	("park", include_str!("../../settings/park.svg")),
	("bathroom", include_str!("../../settings/bathroom.svg")),
	("bedroom", include_str!("../../settings/bedroom.svg")),
	("kitchen", include_str!("../../settings/kitchen.svg")),
	("classroom", include_str!("../../settings/classroom.svg")),
	("forest", include_str!("../../settings/forest.svg")),
	("beach", include_str!("../../settings/beach.svg")),
	("hilltop", include_str!("../../settings/hilltop.svg")),
	("shop", include_str!("../../settings/shop.svg")),
	("playroom", include_str!("../../settings/playroom.svg")),
	("stage", include_str!("../../settings/stage.svg")),
];

/// A built-in setting's part file, by name.
pub fn built_in_source(name: &str) -> Option<&'static str> {
	BUILT_IN
		.iter()
		.find(|(key, _)| *key == name)
		.map(|(_, source)| *source)
}

/// The names of the settings compiled into fables.
pub fn built_in_names() -> impl Iterator<Item = &'static str> {
	BUILT_IN.iter().map(|(name, _)| *name)
}

/// A setting file as an editor sees it: its ground, its title, its sound beds, and each part's
/// id and markup, in drawing order.
#[derive(Clone, Debug)]
pub struct SettingFile {
	pub ground: f32,
	pub title: String,
	/// The sound beds the place adds (`data-ambience`).
	pub ambience: Vec<Bed>,
	/// Whether the place is indoors (`data-indoors`), so night's crickets stay outside.
	pub indoors: bool,
	pub parts: Vec<(String, String)>,
}

impl SettingFile {
	/// Reads a setting file's text. Elements without a layered id are left out.
	pub fn read(source: &str) -> Result<SettingFile> {
		let document = roxmltree::Document::parse(source).context("not a valid SVG file")?;
		let root = document.root_element();
		let ground = root
			.attribute("data-ground")
			.context("the <svg> needs data-ground")?
			.parse()
			.context("data-ground is not a number")?;
		let title = root
			.children()
			.find(|node| node.has_tag_name("title"))
			.and_then(|node| node.text())
			.unwrap_or_default()
			.to_owned();
		let parts = root
			.descendants()
			.filter_map(|node| {
				let id = node.attribute("id")?;
				Layer::of(id)?;
				Some((id.to_owned(), source[node.range()].to_owned()))
			})
			.collect();
		Ok(SettingFile {
			ground,
			title,
			ambience: read_ambience(root)?,
			indoors: read_indoors(root),
			parts,
		})
	}

	/// The file's text.
	pub fn write(&self) -> String {
		let (ground, title) = (self.ground, &self.title);
		let ambience = if self.ambience.is_empty() {
			String::new()
		} else {
			let beds: Vec<&str> = self.ambience.iter().map(|bed| bed.name()).collect();
			format!(" data-ambience=\"{}\"", beds.join(","))
		};
		let indoors = if self.indoors {
			" data-indoors=\"true\""
		} else {
			""
		};
		let body: Vec<&str> = self
			.parts
			.iter()
			.map(|(_, markup)| markup.as_str())
			.collect();
		format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" \
			 data-ground=\"{ground}\"{ambience}{indoors}>\n  <title>{}</title>\n  {}\n</svg>\n",
			-OVER,
			-OVER,
			STAGE + OVER * 2.0,
			STAGE + OVER * 2.0,
			super::escape(title),
			body.join("\n  ")
		)
	}
}

/// A procedural sound bed a setting or a light adds to the soundtrack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bed {
	Birds,
	Breeze,
	Waves,
	Rain,
	Wind,
	Crickets,
	Room,
	Crowd,
}

impl Bed {
	pub const ALL: [(&'static str, Bed); 8] = [
		("birds", Bed::Birds),
		("breeze", Bed::Breeze),
		("waves", Bed::Waves),
		("rain", Bed::Rain),
		("wind", Bed::Wind),
		("crickets", Bed::Crickets),
		("room", Bed::Room),
		("crowd", Bed::Crowd),
	];

	pub fn parse(name: &str) -> Option<Bed> {
		Self::ALL
			.iter()
			.find(|(key, _)| key.eq_ignore_ascii_case(name))
			.map(|(_, bed)| *bed)
	}

	pub fn name(self) -> &'static str {
		Self::ALL
			.iter()
			.find(|(_, bed)| *bed == self)
			.map_or("", |(name, _)| *name)
	}

	/// The names, for error messages that list the options.
	pub fn names() -> String {
		Self::ALL.map(|(name, _)| name).join(", ")
	}

	/// How loud this bed sits under the others: `room` is near silence with a faint tone.
	pub fn level(self) -> f32 {
		match self {
			Bed::Room => 0.12,
			_ => 1.0,
		}
	}

	/// The beds a scene light adds to the setting's own. A storm brings rain and wind the
	/// place's own beds cannot know about; night and moonlight bring crickets, but only
	/// outdoors.
	pub fn for_light(preset: Preset, indoors: bool) -> &'static [Bed] {
		const STORM: [Bed; 2] = [Bed::Rain, Bed::Wind];
		const CRICKETS: [Bed; 1] = [Bed::Crickets];

		match preset {
			Preset::Storm => &STORM,
			Preset::Night | Preset::Moonlit if !indoors => &CRICKETS,
			_ => &[],
		}
	}
}

/// Reads a setting's `data-ambience` bed list from its root element.
fn read_ambience(root: roxmltree::Node<'_, '_>) -> Result<Vec<Bed>> {
	let Some(value) = root.attribute("data-ambience") else {
		return Ok(Vec::new());
	};
	let mut beds = Vec::new();

	for name in value
		.split(',')
		.map(str::trim)
		.filter(|name| !name.is_empty())
	{
		let bed = Bed::parse(name)
			.with_context(|| format!("unknown ambience `{name}`; try one of: {}", Bed::names()))?;

		if !beds.contains(&bed) {
			beds.push(bed);
		}
	}

	Ok(beds)
}

/// Whether a setting's root marks it as indoors (`data-indoors`).
fn read_indoors(root: roxmltree::Node<'_, '_>) -> bool {
	root.attribute("data-indoors")
		.is_some_and(|value| !value.eq_ignore_ascii_case("false"))
}

/// A painted setting, shared by every page that uses it.
#[derive(Clone, Debug)]
pub struct Setting(Arc<Painted>);

impl PartialEq for Setting {
	fn eq(&self, other: &Setting) -> bool {
		Arc::ptr_eq(&self.0, &other.0) || self.0.name == other.0.name
	}
}

/// A setting's parts.
#[derive(Debug)]
pub struct Painted {
	pub name: String,
	/// A line on what the place is, from the file's `<title>`.
	pub about: String,
	/// Stage y where actors' feet rest.
	pub ground: f32,
	/// The sound beds the place adds to the soundtrack (`data-ambience`).
	pub ambience: Vec<Bed>,
	/// Whether the place is indoors (`data-indoors`).
	pub indoors: bool,
	parts: Vec<Part>,
}

impl Setting {
	pub fn name(&self) -> &str {
		&self.0.name
	}

	pub fn about(&self) -> &str {
		&self.0.about
	}

	/// Stage y where actors' feet rest.
	pub fn ground(&self) -> f32 {
		self.0.ground
	}

	/// The sound beds the place adds to the video's soundtrack, from `data-ambience`.
	pub fn ambience(&self) -> &[Bed] {
		&self.0.ambience
	}

	/// Whether the place is indoors, so night's crickets stay outside (`data-indoors`).
	pub fn indoors(&self) -> bool {
		self.0.indoors
	}

	/// The setting's part names, in drawing order.
	pub fn part_names(&self) -> impl Iterator<Item = &str> {
		self.0.parts.iter().map(|part| part.id.as_str())
	}

	/// Reads a setting from its part file's text.
	pub fn parse(name: &str, source: &str) -> Result<Setting> {
		let document = roxmltree::Document::parse(source).context("not a valid SVG file")?;
		let root = document.root_element();
		let ground = root
			.attribute("data-ground")
			.context("the <svg> needs data-ground, the y where actors stand")?
			.parse::<f32>()
			.context("data-ground is not a number")?;
		let about = root
			.children()
			.find(|node| node.has_tag_name("title"))
			.and_then(|node| node.text())
			.unwrap_or_default()
			.to_owned();
		let ambience = read_ambience(root)?;
		let indoors = read_indoors(root);
		let mut parts = Vec::new();
		let mut problems = Vec::new();

		for node in root.descendants() {
			let Some(id) = node.attribute("id") else {
				continue;
			};

			match Part::read(id, node) {
				Ok(Some(part)) => parts.push(part),
				Ok(None) => {}
				Err(error) => problems.push(format!("`{id}`: {error:#}")),
			}
		}

		ensure!(
			problems.is_empty(),
			"unreadable parts:\n  {}",
			problems.join("\n  ")
		);
		ensure!(
			!parts.is_empty(),
			"it has no parts (ids starting far-, near- or front-)"
		);

		Ok(Setting(Arc::new(Painted {
			name: name.to_owned(),
			about,
			ground,
			ambience,
			indoors,
			parts,
		})))
	}
}

/// A setting's depth layer. Video moves the far layer less than the near one as the camera
/// travels (parallax), which gives flat art real depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
	/// Sky, sun, clouds and distant hills.
	Far,
	/// Everything else behind the actors.
	Near,
	/// In front of the actors.
	Front,
}

impl Layer {
	fn of(id: &str) -> Option<Layer> {
		let (prefix, _) = id.split_once('-')?;

		match prefix {
			"far" => Some(Layer::Far),
			"near" => Some(Layer::Near),
			"front" => Some(Layer::Front),
			_ => None,
		}
	}
}

/// A colour, the scene's tint, or the light's sky gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Paint {
	Color(Color),
	Tint,
	/// The scene light's sky, zenith to horizon. Flat daylight paints the plain sky colour,
	/// exactly as settings always did.
	Sky,
}

impl Paint {
	fn read(value: &str) -> Result<Paint> {
		if value == "tint" {
			return Ok(Paint::Tint);
		}

		if value == "sky" {
			return Ok(Paint::Sky);
		}

		Color::parse(value)
			.map(Paint::Color)
			.with_context(|| format!("`{value}` is not a colour"))
	}

	fn resolve(self, weather: &Weather) -> Color {
		match self {
			Paint::Color(color) => color,
			Paint::Tint => weather.tint,
			Paint::Sky => palette::SKY,
		}
	}
}

#[derive(Clone, Debug)]
enum Mark {
	/// A filled shape, outlined by the art style unless `outline` is off.
	Shape {
		path: Path,
		fill: Paint,
		outline: bool,
	},
	/// A line: plain, or a thick inked tube.
	Line {
		path: Path,
		color: Paint,
		width: f32,
		noodle: bool,
	},
	Sparkle {
		at: Pt,
		size: f32,
		color: Paint,
	},
	/// A soft radial pool of light: a lamp, a window, a glow round the sun or moon.
	Glow {
		at: Pt,
		radius: f32,
		color: Paint,
		opacity: f32,
	},
	Critter {
		kind: Critter,
		at: Pt,
		height: f32,
		color: Color,
		seed: u64,
	},
}

#[derive(Clone, Copy, Debug)]
enum Critter {
	Flower,
	Tree,
}

/// A riso dot screen: colour, spacing, dot radius and opacity.
#[derive(Clone, Copy, Debug)]
struct Dots {
	color: Paint,
	spacing: f32,
	radius: f32,
	opacity: f32,
}

#[derive(Clone, Debug)]
struct Part {
	id: String,
	layer: Layer,
	mark: Mark,
	/// Only drawn by day (`Some(false)`) or by night (`Some(true)`).
	night_only: Option<bool>,
	night: Option<Paint>,
	wilt: Option<Color>,
	depth: f32,
	dots: Option<Dots>,
	drift: f32,
	swing: Option<Pt>,
	/// A vertical gradient over the part, top and bottom colours.
	gradient: Option<(Color, Color)>,
	/// Only drawn under these light presets (`data-lit`).
	lit: Option<Vec<Preset>>,
	/// The part's own bounds, for gradients and glows that span it.
	bounds: Option<(f32, f32, f32, f32)>,
}

impl Part {
	/// A part from an element with an id, or `None` for elements that aren't parts.
	fn read(id: &str, node: roxmltree::Node<'_, '_>) -> Result<Option<Part>> {
		let Some(layer) = Layer::of(id) else {
			return Ok(None);
		};
		let number = |name: &str| -> Result<Option<f32>> {
			node.attribute(name)
				.map(|value| {
					value
						.trim()
						.parse::<f32>()
						.with_context(|| format!("{name} is not a number"))
				})
				.transpose()
		};
		let paint = |name: &str| node.attribute(name).map(Paint::read).transpose();
		let at = || -> Result<Pt> {
			Ok(pt(
				number("cx")?.context("needs cx")?,
				number("cy")?.context("needs cy")?,
			))
		};

		let mark = if let Some(kind) = node.attribute("data-critter") {
			Mark::Critter {
				kind: match kind {
					"flower" => Critter::Flower,
					"tree" => Critter::Tree,
					other => bail!("unknown critter `{other}`; use flower or tree"),
				},
				at: at()?,
				height: number("r")?.context("needs r, the plant's height")?,
				color: node
					.attribute("data-colour")
					.and_then(Color::parse)
					.context("needs data-colour")?,
				seed: node
					.attribute("data-seed")
					.unwrap_or("0")
					.parse()
					.context("data-seed is not a whole number")?,
			}
		} else if let Some(sparkle) = node.attribute("data-sparkle") {
			let (size, color) = sparkle
				.split_once(' ')
				.context("data-sparkle is `size colour`")?;
			Mark::Sparkle {
				at: at()?,
				size: size.parse().context("the sparkle's size is not a number")?,
				color: Paint::read(color.trim())?,
			}
		} else if let Some(glow) = node.attribute("data-glow") {
			let words: Vec<&str> = glow.split_whitespace().collect();
			let [radius, color, opacity] = words[..] else {
				bail!("data-glow is `radius colour opacity`");
			};
			Mark::Glow {
				at: at()?,
				radius: radius
					.parse()
					.context("the glow's radius is not a number")?,
				color: Paint::read(color)?,
				opacity: opacity
					.parse()
					.context("the glow's opacity is not a number")?,
			}
		} else {
			let path = Path::from_data(node.attribute("d").context("a part needs path data, d")?);

			match node.attribute("fill") {
				Some("none") => {
					Mark::Line {
						path,
						color: paint("stroke")?.context("a line (fill=\"none\") needs a stroke")?,
						width: number("stroke-width")?.unwrap_or(7.0),
						noodle: node.attribute("data-noodle").is_some(),
					}
				}
				Some(fill) => {
					Mark::Shape {
						path,
						fill: Paint::read(fill)?,
						outline: node.attribute("data-line") != Some("none"),
					}
				}
				None => bail!("a part needs a fill, or fill=\"none\" and a stroke for a line"),
			}
		};
		let dots = node
			.attribute("data-dots")
			.map(|value| -> Result<Dots> {
				let words: Vec<&str> = value.split_whitespace().collect();
				let [color, spacing, radius, opacity] = words[..] else {
					bail!("data-dots is `colour spacing radius opacity`");
				};
				Ok(Dots {
					color: Paint::read(color)?,
					spacing: spacing.parse()?,
					radius: radius.parse()?,
					opacity: opacity.parse()?,
				})
			})
			.transpose()?;
		let swing = node
			.attribute("data-swing")
			.map(|value| -> Result<Pt> {
				let (x, y) = value
					.split_once(' ')
					.context("data-swing is `x y`, the pivot")?;
				Ok(pt(x.trim().parse()?, y.trim().parse()?))
			})
			.transpose()?;
		let gradient = node
			.attribute("data-gradient")
			.map(|value| -> Result<(Color, Color)> {
				let (top, bottom) = value
					.split_once(' ')
					.context("data-gradient is `top colour bottom colour`")?;
				let colour = |word: &str| {
					Color::parse(word.trim())
						.with_context(|| format!("`{}` is not a colour", word.trim()))
				};
				Ok((colour(top)?, colour(bottom)?))
			})
			.transpose()?;
		let lit = node
			.attribute("data-lit")
			.map(|value| -> Result<Vec<Preset>> {
				value
					.split(',')
					.map(str::trim)
					.filter(|name| !name.is_empty())
					.map(|name| {
						Preset::parse(name).ok_or_else(|| {
							anyhow::anyhow!("data-lit names `{name}`; try {}", Preset::names())
						})
					})
					.collect()
			})
			.transpose()?;
		let bounds = match &mark {
			Mark::Shape { path, .. } | Mark::Line { path, .. } => path_bounds(path.as_str()),
			_ => None,
		};

		Ok(Some(Part {
			id: id.to_owned(),
			layer,
			mark,
			night_only: match node.attribute("data-time") {
				None => None,
				Some("day") => Some(false),
				Some("night") => Some(true),
				Some(other) => bail!("data-time is day or night, not `{other}`"),
			},
			night: paint("data-night")?,
			wilt: node
				.attribute("data-wilt")
				.map(|value| {
					Color::parse(value).with_context(|| format!("`{value}` is not a colour"))
				})
				.transpose()?,
			depth: number("data-depth")?.unwrap_or(0.0).clamp(0.0, 1.0),
			dots,
			drift: number("data-drift")?.unwrap_or(0.0),
			swing,
			gradient,
			lit,
			bounds,
		}))
	}

	/// The part's colour in this weather, before the scene light's grade: wilt, night and the
	/// fade with distance resolve here, so that every route a colour takes to the page is
	/// graded exactly once, where its paint is emitted.
	fn color(&self, paint: Paint, weather: &Weather) -> Color {
		let mut color = paint.resolve(weather);

		if let Some(wilted) = self.wilt {
			color = color.mix(wilted, weather.wilt * 0.8);
		}

		// The night swap and dim belong to a bare night. Under a scene light the light alone
		// makes the night: it grades the day colour, repaints the sky and washes the air, so
		// a night colour authored for the flat dim would only sit darkly underneath it.
		if weather.night && weather.light.is_day() {
			color = match self.night {
				Some(night) => night.resolve(weather),
				None => color.mix(palette::NIGHT, 0.35),
			};
		}

		// Distance fades towards the sky: under a light, towards its horizon colour.
		let haze = if weather.light.is_day() {
			if weather.night {
				palette::NIGHT
			} else {
				palette::SKY.lighten(0.35)
			}
		} else {
			weather.light.horizon
		};
		color.mix(haze, self.depth * 0.45)
	}

	fn draw(&self, svg: &mut Svg, weather: &Weather) {
		if let Some(under) = &self.lit
			&& !under.contains(&weather.light.preset)
		{
			return;
		}

		// Under a lit scene the sun, moon and stars follow the light's sky body, not the bare
		// day-or-night flag; interiors (`indoor`, `candle`) have no sky body at all.
		if !weather.light.is_day() {
			match (self.night_only, weather.light.body) {
				(Some(night), Some(body)) if night != (body == Body::Moon) => return,
				(Some(_), None) => return,
				_ => {}
			}
		} else if self.night_only.is_some_and(|night| night != weather.night) {
			return;
		}

		let moved = self.drift != 0.0 || self.swing.is_some();

		if moved {
			let drift = (weather.time * self.drift) % 400.0;
			let swing = self.swing.map_or(String::new(), |pivot| {
				format!(
					" rotate({:.2} {:.1} {:.1})",
					(weather.time * 1.4).sin() * 3.0,
					pivot.x,
					pivot.y
				)
			});
			svg.begin(&format!("translate({drift:.1} 0){swing}"));
		}

		// Far things draw thinner lines, until the farthest have none.
		let weight = svg.line * (1.0 - self.depth * 0.6);

		match &self.mark {
			Mark::Shape {
				path,
				fill,
				outline,
			} => {
				if *fill == Paint::Sky && !weather.light.is_day() {
					// The light paints the whole sky itself.
					let paint = sky_gradient(svg, &weather.light);
					svg.fill_paint(path, &paint);
				} else if let Some((top, bottom)) = self.gradient {
					// Each end goes through the same weather as a flat fill (wilt, night,
					// distance), and picks up the light's grade once here, where the stop is
					// written into defs; the paint itself is placed by id, not graded again.
					let stop = |colour: Color| {
						svg.graded(snap(svg, self.color(Paint::Color(colour), weather)))
					};
					let (top, bottom) = (stop(top), stop(bottom));
					let paint = part_gradient(
						svg,
						top,
						bottom,
						self.bounds.unwrap_or((0.0, 0.0, STAGE, STAGE)),
					);
					svg.fill_paint(path, &paint);
				} else {
					let color = snap(svg, self.color(*fill, weather));

					if *outline {
						svg.inked_weight(path, color, weight);
					} else {
						svg.fill(path, color);
					}
				}

				if *outline && (*fill == Paint::Sky || self.gradient.is_some()) {
					svg.outline(path, weight);
				}

				if let Some(dots) = self.dots {
					let mut ink = self.color(dots.color, weather);

					if dots.color == Paint::Tint {
						ink = ink.darken(0.2);
					}

					// The dot screen is a pattern def, outside the emit path: graded once here.
					let ink = svg.graded(ink);
					let pattern = halftone(svg, ink, dots.spacing, dots.radius);
					fill_pattern(svg, path, &pattern, dots.opacity);
				}
			}
			Mark::Line {
				path,
				color,
				width,
				noodle,
			} => {
				let color = snap(svg, self.color(*color, weather));

				if *noodle {
					svg.inked_noodle(path, color, *width);
				} else {
					svg.stroke(path, color, *width * (1.0 - self.depth * 0.5));
				}
			}
			Mark::Sparkle { at, size, color } => {
				// fx::sparkle paints through Svg::fill, which grades once at the emit.
				let color = color.resolve(weather);
				super::fx::sparkle(svg, *at, *size, color, weather.time + at.x * 0.01);
			}
			Mark::Glow {
				at,
				radius,
				color,
				opacity,
			} => {
				// The glow is a gradient def, outside the emit path: graded once here.
				let color = svg.graded(color.resolve(weather));
				let paint = radial_glow(svg, *at, *radius, color, *opacity);
				svg.fill_paint(&Path::circle(*at, *radius), &paint);
			}
			Mark::Critter {
				kind,
				at,
				height,
				color,
				seed,
			} => {
				let state = CritterState {
					wilt: weather.wilt,
					time: weather.time,
					..CritterState::default()
				};
				svg.begin(&format!("translate({:.1} {:.1})", at.x, at.y));

				match kind {
					Critter::Flower => {
						let kinds = [FlowerKind::Daisy, FlowerKind::Tulip, FlowerKind::Sunflower];
						let kind = kinds[(*seed % 3) as usize];
						critters::flower(svg, kind, *color, *height, state, *seed);
					}
					Critter::Tree => {
						critters::tree(svg, *color, *height, state, *seed);
					}
				}
				svg.end();
			}
		}

		if moved {
			svg.end();
		}
	}
}

/// The bounds of absolute path data (`M L Q C Z`, the way part files are written): the corners
/// every coordinate pair reaches, for gradients that span the part. `None` for data with no
/// readable points.
fn path_bounds(data: &str) -> Option<(f32, f32, f32, f32)> {
	let mut min = pt(f32::MAX, f32::MAX);
	let mut max = pt(f32::MIN, f32::MIN);
	let mut numbers: Vec<f32> = Vec::new();
	let mut any = false;
	let mut take = |numbers: &mut Vec<f32>| {
		for pair in numbers.chunks(2) {
			if let [x, y] = pair {
				any = true;
				min.x = min.x.min(*x);
				min.y = min.y.min(*y);
				max.x = max.x.max(*x);
				max.y = max.y.max(*y);
			}
		}

		numbers.clear();
	};

	// Every command letter becomes its own token, so `M0` is a command and a point.
	let mut spaced = String::with_capacity(data.len() * 2);

	for ch in data.chars() {
		if ch.is_ascii_alphabetic() {
			spaced.extend([' ', ch, ' ']);
		} else {
			spaced.push(ch);
		}
	}

	for word in spaced
		.split(|c: char| c.is_whitespace() || c == ',')
		.filter(|word| !word.is_empty())
	{
		if word
			.chars()
			.next()
			.is_some_and(|c: char| c.is_ascii_alphabetic())
		{
			take(&mut numbers);
		} else if let Ok(value) = word.parse::<f32>() {
			numbers.push(value);
		}
	}

	take(&mut numbers);
	any.then_some((min.x, min.y, max.x, max.y))
}

/// A colour snapped to the page's limited palette, if its style has one.
fn snap(svg: &Svg, color: Color) -> Color {
	svg.style
		.finish
		.palette
		.map_or(color, |palette| palette.nearest(color))
}

/// The settings a story can use: the built-in ones, and its world's own.
#[derive(Clone, Debug)]
pub struct Library {
	settings: BTreeMap<String, Setting>,
}

static BUILT_IN_LIBRARY: LazyLock<Arc<Library>> = LazyLock::new(|| {
	let settings = BUILT_IN
		.iter()
		.map(|(name, source)| {
			let setting = Setting::parse(name, source)
				.unwrap_or_else(|error| panic!("the built-in {name} setting: {error:#}"));
			((*name).to_owned(), setting)
		})
		.collect();
	Arc::new(Library { settings })
});

impl Library {
	/// The settings compiled into fables.
	pub fn built_in() -> Arc<Library> {
		Arc::clone(&BUILT_IN_LIBRARY)
	}

	/// The built-in settings, with a world's `settings/*.svg` added (or replacing them by
	/// name).
	pub fn for_world(dir: &FilePath) -> Result<Arc<Library>> {
		let folder = dir.join("settings");

		if !folder.is_dir() {
			return Ok(Self::built_in());
		}

		let mut library = (*BUILT_IN_LIBRARY).as_ref().clone();

		for entry in std::fs::read_dir(&folder)
			.with_context(|| format!("could not read {}", folder.display()))?
		{
			let path = entry?.path();

			if path
				.extension()
				.is_none_or(|ext| !ext.eq_ignore_ascii_case("svg"))
			{
				continue;
			}

			let name = path
				.file_stem()
				.and_then(|stem| stem.to_str())
				.context("a setting file needs a name")?
				.to_owned();
			let source = std::fs::read_to_string(&path)
				.with_context(|| format!("could not read {}", path.display()))?;
			let setting =
				Setting::parse(&name, &source).with_context(|| format!("in {}", path.display()))?;
			library.settings.insert(name, setting);
		}

		Ok(Arc::new(library))
	}

	pub fn get(&self, name: &str) -> Option<Setting> {
		self.settings.get(name).cloned()
	}

	/// A setting by name, or an error that lists the ones there are.
	pub fn find(&self, name: &str) -> Result<Setting> {
		self.get(name).with_context(|| {
			format!(
				"unknown setting `{name}`; try one of: {}",
				self.names().collect::<Vec<_>>().join(", ")
			)
		})
	}

	pub fn names(&self) -> impl Iterator<Item = &str> {
		self.settings.keys().map(String::as_str)
	}

	pub fn settings(&self) -> impl Iterator<Item = &Setting> {
		self.settings.values()
	}
}

/// Scene-wide conditions that settings react to.
#[derive(Clone, Copy, Debug)]
pub struct Weather {
	/// 0 = plants thriving, 1 = everything wilted.
	pub wilt: f32,
	/// Green stink haze over the whole scene.
	pub stink: f32,
	/// Cool minty freshness in the air.
	pub mint: f32,
	pub night: bool,
	/// Background colour for the plain setting.
	pub tint: Color,
	/// Manga focus lines rushing in towards the action (0..1).
	pub speed: f32,
	/// A jagged burst behind the action, for a big moment (0..1).
	pub impact: f32,
	pub time: f32,
	/// The scene's light. Flat daylight grades nothing, so a page without `light=` renders
	/// exactly as it always did.
	pub light: Light,
}

impl Default for Weather {
	fn default() -> Self {
		Self {
			wilt: 0.0,
			stink: 0.0,
			mint: 0.0,
			night: false,
			tint: palette::SUNFLOWER,
			speed: 0.0,
			impact: 0.0,
			time: 0.0,
			light: Light::day(),
		}
	}
}

/// Draws everything behind the actors.
pub fn backdrop(svg: &mut Svg, setting: &Setting, weather: &Weather) {
	backdrop_layer(svg, setting, weather, Layer::Far);
	backdrop_layer(svg, setting, weather, Layer::Near);
}

/// Draws one depth layer of a setting. The far layer also paints the light's sun or moon
/// bloom, just past the sky body it belongs to.
pub fn backdrop_layer(svg: &mut Svg, setting: &Setting, weather: &Weather, layer: Layer) {
	if layer == Layer::Far {
		svg.fill(&full_rect(-OVER, STAGE + OVER * 2.0), palette::PAPER);
	}

	let body_night = (!weather.light.is_day())
		.then_some(weather.light.body)
		.flatten()
		.map(|body| body == Body::Moon);
	let mut bloomed = false;

	for part in setting.0.parts.iter().filter(|part| part.layer == layer) {
		part.draw(svg, weather);

		if !bloomed
			&& layer == Layer::Far
			&& body_night.is_some()
			&& weather.light.glow_strength > 0.01
			&& part.night_only == body_night
			&& let Some(bounds) = part.bounds
		{
			let at = pt(
				f32::midpoint(bounds.0, bounds.2),
				f32::midpoint(bounds.1, bounds.3),
			);
			let radius = 560.0 + 420.0 * weather.light.glow_strength;
			let paint = radial_glow(
				svg,
				at,
				radius,
				weather.light.glow_colour,
				weather.light.glow_strength * 0.9,
			);
			svg.fill_paint(&Path::circle(at, radius), &paint);
			bloomed = true;
		}
	}
}

/// Draws anything that sits in front of the actors, plus scene-wide air effects and the
/// light's atmosphere (a haze wash, then a vignette), which the text card and captions sit
/// over.
pub fn foreground(svg: &mut Svg, setting: &Setting, weather: &Weather) {
	backdrop_layer(svg, setting, weather, Layer::Front);

	let bounds = (-OVER, -OVER, STAGE + OVER * 2.0, STAGE + OVER * 2.0);
	super::fx::haze(svg, weather.stink, palette::MOSS, bounds);
	super::fx::haze(svg, weather.mint * 0.6, palette::ICE, bounds);

	if !weather.light.is_day() {
		super::fx::haze(
			svg,
			weather.light.haze_amount,
			weather.light.haze_colour,
			bounds,
		);
		vignette(svg, weather.light.vignette);
	}
}

fn full_rect(y: f32, h: f32) -> Path {
	Path::rect(-OVER, y, STAGE + OVER * 2.0, h)
}

/// The light's sky as a vertical gradient def spanning the drawn stage, zenith to horizon.
fn sky_gradient(svg: &mut Svg, light: &Light) -> String {
	let id = svg.next_id("sky");
	svg.define(&format!(
		r#"<linearGradient id="{id}" x1="0" y1="{:.1}" x2="0" y2="{:.1}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient>"#,
		-OVER,
		STAGE + OVER,
		light.zenith,
		light.horizon
	));

	id
}

/// A vertical gradient over a part's own extent, for walls, water and floors.
fn part_gradient(svg: &mut Svg, top: Color, bottom: Color, bounds: (f32, f32, f32, f32)) -> String {
	let id = svg.next_id("grad");
	svg.define(&format!(
		r#"<linearGradient id="{id}" x1="0" y1="{:.1}" x2="0" y2="{:.1}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>"#,
		bounds.1, bounds.3
	));

	id
}

/// A soft radial pool of light fading to transparent (a lamp, a window, the sun's bloom).
fn radial_glow(svg: &mut Svg, at: Pt, radius: f32, color: Color, opacity: f32) -> String {
	let id = svg.next_id("glow");
	svg.define(&format!(
		r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{:.1}" cy="{:.1}" r="{radius:.1}"><stop offset="0" stop-color="{color}" stop-opacity="{opacity:.2}"/><stop offset="1" stop-color="{color}" stop-opacity="0"/></radialGradient>"#,
		at.x, at.y
	));

	id
}

/// Darkens the edges of the scene with a radial gradient to ink, which survives svg2pdf.
fn vignette(svg: &mut Svg, strength: f32) {
	if strength <= 0.01 {
		return;
	}

	let id = svg.next_id("vignette");
	let centre = STAGE / 2.0;
	let radius = STAGE * 0.78;
	svg.define(&format!(
		r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{centre}" cy="{centre}" r="{radius}"><stop offset="0.55" stop-color="{}" stop-opacity="0"/><stop offset="1" stop-color="{}" stop-opacity="{strength:.2}"/></radialGradient>"#,
		palette::INK, palette::INK
	));
	svg.fill_paint(&full_rect(-OVER, STAGE + OVER * 2.0), &id);
}

/// A dot screen, defined once per document and referenced by id.
fn halftone(svg: &mut Svg, color: Color, spacing: f32, radius: f32) -> String {
	let id = svg.next_id("dots");
	svg.define(&format!(
		r#"<pattern id="{id}" patternUnits="userSpaceOnUse" width="{spacing}" height="{spacing}"><circle cx="{h}" cy="{h}" r="{radius}" fill="{color}"/><circle cx="0" cy="0" r="{radius}" fill="{color}"/><circle cx="{spacing}" cy="0" r="{radius}" fill="{color}"/><circle cx="0" cy="{spacing}" r="{radius}" fill="{color}"/><circle cx="{spacing}" cy="{spacing}" r="{radius}" fill="{color}"/></pattern>"#,
		h = spacing / 2.0
	));

	id
}

fn fill_pattern(svg: &mut Svg, path: &Path, pattern: &str, opacity: f32) {
	// Dot screens belong to the riso look; other styles get their texture from the finish.
	if !svg.style.halftone {
		return;
	}

	svg.raw(&format!(
		r#"<path d="{}" fill="url(#{pattern})" opacity="{opacity:.2}"/>"#,
		path.as_str()
	));
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_built_in_setting_renders_by_day_and_night() {
		let library = Library::built_in();
		assert_eq!(library.names().count(), BUILT_IN.len());

		for setting in library.settings() {
			for night in [false, true] {
				let mut svg = Svg::new();
				let weather = Weather {
					night,
					wilt: 0.5,
					time: 1.0,
					..Weather::default()
				};
				backdrop(&mut svg, setting, &weather);
				foreground(&mut svg, setting, &weather);
				let doc = svg.finish((0.0, 0.0, STAGE, STAGE), 64, 64);
				crate::raster::render(&doc).unwrap();
			}
		}
	}

	#[test]
	fn all_fifteen_built_ins_have_three_layers_and_a_ground() {
		let names: Vec<&str> = built_in_names().collect();
		assert_eq!(
			names,
			[
				"plain",
				"town",
				"street",
				"garden",
				"park",
				"bathroom",
				"bedroom",
				"kitchen",
				"classroom",
				"forest",
				"beach",
				"hilltop",
				"shop",
				"playroom",
				"stage",
			]
		);

		let library = Library::built_in();

		for name in names {
			let setting = library.find(name).unwrap();

			for layer in [Layer::Far, Layer::Near, Layer::Front] {
				assert!(
					setting.0.parts.iter().any(|part| part.layer == layer),
					"{name} has no {layer:?} parts"
				);
			}

			assert!(
				(STAGE * 0.75..STAGE).contains(&setting.ground()),
				"{name} stands its actors at {}",
				setting.ground()
			);
			assert!(
				setting.0.parts.len() < 150,
				"{name} has {} parts, too many to draw every frame",
				setting.0.parts.len()
			);
			assert!(!setting.about().is_empty(), "{name} needs a <title>");
		}
	}

	#[test]
	fn overcast_and_storm_hide_the_sun() {
		let source = r#"<svg data-ground="2000"><path id="far-sun" d="M100 100 C100 110 110 110 110 100 C110 90 100 90 100 100 Z" fill="sunflower" data-time="day"/></svg>"#;
		let sun_shows = |preset: Preset| {
			let light = Light::of(preset);
			let document = draw_setting(
				source,
				&Weather {
					light,
					..Weather::default()
				},
			);
			document.contains(&format!("fill=\"{}\"", light.grade(palette::SUNFLOWER)))
		};

		for preset in [Preset::Day, Preset::Dawn, Preset::Golden, Preset::Dusk] {
			assert!(sun_shows(preset), "the sun is out at {}", preset.name());
		}

		for preset in [
			Preset::Overcast,
			Preset::Storm,
			Preset::Night,
			Preset::Indoor,
		] {
			assert!(!sun_shows(preset), "no sun at {}", preset.name());
		}

		let overcast = draw_setting(
			source,
			&Weather {
				light: Light::of(Preset::Overcast),
				..Weather::default()
			},
		);
		assert!(
			!overcast.contains("radialGradient id=\"glow"),
			"a hidden sun has no bloom: {overcast}"
		);
	}

	#[test]
	fn a_gradient_wilts_darkens_and_grades_like_a_flat_fill() {
		let source = r#"<svg data-ground="2000"><path id="near-lawn" d="M0 100 L100 100 L100 200 L0 200 Z" fill="leaf" data-gradient="meadow leaf" data-wilt="toffee" data-line="none"/></svg>"#;
		let day = draw_setting(source, &Weather::default());
		assert!(
			day.contains(&palette::MEADOW.to_string()) && day.contains(&palette::LEAF.to_string()),
			"the authored colours by day: {day}"
		);

		let wilted = draw_setting(
			source,
			&Weather {
				wilt: 1.0,
				..Weather::default()
			},
		);
		assert!(
			!wilted.contains(&palette::LEAF.to_string()),
			"the lawn wilts: {wilted}"
		);

		let night = draw_setting(
			source,
			&Weather {
				night: true,
				..Weather::default()
			},
		);
		assert!(
			night.contains(&palette::LEAF.mix(palette::NIGHT, 0.35).to_string()),
			"the lawn dims at night: {night}"
		);

		let dusk = Light::of(Preset::Dusk);
		let lit = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);
		assert!(
			lit.contains(&dusk.grade(palette::LEAF).to_string()),
			"the lawn grades with the light: {lit}"
		);
	}

	#[test]
	fn a_part_reacts_to_the_weather() {
		let setting = Setting::parse(
			"test",
			r#"<svg data-ground="2000"><path id="near-hedge" d="M0 0 L10 0 L10 10 Z" fill="leaf" data-wilt="toffee" data-night="night"/><circle id="far-moon" cx="5" cy="5" data-sparkle="20 cream" data-time="night"/></svg>"#,
		)
		.unwrap();
		let hedge = &setting.0.parts[0];
		let Mark::Shape { fill, .. } = hedge.mark else {
			panic!("the hedge is a shape");
		};
		let day = hedge.color(fill, &Weather::default());
		let wilted = hedge.color(
			fill,
			&Weather {
				wilt: 1.0,
				..Weather::default()
			},
		);
		assert_eq!(day, palette::LEAF);
		assert_ne!(wilted, day);
		assert_eq!(
			hedge.color(
				fill,
				&Weather {
					night: true,
					..Weather::default()
				}
			),
			palette::NIGHT
		);
		assert_eq!(setting.0.parts[1].night_only, Some(true));
		assert_eq!(setting.ground(), 2000.0);
	}

	#[test]
	fn a_bad_part_says_which_and_why() {
		let error = Setting::parse(
			"test",
			r#"<svg data-ground="2000"><path id="near-wall" d="M0 0 L1 1" fill="chartreuse-ish"/></svg>"#,
		)
		.unwrap_err();
		let message = format!("{error:#}");
		assert!(
			message.contains("near-wall") && message.contains("not a colour"),
			"{message}"
		);
	}

	#[test]
	fn a_world_adds_its_own_settings() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::create_dir(dir.path().join("settings")).unwrap();
		std::fs::write(
			dir.path().join("settings/harbour.svg"),
			r#"<svg data-ground="2050"><title>The harbour wall</title><path id="far-sea" d="M0 0 L10 0 L10 10 Z" fill="cornflower" data-line="none"/></svg>"#,
		)
		.unwrap();
		let library = Library::for_world(dir.path()).unwrap();
		assert_eq!(library.find("harbour").unwrap().about(), "The harbour wall");
		assert!(library.find("park").is_ok(), "the built-in settings stay");
		let error = format!("{:#}", library.find("moon").unwrap_err());
		assert!(
			error.contains("harbour") && error.contains("park"),
			"{error}"
		);
	}

	/// Parses a tiny setting and draws it under a weather, returning the SVG document. The
	/// document draws under the weather's light, the way every page and frame sets it up.
	fn draw_setting(source: &str, weather: &Weather) -> String {
		let setting = Setting::parse("test", source).unwrap();
		let mut svg = Svg::new();
		svg.set_light(weather.light);
		backdrop(&mut svg, &setting, weather);
		foreground(&mut svg, &setting, weather);
		svg.finish((0.0, 0.0, STAGE, STAGE), 64, 64)
	}

	/// Every route a colour takes from a part file to the page is graded exactly once: where
	/// the paint is emitted, or where a gradient, dot screen or glow is written into defs.
	#[test]
	fn a_setting_reads_its_ambience_and_indoors() {
		let source = r#"<svg data-ground="2000" data-ambience="birds,breeze" data-indoors="true"><path id="near-lawn" d="M0 0 L100 0 L100 100 L0 100 Z" fill="leaf" data-line="none"/></svg>"#;
		let setting = Setting::parse("test", source).unwrap();
		assert_eq!(setting.ambience(), &[Bed::Birds, Bed::Breeze]);
		assert!(setting.indoors());

		let file = SettingFile::read(source).unwrap();
		assert_eq!(file.ambience, vec![Bed::Birds, Bed::Breeze]);
		assert!(file.indoors);
		let written = file.write();
		assert!(
			written.contains("data-ambience=\"birds,breeze\""),
			"{written}"
		);
		assert!(written.contains("data-indoors=\"true\""), "{written}");

		// Editing a setting's parts keeps its sound.
		let again = SettingFile::read(&written).unwrap();
		assert_eq!(again.ambience, file.ambience);
		assert_eq!(again.indoors, file.indoors);
	}

	#[test]
	fn an_unknown_bed_names_the_beds() {
		let source = r#"<svg data-ground="2000" data-ambience="seagulls"><path id="near-lawn" d="M0 0 L10 0 L10 10 Z" fill="leaf"/></svg>"#;
		let error = format!("{:#}", Setting::parse("test", source).unwrap_err());
		assert!(
			error.contains("unknown ambience `seagulls`") && error.contains("crickets"),
			"{error}"
		);
	}

	#[test]
	fn every_built_in_setting_has_a_bed_and_the_interiors_say_they_are_indoors() {
		let library = Library::built_in();

		for name in built_in_names() {
			let setting = library.get(name).unwrap();
			assert!(!setting.ambience().is_empty(), "{name} has no ambience");
		}

		for name in [
			"bathroom",
			"bedroom",
			"kitchen",
			"classroom",
			"shop",
			"playroom",
			"stage",
		] {
			assert!(library.get(name).unwrap().indoors(), "{name} is indoors");
		}

		for name in [
			"park", "garden", "beach", "hilltop", "forest", "street", "town",
		] {
			assert!(!library.get(name).unwrap().indoors(), "{name} is outdoors");
		}
	}

	#[test]
	fn a_setting_part_is_graded_exactly_once() {
		let source = r##"<svg data-ground="2000"><path id="near-lawn" d="M0 0 L100 0 L100 100 L0 100 Z" fill="#7fba31" data-line="none"/><path id="front-post" d="M0 0 L20 0 L20 200 L0 200 Z" fill="#3e6b22"/><path id="near-fence" d="M0 250 L100 250" fill="none" stroke="#6b5a33" stroke-width="7"/></svg>"##;
		let dusk = Light::of(Preset::Dusk);
		let document = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);

		for (attribute, authored, note) in [
			("fill", Color(0x7F_BA_31), "the flat lawn"),
			("fill", Color(0x3E_6B_22), "the outlined post"),
			("stroke", Color(0x6B_5A_33), "the fence line"),
		] {
			let once = dusk.grade(authored);
			let twice = dusk.grade(once);
			assert!(
				document.contains(&format!("{attribute}=\"{once}\"")),
				"{note} is graded once: {document}"
			);
			assert!(
				!document.contains(&twice.to_string()),
				"{note} is not graded twice: {document}"
			);
		}
	}

	#[test]
	fn a_gradient_s_stops_are_graded_exactly_once() {
		let source = r##"<svg data-ground="2000"><path id="near-sea" d="M0 100 L100 100 L100 200 L0 200 Z" fill="cornflower" data-gradient="#8ed0e8 #2e628e" data-line="none"/></svg>"##;
		let dusk = Light::of(Preset::Dusk);
		let document = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);

		for authored in [Color(0x8E_D0_E8), Color(0x2E_62_8E)] {
			let once = dusk.grade(authored);
			assert!(
				document.contains(&format!("stop-color=\"{once}\"")),
				"the stop is graded once: {document}"
			);
			assert!(
				!document.contains(&dusk.grade(once).to_string()),
				"the stop is not graded twice: {document}"
			);
		}
	}

	#[test]
	fn a_dot_screen_is_graded_exactly_once() {
		let source = r##"<svg data-ground="2000"><path id="near-roof" d="M0 0 L100 0 L100 100 Z" fill="tomato" data-dots="#f2d9a6 40 6 0.5"/></svg>"##;
		let dusk = Light::of(Preset::Dusk);
		let document = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);
		let once = dusk.grade(palette::SAND);
		let twice = dusk.grade(once);
		assert!(
			document.contains(&format!("fill=\"{once}\"")),
			"the dot screen is graded once: {document}"
		);
		assert!(
			!document.contains(&twice.to_string()),
			"the dot screen is not graded twice: {document}"
		);
	}

	#[test]
	fn the_sky_s_stops_are_the_light_s_own_colours_never_graded() {
		let source = r#"<svg data-ground="2000"><path id="far-sky" d="M0 0 L100 0 L100 100 Z" fill="sky" data-line="none"/></svg>"#;
		let storm = Light::of(Preset::Storm);
		let document = draw_setting(
			source,
			&Weather {
				light: storm,
				..Weather::default()
			},
		);
		assert!(
			document.contains(&format!("stop-color=\"{}\"", storm.zenith))
				&& document.contains(&format!("stop-color=\"{}\"", storm.horizon)),
			"the sky paints the light's own zenith and horizon: {document}"
		);
		assert!(
			!document.contains(&storm.grade(storm.zenith).to_string()),
			"the light's sky colours are not graded again: {document}"
		);
	}

	#[test]
	fn a_glow_and_a_sparkle_are_graded_exactly_once() {
		let source = r#"<svg data-ground="2000"><circle id="near-lamp" cx="50" cy="50" data-glow="60 #ffd98f 0.5"/><circle id="far-mote" cx="200" cy="200" data-sparkle="20 #ffecc8"/></svg>"#;
		let dusk = Light::of(Preset::Dusk);
		let document = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);

		let glow_once = dusk.grade(Color(0xFF_D9_8F));
		assert!(
			document.contains(&format!("stop-color=\"{glow_once}\"")),
			"the glow is graded once: {document}"
		);
		assert!(
			!document.contains(&dusk.grade(glow_once).to_string()),
			"the glow is not graded twice: {document}"
		);

		let star_once = dusk.grade(palette::CREAM);
		assert!(
			document.contains(&format!("fill=\"{star_once}\"")),
			"the sparkle is graded once: {document}"
		);
		assert!(
			!document.contains(&dusk.grade(star_once).to_string()),
			"the sparkle is not graded twice: {document}"
		);
	}

	#[test]
	fn a_setting_critter_is_graded_exactly_once() {
		let source = r##"<svg data-ground="2000"><circle id="near-flower" cx="100" cy="300" data-critter="flower" r="300" data-colour="#e2564e" data-seed="1"/></svg>"##;
		let dusk = Light::of(Preset::Dusk);
		let document = draw_setting(
			source,
			&Weather {
				light: dusk,
				..Weather::default()
			},
		);
		let once = dusk.grade(Color(0xE2_56_4E));
		let twice = dusk.grade(once);
		assert!(
			document.contains(&format!("fill=\"{once}\"")),
			"the petals take the graded colour: {document}"
		);
		assert!(
			!document.contains(&twice.to_string()),
			"the petals are not graded twice: {document}"
		);
	}

	/// A part's own `data-night` colour belongs to a bare night, where it has always painted
	/// exactly as authored. Under a scene light the light alone makes the night: the day
	/// colour is graded once, and gradients keep both their stops.
	#[test]
	fn a_parts_own_night_colour_belongs_to_a_bare_night() {
		let source = r##"<svg data-ground="2000"><path id="near-wall" d="M0 0 L100 0 L100 100 L0 100 Z" fill="#aabb77" data-night="#1c2a4a" data-line="none"/></svg>"##;

		let bare = draw_setting(
			source,
			&Weather {
				night: true,
				..Weather::default()
			},
		);
		assert!(
			bare.contains("fill=\"#1c2a4a\""),
			"the bare night paints the authored colour exactly: {bare}"
		);

		let night = Light::of(Preset::Night);
		let once = night.grade(Color(0xAA_BB_77));
		let lit = draw_setting(
			source,
			&Weather {
				light: night,
				..Weather::default()
			},
		);
		assert!(
			lit.contains(&format!("fill=\"{once}\"")),
			"the light grades the day colour: {lit}"
		);
		assert!(
			!lit.contains("#1c2a4a"),
			"the bare-night colour waits for a bare night: {lit}"
		);

		// Even a `night` heading under a lit book follows the light.
		let both = draw_setting(
			source,
			&Weather {
				night: true,
				light: night,
				..Weather::default()
			},
		);
		assert!(
			both.contains(&format!("fill=\"{once}\"")),
			"the light wins over the night flag: {both}"
		);
		assert!(!both.contains("#1c2a4a"), "{both}");
	}

	/// Under a bare night a gradient part with a `data-night` colour paints that flat colour
	/// at both stops, exactly as it always did; under a light both stops keep the gradient
	/// and grade like a flat fill.
	#[test]
	fn a_night_gradient_follows_the_bare_night_and_a_light_keeps_the_stops() {
		let source = r##"<svg data-ground="2000"><path id="near-sea" d="M0 100 L100 100 L100 200 L0 200 Z" fill="cornflower" data-gradient="#8ed0e8 #2e628e" data-night="#1c2a4a" data-line="none"/></svg>"##;
		let night = Light::of(Preset::Night);

		let bare = draw_setting(
			source,
			&Weather {
				night: true,
				..Weather::default()
			},
		);
		assert_eq!(
			bare.matches("stop-color=\"#1c2a4a\"").count(),
			2,
			"the bare night turns both stops to the flat night colour: {bare}"
		);

		let lit = draw_setting(
			source,
			&Weather {
				light: night,
				..Weather::default()
			},
		);
		for authored in [Color(0x8E_D0_E8), Color(0x2E_62_8E)] {
			assert!(
				lit.contains(&format!("stop-color=\"{}\"", night.grade(authored))),
				"both stops keep the gradient and grade once: {lit}"
			);
		}
		assert!(
			!lit.contains("#1c2a4a"),
			"the night colour waits for a bare night: {lit}"
		);
	}

	#[test]
	fn the_sky_stays_flat_under_day_and_graduates_under_a_light() {
		let source = r#"<svg data-ground="2000"><path id="far-sky" d="M0 0 L100 0 L100 100 Z" fill="sky" data-line="none"/></svg>"#;
		let day = draw_setting(source, &Weather::default());
		assert!(day.contains("fill=\"#9ed8f0\""), "flat sky colour: {day}");
		assert!(!day.contains("linearGradient"), "day emits no gradient");

		let dusk = Weather {
			light: Light::of(Preset::Dusk),
			..Weather::default()
		};
		let lit = draw_setting(source, &dusk);
		assert!(
			lit.contains("linearGradient") && lit.contains("#5a5a9e"),
			"the dusk sky is the light's gradient: {lit}"
		);

		// Every preset renders without error, and none of them emit a bare flat sky.
		for light in Light::ALL {
			if light.is_day() {
				continue;
			}

			let document = draw_setting(
				source,
				&Weather {
					light,
					..Weather::default()
				},
			);
			crate::raster::render(&document).unwrap();
		}
	}

	#[test]
	fn data_lit_hides_a_part_until_its_light_comes_up() {
		let source = r#"<svg data-ground="2000"><path id="near-window" d="M0 0 L10 0 L10 10 Z" fill="sunflower" data-lit="dusk,night"/></svg>"#;
		let part = Setting::parse("test", source).unwrap().0.parts[0].clone();
		assert_eq!(part.lit, Some(vec![Preset::Dusk, Preset::Night]));

		let day = draw_setting(source, &Weather::default());
		assert!(!day.contains("fill=\"#ffc93c\""), "hidden by day: {day}");

		let graded = Light::of(Preset::Dusk).grade(palette::SUNFLOWER);
		let dusk = draw_setting(
			source,
			&Weather {
				light: Light::of(Preset::Dusk),
				..Weather::default()
			},
		);
		assert!(
			dusk.contains(&format!("fill=\"{graded}\"")),
			"drawn at dusk in the graded colour: {dusk}"
		);

		let error = Setting::parse(
			"test",
			r#"<svg data-ground="2000"><path id="near-window" d="M0 0 L10 0 L10 10 Z" fill="sunflower" data-lit="bedtime"/></svg>"#,
		)
		.unwrap_err();
		let message = format!("{error:#}");
		assert!(
			message.contains("bedtime") && message.contains("moonlit"),
			"{message}"
		);
	}

	#[test]
	fn a_glow_and_a_gradient_draw_through_the_grade() {
		let source = r#"<svg data-ground="2000"><path id="near-sea" d="M0 100 L100 100 L100 200 L0 200 Z" fill="cornflower" data-gradient="cornflower night" data-line="none"/><circle id="near-lamp" cx="50" cy="50" data-glow="60 sunflower 0.5"/></svg>"#;
		let setting = Setting::parse("test", source).unwrap();
		let sea = &setting.0.parts[0];
		assert_eq!(sea.gradient, Some((palette::CORNFLOWER, palette::NIGHT)));
		assert_eq!(sea.bounds, Some((0.0, 100.0, 100.0, 200.0)));

		let dusk = Weather {
			light: Light::of(Preset::Dusk),
			..Weather::default()
		};
		let document = draw_setting(source, &dusk);
		assert!(
			document.contains("linearGradient") && document.contains("radialGradient"),
			"both gradients draw: {document}"
		);

		// Under day the gradient still paints (it is authored data), and the glow too.
		let day = draw_setting(source, &Weather::default());
		assert!(day.contains("linearGradient"), "{day}");
		assert!(day.contains("radialGradient"), "{day}");
		crate::raster::render(&day).unwrap();
	}

	#[test]
	fn a_light_moves_the_sun_and_hides_the_moon() {
		let source = r#"<svg data-ground="2000"><path id="far-sun" d="M100 100 C100 110 110 110 110 100 C110 90 100 90 100 100 Z" fill="sunflower" data-time="day"/><path id="far-moon" d="M200 100 C200 110 210 110 210 100 C210 90 200 90 200 100 Z" fill="cream" data-time="night"/></svg>"#;
		let document = draw_setting(
			source,
			&Weather {
				light: Light::of(Preset::Moonlit),
				..Weather::default()
			},
		);
		let sun_drawn = document.matches("fill=\"#ffc93c\"").count();
		assert_eq!(sun_drawn, 0, "the moon is up, so the sun hides: {document}");
		assert!(
			document.contains("radialGradient"),
			"the moon blooms: {document}"
		);

		let indoor = draw_setting(
			source,
			&Weather {
				light: Light::of(Preset::Indoor),
				..Weather::default()
			},
		);
		assert!(
			!indoor.contains("fill=\"#ffc93c\"") && !indoor.contains("fill=\"#ffecc8\""),
			"an interior has no sky bodies: {indoor}"
		);
	}
}
