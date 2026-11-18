//! Props: the things a story is about, as part files found by name.
//!
//! A prop is a small SVG drawing in its own coordinate space. It is found the way a setting is:
//! the world's `props/<name>.svg` first, then the built-in `props/<name>.svg` compiled into
//! fables. A script holds one with `holds=<name>`, or stands it on the stage as an actor
//! (`@kite x=0.7 y=sky tether=hana`).
//!
//! The root `<svg>` says how it is held:
//!
//! | Attribute | Meaning |
//! | --- | --- |
//! | `data-grip` | `x y`: the point that sits in the fist. |
//! | `data-hands` | `one`, or `two` to bring both hands to the grip. |
//! | `data-angle` | Degrees the prop leans in a relaxed hand. |
//! | `data-carry` | How it is carried standing at ease: `hand` (in the fist at the side), `up` (held up beside the head), `front` (in both hands before the chest), `string` (floating on a string from the fist) or `head` (worn). |
//! | `data-held-scale` | A factor on the held-scale rule, which draws small things bigger than life. |
//! | `data-about` | A sentence on what it is (a `<title>` does the same). |
//! | `data-use` | `brush`: a toothbrush's behaviour — it points at the mouth when it comes near, and scrubs it with foam. |
//! | `data-foam` | A brush's foam colour. |
//!
//! Each part is a `<path>` whose `id` is its name, drawn through the page's finish like a
//! setting's parts. A filled part takes `fill` (a palette name or hex code) and is outlined by
//! the style's line; `data-shade="1"` gives it the style's cel shadow, `data-line="detail"`
//! draws its outline at detail weight and `data-line="none"` leaves it out. A line part has
//! `fill="none"`, a `stroke` and a `stroke-width`, with `data-opacity` for a soft one (a
//! highlight). Parts draw in file order.
//!
//! The built-in thirty-five are `apple`, `ball`, `balloon`, `basket`, `bell`, `book`, `box`,
//! `bucket`, `cake`, `cup`, `drum`, `flower`, `hat`, `jam`, `jar`, `key`, `kite`, `kite-spool`,
//! `ladder`, `lantern`, `leaf`, `letter`, `map`, `paintbrush`, `parcel`, `pebble`, `ruler`,
//! `sandwich`, `spoon`, `star`, `teddy`, `telescope`, `toothbrush`, `torch` and `umbrella`.

use std::collections::BTreeMap;
use std::path::Path as FilePath;
use std::sync::Arc;
use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;

use super::Brush;
use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::figure::Build;
use super::noise;
use super::pack::Finish;
use super::pack::StylePack;
use super::palette;
use super::parts;
use super::pt;
use super::shape::Shape;

/// The ink an authored prop is outlined in, the same as a puppet's.
pub const INK: Color = Color(0x2B_1D_2E);

/// The props compiled into fables, by name.
const BUILT_IN: [(&str, &str); 35] = [
	("apple", include_str!("../../props/apple.svg")),
	("ball", include_str!("../../props/ball.svg")),
	("balloon", include_str!("../../props/balloon.svg")),
	("basket", include_str!("../../props/basket.svg")),
	("bell", include_str!("../../props/bell.svg")),
	("book", include_str!("../../props/book.svg")),
	("box", include_str!("../../props/box.svg")),
	("bucket", include_str!("../../props/bucket.svg")),
	("cake", include_str!("../../props/cake.svg")),
	("cup", include_str!("../../props/cup.svg")),
	("drum", include_str!("../../props/drum.svg")),
	("flower", include_str!("../../props/flower.svg")),
	("hat", include_str!("../../props/hat.svg")),
	("jam", include_str!("../../props/jam.svg")),
	("jar", include_str!("../../props/jar.svg")),
	("key", include_str!("../../props/key.svg")),
	("kite", include_str!("../../props/kite.svg")),
	("kite-spool", include_str!("../../props/kite-spool.svg")),
	("ladder", include_str!("../../props/ladder.svg")),
	("lantern", include_str!("../../props/lantern.svg")),
	("leaf", include_str!("../../props/leaf.svg")),
	("letter", include_str!("../../props/letter.svg")),
	("map", include_str!("../../props/map.svg")),
	("paintbrush", include_str!("../../props/paintbrush.svg")),
	("parcel", include_str!("../../props/parcel.svg")),
	("pebble", include_str!("../../props/pebble.svg")),
	("ruler", include_str!("../../props/ruler.svg")),
	("sandwich", include_str!("../../props/sandwich.svg")),
	("spoon", include_str!("../../props/spoon.svg")),
	("star", include_str!("../../props/star.svg")),
	("teddy", include_str!("../../props/teddy.svg")),
	("telescope", include_str!("../../props/telescope.svg")),
	("toothbrush", include_str!("../../props/toothbrush.svg")),
	("torch", include_str!("../../props/torch.svg")),
	("umbrella", include_str!("../../props/umbrella.svg")),
];

/// A built-in prop's part file, by name.
pub fn built_in_source(name: &str) -> Option<&'static str> {
	BUILT_IN
		.iter()
		.find(|(key, _)| *key == name)
		.map(|(_, source)| *source)
}

/// The names of the props compiled into fables.
pub fn built_in_names() -> impl Iterator<Item = &'static str> {
	BUILT_IN.iter().map(|(name, _)| *name)
}

/// How many hands a held prop brings to its grip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hands {
	One,
	Two,
}

impl Hands {
	pub fn parse(value: &str) -> Option<Hands> {
		match value {
			"one" => Some(Hands::One),
			"two" => Some(Hands::Two),
			_ => None,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Hands::One => "one",
			Hands::Two => "two",
		}
	}
}

/// How a prop is carried by somebody standing at ease (`data-carry`). A pose that does
/// something with its hands (`hold` shows a prop off, `reach-out` offers it) overrides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
	/// In the fist at the side: a cup, a key, a jar of jam.
	Hand,
	/// Held up beside the head at shoulder height, clear of the face: a torch, a flower.
	Up,
	/// In both hands in front of the chest: a cake, a drum.
	Front,
	/// The grip is the end of a string, and the prop floats above and to the side: a balloon.
	String,
	/// Worn on the head, leaving both hands free: a hat.
	Head,
}

impl Carry {
	pub const ALL: [Carry; 5] = [
		Carry::Hand,
		Carry::Up,
		Carry::Front,
		Carry::String,
		Carry::Head,
	];

	pub fn parse(value: &str) -> Option<Carry> {
		Carry::ALL.into_iter().find(|carry| carry.name() == value)
	}

	pub fn name(self) -> &'static str {
		match self {
			Carry::Hand => "hand",
			Carry::Up => "up",
			Carry::Front => "front",
			Carry::String => "string",
			Carry::Head => "head",
		}
	}
}

/// How much bigger than its drawing a prop stands on the stage at `size=1`: the size a kid's
/// hand holds it at, so a kite let go of is the kite that was held. `size=` scales from here.
pub const STAGE_SIZE: f32 = 2.25;

/// Below this many units on its longest side a held prop is drawn bigger than life, so a key
/// or a pebble still reads in a hand on a page seen across a room. The jar of jam (83), the
/// toothbrush (85) and the parcel (100) are big enough already, and are drawn as they always
/// were.
const HELD_LONGEST: f32 = 80.0;

/// Below this much presence (the side of a square of the same area) a held prop is drawn
/// bigger too, so a thin thing (a ruler, a spoon) is not lost.
const HELD_PRESENCE: f32 = 60.0;

/// How much bigger than life the held-scale rule may draw a prop: enough for a pebble to read,
/// not so much that it becomes a boulder.
const HELD_GROWTH: f32 = 2.4;

/// A behaviour a prop has beyond its drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
	/// Drawn where it is held.
	Plain,
	/// A brush: it points at the mouth when it comes near it, and scrubs with foam.
	Brush,
}

/// How a filled part is outlined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
	/// The style's silhouette weight.
	Line,
	/// The style's detail weight.
	Detail,
	/// No outline (a flat patch of colour).
	None,
}

/// One drawing in a prop, in file order.
#[derive(Clone, Debug)]
pub struct PropPart {
	pub id: String,
	pub shape: Shape,
	mark: Mark,
}

#[derive(Clone, Debug)]
enum Mark {
	Shape {
		fill: Color,
		shade: bool,
		line: Line,
	},
	Stroke {
		colour: Color,
		width: f32,
		opacity: f32,
	},
}

/// What inks a prop as it is drawn: the holder's brushed finish, or the flat skin.
#[derive(Clone, Copy, Debug)]
pub enum Ink<'a> {
	/// The authored-puppet finish, the way every held thing is drawn.
	Brushed { finish: &'a Finish, ink: Color },
	/// The older parametric puppet's flat inked look.
	Skin,
}

/// Something held, and what the hand needs to know to draw it.
#[derive(Clone, Copy, Debug)]
pub struct Held<'a> {
	/// Where the grip sits: the fist's centre.
	pub at: Pt,
	/// Props are drawn for a kid's hand; a grown-up's bigger hand holds them bigger.
	pub scale: f32,
	/// The forearm's direction, from the elbow to the wrist.
	pub along: Pt,
	/// The hand's side: -1 page-left, 1 page-right.
	pub side: f32,
	/// Where the mouth is, for a brush.
	pub mouth: Pt,
	/// Seconds since the scene began, for a brush's scrub and foam.
	pub time: f32,
	/// How many of the cartoon fist's 32 units the kit draws its fist.
	pub fist_size: f32,
	/// Degrees the prop leans, when the way it is carried decides rather than its file.
	pub angle: Option<f32>,
	pub ink: Ink<'a>,
}

/// How big a prop is drawn in a figure's hand: the figure's hand scale over a kid's, so a
/// grown-up's bigger hand holds the same prop bigger.
pub fn held_scale(style: &StylePack, build: Build) -> f32 {
	style.body(build).hand_scale / style.body(Build::Kid).hand_scale
}

/// The curve a tether hangs in from `start` to `end`: a cubic whose two handles sit a third
/// and two thirds along, pushed off the straight line on its lower side by the sag (see
/// [`Prop::draw_tether`]). Returns the path and is pure, so its shape can be tested.
pub fn tether_path(start: Pt, end: Pt, sway: f32) -> Path {
	tether_handles(start, end, sway).map_or_else(
		|| Path::new().move_to(start).line_to(end),
		|(a, b)| Path::new().move_to(start).cubic_to(a, b, end),
	)
}

/// The tether curve's two handles, or `None` for a string too short to hang.
pub fn tether_handles(start: Pt, end: Pt, sway: f32) -> Option<(Pt, Pt)> {
	let run = end - start;
	let length = run.len();

	if length < 1.0 {
		return None;
	}

	let along = run * (1.0 / length);
	// Square to the string, on the side towards the ground.
	let across = if along.x >= 0.0 {
		pt(-along.y, along.x)
	} else {
		pt(along.y, -along.x)
	};
	let sag = length * (0.05 + 0.2 * along.x.abs()) + sway * length * 0.04;
	let bow = across * sag;

	Some((
		start + run * (1.0 / 3.0) + bow,
		start + run * (2.0 / 3.0) + bow,
	))
}

/// A prop: a small drawing whose grip sits in a fist, or whose base stands on the stage.
#[derive(Clone, Debug)]
pub struct Prop {
	pub name: String,
	pub about: String,
	/// The point that sits in the fist, in the prop's own units.
	pub grip: Pt,
	pub hands: Hands,
	/// Degrees it leans in a relaxed hand.
	pub angle: f32,
	/// How it is carried by somebody standing at ease.
	pub carry: Carry,
	/// A factor on the held-scale rule's size, for a prop that should be bigger or smaller in
	/// a hand than the rule makes it.
	pub held_scale: f32,
	pub use_kind: Use,
	pub foam: Color,
	pub parts: Arc<Vec<PropPart>>,
	/// The style a prop standing on stage is drawn in.
	style: Arc<StylePack>,
}

/// A prop file as an editor sees it: its grip and behaviour, and each part's id and markup.
#[derive(Clone, Debug)]
pub struct PropFile {
	pub grip: Pt,
	pub hands: Hands,
	pub angle: f32,
	pub carry: Carry,
	pub held_scale: f32,
	pub about: String,
	pub use_kind: Use,
	pub foam: Color,
	pub parts: Vec<(String, String)>,
}

impl Default for PropFile {
	/// A one-handed prop carried in the fist, with its grip at the origin and no parts yet.
	fn default() -> PropFile {
		PropFile {
			grip: pt(0.0, 0.0),
			hands: Hands::One,
			angle: 0.0,
			carry: Carry::Hand,
			held_scale: 1.0,
			about: String::new(),
			use_kind: Use::Plain,
			foam: palette::WHITE,
			parts: Vec::new(),
		}
	}
}

impl PropFile {
	/// Reads a prop file's text. Paths without an id are left out (and reported when the prop
	/// is drawn, where a named part is what a prop is made of).
	pub fn read(source: &str) -> Result<PropFile> {
		let document = roxmltree::Document::parse(source).context("not a valid SVG file")?;
		let root = document.root_element();
		let meta = read_root(root)?;
		let parts = root
			.descendants()
			.filter(|node| node.has_tag_name("path"))
			.filter_map(|node| {
				let id = node.attribute("id")?;
				Some((id.to_owned(), source[node.range()].to_owned()))
			})
			.collect();

		Ok(PropFile {
			grip: meta.grip,
			hands: meta.hands,
			angle: meta.angle,
			carry: meta.carry,
			held_scale: meta.held_scale,
			about: meta.about,
			use_kind: meta.use_kind,
			foam: meta.foam,
			parts,
		})
	}

	/// The file's text.
	pub fn write(&self) -> String {
		use std::fmt::Write as _;

		let mut root = format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" data-grip=\"{:.1} {:.1}\" data-hands=\"{}\"",
			self.grip.x,
			self.grip.y,
			self.hands.name()
		);

		if self.angle != 0.0 {
			let _ = write!(root, " data-angle=\"{:.1}\"", self.angle);
		}

		if self.carry != Carry::Hand {
			let _ = write!(root, " data-carry=\"{}\"", self.carry.name());
		}

		if (self.held_scale - 1.0).abs() > f32::EPSILON {
			let _ = write!(root, " data-held-scale=\"{:.2}\"", self.held_scale);
		}

		if self.use_kind == Use::Brush {
			let _ = write!(root, " data-use=\"brush\" data-foam=\"{}\"", self.foam);
		}

		let body: Vec<&str> = self
			.parts
			.iter()
			.map(|(_, markup)| markup.as_str())
			.collect();
		format!(
			"{root} data-about=\"{}\">\n  <title>{}</title>\n  {}\n</svg>\n",
			super::escape(&self.about),
			super::escape(&self.about),
			body.join("\n  ")
		)
	}
}

/// The grip and behaviour on a prop file's root element.
struct Root {
	grip: Pt,
	hands: Hands,
	angle: f32,
	carry: Carry,
	held_scale: f32,
	about: String,
	use_kind: Use,
	foam: Color,
}

/// A number in a prop file: finite, or an error naming the attribute and what it should be.
fn finite(value: &str, what: &str, example: &str) -> Result<f32> {
	let parsed = value
		.trim()
		.parse::<f32>()
		.ok()
		.filter(|number| number.is_finite());

	parsed.with_context(|| format!("{what} needs a finite number, e.g. `{example}`, not `{value}`"))
}

/// An optional number attribute on `node`, checked by [`finite`].
fn attribute_number(
	node: roxmltree::Node<'_, '_>,
	name: &str,
	example: &str,
) -> Result<Option<f32>> {
	node.attribute(name)
		.map(|value| finite(value, name, example))
		.transpose()
}

fn read_root(root: roxmltree::Node<'_, '_>) -> Result<Root> {
	let grip = root
		.attribute("data-grip")
		.context("the <svg> needs data-grip, the point that sits in the hand")?;
	let (x, y) = grip
		.trim()
		.split_once(' ')
		.with_context(|| format!("data-grip is `x y`, e.g. `0 -40`, not `{grip}`"))?;
	let grip = pt(
		finite(x, "the grip's x", "0 -40")?,
		finite(y, "the grip's y", "0 -40")?,
	);
	let hands = match root.attribute("data-hands") {
		None => Hands::One,
		Some(value) => {
			Hands::parse(value)
				.with_context(|| format!("data-hands is one or two, not `{value}`"))?
		}
	};
	let angle = attribute_number(root, "data-angle", "-20")?.unwrap_or(0.0);
	let carry = match root.attribute("data-carry") {
		None => Carry::Hand,
		Some(value) => {
			Carry::parse(value).with_context(|| {
				let names: Vec<&str> = Carry::ALL.iter().map(|carry| carry.name()).collect();
				format!("data-carry is {}, not `{value}`", names.join(", "))
			})?
		}
	};
	let held_scale = attribute_number(root, "data-held-scale", "1.2")?.unwrap_or(1.0);
	ensure!(
		held_scale > 0.0,
		"data-held-scale is a size above 0, e.g. `1.2`, not `{held_scale}`"
	);
	let about = root
		.attribute("data-about")
		.or_else(|| {
			root.children()
				.find(|node| node.has_tag_name("title"))
				.and_then(|node| node.text())
		})
		.unwrap_or_default()
		.to_owned();
	let use_kind = match root.attribute("data-use") {
		None | Some("plain") => Use::Plain,
		Some("brush") => Use::Brush,
		Some(other) => bail!("data-use is plain or brush, not `{other}`"),
	};
	let foam = root
		.attribute("data-foam")
		.map(|value| Color::parse(value).with_context(|| format!("`{value}` is not a colour")))
		.transpose()?
		.unwrap_or(palette::WHITE);

	Ok(Root {
		grip,
		hands,
		angle,
		carry,
		held_scale,
		about,
		use_kind,
		foam,
	})
}

impl Prop {
	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn about(&self) -> &str {
		&self.about
	}

	/// Reads a prop from its part file's text, drawn in `style` when it stands on stage.
	pub fn parse(name: &str, source: &str, style: &Arc<StylePack>) -> Result<Prop> {
		let document = roxmltree::Document::parse(source).context("not a valid SVG file")?;
		let meta = read_root(document.root_element())?;
		let mut parts = Vec::new();
		let mut problems = Vec::new();

		for node in document
			.descendants()
			.filter(|node| node.has_tag_name("path"))
		{
			match PropPart::read(node) {
				Ok(part) => parts.push(part),
				Err(error) => {
					let id = node.attribute("id").unwrap_or("a part");
					problems.push(format!("`{id}`: {error:#}"));
				}
			}
		}

		ensure!(
			problems.is_empty(),
			"unreadable parts:\n  {}",
			problems.join("\n  ")
		);
		ensure!(
			!parts.is_empty(),
			"it has no parts (a <path> whose id is its name)"
		);

		Ok(Prop {
			name: name.to_owned(),
			about: meta.about,
			grip: meta.grip,
			hands: meta.hands,
			angle: meta.angle,
			carry: meta.carry,
			held_scale: meta.held_scale,
			use_kind: meta.use_kind,
			foam: meta.foam,
			parts: Arc::new(parts),
			style: Arc::clone(style),
		})
	}

	/// How many times its own size the prop is drawn in a hand: small things larger than life,
	/// so they read on the page (see [`HELD_LONGEST`]), times the file's `data-held-scale`.
	pub fn held_size(&self) -> f32 {
		let (lo, hi) = self.bounds();
		let (width, height) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
		let grow = (HELD_LONGEST / width.max(height))
			.max(HELD_PRESENCE / (width * height).sqrt())
			.clamp(1.0, HELD_GROWTH);

		grow * self.held_scale
	}

	/// The part names, in drawing order.
	pub fn part_names(&self) -> impl Iterator<Item = &str> {
		self.parts.iter().map(|part| part.id.as_str())
	}

	/// The prop's drawing bounds in its own units.
	pub fn bounds(&self) -> (Pt, Pt) {
		self.parts.iter().fold(
			(pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN)),
			|(lo, hi), part| {
				let (a, b) = part.shape.bounds();
				(
					pt(lo.x.min(a.x), lo.y.min(a.y)),
					pt(hi.x.max(b.x), hi.y.max(b.y)),
				)
			},
		)
	}

	/// Draws the prop with its grip at `at`, in its own style's finish, leaning by its
	/// `data-angle`.
	pub fn draw(&self, svg: &mut Svg, at: Pt, scale: f32) {
		let saved = svg.use_brush(Brush::new(self.style.line, &self.style.shade, INK));
		self.draw_at(
			svg,
			at,
			scale,
			self.angle,
			&Ink::Brushed {
				finish: &self.style.finish,
				ink: INK,
			},
		);
		svg.brush = saved;
	}

	/// Draws the prop standing upright with its base at the current origin, for a prop on the
	/// stage (its lowest point stands on the ground).
	pub fn draw_standing(&self, svg: &mut Svg, scale: f32) {
		let (lo, hi) = self.bounds();
		let base = pt(f32::midpoint(lo.x, hi.x), hi.y);
		let saved = svg.use_brush(Brush::new(self.style.line, &self.style.shade, INK));
		self.draw_at(
			svg,
			pt(-base.x * scale, -base.y * scale),
			scale,
			0.0,
			&Ink::Brushed {
				finish: &self.style.finish,
				ink: INK,
			},
		);
		svg.brush = saved;
	}

	/// Draws the prop upright with its grip at `at`, `scale` times its own size: worn (a hat on
	/// a head), not held in a hand.
	pub fn draw_worn(&self, svg: &mut Svg, at: Pt, scale: f32, ink: Ink<'_>) {
		let corner = pt(at.x - self.grip.x * scale, at.y - self.grip.y * scale);
		self.draw_at(svg, corner, scale, 0.0, &ink);
	}

	/// Draws the string a stage prop is tied by, from its grip at `start` to the hand at `end`
	/// (stage units), in its own style's detail line, as thick however small the prop flies.
	/// It hangs like string: slack, bowing downwards from the straight line by an amount that
	/// grows with its length and with how level it runs (a string straight up hardly bows),
	/// and swinging a little either side by `sway` (-1 to 1) as the prop bobs.
	pub fn draw_tether(&self, svg: &mut Svg, (start, end): (Pt, Pt), sway: f32) {
		let path = tether_path(start, end, sway);
		let saved = svg.use_brush(Brush::new(self.style.line, &self.style.shade, INK));
		svg.thread(&path, INK, self.style.finish.detail * STAGE_SIZE);
		svg.brush = saved;
	}

	/// Where the grip sits when the prop stands with its base at `base` (bottom centre).
	pub fn base_grip(&self, base: Pt, scale: f32) -> Pt {
		let (lo, hi) = self.bounds();
		let centre = pt(f32::midpoint(lo.x, hi.x), hi.y);
		base + (self.grip - centre) * scale
	}

	fn draw_at(&self, svg: &mut Svg, at: Pt, scale: f32, angle: f32, ink: &Ink<'_>) {
		let transform = if angle.abs() > 0.001 {
			format!(
				"translate({:.2} {:.2}) rotate({angle:.2}) scale({scale:.3})",
				at.x, at.y
			)
		} else {
			format!("translate({:.2} {:.2}) scale({scale:.3})", at.x, at.y)
		};
		svg.begin(&transform);
		self.draw_parts(svg, *ink);
		svg.end();
	}

	/// Draws the prop held in a fist at `held.at`, at the grip, turned for the hand.
	pub fn draw_held(&self, svg: &mut Svg, held: &Held<'_>) {
		if self.use_kind == Use::Brush {
			self.draw_brush(svg, held);
			return;
		}

		let scale = held.scale * self.held_size();
		let angle = held.angle.unwrap_or(self.angle);
		// Leaning turns it about the grip, which stays in the fist.
		let at = held.at - (self.grip * scale).rotate(angle);
		self.draw_at(svg, at, scale, angle, &held.ink);
	}

	/// The box the prop covers when held at `at` (its grip) by a hand of `scale`, leaning by
	/// `angle` degrees: each part's outline turned about the grip, so a tilted umbrella's canopy
	/// and its handle are boxed apart rather than as one square.
	pub fn held_box(&self, at: Pt, scale: f32, angle: f32) -> (Pt, Pt) {
		let scale = scale * self.held_size();

		self.parts
			.iter()
			.map(|part| {
				part.shape
					.map(|p| at + ((p - self.grip) * scale).rotate(angle))
					.bounds()
			})
			.fold(
				(pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN)),
				|(lo, hi), (a, b)| {
					(
						pt(lo.x.min(a.x), lo.y.min(a.y)),
						pt(hi.x.max(b.x), hi.y.max(b.y)),
					)
				},
			)
	}

	/// A brush points at the mouth when it comes near it, and scrubs; otherwise it lies across
	/// the fist, away from the body. This is the toothbrush's behaviour, keyed on `data-use`.
	fn draw_brush(&self, svg: &mut Svg, held: &Held<'_>) {
		let brushing = (held.at - held.mouth).len() < 70.0 * held.fist_size;
		let (dir, scrub) = if brushing {
			let towards = if held.mouth.x < held.at.x { -1.0 } else { 1.0 };
			(
				pt(towards, 0.0),
				(held.time * 22.0).sin() * 8.0 * held.fist_size,
			)
		} else {
			(held.along.perp() * -held.side, 0.0)
		};
		let angle = dir.y.atan2(dir.x).to_degrees();
		let flip = if dir.x < 0.0 { -1.0 } else { 1.0 };
		let scale = held.scale * self.held_size();
		svg.begin(&format!(
			"translate({:.2} {:.2}) rotate({angle:.2}) scale({:.3} {:.3})",
			held.at.x + scrub,
			held.at.y,
			scale,
			flip * scale
		));
		self.draw_parts(svg, held.ink);
		svg.end();

		if brushing {
			for i in 0..5 {
				let at = held.mouth
					+ pt(noise(77, i) * 34.0, 6.0 + noise(78, i) * 16.0)
					+ pt(0.0, (held.time * 6.0 + i as f32).sin() * 2.0);
				let r = 5.0 + noise(79, i).abs() * 5.0;

				match held.ink {
					Ink::Brushed { ink, .. } => {
						svg.brushed(&Path::circle(at, r), self.foam, ink, 1.6);
					}
					Ink::Skin => svg.inked(&Path::circle(at, r), self.foam),
				}
			}
		}
	}

	fn draw_parts(&self, svg: &mut Svg, ink: Ink<'_>) {
		for part in self.parts.iter() {
			let path = part.shape.path();

			match &part.mark {
				Mark::Shape { fill, shade, line } => {
					match ink {
						Ink::Brushed { finish, ink } => {
							if *shade {
								svg.brushed_shaded(
									&part.shape,
									*fill,
									pt(-5.0, -5.0),
									finish.cel_shadow,
									ink,
									finish.line,
								);
							} else if *line == Line::None {
								svg.fill(&path, *fill);
							} else {
								let weight = if *line == Line::Detail {
									finish.detail
								} else {
									finish.line
								};
								svg.brushed(&path, *fill, ink, weight);
							}
						}
						Ink::Skin => {
							if *line == Line::None {
								svg.fill(&path, *fill);
							} else {
								svg.inked(&path, *fill);
							}
						}
					}
				}
				Mark::Stroke {
					colour,
					width,
					opacity,
				} => {
					match ink {
						Ink::Brushed { .. } if *opacity >= 0.999 => {
							svg.thread(&path, *colour, *width);
						}
						_ => svg.stroke_alpha(&path, *colour, *width, *opacity),
					}
				}
			}
		}
	}
}

impl PropPart {
	fn read(node: roxmltree::Node<'_, '_>) -> Result<PropPart> {
		let id = node
			.attribute("id")
			.context("a part needs an id (its name)")?;
		let data = node.attribute("d").context("a part needs path data, d")?;
		let shape = parts::read_part(id, data)?;
		let mark = match node.attribute("fill") {
			Some("none") => {
				let colour = node
					.attribute("stroke")
					.context("a line (fill=\"none\") needs a stroke")?;
				let colour =
					Color::parse(colour).with_context(|| format!("`{colour}` is not a colour"))?;
				let width = attribute_number(node, "stroke-width", "4")?.unwrap_or(4.0);
				let opacity = attribute_number(node, "data-opacity", "0.6")?
					.unwrap_or(1.0)
					.clamp(0.0, 1.0);
				Mark::Stroke {
					colour,
					width,
					opacity,
				}
			}
			Some(fill) => {
				let fill =
					Color::parse(fill).with_context(|| format!("`{fill}` is not a colour"))?;
				let shade = node
					.attribute("data-shade")
					.is_some_and(|value| !matches!(value.trim(), "0" | "none" | "false"));
				let line = match node.attribute("data-line").unwrap_or("line") {
					"line" => Line::Line,
					"detail" => Line::Detail,
					"none" => Line::None,
					other => bail!("data-line is line, detail or none, not `{other}`"),
				};
				Mark::Shape { fill, shade, line }
			}
			None => bail!("a part needs a fill, or fill=\"none\" and a stroke for a line"),
		};

		Ok(PropPart {
			id: id.to_owned(),
			shape,
			mark,
		})
	}
}

/// The props a story can use: the built-in ones, and its world's own.
#[derive(Clone, Debug)]
pub struct Library {
	props: BTreeMap<String, Arc<Prop>>,
}

static BUILT_IN_LIBRARY: LazyLock<Arc<Library>> = LazyLock::new(|| {
	let style = StylePack::default_pack();
	let props = BUILT_IN
		.iter()
		.map(|(name, source)| {
			let prop = Prop::parse(name, source, &style)
				.unwrap_or_else(|error| panic!("the built-in {name} prop: {error:#}"));
			((*name).to_owned(), Arc::new(prop))
		})
		.collect();
	Arc::new(Library { props })
});

impl Library {
	/// The props compiled into fables.
	pub fn built_in() -> Arc<Library> {
		Arc::clone(&BUILT_IN_LIBRARY)
	}

	/// The built-in props, with a world's `props/*.svg` added (or replacing them by name), all
	/// drawn in the world's style when they stand on stage.
	pub fn for_world(dir: &FilePath, style: &Arc<StylePack>) -> Result<Arc<Library>> {
		let mut library = (*BUILT_IN_LIBRARY).restyled(style);
		let folder = dir.join("props");

		if !folder.is_dir() {
			return Ok(Arc::new(library));
		}

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
				.context("a prop file needs a name")?
				.to_owned();
			let source = std::fs::read_to_string(&path)
				.with_context(|| format!("could not read {}", path.display()))?;
			let prop = Prop::parse(&name, &source, style)
				.with_context(|| format!("in {}", path.display()))?;
			library.props.insert(name, Arc::new(prop));
		}

		Ok(Arc::new(library))
	}

	/// The same props, drawn in `style` when they stand on stage.
	fn restyled(&self, style: &Arc<StylePack>) -> Library {
		Library {
			props: self
				.props
				.iter()
				.map(|(name, prop)| {
					let restyled = Prop {
						style: Arc::clone(style),
						..prop.as_ref().clone()
					};
					(name.clone(), Arc::new(restyled))
				})
				.collect(),
		}
	}

	pub fn get(&self, name: &str) -> Option<Arc<Prop>> {
		self.props.get(name).cloned()
	}

	/// A prop by name, or an error that lists the ones there are.
	pub fn find(&self, name: &str) -> Result<Arc<Prop>> {
		self.get(name).with_context(|| {
			format!(
				"unknown prop `{name}`; try one of: {}",
				self.names().collect::<Vec<_>>().join(", ")
			)
		})
	}

	pub fn names(&self) -> impl Iterator<Item = &str> {
		self.props.keys().map(String::as_str)
	}

	pub fn props(&self) -> impl Iterator<Item = &Arc<Prop>> {
		self.props.values()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure::Build;

	fn cartoon() -> Arc<StylePack> {
		StylePack::named("cartoon").unwrap()
	}

	#[test]
	fn every_built_in_prop_reads_and_draws() {
		let library = Library::built_in();
		assert_eq!(library.names().count(), BUILT_IN.len());

		for prop in library.props() {
			assert!(!prop.about().is_empty(), "{} needs a data-about", prop.name);
			assert!(prop.parts.len() <= 12, "{} has too many parts", prop.name);
			let (lo, hi) = prop.bounds();
			assert!(hi.x > lo.x && hi.y > lo.y, "{} draws nothing", prop.name);
			// The grip sits at or inside the drawing: within a fist's reach of it.
			assert!(
				lo.x - 80.0 <= prop.grip.x
					&& prop.grip.x <= hi.x + 80.0
					&& lo.y - 80.0 <= prop.grip.y
					&& prop.grip.y <= hi.y + 80.0,
				"{}'s grip {:?} is far from its drawing {lo:?}..{hi:?}",
				prop.name,
				prop.grip
			);

			let mut svg = Svg::new();
			prop.draw(&mut svg, pt(100.0, 100.0), 1.0);
			crate::raster::render(&svg.finish((0.0, 0.0, 400.0, 400.0), 64, 64)).unwrap();
		}
	}

	#[test]
	fn the_built_ins_are_the_starter_set() {
		let names: Vec<&str> = built_in_names().collect();
		assert_eq!(
			names,
			[
				"apple",
				"ball",
				"balloon",
				"basket",
				"bell",
				"book",
				"box",
				"bucket",
				"cake",
				"cup",
				"drum",
				"flower",
				"hat",
				"jam",
				"jar",
				"key",
				"kite",
				"kite-spool",
				"ladder",
				"lantern",
				"leaf",
				"letter",
				"map",
				"paintbrush",
				"parcel",
				"pebble",
				"ruler",
				"sandwich",
				"spoon",
				"star",
				"teddy",
				"telescope",
				"toothbrush",
				"torch",
				"umbrella",
			]
		);
	}

	#[test]
	fn a_world_prop_replaces_a_built_in_of_the_same_name() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::create_dir(dir.path().join("props")).unwrap();
		std::fs::write(
			dir.path().join("props/ball.svg"),
			r#"<svg data-grip="0 0" data-hands="two" data-about="Our own ball"><path id="ball" d="M0 0 C10 0 20 0 30 0 C30 10 30 20 30 30 Z" fill="teal"/></svg>"#,
		)
		.unwrap();
		let library = Library::for_world(dir.path(), &cartoon()).unwrap();
		let ball = library.find("ball").unwrap();
		assert_eq!(ball.about(), "Our own ball");
		assert_eq!(ball.hands, Hands::Two);
		assert!(library.find("kite").is_ok(), "the built-ins stay");
		let message = format!("{:#}", library.find("moon").unwrap_err());
		assert!(message.contains("kite"), "{message}");
	}

	#[test]
	fn a_bad_prop_says_which_part_and_why() {
		let error = Prop::parse(
			"test",
			r#"<svg data-grip="0 0"><path id="ball" d="M0 0 C1 1 2 2 3 3 Z" fill="chartreuse"/></svg>"#,
			&cartoon(),
		)
		.unwrap_err();
		let message = format!("{error:#}");
		assert!(
			message.contains("ball") && message.contains("not a colour"),
			"{message}"
		);

		let error = Prop::parse("test", r#"<svg data-hands="one"/>"#, &cartoon()).unwrap_err();
		assert!(format!("{error:#}").contains("data-grip"), "{error:#}");

		let error = Prop::parse(
			"test",
			r#"<svg data-grip="0 0" data-hands="three"/>"#,
			&cartoon(),
		)
		.unwrap_err();
		assert!(format!("{error:#}").contains("one or two"), "{error:#}");
	}

	#[test]
	fn a_number_that_is_not_finite_names_its_attribute_and_an_example() {
		let refused = |attributes: &str| {
			let source = format!(r#"<svg {attributes}><path id="a" d="M0 0 L9 9 Z"/></svg>"#);
			format!(
				"{:#}",
				Prop::parse("test", &source, &cartoon()).unwrap_err()
			)
		};

		let message = refused(r#"data-grip="0 0" data-angle="NaN""#);
		assert!(
			message.contains("data-angle") && message.contains("e.g. `-20`"),
			"{message}"
		);
		let message = refused(r#"data-grip="0 inf""#);
		assert!(
			message.contains("grip's y") && message.contains("e.g. `0 -40`"),
			"{message}"
		);
		let message = refused(r#"data-grip="0 0" data-held-scale="-inf""#);
		assert!(message.contains("data-held-scale"), "{message}");
		let message = refused(r#"data-grip="0 0" data-held-scale="0""#);
		assert!(message.contains("above 0"), "{message}");
	}

	#[test]
	fn a_carry_reads_back_as_written_and_a_wrong_one_lists_the_ways() {
		for carry in Carry::ALL {
			assert_eq!(Carry::parse(carry.name()), Some(carry));
		}

		let file = PropFile {
			carry: Carry::String,
			held_scale: 0.8,
			parts: vec![("a".into(), r#"<path id="a" d="M0 0 L9 9 Z"/>"#.into())],
			..PropFile::default()
		};
		let again = PropFile::read(&file.write()).unwrap();
		assert_eq!(again.carry, Carry::String);
		assert!((again.held_scale - 0.8).abs() < 1e-6);
		// The defaults are left out of the file, so an old prop is written as it was.
		assert!(!PropFile::default().write().contains("data-carry"));
		assert!(!PropFile::default().write().contains("data-held-scale"));

		let error = Prop::parse(
			"test",
			r#"<svg data-grip="0 0" data-carry="pocket"/>"#,
			&cartoon(),
		)
		.unwrap_err();
		assert!(
			format!("{error:#}").contains("hand, up, front, string, head"),
			"{error:#}"
		);
	}

	#[test]
	fn the_held_scale_rule_leaves_the_old_props_as_they_were_and_grows_small_ones() {
		// Kerfuffle's books hold these three: they must draw exactly as before the rule.
		let library = Library::built_in();

		for name in ["jam", "toothbrush", "parcel"] {
			let prop = library.find(name).unwrap();
			assert!(
				(prop.held_size() - 1.0).abs() < f32::EPSILON,
				"{name} is held at {}",
				prop.held_size()
			);
		}

		// A pebble would be lost in a fist on a page seen across a room, so the rule grows it,
		// and its file's own factor scales that.
		let pebble = library.find("pebble").unwrap();
		let grown = pebble.held_size() / pebble.held_scale;
		assert!(grown > 1.2 && grown <= HELD_GROWTH, "{grown}");
	}

	#[test]
	fn a_prop_file_written_reads_back_as_the_same_prop() {
		let source = std::fs::read_to_string(
			FilePath::new(env!("CARGO_MANIFEST_DIR")).join("props/kite.svg"),
		)
		.unwrap();
		let file = PropFile::read(&source).unwrap();
		let written = file.write();
		let again = PropFile::read(&written).unwrap();
		assert_eq!(again.grip, file.grip);
		assert_eq!(again.hands, file.hands);
		assert_eq!(again.about, file.about);
		assert_eq!(again.parts.len(), file.parts.len());

		let prop = Prop::parse("kite", &written, &cartoon()).unwrap();
		assert_eq!(prop.name, "kite");
		assert_eq!(prop.parts.len(), file.parts.len());
	}

	#[test]
	fn a_props_scale_follows_the_hand_that_holds_it() {
		// A grown-up's hand is bigger than a kid's, in each style, so the same prop grows
		// with it; and the anime hand's size is its own.
		let scale = |style: &Arc<StylePack>, build: Build| {
			style.body(build).hand_scale / style.body(Build::Kid).hand_scale
		};
		let anime = StylePack::named("anime").unwrap();
		let (cartoon_kid, cartoon_grown) = (
			scale(&cartoon(), Build::Kid),
			scale(&cartoon(), Build::Grownup),
		);
		let (anime_kid, anime_grown) = (scale(&anime, Build::Kid), scale(&anime, Build::Grownup));

		assert!((cartoon_kid - 1.0).abs() < f32::EPSILON);
		assert!(cartoon_grown > cartoon_kid);
		assert!(anime_grown > anime_kid);
	}

	/// How far a tether's first handle sits off the straight line between its ends, as a share
	/// of its length: positive when it is pushed towards the ground.
	#[test]
	fn a_string_is_drawn_even_where_a_style_draws_no_line() {
		for name in ["cartoon", "silhouette", "paper-theatre", "soft-flat"] {
			let style = StylePack::named(name).unwrap();
			let library = Library::for_world(FilePath::new("no-such-world"), &style).unwrap();
			let mut svg = Svg::new();
			library.find("kite").unwrap().draw_tether(
				&mut svg,
				(pt(0.0, 0.0), pt(200.0, 300.0)),
				0.0,
			);
			assert!(svg.body.contains("stroke="), "{name} lost the string");
		}
	}

	fn sag(start: Pt, end: Pt) -> f32 {
		let (a, _) = tether_handles(start, end, 0.0).unwrap();
		let off = a - (start + (end - start) * (1.0 / 3.0));

		off.len().copysign(off.y) / (end - start).len()
	}

	#[test]
	fn a_tether_hangs_below_its_line_and_more_when_level() {
		let kite = pt(1400.0, 600.0);
		let level = sag(kite, pt(600.0, 700.0));
		let steep = sag(kite, pt(1300.0, 1500.0));

		assert!(level > 0.15, "a level string bows well down: {level}");
		assert!(
			steep > 0.0,
			"a steep one still hangs below its line: {steep}"
		);
		assert!(level > steep * 2.0, "level {level} against steep {steep}");

		// Mirrored, it bows down just the same.
		let mirrored = sag(pt(600.0, 600.0), pt(1400.0, 700.0));
		assert!(
			(mirrored - level).abs() < 0.02,
			"{mirrored} against {level}"
		);

		assert!(
			tether_handles(kite, kite, 0.0).is_none(),
			"no string, no curve"
		);
	}
}
