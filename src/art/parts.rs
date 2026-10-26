//! Puppet part files: each character's authored drawings, kept in SVG files people can edit.
//!
//! An authored puppet is built from parts (a head, hair, a torso, hands, a chart of mouths),
//! and every part is a drawing that can be opened and changed in Figma, Inkscape or Affinity.
//! Parts every character starts with live in their style's kit (`styles/cartoon/kit.svg`, see
//! [`pack`](super::pack)), and a world can add its own props in `puppets/kit.svg`. A
//! character's own file, such as `worlds/kerfuffle/puppets/stinky.svg`, adds their parts and
//! can redraw any kit part.
//!
//! The rules for a part file:
//!
//! - A part is a `<path>` whose `id` is the part's name: `head.front`, `ear`,
//!   `hand-fist-line-2`. [`FIGURE_PARTS`] lists every name a figure uses.
//! - A part that turns with the head or body has two drawings, `name.front` and `name.3q`.
//!   With the same number of curves they blend smoothly as the character turns; otherwise the
//!   drawing swaps half-way. Draw the three-quarter view by duplicating the front and moving
//!   its points, and the two will blend.
//! - A part's drawing is its own path data plus its own `transform`. Groups around it only lay
//!   the sheet out: move a group to tidy the file, edit a path to change the drawing.
//! - Each part is one outline. Any path commands work (relative, arcs, smooth curves).
//! - Everything else (guides, labels, groups) is ignored.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use svgtypes::SimplePathSegment;
use svgtypes::SimplifyingPathParser;

use super::Pt;
use super::escape;
use super::pt;
use super::shape::Shape;

/// The file of kit parts: in a style pack, and optionally in a world's `puppets/` for its props.
pub const KIT_FILE: &str = "kit.svg";

/// The coordinate space a part is drawn in. Parts are placed by the rig, so each kind of part
/// has its own origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
	/// Head units: the origin between the eyes, the head about 196 across.
	Head,
	/// Figure units: the soles at y = 0, about 500 tall with the hair.
	Body,
	/// Bone units: the joint at the origin and the bone running down +y.
	Bone,
	/// The wrist at the origin, fingers down +y, the thumb on the -x side.
	Hand,
	/// The ankle at the origin, the sole at y = 24, toes to the page right.
	Foot,
	/// A face feature (mouth, brow, a glasses lens, the lashes of the page-right eye) centred on
	/// its own origin.
	Feature,
	/// Something held: the fist's centre at the origin, upright. A toothbrush lies along +x with
	/// its bristles at the far end.
	Prop,
}

impl Space {
	fn describe(self) -> &'static str {
		match self {
			Space::Head => "head units: origin between the eyes",
			Space::Body => "figure units: soles at y = 0",
			Space::Bone => "bone units: joint at the origin, bone down +y",
			Space::Hand => "wrist at the origin, fingers down +y, thumb on the -x side",
			Space::Foot => "ankle at the origin, sole at y = 24",
			Space::Feature => "centred on the origin",
			Space::Prop => "the fist's centre at the origin, upright",
		}
	}
}

/// Whether a part has one drawing or turns with the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Views {
	/// Drawn once, as `name`.
	Once,
	/// Drawn twice, as `name.front` and `name.3q`.
	Turning,
}

/// Whether a figure needs a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
	/// A figure can't be drawn without it.
	Required,
	Optional,
	/// Any number of copies, `name-1`, `name-2`, …, including none.
	Numbered,
}

/// Which file a part normally lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Home {
	/// The character's own file.
	Character,
	/// The shared kit, which a character's file can override.
	Kit,
}

/// A part a figure is drawn with.
#[derive(Clone, Copy, Debug)]
pub struct PartSpec {
	pub name: &'static str,
	pub views: Views,
	pub need: Need,
	pub home: Home,
	pub space: Space,
}

const fn part(name: &'static str, space: Space) -> PartSpec {
	PartSpec {
		name,
		views: Views::Once,
		need: Need::Required,
		home: Home::Character,
		space,
	}
}

impl PartSpec {
	const fn turning(mut self) -> Self {
		self.views = Views::Turning;
		self
	}

	const fn optional(mut self) -> Self {
		self.need = Need::Optional;
		self
	}

	const fn numbered(mut self) -> Self {
		self.need = Need::Numbered;
		self
	}

	const fn kit(mut self) -> Self {
		self.home = Home::Kit;
		self
	}
}

/// Every part an authored figure is drawn with, in the order sheets show them.
pub const FIGURE_PARTS: &[PartSpec] = &[
	part("head", Space::Head).turning(),
	part("hair", Space::Head).turning(),
	part("hair-strand", Space::Head).turning().numbered(),
	// Sheen on the hair, drawn in a lighter shade inside it.
	part("hair-shine", Space::Head).turning().numbered(),
	// Shine on a bald head, drawn in a lighter shade of the skin inside it.
	part("head-shine", Space::Head).turning().numbered(),
	// Loose hair (pigtails, bunches) behind the head. Each lock's drawing starts at its root,
	// where it swings from.
	part("hair-lock", Space::Head).turning().numbered(),
	part("hair-lock-tie", Space::Head).turning().numbered(),
	part("tuft", Space::Head).turning().optional(),
	part("tuft-shine", Space::Head).turning().optional(),
	part("torso", Space::Body).turning(),
	part("torso-hem", Space::Body).optional(),
	part("collar", Space::Body).turning().optional(),
	part("legwear", Space::Body).turning(),
	part("legwear-waist", Space::Body).optional(),
	part("sleeve", Space::Bone),
	part("sleeve-cuff", Space::Bone).optional(),
	part("legwear-leg", Space::Bone).optional(),
	part("legwear-leg-hem", Space::Bone).optional(),
	// Long trousers carry on down the shin, over the socks.
	part("legwear-shin", Space::Bone).optional(),
	part("legwear-shin-cuff", Space::Bone).optional(),
	// Long sleeves carry on down the forearm.
	part("forearm-sleeve", Space::Bone).optional(),
	part("forearm-sleeve-cuff", Space::Bone).optional(),
	part("ear", Space::Head).kit(),
	part("ear-curl", Space::Head).optional().kit(),
	part("brow", Space::Feature).kit(),
	part("nose", Space::Feature).kit(),
	part("nose-wrinkle", Space::Head).numbered().kit(),
	part("hand-relaxed", Space::Hand).kit(),
	part("hand-relaxed-line", Space::Hand).numbered().kit(),
	part("hand-open", Space::Hand).kit(),
	part("hand-fist", Space::Hand).kit(),
	part("hand-fist-line", Space::Hand).numbered().kit(),
	part("hand-point", Space::Hand).kit(),
	part("hand-point-line", Space::Hand).numbered().kit(),
	part("shoe", Space::Foot).turning().kit(),
	part("shoe-lace", Space::Foot).turning().optional().kit(),
	part("mouth-rest", Space::Feature).kit(),
	part("mouth-mbp", Space::Feature).kit(),
	part("mouth-mbp-lip", Space::Feature).kit(),
	part("mouth-teeth", Space::Feature).kit(),
	part("mouth-open", Space::Feature).kit(),
	part("mouth-wide", Space::Feature).kit(),
	part("mouth-fv", Space::Feature).kit(),
	part("mouth-fv-lip", Space::Feature).kit(),
	part("mouth-smile", Space::Feature).kit(),
	part("mouth-grin", Space::Feature).kit(),
	part("mouth-grin-teeth", Space::Feature).kit(),
	part("mouth-grin-gap", Space::Feature).optional().kit(),
	part("mouth-pout", Space::Feature).kit(),
	part("mouth-pout-lip", Space::Feature).kit(),
	part("mouth-bleh", Space::Feature).kit(),
	part("mouth-bleh-tongue", Space::Feature).kit(),
	part("mouth-bleh-groove", Space::Feature).optional().kit(),
	// Things to hold.
	part("prop-jam", Space::Prop).kit(),
	part("prop-jam-lid", Space::Prop).kit(),
	part("prop-jam-label", Space::Prop).kit(),
	part("prop-jam-shine", Space::Prop).optional().kit(),
	part("prop-toothbrush", Space::Prop).kit(),
	part("prop-toothbrush-bristles", Space::Prop).kit(),
	part("prop-toothbrush-bristle-line", Space::Prop)
		.numbered()
		.kit(),
	part("prop-toothpaste", Space::Prop).optional().kit(),
	part("prop-parcel", Space::Prop).kit(),
	part("prop-parcel-top", Space::Prop).optional().kit(),
	part("prop-parcel-side", Space::Prop).optional().kit(),
	part("prop-parcel-string", Space::Prop).numbered().kit(),
	// Extras, drawn when the cast file lists them for a character.
	part("glasses-lens", Space::Feature).kit(),
	part("glasses-bridge", Space::Head).kit(),
	part("moustache", Space::Head).kit(),
	part("bogey", Space::Head).kit(),
	part("lash", Space::Feature).numbered().kit(),
	part("bow", Space::Head).kit(),
	part("bow-knot", Space::Head).optional().kit(),
	part("cap", Space::Head).turning().kit(),
	part("cap-brim", Space::Head).turning().kit(),
	part("cap-badge", Space::Head).optional().kit(),
	part("satchel", Space::Body).kit(),
	part("satchel-flap", Space::Body).optional().kit(),
	part("satchel-strap", Space::Body).turning().kit(),
	part("apron", Space::Body).turning().kit(),
	part("apron-pocket", Space::Body).optional().kit(),
	// Motifs a top's pattern repeats.
	part("pattern-dot", Space::Feature).kit(),
	part("pattern-star", Space::Feature).kit(),
];

/// A part as it is at some turn of the body: one drawing, a blend of two, or a swap.
#[derive(Clone, Copy, Debug)]
pub enum Turn<'a> {
	/// Drawn once; the same at every turn.
	Fixed(&'a Shape),
	/// Front and three-quarter drawings with the same commands, blended point by point.
	Blend { front: &'a Shape, three: &'a Shape },
	/// Drawings that can't blend: the front until half-way, then the three-quarter.
	Swap { front: &'a Shape, three: &'a Shape },
}

impl Turn<'_> {
	/// The drawing at `t`: 0 is front-on, 1 is three-quarters turned.
	pub fn at(&self, t: f32) -> Shape {
		match *self {
			Turn::Fixed(shape) => shape.clone(),
			Turn::Blend { front, three } => front.lerp(three, t.clamp(0.0, 1.0)),
			Turn::Swap { front, three } => {
				if t < 0.5 {
					front.clone()
				} else {
					three.clone()
				}
			}
		}
	}
}

/// How serious a finding from [`Parts::check`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
	/// The puppet can't be drawn until this is fixed.
	Error,
	/// Probably a mistake, such as a part no figure uses.
	Warning,
	/// Worth knowing, such as a pair that swaps instead of blending.
	Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
	pub level: Level,
	pub part: String,
	pub message: String,
}

impl fmt::Display for Finding {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let level = match self.level {
			Level::Error => "error",
			Level::Warning => "warning",
			Level::Note => "note",
		};
		write!(f, "{level}: {}: {}", self.part, self.message)
	}
}

/// A set of named drawings loaded from part files.
#[derive(Clone, Debug, Default)]
pub struct Parts {
	shapes: BTreeMap<String, Shape>,
}

impl Parts {
	/// Reads a part file.
	pub fn load(path: &Path) -> Result<Parts> {
		let source = std::fs::read_to_string(path)
			.with_context(|| format!("could not read part file {}", path.display()))?;
		Self::parse(&source).with_context(|| format!("in {}", path.display()))
	}

	/// Reads a character's part file over their style's `kit`, with a world's own kit (a
	/// `kit.svg` beside the file, for the world's props) in between.
	pub fn load_puppet(path: &Path, kit: &Parts) -> Result<Parts> {
		let own = Self::load(path)?;
		let world_kit = path.with_file_name(KIT_FILE);

		if world_kit == path || !world_kit.is_file() {
			return Ok(own.over(kit.clone()));
		}

		Ok(own.over(Self::load(&world_kit)?.over(kit.clone())))
	}

	/// Reads parts from SVG text.
	pub fn parse(svg: &str) -> Result<Parts> {
		let document = roxmltree::Document::parse(svg).context("not a valid SVG file")?;
		let mut shapes = BTreeMap::new();
		let mut problems = Vec::new();

		for node in document
			.descendants()
			.filter(|node| node.has_tag_name("path"))
		{
			let Some(id) = node.attribute("id").filter(|id| is_part_name(id)) else {
				continue;
			};
			let read = node
				.attribute("d")
				.context("it has no path data")
				.and_then(read_outline)
				.and_then(|shape| {
					match node.attribute("transform") {
						Some(text) => Ok(shape.map(transform(text)?)),
						None => Ok(shape),
					}
				});

			match read {
				Ok(_) if shapes.contains_key(id) => {
					problems.push(format!("`{id}` is used by more than one path"));
				}
				Ok(shape) => {
					shapes.insert(id.to_owned(), shape);
				}
				Err(error) => problems.push(format!("`{id}`: {error:#}")),
			}
		}

		if !problems.is_empty() {
			bail!("unreadable parts:\n  {}", problems.join("\n  "));
		}

		Ok(Parts { shapes })
	}

	/// These parts, with any part they don't draw taken from `base`.
	#[must_use]
	pub fn over(mut self, base: Parts) -> Parts {
		for (name, shape) in base.shapes {
			self.shapes.entry(name).or_insert(shape);
		}

		self
	}

	pub fn get(&self, name: &str) -> Option<&Shape> {
		self.shapes.get(name)
	}

	/// Every drawing, by name.
	pub fn iter(&self) -> impl Iterator<Item = (&str, &Shape)> {
		self.shapes
			.iter()
			.map(|(name, shape)| (name.as_str(), shape))
	}

	/// A part as drawn at any turn: `name.front` and `name.3q`, or a single `name`.
	pub fn turn(&self, name: &str) -> Option<Turn<'_>> {
		let front = self.get(&format!("{name}.front"));
		let three = self.get(&format!("{name}.3q"));

		match (front, three) {
			(Some(front), Some(three)) if front.blends_with(three) => {
				Some(Turn::Blend { front, three })
			}
			(Some(front), Some(three)) => Some(Turn::Swap { front, three }),
			_ => self.get(name).map(Turn::Fixed),
		}
	}

	/// Every copy of a numbered part (`name-1`, `name-2`, …) in order, stopping at the first gap.
	pub fn numbered(&self, name: &str) -> Vec<Turn<'_>> {
		(1..)
			.map_while(|n| self.turn(&format!("{name}-{n}")))
			.collect()
	}

	/// Checks the parts against [`FIGURE_PARTS`]: what is missing, what can't blend, and what
	/// no figure uses. Errors first.
	pub fn check(&self) -> Vec<Finding> {
		let mut findings = Vec::new();
		let mut used = Vec::new();
		let mut find = |level, part: &str, message: String| {
			findings.push(Finding {
				level,
				part: part.to_owned(),
				message,
			});
		};

		for spec in FIGURE_PARTS {
			let names: Vec<String> = if spec.need == Need::Numbered {
				let drawn = |name: &str| {
					self.shapes
						.keys()
						.any(|key| key.split('.').next() == Some(name))
				};
				let mut names = Vec::new();

				while drawn(&format!("{}-{}", spec.name, names.len() + 1)) {
					names.push(format!("{}-{}", spec.name, names.len() + 1));
				}

				names
			} else {
				vec![spec.name.to_owned()]
			};

			for name in &names {
				let (front, three) = (format!("{name}.front"), format!("{name}.3q"));
				used.extend([name.clone(), front.clone(), three.clone()]);

				let (front_shape, three_shape) = (self.get(&front), self.get(&three));

				if spec.views == Views::Turning && self.get(name).is_some() {
					find(
						Level::Error,
						name,
						format!("turns with the body, so draw it as `{front}` and `{three}`"),
					);
				} else if spec.views == Views::Once
					&& (front_shape.is_some() || three_shape.is_some())
				{
					find(
						Level::Error,
						name,
						format!("doesn't turn, so draw it once as `{name}`"),
					);
				} else {
					match (front_shape, three_shape) {
						(Some(_), None) => {
							find(Level::Error, name, format!("`{front}` has no `{three}`"));
						}
						(None, Some(_)) => {
							find(Level::Error, name, format!("`{three}` has no `{front}`"));
						}
						(Some(a), Some(b)) if !a.blends_with(b) => {
							find(
								Level::Note,
								name,
								format!(
									"swaps drawings half-way through a turn: the front has {} \
									 curves and the three-quarter view {} (redraw one from the \
									 other to blend)",
									a.curves(),
									b.curves()
								),
							);
						}
						(None, None) if spec.need == Need::Required && self.get(name).is_none() => {
							let wanted = if spec.views == Views::Turning {
								format!("`{front}` and `{three}`")
							} else {
								format!("`{name}`")
							};
							find(
								Level::Error,
								name,
								format!("missing: draw {wanted} ({})", spec.space.describe()),
							);
						}
						_ => {}
					}
				}
			}
		}

		for name in self.shapes.keys().filter(|name| !used.contains(name)) {
			find(
				Level::Warning,
				name,
				"no figure uses this part (check the spelling against the part list)".to_owned(),
			);
		}

		findings.sort_by_key(|finding| finding.level);
		findings
	}
}

/// Part names are lower-case words joined by hyphens, with an optional `.front` or `.3q`.
fn is_part_name(id: &str) -> bool {
	let base = id
		.strip_suffix(".front")
		.or_else(|| id.strip_suffix(".3q"))
		.unwrap_or(id);

	base.starts_with(|ch: char| ch.is_ascii_lowercase())
		&& base
			.chars()
			.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// One outline from path data, with lines and quadratic curves raised to cubics so drawings
/// blend whatever commands the drawing tool wrote.
fn read_outline(data: &str) -> Result<Shape> {
	let mut points: Vec<Pt> = Vec::new();
	let mut closed = false;
	let p = |x: f64, y: f64| pt(x as f32, y as f32);

	for segment in SimplifyingPathParser::from(data) {
		let segment = segment.map_err(|error| anyhow::anyhow!("bad path data: {error}"))?;

		let last = points.last().copied();

		match (segment, last) {
			(SimplePathSegment::MoveTo { x, y }, None) => points.push(p(x, y)),
			(SimplePathSegment::MoveTo { .. }, Some(_)) => {
				bail!("it has more than one outline (draw each part as its own path)");
			}
			(_, None) => bail!("it doesn't start with a move"),
			(SimplePathSegment::LineTo { x, y }, Some(from)) => {
				let to = p(x, y);
				points.extend([from.lerp(to, 1.0 / 3.0), from.lerp(to, 2.0 / 3.0), to]);
			}
			(SimplePathSegment::Quadratic { x1, y1, x, y }, Some(from)) => {
				let (control, to) = (p(x1, y1), p(x, y));
				points.extend([
					from + (control - from) * (2.0 / 3.0),
					to + (control - to) * (2.0 / 3.0),
					to,
				]);
			}
			(
				SimplePathSegment::CurveTo {
					x1,
					y1,
					x2,
					y2,
					x,
					y,
				},
				Some(_),
			) => points.extend([p(x1, y1), p(x2, y2), p(x, y)]),
			(SimplePathSegment::ClosePath, Some(_)) => closed = true,
		}
	}

	Shape::from_points(points, closed).context("it has no curves")
}

/// The point mapping for an SVG `transform` attribute.
fn transform(text: &str) -> Result<impl Fn(Pt) -> Pt> {
	let svgtypes::Transform { a, b, c, d, e, f } = text
		.parse()
		.map_err(|error| anyhow::anyhow!("bad transform `{text}`: {error}"))?;
	let [a, b, c, d, e, f] = [a, b, c, d, e, f].map(|value| value as f32);

	Ok(move |p: Pt| pt(a * p.x + c * p.y + e, b * p.x + d * p.y + f))
}

/// The spec a drawing's name belongs to: `hair-strand-2.3q` belongs to `hair-strand`.
pub fn spec_of(name: &str) -> Option<&'static PartSpec> {
	let base = name.split('.').next().unwrap_or(name);

	FIGURE_PARTS.iter().find(|spec| {
		spec.name == base
			|| spec.need == Need::Numbered
				&& base
					.strip_prefix(spec.name)
					.and_then(|rest| rest.strip_prefix('-'))
					.is_some_and(|n| n.parse::<u32>().is_ok())
	})
}

/// How much bigger a part is shown on a sheet than it is drawn, so hands and mouths are as easy
/// to edit as heads. Only the sheet's layout groups are scaled; the drawings are not.
fn sheet_zoom(name: &str) -> f32 {
	match spec_of(name).map(|spec| spec.space) {
		Some(Space::Hand) => 3.0,
		Some(Space::Feature | Space::Bone | Space::Foot | Space::Prop) => 2.0,
		Some(Space::Head | Space::Body) | None => 1.0,
	}
}

/// Writes drawings as a part file, laid out as a sheet in the order of [`FIGURE_PARTS`] with
/// each part labelled and its origin marked.
pub fn write(title: &str, parts: &[(&str, &Shape)]) -> String {
	const WIDTH: f32 = 1800.0;
	const GAP: f32 = 50.0;
	const LABEL: f32 = 34.0;
	let mut body = String::new();
	let (mut x, mut y, mut row) = (GAP, 150.0, 0.0_f32);
	let mut parts = parts.to_vec();
	parts.sort_by_key(|&(name, _)| {
		let order = spec_of(name).map_or(usize::MAX, |spec| {
			FIGURE_PARTS
				.iter()
				.position(|other| other.name == spec.name)
				.unwrap_or(usize::MAX)
		});
		// Numbered copies in number order, each with its front before its three-quarter view.
		let number: u32 = name
			.split('.')
			.next()
			.and_then(|base| base.rsplit('-').next())
			.and_then(|n| n.parse().ok())
			.unwrap_or(0);
		(order, number, name.split('.').nth(1) == Some("3q"))
	});

	for (name, shape) in parts {
		let zoom = sheet_zoom(name);
		let (lo, hi) = shape.bounds();
		let (lo, hi) = (
			pt(lo.x.min(0.0), lo.y.min(0.0)),
			pt(hi.x.max(0.0), hi.y.max(0.0)),
		);
		let size = (hi - lo) * zoom;
		let cell = size.x.max(150.0);

		if x + cell > WIDTH - GAP && x > GAP {
			(x, y, row) = (GAP, y + row + LABEL + GAP, 0.0);
		}

		let paint = if shape.is_closed() {
			r##"fill="#f3e7d3" stroke="#2b1d2e" stroke-width="2""##
		} else {
			r##"fill="none" stroke="#2b1d2e" stroke-width="3" stroke-linecap="round""##
		};
		let _ = write!(
			body,
			r##"<g transform="translate({} {}) scale({zoom})"><path d="M-8 0 H8 M0 -8 V8" stroke="#c9a7d6" stroke-width="1.5" vector-effect="non-scaling-stroke"/><path id="{}" d="{}" {paint} vector-effect="non-scaling-stroke"/></g><text x="{x}" y="{}" font-family="sans-serif" font-size="14" fill="#7a6a80">{}</text>"##,
			x - lo.x * zoom,
			y - lo.y * zoom,
			escape(name),
			shape.exact_data(),
			y + size.y + 24.0,
			escape(name)
		);
		x += cell + GAP;
		row = row.max(size.y);
	}

	let height = y + row + LABEL + GAP;
	format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" viewBox="0 0 {WIDTH} {height}"><rect width="100%" height="100%" fill="#fbf3e4"/><text x="{GAP}" y="70" font-family="sans-serif" font-size="28" fill="#2b1d2e">{}</text><text x="{GAP}" y="102" font-family="sans-serif" font-size="14" fill="#7a6a80">Each part is the path whose id is its name. Edit a path to change the drawing; move or scale its group to tidy the sheet.</text><text x="{GAP}" y="122" font-family="sans-serif" font-size="14" fill="#7a6a80">The small cross marks the part's origin. Hands are shown three times their size, mouths, limbs and shoes twice.</text>{body}</svg>"##,
		escape(title)
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	const SQUARE: &str =
		"M0 0 C10 0 20 0 30 0 C30 10 30 20 30 30 C20 30 10 30 0 30 C0 20 0 10 0 0 Z";

	fn file(paths: &str) -> String {
		format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{paths}</svg>"#)
	}

	#[test]
	fn reads_parts_by_id_and_ignores_everything_else() {
		let parts = Parts::parse(&file(&format!(
			r#"<g transform="translate(500 500)"><path id="ear" d="{SQUARE}"/></g><path d="M0 0 L5 5"/><path id="Guide" d="M0 0 L5 5"/>"#
		)))
		.unwrap();
		assert_eq!(parts.iter().count(), 1);
		// The group only lays the sheet out; the part keeps its own coordinates.
		assert_eq!(parts.get("ear").unwrap(), &Shape::parse(SQUARE));
	}

	#[test]
	fn any_path_commands_become_curves() {
		let parts = Parts::parse(&file(
			r#"<path id="brow" d="m0 0 h30 v30 q-15 10 -30 0 z" transform="translate(10 0)"/>"#,
		))
		.unwrap();
		let brow = parts.get("brow").unwrap();
		assert_eq!(brow.curves(), 3);
		assert!(brow.is_closed());
		assert_eq!(brow.points()[0], pt(10.0, 0.0));
		assert_eq!(brow.points()[3], pt(40.0, 0.0));
	}

	#[test]
	fn a_part_is_one_outline() {
		let error =
			Parts::parse(&file(r#"<path id="ear" d="M0 0 L5 5 Z M10 10 L20 20 Z"/>"#)).unwrap_err();
		assert!(format!("{error:#}").contains("more than one outline"));
	}

	#[test]
	fn matching_drawings_blend_and_others_swap() {
		let parts = Parts::parse(&file(&format!(
			r#"<path id="head.front" d="{SQUARE}"/><path id="head.3q" d="{SQUARE}" transform="translate(10 0)"/><path id="hair.front" d="{SQUARE}"/><path id="hair.3q" d="M0 0 C1 1 2 2 3 3 Z"/>"#
		)))
		.unwrap();
		assert!(matches!(parts.turn("head"), Some(Turn::Blend { .. })));
		assert_eq!(
			parts.turn("head").unwrap().at(0.5).points()[0],
			pt(5.0, 0.0)
		);
		let hair = parts.turn("hair").unwrap();
		assert!(matches!(hair, Turn::Swap { .. }));
		assert_eq!(hair.at(0.4).curves(), 4);
		assert_eq!(hair.at(0.6).curves(), 1);
	}

	#[test]
	fn a_character_redraws_kit_parts() {
		let kit = Parts::parse(&file(&format!(
			r#"<path id="ear" d="{SQUARE}"/><path id="brow" d="{SQUARE}"/>"#
		)))
		.unwrap();
		let own = Parts::parse(&file(r#"<path id="ear" d="M0 0 C1 1 2 2 3 3 Z"/>"#)).unwrap();
		let parts = own.over(kit);
		assert_eq!(parts.get("ear").unwrap().curves(), 1);
		assert_eq!(parts.get("brow").unwrap().curves(), 4);
	}

	#[test]
	fn the_check_names_missing_unpaired_and_unknown_parts() {
		let parts = Parts::parse(&file(&format!(
			r#"<path id="head.front" d="{SQUARE}"/><path id="ear.front" d="{SQUARE}"/><path id="path12" d="{SQUARE}"/>"#
		)))
		.unwrap();
		let findings = parts.check();
		let says = |level: Level, part: &str, text: &str| {
			findings
				.iter()
				.any(|f| f.level == level && f.part == part && f.message.contains(text))
		};
		assert!(says(Level::Error, "head", "has no `head.3q`"));
		assert!(says(Level::Error, "ear", "draw it once as `ear`"));
		assert!(says(Level::Error, "torso", "missing"));
		assert!(says(Level::Warning, "path12", "no figure uses"));
		assert!(
			!findings.iter().any(|f| f.part == "tuft"),
			"optional parts may be absent"
		);
		assert_eq!(findings[0].level, Level::Error, "errors come first");
	}

	#[test]
	fn names_find_their_specs() {
		assert_eq!(spec_of("head.front").map(|spec| spec.name), Some("head"));
		assert_eq!(
			spec_of("hair-strand-12.3q").map(|spec| spec.name),
			Some("hair-strand")
		);
		assert_eq!(
			spec_of("hand-fist-line-2").map(|spec| spec.name),
			Some("hand-fist-line")
		);
		assert!(spec_of("hair-strand-x").is_none());
		assert!(spec_of("path12").is_none());
	}

	#[test]
	fn written_files_read_back_exactly() {
		let head = Shape::parse("M-36.5 -6 C-18 2 18 2 36 -6 C33 20 17 34 0 34 Z");
		let strand = Shape::parse("M-46 -100 C-38 -94 -32 -88 -28 -80");
		let svg = write(
			"Test",
			&[("head.front", &head), ("hair-strand-1.front", &strand)],
		);
		let parts = Parts::parse(&svg).unwrap();
		assert_eq!(parts.get("head.front"), Some(&head));
		assert_eq!(parts.get("hair-strand-1.front"), Some(&strand));
	}
}
