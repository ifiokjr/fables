//! Drawing primitives shared by every illustration.
//!
//! Everything in a Kerfuffle book is drawn from code as SVG. The same drawing functions feed
//! the print PDF, the ePub page images and every video frame, which is what keeps characters
//! identical across formats. The house style is a riso-print look: flat spot colours, a
//! slightly hand-wobbled ink line, and a fill that sits a few units off its outline as if
//! two print passes did not quite line up.

pub mod critters;
pub mod dials;
pub mod figure;
pub mod fx;
pub mod pack;
pub mod palette;
pub mod parts;
pub mod puppet;
pub mod rig;
pub mod settings;
pub mod shape;
pub mod style;

use std::fmt::Write as _;
use std::ops::Add;
use std::ops::Mul;
use std::ops::Sub;

pub use palette::Color;
use shape::Shape;
use style::Shading;
use style::Style;
use style::Texture;
use style::texture_pattern;

/// Width and height of the square stage every scene is composed on.
///
/// One stage unit is 1/282 of an inch on an 8.5" page, so the stage maps onto the trim box
/// of the printed page and video cameras crop windows out of it.
pub const STAGE: f32 = 2400.0;

/// A point (or vector) in stage units. `y` grows downwards, as in SVG.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
	pub x: f32,
	pub y: f32,
}

/// Points in data files are `[x, y]`.
impl<'de> serde::Deserialize<'de> for Pt {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Pt, D::Error> {
		let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
		Ok(pt(x, y))
	}
}

impl serde::Serialize for Pt {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		[self.x, self.y].serialize(serializer)
	}
}

/// Shorthand constructor for [`Pt`].
pub const fn pt(x: f32, y: f32) -> Pt {
	Pt { x, y }
}

impl Pt {
	#[must_use]
	pub fn len(self) -> f32 {
		self.x.hypot(self.y)
	}

	/// Unit vector in the same direction, or zero for the zero vector.
	#[must_use]
	pub fn norm(self) -> Pt {
		let len = self.len();

		if len < f32::EPSILON {
			return Pt::default();
		}

		pt(self.x / len, self.y / len)
	}

	/// The vector rotated a quarter turn clockwise (in screen space).
	#[must_use]
	pub fn perp(self) -> Pt {
		pt(-self.y, self.x)
	}

	#[must_use]
	pub fn lerp(self, other: Pt, t: f32) -> Pt {
		pt(lerp(self.x, other.x, t), lerp(self.y, other.y, t))
	}

	/// Rotates the point around the origin by `degrees`.
	#[must_use]
	pub fn rotate(self, degrees: f32) -> Pt {
		let (sin, cos) = degrees.to_radians().sin_cos();
		pt(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
	}
}

impl Add for Pt {
	type Output = Pt;

	fn add(self, rhs: Pt) -> Pt {
		pt(self.x + rhs.x, self.y + rhs.y)
	}
}

impl Sub for Pt {
	type Output = Pt;

	fn sub(self, rhs: Pt) -> Pt {
		pt(self.x - rhs.x, self.y - rhs.y)
	}
}

impl Mul<f32> for Pt {
	type Output = Pt;

	fn mul(self, rhs: f32) -> Pt {
		pt(self.x * rhs, self.y * rhs)
	}
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
	a + (b - a) * t
}

/// Deterministic hash noise in `-1.0..=1.0`.
///
/// Illustrations must render identically every time (print proofs, ePub and video all have to
/// agree), so all "randomness" comes from hashing a seed and an index.
pub fn noise(seed: u64, index: u64) -> f32 {
	let mut z = seed
		.wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
		.wrapping_add(0x6A09_E667_F3BC_C909);
	z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	z ^= z >> 31;

	(z >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// Hashes a string into a seed so named things get stable wobble.
pub fn seed_of(name: &str) -> u64 {
	name.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
		(hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
	})
}

/// SVG path data, built fluently.
#[derive(Clone, Debug, Default)]
pub struct Path(String);

impl Path {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Path data written elsewhere, such as a part file's `d`.
	#[must_use]
	pub fn from_data(data: &str) -> Self {
		Self(data.trim().to_owned())
	}

	#[must_use]
	pub fn move_to(mut self, p: Pt) -> Self {
		let _ = write!(self.0, "M{:.1} {:.1}", p.x, p.y);
		self
	}

	#[must_use]
	pub fn line_to(mut self, p: Pt) -> Self {
		let _ = write!(self.0, "L{:.1} {:.1}", p.x, p.y);
		self
	}

	#[must_use]
	pub fn quad_to(mut self, c: Pt, p: Pt) -> Self {
		let _ = write!(self.0, "Q{:.1} {:.1} {:.1} {:.1}", c.x, c.y, p.x, p.y);
		self
	}

	#[must_use]
	pub fn cubic_to(mut self, c1: Pt, c2: Pt, p: Pt) -> Self {
		let _ = write!(
			self.0,
			"C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
			c1.x, c1.y, c2.x, c2.y, p.x, p.y
		);
		self
	}

	#[must_use]
	pub fn close(mut self) -> Self {
		self.0.push('Z');
		self
	}

	/// Appends another path's commands (for compound shapes).
	#[must_use]
	pub fn append(mut self, other: &Path) -> Self {
		self.0.push_str(&other.0);
		self
	}

	#[must_use]
	pub fn ellipse(c: Pt, rx: f32, ry: f32) -> Self {
		let k = 0.552_284_8;
		Path::new()
			.move_to(pt(c.x + rx, c.y))
			.cubic_to(
				pt(c.x + rx, c.y + ry * k),
				pt(c.x + rx * k, c.y + ry),
				pt(c.x, c.y + ry),
			)
			.cubic_to(
				pt(c.x - rx * k, c.y + ry),
				pt(c.x - rx, c.y + ry * k),
				pt(c.x - rx, c.y),
			)
			.cubic_to(
				pt(c.x - rx, c.y - ry * k),
				pt(c.x - rx * k, c.y - ry),
				pt(c.x, c.y - ry),
			)
			.cubic_to(
				pt(c.x + rx * k, c.y - ry),
				pt(c.x + rx, c.y - ry * k),
				pt(c.x + rx, c.y),
			)
			.close()
	}

	#[must_use]
	pub fn circle(c: Pt, r: f32) -> Self {
		Self::ellipse(c, r, r)
	}

	#[must_use]
	pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Self {
		Path::new()
			.move_to(pt(x, y))
			.line_to(pt(x + w, y))
			.line_to(pt(x + w, y + h))
			.line_to(pt(x, y + h))
			.close()
	}

	/// A smooth closed curve through `points` (Catmull-Rom converted to cubic Béziers).
	#[must_use]
	pub fn blob(points: &[Pt]) -> Self {
		let n = points.len();

		if n < 3 {
			return Path::new();
		}

		let mut path = Path::new().move_to(points[0]);

		for i in 0..n {
			let p0 = points[(i + n - 1) % n];
			let p1 = points[i];
			let p2 = points[(i + 1) % n];
			let p3 = points[(i + 2) % n];
			path = path.cubic_to(
				p1 + (p2 - p0) * (1.0 / 6.0),
				p2 - (p3 - p1) * (1.0 / 6.0),
				p2,
			);
		}

		path.close()
	}

	/// A smooth open curve through `points`.
	#[must_use]
	pub fn spline(points: &[Pt]) -> Self {
		let n = points.len();

		if n < 2 {
			return Path::new();
		}

		let mut path = Path::new().move_to(points[0]);

		for i in 0..n - 1 {
			let p0 = points[i.saturating_sub(1)];
			let p1 = points[i];
			let p2 = points[i + 1];
			let p3 = points[(i + 2).min(n - 1)];
			path = path.cubic_to(
				p1 + (p2 - p0) * (1.0 / 6.0),
				p2 - (p3 - p1) * (1.0 / 6.0),
				p2,
			);
		}

		path
	}

	/// A closed, smooth outline around a centre line whose width tapers from `start` to `end`,
	/// with rounded ends: a limb drawn the way an inker would, thick at the root and fine at the
	/// tip, instead of a uniform tube.
	#[must_use]
	pub fn taper(centre: &[Pt], start: f32, end: f32) -> Self {
		let n = centre.len();

		if n < 2 {
			return Path::new();
		}

		let tangent = |i: usize| (centre[(i + 1).min(n - 1)] - centre[i.saturating_sub(1)]).norm();
		let half = |i: usize| lerp(start, end, i as f32 / (n - 1) as f32) / 2.0;
		let mut outline = Vec::with_capacity(n * 2 + 2);
		outline.push(centre[0] - tangent(0) * half(0));

		for (i, point) in centre.iter().enumerate() {
			outline.push(*point + tangent(i).perp() * half(i));
		}

		outline.push(centre[n - 1] + tangent(n - 1) * half(n - 1));

		for (i, point) in centre.iter().enumerate().rev() {
			outline.push(*point - tangent(i).perp() * half(i));
		}

		Self::blob(&outline)
	}

	/// An ellipse whose edge is nudged by seeded noise so it reads as hand drawn.
	#[must_use]
	pub fn wobbly_ellipse(c: Pt, rx: f32, ry: f32, wobble: f32, seed: u64) -> Self {
		let steps = 14;
		let points: Vec<Pt> = (0..steps)
			.map(|i| {
				let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
				let jitter = 1.0 + noise(seed, i) * wobble;
				pt(
					c.x + angle.cos() * rx * jitter,
					c.y + angle.sin() * ry * jitter,
				)
			})
			.collect();

		Self::blob(&points)
	}

	/// A closed polygon with every corner rounded by roughly `radius`, slightly wobbled.
	#[must_use]
	pub fn wobbly_poly(corners: &[Pt], radius: f32, wobble: f32, seed: u64) -> Self {
		let n = corners.len();
		let jittered: Vec<Pt> = corners
			.iter()
			.enumerate()
			.map(|(i, p)| {
				*p + pt(noise(seed, i as u64 * 2), noise(seed, i as u64 * 2 + 1)) * wobble
			})
			.collect();
		let mut path = Path::new();

		for i in 0..n {
			let prev = jittered[(i + n - 1) % n];
			let here = jittered[i];
			let next = jittered[(i + 1) % n];
			let into = here + (prev - here).norm() * radius.min((prev - here).len() / 2.0);
			let out = here + (next - here).norm() * radius.min((next - here).len() / 2.0);

			path = if i == 0 {
				path.move_to(into)
			} else {
				path.line_to(into)
			};
			path = path.quad_to(here, out);
		}

		path.close()
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// An SVG document under construction.
///
/// `boil` is mixed into every wobble seed. Video frames bump it a few times a second so ink
/// lines shimmer like hand-drawn animation; print renders keep it at zero. `style` decides how
/// every inked shape, outline and detail line is rendered (see [`style`]).
pub struct Svg {
	body: String,
	defs: String,
	ids: u32,
	textures: Vec<Texture>,
	pub boil: u64,
	/// Offset of fills from their ink outlines (the riso misregistration), set from the style.
	pub registration: Pt,
	/// Base ink outline width in stage units.
	pub line: f32,
	pub style: Style,
	/// How the brushed finish inks and shades, set by whoever is drawing (an authored puppet
	/// sets its style's line and shade dials while it draws, with [`Svg::use_brush`]).
	pub brush: Brush,
	/// How each open group's units map to the document's, innermost last, so an offset can be
	/// kept the same on the page however a part is turned.
	frames: Vec<Linear>,
}

/// The linear part of an SVG transform (translation dropped): how a vector in a group's own
/// units maps into the units outside it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Linear {
	a: f32,
	b: f32,
	c: f32,
	d: f32,
}

impl Linear {
	const IDENTITY: Linear = Linear {
		a: 1.0,
		b: 0.0,
		c: 0.0,
		d: 1.0,
	};

	fn apply(self, v: Pt) -> Pt {
		pt(self.a * v.x + self.c * v.y, self.b * v.x + self.d * v.y)
	}

	/// This frame with `inner` nested inside it.
	fn then(self, inner: Linear) -> Linear {
		Linear {
			a: self.a * inner.a + self.c * inner.b,
			b: self.b * inner.a + self.d * inner.b,
			c: self.a * inner.c + self.c * inner.d,
			d: self.b * inner.c + self.d * inner.d,
		}
	}

	fn inverse(self) -> Option<Linear> {
		let det = self.a * self.d - self.b * self.c;

		(det.abs() > 1e-6).then(|| {
			Linear {
				a: self.d / det,
				b: -self.b / det,
				c: -self.c / det,
				d: self.a / det,
			}
		})
	}

	/// The linear part of a transform list such as `translate(4 5) rotate(30) scale(-1 1)`,
	/// as fables writes them.
	fn of_transform(transform: &str) -> Linear {
		transform
			.split(')')
			.filter_map(|item| item.split_once('('))
			.fold(Linear::IDENTITY, |frame, (name, args)| {
				let n: Vec<f32> = args
					.split(|c: char| c == ',' || c.is_whitespace())
					.filter_map(|arg| arg.parse().ok())
					.collect();
				let arg = |i: usize| n.get(i).copied();
				let step = match name.trim() {
					"rotate" => {
						let (sin, cos) = arg(0).unwrap_or(0.0).to_radians().sin_cos();
						Linear {
							a: cos,
							b: sin,
							c: -sin,
							d: cos,
						}
					}
					"scale" => {
						let x = arg(0).unwrap_or(1.0);
						Linear {
							a: x,
							b: 0.0,
							c: 0.0,
							d: arg(1).unwrap_or(x),
						}
					}
					"matrix" => {
						Linear {
							a: arg(0).unwrap_or(1.0),
							b: arg(1).unwrap_or(0.0),
							c: arg(2).unwrap_or(0.0),
							d: arg(3).unwrap_or(1.0),
						}
					}
					"skewX" => {
						Linear {
							c: arg(0).unwrap_or(0.0).to_radians().tan(),
							..Linear::IDENTITY
						}
					}
					"skewY" => {
						Linear {
							b: arg(0).unwrap_or(0.0).to_radians().tan(),
							..Linear::IDENTITY
						}
					}
					_ => Linear::IDENTITY,
				};
				frame.then(step)
			})
	}
}

/// The line and shade dials as the brushed finish uses them (see [`dials`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
	pub line: dials::Line,
	pub tones: u8,
	pub highlight: dials::Highlight,
	/// The ink a self-coloured line replaces with the fill it outlines.
	pub ink: Color,
	/// The last fill laid down, which a self-coloured line takes its colour from.
	last_fill: Option<Color>,
	/// The frame the line's `register` offset is measured in: wherever the brush was picked
	/// up, so every part's colour slips the same way on the page.
	frame: Linear,
}

impl Default for Brush {
	fn default() -> Brush {
		Brush {
			line: dials::Line::default(),
			tones: 1,
			highlight: dials::Highlight::None,
			ink: palette::INK,
			last_fill: None,
			frame: Linear::IDENTITY,
		}
	}
}

impl Brush {
	/// A style's line and shade dials, replacing `ink` when lines are self-coloured.
	pub fn new(line: dials::Line, shade: &dials::Shade, ink: Color) -> Brush {
		Brush {
			line,
			tones: shade.tones,
			highlight: shade.highlight,
			ink,
			last_fill: None,
			frame: Linear::IDENTITY,
		}
	}
}

impl Default for Svg {
	fn default() -> Self {
		Self::new()
	}
}

impl Svg {
	#[must_use]
	pub fn new() -> Self {
		Self::styled(Style::default())
	}

	/// A document drawn in `style`.
	#[must_use]
	pub fn styled(style: Style) -> Self {
		Self {
			body: String::with_capacity(64 * 1024),
			defs: String::new(),
			ids: 0,
			textures: Vec::new(),
			boil: 0,
			registration: style.registration,
			line: 7.0,
			style,
			brush: Brush::default(),
			frames: Vec::new(),
		}
	}

	/// Picks up `brush` for what's drawn from here on, measuring its register offset in the
	/// current frame. Returns the brush it replaces, to put back with `svg.brush = …`.
	pub fn use_brush(&mut self, brush: Brush) -> Brush {
		let frame = self.frame();
		std::mem::replace(&mut self.brush, Brush { frame, ..brush })
	}

	fn frame(&self) -> Linear {
		self.frames.last().copied().unwrap_or(Linear::IDENTITY)
	}

	/// The brush's register offset in the current group's units: the same slip on the page,
	/// however the group is turned, scaled or mirrored.
	fn register_here(&self) -> Pt {
		let register = self.brush.frame.apply(self.brush.line.register);
		self.frame()
			.inverse()
			.map_or(self.brush.line.register, |into| into.apply(register))
	}

	/// Switches style for everything drawn from now on.
	pub fn set_style(&mut self, style: Style) {
		self.style = style;
		self.registration = style.registration;
	}

	/// A document-unique id for clip paths and gradients.
	pub fn next_id(&mut self, prefix: &str) -> String {
		self.ids += 1;
		format!("{prefix}{}", self.ids)
	}

	/// A seed that changes when the line boils.
	pub fn seed(&self, base: u64) -> u64 {
		base ^ self.boil.wrapping_mul(0x2545_F491_4F6C_DD1D)
	}

	/// Opens a `<g>` with an SVG transform; pair with [`Svg::end`].
	pub fn begin(&mut self, transform: &str) {
		let _ = write!(self.body, r#"<g transform="{transform}">"#);
		self.frames
			.push(self.frame().then(Linear::of_transform(transform)));
	}

	/// Opens a `<g>` with group opacity; pair with [`Svg::end`].
	pub fn begin_opacity(&mut self, opacity: f32) {
		let _ = write!(self.body, r#"<g opacity="{opacity:.3}">"#);
		self.frames.push(self.frame());
	}

	/// Opens a layer whose edges the style roughens (torn paper, crayon, paint); pair with
	/// [`Svg::end`]. Applied once per layer rather than per shape, which keeps it cheap.
	pub fn begin_layer(&mut self) {
		self.frames.push(self.frame());

		if self.style.roughen <= 0.0 {
			self.body.push_str("<g>");
			return;
		}

		let id = self.next_id("rough");
		let _ = write!(
			self.defs,
			r#"<filter id="{id}" x="-2%" y="-2%" width="104%" height="104%"><feTurbulence type="fractalNoise" baseFrequency="{:.4}" numOctaves="1" seed="{}"/><feDisplacementMap in="SourceGraphic" scale="{:.1}" xChannelSelector="R" yChannelSelector="G"/></filter>"#,
			self.style.roughen_frequency,
			self.boil % 997,
			self.style.roughen * 2.0
		);
		let _ = write!(self.body, r#"<g filter="url(#{id})">"#);
	}

	pub fn end(&mut self) {
		self.body.push_str("</g>");
		self.frames.pop();
	}

	pub fn fill(&mut self, path: &Path, color: Color) {
		let _ = write!(self.body, r#"<path d="{}" fill="{color}"/>"#, path.as_str());
	}

	pub fn fill_alpha(&mut self, path: &Path, color: Color, opacity: f32) {
		let _ = write!(
			self.body,
			r#"<path d="{}" fill="{color}" fill-opacity="{opacity:.3}"/>"#,
			path.as_str()
		);
	}

	/// A line. Lines in the ink colour are detail work (faces, folds), so the style decides their
	/// colour, weight and sketchiness; coloured lines (stems, ribbons) are drawn as given.
	pub fn stroke(&mut self, path: &Path, color: Color, width: f32) {
		if color == palette::INK {
			let ink = self.style.ink;
			let weight = width * self.style.detail;
			self.sketchy(path, ink, weight, 1.0);
			return;
		}

		self.plain_stroke(path, color, width, 1.0);
	}

	pub fn stroke_alpha(&mut self, path: &Path, color: Color, width: f32, opacity: f32) {
		self.plain_stroke(path, color, width, opacity);
	}

	fn plain_stroke(&mut self, path: &Path, color: Color, width: f32, opacity: f32) {
		let _ = write!(
			self.body,
			r#"<path d="{}" fill="none" stroke="{color}" stroke-opacity="{opacity:.3}" stroke-width="{width:.1}" stroke-linecap="round" stroke-linejoin="round"/>"#,
			path.as_str()
		);
	}

	/// A line drawn once, or several times with a little drift for pencil and crayon.
	fn sketchy(&mut self, path: &Path, color: Color, width: f32, opacity: f32) {
		let Some((passes, drift)) = self.style.sketch else {
			self.plain_stroke(path, color, width, opacity);
			return;
		};

		for pass in 0..passes {
			let seed = self.seed(u64::from(pass) * 7919 + u64::from(self.ids));
			let dx = noise(seed, 1) * drift;
			let dy = noise(seed, 2) * drift;
			let _ = write!(
				self.body,
				r#"<path d="{}" transform="translate({dx:.1} {dy:.1})" fill="none" stroke="{color}" stroke-opacity="{:.3}" stroke-width="{:.1}" stroke-linecap="round" stroke-linejoin="round"/>"#,
				path.as_str(),
				opacity * if pass == 0 { 0.9 } else { 0.55 },
				width * if pass == 0 { 1.0 } else { 0.7 }
			);
		}
	}

	/// A silhouette outline, in the style's weight (none at all in outline-free styles).
	pub fn outline(&mut self, path: &Path, width: f32) {
		if self.style.outline <= 0.0 {
			return;
		}

		let ink = self.style.ink;
		let weight = width * self.style.outline;
		self.sketchy(path, ink, weight, 1.0);
	}

	/// Opens a `<g>` clipped to `path`; pair with [`Svg::end`].
	pub fn begin_clip(&mut self, path: &Path) {
		let id = self.next_id("clip");
		let _ = write!(
			self.defs,
			r#"<clipPath id="{id}"><path d="{}"/></clipPath>"#,
			path.as_str()
		);
		let _ = write!(self.body, r#"<g clip-path="url(#{id})">"#);
		self.frames.push(self.frame());
	}

	/// A fill painted with a gradient or pattern from the document's defs, by id.
	pub fn fill_paint(&mut self, path: &Path, paint: &str) {
		let _ = write!(
			self.body,
			r#"<path d="{}" fill="url(#{paint})"/>"#,
			path.as_str()
		);
	}

	/// The brushed finish, step one: the part's fill over the heavy pass of its line. The heavy
	/// pass sits a little down and to the right, so once [`Svg::brush_line`] is drawn on top
	/// the edge swells on the shadow side like a brush line. Unlike [`Svg::inked`], the brushed
	/// finish ignores the style: it is the authored-puppet look.
	pub fn brush_fill(&mut self, path: &Path, fill: Color, line: Color, weight: f32) {
		self.brush.last_fill = Some(fill);

		if self.brush.line.stroke == dials::Stroke::Brush {
			let line = self.brush_ink(line);
			self.begin(&format!(
				"translate({:.2} {:.2})",
				weight * 0.3,
				weight * 0.45
			));
			self.plain_stroke(path, line, weight * 1.3, 1.0);
			self.end();
		}

		if self.brush.line.register == pt(0.0, 0.0) {
			self.fill(path, fill);
		} else {
			let register = self.register_here();
			self.begin(&format!("translate({:.2} {:.2})", register.x, register.y));
			self.fill(path, fill);
			self.end();
		}
	}

	/// The brushed finish, last step: the line on top.
	pub fn brush_line(&mut self, path: &Path, line: Color, weight: f32) {
		let line = self.brush_ink(line);

		match self.brush.line.stroke {
			dials::Stroke::None => {}
			dials::Stroke::Broken => {
				let _ = write!(
					self.body,
					r#"<path d="{}" fill="none" stroke="{line}" stroke-width="{weight:.1}" stroke-linecap="round" stroke-dasharray="{:.1} {:.1}"/>"#,
					path.as_str(),
					weight * 26.0,
					weight * 3.5
				);
			}
			dials::Stroke::Brush | dials::Stroke::Uniform => {
				self.plain_stroke(path, line, weight, 1.0);
			}
		}
	}

	/// The line colour: ink, or the fill just laid down, darkened, for self-coloured lines.
	fn brush_ink(&self, line: Color) -> Color {
		match (self.brush.line.colour, self.brush.last_fill) {
			(dials::LineColour::SelfColoured, Some(fill)) if line == self.brush.ink => {
				fill.darken(0.45)
			}
			_ => line,
		}
	}

	/// One tone of cel shadow in `shadow`, multiplied over whatever is already painted inside
	/// `shape` (stripes included). The lit area is the shape itself moved towards the light, so
	/// the shadow hugs the far edge and follows the part's own curvature.
	pub fn cel_shade(&mut self, shape: &Shape, light: Pt, shadow: Color) {
		if self.brush.tones == 0 {
			return;
		}

		self.shadow_band(shape, light, shadow);

		// A second, deeper tone in the far edge of the first.
		if self.brush.tones >= 2 {
			self.shadow_band(shape, light * 0.45, shadow.darken(0.12));
		}

		// A light rim along the lit edge.
		if self.brush.highlight == dials::Highlight::Rim {
			let path = shape.path();
			let rim = Path::new()
				.append(&path)
				.append(&shape.shift(light * -0.55).path());
			self.begin_clip(&path);
			let _ = write!(
				self.body,
				r#"<path d="{}" fill="{}" fill-opacity="0.38" fill-rule="evenodd"/>"#,
				rim.as_str(),
				palette::WHITE
			);
			self.end();
		}
	}

	/// One band of cel shadow: the shape minus itself moved towards the light, multiplied.
	fn shadow_band(&mut self, shape: &Shape, light: Pt, shadow: Color) {
		let path = shape.path();
		let lit = Path::new().append(&path).append(&shape.shift(light).path());
		let id = self.next_id("cel");
		let _ = write!(
			self.defs,
			r#"<clipPath id="{id}"><path d="{}"/></clipPath>"#,
			path.as_str()
		);
		// Clip and blend sit on one group: a clip on a parent group would isolate it, leaving
		// the multiply nothing underneath to darken.
		let _ = write!(
			self.body,
			r#"<g clip-path="url(#{id})" style="mix-blend-mode:multiply"><path d="{}" fill="{}" fill-rule="evenodd"/></g>"#,
			lit.as_str(),
			shadow
		);
	}

	/// A part in the brushed finish with no shading.
	pub fn brushed(&mut self, path: &Path, fill: Color, line: Color, weight: f32) {
		self.brush_fill(path, fill, line, weight);
		self.brush_line(path, line, weight);
	}

	/// A part in the brushed finish with one tone of `shadow` away from `light`.
	pub fn brushed_shaded(
		&mut self,
		shape: &Shape,
		fill: Color,
		light: Pt,
		shadow: Color,
		line: Color,
		weight: f32,
	) {
		let path = shape.path();
		self.brush_fill(&path, fill, line, weight);
		self.cel_shade(shape, light, shadow);
		self.brush_line(&path, line, weight);
	}

	/// A filled shape with an outline: the basic stroke of every illustration.
	pub fn inked(&mut self, path: &Path, fill: Color) {
		let line = self.line;
		self.inked_weight(path, fill, line);
	}

	/// Like [`Svg::inked`] with an explicit line weight: silhouettes read best a little heavier
	/// than interior detail, as in hand-inked cartoons.
	pub fn inked_weight(&mut self, path: &Path, fill: Color, line: f32) {
		self.cast_shadow(path);
		self.paint(path, fill);
		self.finish_shape(path, fill);
		self.outline(path, line);
	}

	/// An inked shape with one tone of shadow: light comes from the top left (`light`, within
	/// `reach`), so the shadow lands on the far side. Each style paints that tone its own way.
	pub fn shaded(
		&mut self,
		path: &Path,
		base: Color,
		shade: Color,
		light: Pt,
		reach: f32,
		line: f32,
	) {
		self.cast_shadow(path);

		let clip = self.next_id("shade");
		self.define(&format!(
			r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
			path.as_str()
		));
		let offset = self.registration;
		let opacity = self.style.fill_opacity;
		self.begin(&format!("translate({:.1} {:.1})", offset.x, offset.y));

		match self.style.shading {
			Shading::Gradient => {
				let gradient = self.next_id("glow");
				self.define(&format!(
					r#"<radialGradient id="{gradient}" gradientUnits="userSpaceOnUse" cx="{:.1}" cy="{:.1}" r="{:.1}"><stop offset="0" stop-color="{}"/><stop offset="0.6" stop-color="{base}"/><stop offset="1" stop-color="{shade}"/></radialGradient>"#,
					light.x,
					light.y,
					reach * 1.45,
					base.lighten(0.12)
				));
				let _ = write!(
					self.body,
					r#"<path d="{}" fill="url(#{gradient})"/>"#,
					path.as_str()
				);
			}
			Shading::Flat => self.fill_alpha(path, base, opacity),
			Shading::Crescent | Shading::Cel | Shading::Wash | Shading::Hatch => {
				let shade = if self.style.shading == Shading::Cel {
					shade.darken(0.1)
				} else {
					shade
				};
				self.fill_alpha(path, shade, opacity);
				self.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
				self.fill_alpha(
					&Path::circle(light, reach),
					base,
					if self.style.shading == Shading::Wash {
						0.8
					} else {
						1.0
					},
				);

				if self.style.shading == Shading::Cel {
					let shine = Path::ellipse(
						light + pt(-reach * 0.32, -reach * 0.42),
						reach * 0.2,
						reach * 0.12,
					);
					self.fill_alpha(&shine, palette::WHITE, 0.55);
				}

				self.raw("</g>");
			}
		}

		self.end();
		self.finish_shape(path, base);
		self.outline(path, line);
	}

	/// A thick noodle (arm, leg, stem) drawn as an inked stroke.
	pub fn inked_noodle(&mut self, path: &Path, fill: Color, width: f32) {
		if let Some((offset, opacity)) = self.style.shadow {
			self.begin(&format!("translate({:.1} {:.1})", offset.x, offset.y));
			self.plain_stroke(path, palette::INK, width, opacity);
			self.end();
		}

		if self.style.outline > 0.0 {
			let ink = self.style.ink;
			let edge = width + self.line * 2.0 * self.style.outline;
			self.sketchy(path, ink, edge, 1.0);
		}

		let offset = self.registration;
		self.begin(&format!(
			"translate({:.1} {:.1})",
			offset.x * 0.5,
			offset.y * 0.5
		));
		self.plain_stroke(path, fill, width, self.style.fill_opacity);

		if let Some(texture) = self.texture(fill) {
			let _ = write!(
				self.body,
				r#"<path d="{}" fill="none" stroke="url(#{texture})" stroke-width="{width:.1}" stroke-linecap="round" stroke-linejoin="round"/>"#,
				path.as_str()
			);
		}

		self.end();
	}

	/// Cut paper: every shape lifts off the page and casts a shadow.
	pub fn cast_shadow(&mut self, path: &Path) {
		if let Some((offset, opacity)) = self.style.shadow {
			self.begin(&format!("translate({:.1} {:.1})", offset.x, offset.y));
			self.fill_alpha(path, palette::INK, opacity);
			self.end();
		}
	}

	/// The flat fill, offset by the style's registration.
	fn paint(&mut self, path: &Path, fill: Color) {
		let offset = self.registration;
		let opacity = self.style.fill_opacity;
		let _ = write!(
			self.body,
			r#"<path d="{}" fill="{fill}" fill-opacity="{opacity:.3}" transform="translate({:.1} {:.1})"/>"#,
			path.as_str(),
			offset.x,
			offset.y
		);
	}

	/// Texture and pigment pooling on top of a shape's paint.
	pub fn finish_shape(&mut self, path: &Path, fill: Color) {
		let offset = self.registration;

		if let Some(texture) = self.texture(fill) {
			let _ = write!(
				self.body,
				r#"<path d="{}" fill="url(#{texture})" transform="translate({:.1} {:.1})"/>"#,
				path.as_str(),
				offset.x,
				offset.y
			);
		}

		if self.style.pooling > 0.0 {
			// Watercolour pigment gathers at the edge of a wash: a soft dark rim inside the shape.
			let clip = self.next_id("pool");
			self.define(&format!(
				r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
				path.as_str()
			));
			let width = self.line * 3.0;
			let pooling = self.style.pooling;
			self.raw(&format!(
				r#"<g clip-path="url(#{clip})" transform="translate({:.1} {:.1})">"#,
				offset.x, offset.y
			));
			self.plain_stroke(path, fill.darken(0.35), width, pooling);
			self.raw("</g>");
		}
	}

	/// The id of the style's surface texture, defining it on first use.
	fn texture(&mut self, _fill: Color) -> Option<String> {
		let texture = self.style.texture;

		if texture == Texture::None {
			return None;
		}

		let id = match texture {
			Texture::Paper => "tex-paper",
			Texture::Hatch => "tex-hatch",
			Texture::Mottle => "tex-mottle",
			Texture::None => return None,
		};

		if !self.textures.contains(&texture) {
			self.textures.push(texture);
			let markup = texture_pattern(texture, id);
			self.defs.push_str(&markup);
		}

		Some(id.to_owned())
	}

	/// Inserts raw SVG markup (text, images, gradients in use).
	pub fn raw(&mut self, markup: &str) {
		self.body.push_str(markup);
	}

	/// Adds a definition (gradient, pattern, clip path) to `<defs>`.
	pub fn define(&mut self, markup: &str) {
		self.defs.push_str(markup);
	}

	/// Finishes the document with the given view box and output pixel size.
	pub fn finish(self, view: (f32, f32, f32, f32), width: u32, height: u32) -> String {
		let (x, y, w, h) = view;
		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="{x:.2} {y:.2} {w:.2} {h:.2}"><defs>{}</defs>{}</svg>"#,
			self.defs, self.body
		)
	}
}

/// Escapes text for inclusion in SVG or XHTML.
pub fn escape(text: &str) -> String {
	let mut out = String::with_capacity(text.len());

	for ch in text.chars() {
		match ch {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			_ => out.push(ch),
		}
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_register_slips_the_same_way_on_the_page_however_a_part_is_turned() {
		let mut svg = Svg::new();
		svg.begin("translate(300 200) scale(0.5)");
		let line = dials::Line {
			register: pt(7.0, 5.0),
			..dials::Line::default()
		};
		let _ = svg.use_brush(Brush::new(line, &dials::Shade::default(), palette::INK));

		for transform in [
			"rotate(37)",
			"scale(-1 1) translate(0 -40) rotate(-90)",
			"translate(4 9) rotate(120) scale(1.6 0.7)",
		] {
			svg.begin(transform);
			let slip = svg.frame().apply(svg.register_here());
			let page = Linear::of_transform("scale(0.5)").apply(pt(7.0, 5.0));
			assert!(
				(slip - page).len() < 1e-3,
				"{transform}: {slip:?} vs {page:?}"
			);
			svg.end();
		}
	}

	#[test]
	fn noise_is_deterministic_and_bounded() {
		for i in 0..1000 {
			let value = noise(42, i);
			assert!((-1.0..=1.0).contains(&value), "{value} out of range");
			assert!((value - noise(42, i)).abs() < f32::EPSILON);
		}
	}

	#[test]
	fn noise_differs_between_seeds() {
		let a: Vec<f32> = (0..16).map(|i| noise(1, i)).collect();
		let b: Vec<f32> = (0..16).map(|i| noise(2, i)).collect();
		assert_ne!(a, b);
	}

	#[test]
	fn blob_needs_three_points() {
		assert_eq!(Path::blob(&[pt(0.0, 0.0), pt(1.0, 1.0)]).as_str(), "");
		assert!(
			Path::blob(&[pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)])
				.as_str()
				.ends_with('Z')
		);
	}

	#[test]
	fn the_brushed_finish_shades_the_far_side_whatever_the_style() {
		let square = Shape::parse(
			"M10 10 C40 10 70 10 90 10 C90 40 90 70 90 90 C70 90 40 90 10 90 C10 70 10 40 10 10 Z",
		);
		// The tv style draws no outlines; the brushed finish draws its own line regardless.
		let mut svg = Svg::styled(Style::named(style::StyleName::Tv));
		svg.fill(&Path::rect(0.0, 0.0, 100.0, 100.0), Color(0xFF_FF_FF));
		svg.brushed_shaded(
			&square,
			Color(0xFF_FF_FF),
			pt(-20.0, -20.0),
			Color(0xD4_BE_D8),
			palette::INK,
			4.0,
		);
		let pixmap =
			crate::raster::render(&svg.finish((0.0, 0.0, 100.0, 100.0), 100, 100)).unwrap();
		let at = |x: u32, y: u32| pixmap.pixel(x, y).unwrap();

		assert_eq!(at(30, 30).red(), 255, "the lit side keeps its colour");
		assert!(at(80, 80).red() < 240, "the far side is shaded");
		assert!(at(10, 50).red() < 100, "the line is drawn");
	}

	#[test]
	fn escape_handles_markup() {
		assert_eq!(
			escape(r#"<"Hi" & bye>"#),
			"&lt;&quot;Hi&quot; &amp; bye&gt;"
		);
	}
}
