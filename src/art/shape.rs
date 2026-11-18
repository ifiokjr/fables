//! Authored shapes: drawings kept as cubic Bézier points, so they can be blended and warped.
//!
//! [`Path`] is write-only SVG text; a [`Shape`] keeps its points. That is what cut-out rigs in
//! Moho and Toon Boom rely on: two drawings of the same part with the same commands (a front
//! view and a three-quarter view) blend point by point, so a head turns without a separate
//! drawing for every angle, and every point can be pushed through a warp (wrapping a face
//! round a head) before it is drawn.

use std::fmt::Write as _;

use super::Path;
use super::Pt;
use super::lerp;
use super::pt;

/// A closed or open run of cubic Béziers: a start point followed by `(c1, c2, end)` triples.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
	points: Vec<Pt>,
	closed: bool,
}

impl Shape {
	/// Parses authored path data written with absolute `M`, `C` and `Z` commands, the way a
	/// drawing tool exports a path. The data is part of the program, so malformed data is a bug
	/// and panics with the offending text.
	pub fn parse(data: &str) -> Shape {
		let mut points = Vec::new();
		let mut closed = false;
		let mut numbers: Vec<f32> = Vec::new();
		let mut command = ' ';
		let flush = |command: char, numbers: &mut Vec<f32>, points: &mut Vec<Pt>| {
			let expected = match command {
				'M' => 2,
				'C' => 6,
				' ' if numbers.is_empty() => 0,
				_ => panic!("unsupported path command {command:?} in {data:?}"),
			};
			assert!(
				numbers.len() == expected,
				"{command} takes {expected} numbers, got {} in {data:?}",
				numbers.len()
			);

			for pair in numbers.chunks(2) {
				points.push(pt(pair[0], pair[1]));
			}

			numbers.clear();
		};

		let mut spaced = String::with_capacity(data.len() * 2);

		for ch in data.chars() {
			// Every command letter becomes its own token; `e` is an exponent, not a command.
			if ch.is_ascii_alphabetic() && !matches!(ch, 'e' | 'E') {
				spaced.extend([' ', ch, ' ']);
			} else {
				spaced.push(ch);
			}
		}

		for token in spaced
			.split(|ch: char| ch.is_whitespace() || ch == ',')
			.filter(|token| !token.is_empty())
		{
			match token {
				"M" | "C" => {
					flush(command, &mut numbers, &mut points);
					command = token.chars().next().unwrap_or(' ');
				}
				"Z" => {
					flush(command, &mut numbers, &mut points);
					command = ' ';
					closed = true;
				}
				letter if letter.starts_with(|ch: char| ch.is_ascii_alphabetic()) => {
					panic!("unsupported path command {letter:?} in {data:?}");
				}
				number => {
					numbers.push(
						number
							.parse()
							.unwrap_or_else(|_| panic!("bad number {number:?} in {data:?}")),
					);
				}
			}
		}

		flush(command, &mut numbers, &mut points);
		assert!(
			points.len() >= 4 && (points.len() - 1).is_multiple_of(3),
			"a shape is a start point plus whole curves: {data:?}"
		);

		Shape { points, closed }
	}

	/// A shape from a start point and whole curves (`1 + 3n` points, `n >= 1`), or `None` if
	/// the count does not make whole curves.
	pub fn from_points(points: Vec<Pt>, closed: bool) -> Option<Shape> {
		(points.len() >= 4 && (points.len() - 1).is_multiple_of(3))
			.then_some(Shape { points, closed })
	}

	/// The start point followed by each curve's two controls and end point.
	pub fn points(&self) -> &[Pt] {
		&self.points
	}

	pub fn is_closed(&self) -> bool {
		self.closed
	}

	/// How many curves the drawing has: two drawings blend only when these match.
	pub fn curves(&self) -> usize {
		(self.points.len() - 1) / 3
	}

	/// Whether `other` has the same commands, so the two can blend point by point.
	pub fn blends_with(&self, other: &Shape) -> bool {
		self.points.len() == other.points.len() && self.closed == other.closed
	}

	/// The smallest box holding every point, controls included: `(min, max)`.
	pub fn bounds(&self) -> (Pt, Pt) {
		self.points.iter().fold(
			(pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN)),
			|(lo, hi), p| {
				(
					pt(lo.x.min(p.x), lo.y.min(p.y)),
					pt(hi.x.max(p.x), hi.y.max(p.y)),
				)
			},
		)
	}

	/// SVG path data with every number written exactly, so a drawing saved to a file and read
	/// back is the same drawing. ([`Shape::path`] rounds to one decimal place for rendering.)
	pub fn exact_data(&self) -> String {
		let mut data = format!("M{} {}", self.points[0].x, self.points[0].y);

		for curve in self.points[1..].chunks(3) {
			let _ = write!(
				data,
				" C{} {} {} {} {} {}",
				curve[0].x, curve[0].y, curve[1].x, curve[1].y, curve[2].x, curve[2].y
			);
		}

		if self.closed {
			data.push_str(" Z");
		}

		data
	}

	/// An ellipse as four curves, so it can be warped and blended like authored parts.
	pub fn ellipse(c: Pt, rx: f32, ry: f32) -> Shape {
		let k = 0.552_284_8;
		let points = vec![
			pt(c.x + rx, c.y),
			pt(c.x + rx, c.y + ry * k),
			pt(c.x + rx * k, c.y + ry),
			pt(c.x, c.y + ry),
			pt(c.x - rx * k, c.y + ry),
			pt(c.x - rx, c.y + ry * k),
			pt(c.x - rx, c.y),
			pt(c.x - rx, c.y - ry * k),
			pt(c.x - rx * k, c.y - ry),
			pt(c.x, c.y - ry),
			pt(c.x + rx * k, c.y - ry),
			pt(c.x + rx, c.y - ry * k),
			pt(c.x + rx, c.y),
		];

		Shape {
			points,
			closed: true,
		}
	}

	/// A smooth closed curve through `points` (Catmull-Rom as cubic Béziers).
	pub fn blob(points: &[Pt]) -> Shape {
		let n = points.len();
		let mut out = vec![points[0]];

		for i in 0..n {
			let (p0, p1) = (points[(i + n - 1) % n], points[i]);
			let (p2, p3) = (points[(i + 1) % n], points[(i + 2) % n]);
			out.extend([
				p1 + (p2 - p0) * (1.0 / 6.0),
				p2 - (p3 - p1) * (1.0 / 6.0),
				p2,
			]);
		}

		Shape {
			points: out,
			closed: true,
		}
	}

	/// A limb: a smooth outline round a centre line, `start` wide at the root tapering to
	/// `end`, with round ends. Bent through a joint, it has no seam at the knee or elbow.
	pub fn taper(centre: &[Pt], start: f32, end: f32) -> Shape {
		let n = centre.len();
		let tangent = |i: usize| (centre[(i + 1).min(n - 1)] - centre[i.saturating_sub(1)]).norm();
		let half = |i: usize| lerp(start, end, i as f32 / (n - 1) as f32) / 2.0;
		let mut outline = Vec::with_capacity(n * 2 + 2);
		outline.push(centre[0] - tangent(0) * half(0));

		for (i, &p) in centre.iter().enumerate() {
			outline.push(p + tangent(i).perp() * half(i));
		}

		outline.push(centre[n - 1] + tangent(n - 1) * half(n - 1));

		for (i, &p) in centre.iter().enumerate().rev() {
			outline.push(p - tangent(i).perp() * half(i));
		}

		Shape::blob(&outline)
	}

	/// A limb drawn with anatomy round two bones, `root` to `joint` to `end`. `widths` are the
	/// limb's width at seven stations: the root, a third and two thirds of the way down the
	/// first bone, the joint, a third and two thirds of the way down the second bone, and the
	/// end. So a thigh can be fuller than the calf and a calf can swell and narrow to the ankle.
	///
	/// The two ends are round. Bent, the limb keeps one silhouette: the outside of the bend
	/// rounds over the joint, and the inside creases where the two bones' edges meet. Folded so
	/// far that one bone lies along the other, only their outside edges show. There is never a
	/// gap or a ball at the elbow or knee.
	pub fn limb(root: Pt, joint: Pt, end: Pt, widths: &[f32; 7]) -> Shape {
		let (first, second) = ((joint - root).norm(), (end - joint).norm());
		let half = |i: usize| widths[i] / 2.0;
		let station = |i: usize| {
			if i <= 3 {
				root.lerp(joint, i as f32 / 3.0)
			} else {
				joint.lerp(end, (i - 3) as f32 / 3.0)
			}
		};
		// How far the second bone turns from the first, positive towards the first's `perp`.
		let turn = (first.x * second.y - first.y * second.x)
			.atan2(first.x * second.x + first.y * second.y);
		let inside = if turn >= 0.0 { 1.0 } else { -1.0 };
		let edge = |side: f32| {
			let upper: Vec<Pt> = (0..=3)
				.map(|i| station(i) + first.perp() * (side * half(i)))
				.collect();
			let lower: Vec<Pt> = (3..=6)
				.map(|i| station(i) + second.perp() * (side * half(i)))
				.collect();
			(upper, lower)
		};

		// The outside of the bend: an arc over the joint joins the two bones' edges.
		let (upper, lower) = edge(-inside);
		let mut outside: Vec<(Pt, bool)> = upper[..3].iter().map(|&p| (p, false)).collect();
		let out = first.perp() * -inside;
		let from = out.y.atan2(out.x);
		// One point over a straight joint; more round a sharper bend.
		let steps = (turn.abs() / 0.5).ceil() as usize;

		for step in 0..=steps {
			let angle = from + turn * step as f32 / steps.max(1) as f32;
			outside.push((joint + pt(angle.cos(), angle.sin()) * half(3), false));
		}

		outside.extend(lower[1..].iter().map(|&p| (p, false)));

		// The inside of the bend: the two edges overlap, so they meet where they cross.
		let (upper, lower) = edge(inside);
		let crease = turn.abs() > 0.35;
		let mut inner: Vec<(Pt, bool)> = Vec::with_capacity(8);

		if let Some((a, b, at)) = crossing(&upper, &lower) {
			inner.extend(upper[..=a].iter().map(|&p| (p, false)));
			inner.push((at, crease));
			inner.extend(lower[b + 1..].iter().map(|&p| (p, false)));
		} else if turn.abs() < std::f32::consts::FRAC_PI_2 {
			// Nearly straight: the edges meet end to end at the joint.
			inner.extend(upper[..3].iter().map(|&p| (p, false)));
			inner.push((upper[3].lerp(lower[0], 0.5), false));
			inner.extend(lower[1..].iter().map(|&p| (p, false)));
		} else {
			// Folded flat: the inside edges are hidden inside the limb.
			inner.push((upper[0], false));
			inner.push((lower[3], false));
		}

		// Round the far end, back up the other side, and round the root.
		let (left, right) = if inside > 0.0 {
			(inner, outside)
		} else {
			(outside, inner)
		};
		let mut outline = left;
		outline.push((end + second * half(6), false));
		outline.extend(right.into_iter().rev());
		outline.push((root - first * half(0), false));

		Shape::smooth(&outline)
	}

	/// A smooth closed curve through `points`, with sharp corners where a point says so. Each
	/// curve's handles run along the curve's direction through the point, a third of the way to
	/// its neighbour, so unevenly spaced points don't overshoot.
	fn smooth(points: &[(Pt, bool)]) -> Shape {
		let n = points.len();
		let at = |i: usize| points[i % n].0;
		let direction = |i: usize| (at(i + 1) - at(i + n - 1)).norm();
		let mut out = vec![at(0)];

		for i in 0..n {
			let (from, to) = (at(i), at(i + 1));
			let reach = (to - from).len() / 3.0;
			let start = if points[i].1 {
				from.lerp(to, 1.0 / 3.0)
			} else {
				from + direction(i) * reach
			};
			let finish = if points[(i + 1) % n].1 {
				from.lerp(to, 2.0 / 3.0)
			} else {
				to - direction(i + 1) * reach
			};
			out.extend([start, finish, to]);
		}

		Shape {
			points: out,
			closed: true,
		}
	}

	/// Blends towards `other`, which must be drawn with the same commands.
	#[must_use]
	pub fn lerp(&self, other: &Shape, t: f32) -> Shape {
		assert_eq!(
			self.points.len(),
			other.points.len(),
			"only drawings with matching commands can blend"
		);

		Shape {
			points: self
				.points
				.iter()
				.zip(&other.points)
				.map(|(a, b)| a.lerp(*b, t))
				.collect(),
			closed: self.closed,
		}
	}

	/// Moves every point through `f` (offsets, rotations, warps).
	#[must_use]
	pub fn map(&self, f: impl Fn(Pt) -> Pt) -> Shape {
		Shape {
			points: self.points.iter().map(|&p| f(p)).collect(),
			closed: self.closed,
		}
	}

	#[must_use]
	pub fn shift(&self, by: Pt) -> Shape {
		self.map(|p| p + by)
	}

	pub fn path(&self) -> Path {
		let mut path = Path::new().move_to(self.points[0]);

		for curve in self.points[1..].chunks(3) {
			path = path.cubic_to(curve[0], curve[1], curve[2]);
		}

		if self.closed { path.close() } else { path }
	}

	/// The outline as a polygon (or a polyline, if the shape is open): the start point, then
	/// `steps` points along each curve, ending on its end point.
	pub fn polygon(&self, steps: usize) -> Vec<Pt> {
		let steps = steps.max(1);
		let mut points = vec![self.points[0]];

		for curve in self.points.windows(4).step_by(3) {
			for step in 1..=steps {
				let t = step as f32 / steps as f32;
				let u = 1.0 - t;
				points.push(
					curve[0] * (u * u * u)
						+ curve[1] * (3.0 * u * u * t)
						+ curve[2] * (3.0 * u * t * t)
						+ curve[3] * (t * t * t),
				);
			}
		}

		points
	}

	/// Whether `p` lies inside the outline. An open shape has no inside.
	pub fn contains(&self, p: Pt) -> bool {
		self.closed && inside(&self.polygon(MEASURE_STEPS), p)
	}

	/// How far `p` is from the shape: 0 inside it, otherwise the distance to its outline.
	pub fn distance(&self, p: Pt) -> f32 {
		let outline = self.polygon(MEASURE_STEPS);

		if self.closed && inside(&outline, p) {
			return 0.0;
		}

		edges(&outline, self.closed)
			.map(|(a, b)| to_segment(p, a, b))
			.fold(f32::MAX, f32::min)
	}

	/// How far apart two shapes are: 0 when they touch or overlap, otherwise the shortest
	/// distance between their outlines.
	pub fn gap(&self, other: &Shape) -> f32 {
		let (mine, theirs) = (self.polygon(MEASURE_STEPS), other.polygon(MEASURE_STEPS));
		let overlap = (other.closed && mine.iter().any(|&p| inside(&theirs, p)))
			|| (self.closed && theirs.iter().any(|&p| inside(&mine, p)));

		if overlap {
			return 0.0;
		}

		let nearest = |points: &[Pt], outline: &[Pt], closed: bool| {
			points
				.iter()
				.flat_map(|&p| edges(outline, closed).map(move |(a, b)| to_segment(p, a, b)))
				.fold(f32::MAX, f32::min)
		};

		nearest(&mine, &theirs, other.closed).min(nearest(&theirs, &mine, self.closed))
	}
}

/// How many points along each curve [`Shape`] measurements flatten it to: about a tenth of a
/// unit from the true curve for the parts a figure is drawn with.
const MEASURE_STEPS: usize = 8;

/// The polygon's edges, closing it back to the start if `closed`.
fn edges(points: &[Pt], closed: bool) -> impl Iterator<Item = (Pt, Pt)> + '_ {
	let back = closed.then(|| (points[points.len() - 1], points[0]));
	points.windows(2).map(|pair| (pair[0], pair[1])).chain(back)
}

/// Whether `p` is inside the closed polygon, by the even-odd rule.
fn inside(polygon: &[Pt], p: Pt) -> bool {
	edges(polygon, true)
		.filter(|&(a, b)| {
			(a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x)
		})
		.count()
		% 2 == 1
}

/// The distance from `p` to the segment from `a` to `b`.
fn to_segment(p: Pt, a: Pt, b: Pt) -> f32 {
	let along = b - a;
	let length = along.x * along.x + along.y * along.y;
	let t = if length > 0.0 {
		(((p - a).x * along.x + (p - a).y * along.y) / length).clamp(0.0, 1.0)
	} else {
		0.0
	};
	(p - (a + along * t)).len()
}

/// Where two edges first cross, searching outwards from the joint between them: `upper` runs
/// down to the joint and `lower` away from it. Returns the crossing segments' indices and the
/// point.
fn crossing(upper: &[Pt], lower: &[Pt]) -> Option<(usize, usize, Pt)> {
	let mut pairs: Vec<(usize, usize)> = (0..upper.len() - 1)
		.flat_map(|a| (0..lower.len() - 1).map(move |b| (a, b)))
		.collect();
	pairs.sort_by_key(|&(a, b)| (upper.len() - 2 - a) + b);

	pairs.into_iter().find_map(|(a, b)| {
		let (p, r) = (upper[a], upper[a + 1] - upper[a]);
		let (q, s) = (lower[b], lower[b + 1] - lower[b]);
		let cross = |u: Pt, v: Pt| u.x * v.y - u.y * v.x;
		let denominator = cross(r, s);

		if denominator.abs() < 1e-6 {
			return None;
		}

		let t = cross(q - p, s) / denominator;
		let u = cross(q - p, r) / denominator;
		((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| (a, b, p + r * t))
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_moves_and_curves() {
		let shape = Shape::parse("M0 0 C1 2 3 4 5 6 C7 8, 9 10, 0 0 Z");
		assert_eq!(shape.points.len(), 7);
		assert!(shape.closed);
		assert_eq!(shape.points[3], pt(5.0, 6.0));
		assert_eq!(
			shape.path().as_str(),
			"M0.0 0.0C1.0 2.0 3.0 4.0 5.0 6.0C7.0 8.0 9.0 10.0 0.0 0.0Z"
		);
	}

	#[test]
	fn exact_data_reads_back_as_the_same_drawing() {
		let shape = Shape::parse("M-0.25 3 C1.5 -2.125 3 4 5.75 6 Z");
		assert_eq!(Shape::parse(&shape.exact_data()), shape);
		assert_eq!(shape.curves(), 1);
		assert_eq!(shape.bounds(), (pt(-0.25, -2.125), pt(5.75, 6.0)));
	}

	#[test]
	fn whole_curves_only() {
		assert!(Shape::from_points(vec![pt(0.0, 0.0); 4], true).is_some());
		assert!(Shape::from_points(vec![pt(0.0, 0.0); 5], true).is_none());
		assert!(Shape::from_points(vec![pt(0.0, 0.0)], true).is_none());
	}

	#[test]
	#[should_panic(expected = "C takes 6 numbers")]
	fn rejects_short_curves() {
		let _ = Shape::parse("M0 0 C1 2 3 4 5");
	}

	#[test]
	#[should_panic(expected = "unsupported path command")]
	fn rejects_commands_it_cannot_blend() {
		let _ = Shape::parse("M0 0 L10 10 Z");
	}

	#[test]
	fn blending_meets_both_drawings() {
		let a = Shape::parse("M0 0 C0 0 10 0 10 0 Z");
		let b = Shape::parse("M0 10 C0 10 10 10 10 10 Z");
		assert_eq!(a.lerp(&b, 0.0), a);
		assert_eq!(a.lerp(&b, 1.0), b);
		assert_eq!(a.lerp(&b, 0.5).points[0], pt(0.0, 5.0));
	}

	#[test]
	#[should_panic(expected = "matching commands")]
	fn drawings_with_different_commands_do_not_blend() {
		let a = Shape::parse("M0 0 C0 0 10 0 10 0 Z");
		let _ = a.lerp(&Shape::ellipse(pt(0.0, 0.0), 1.0, 1.0), 0.5);
	}

	/// Whether `p` is inside the polygon through a shape's anchor points.
	fn inside(shape: &Shape, p: Pt) -> bool {
		let anchors: Vec<Pt> = shape.points.iter().step_by(3).copied().collect();
		let mut inside = false;

		for (i, &a) in anchors.iter().enumerate() {
			let b = anchors[(i + 1) % anchors.len()];

			if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
				inside = !inside;
			}
		}

		inside
	}

	/// How far `p` is from the segment `a`–`b`.
	fn distance(p: Pt, a: Pt, b: Pt) -> f32 {
		let ab = b - a;
		let t =
			(((p - a).x * ab.x + (p - a).y * ab.y) / (ab.x * ab.x + ab.y * ab.y)).clamp(0.0, 1.0);
		(p - (a + ab * t)).len()
	}

	const LEG: [f32; 7] = [40.0, 36.0, 28.0, 24.0, 30.0, 22.0, 16.0];

	#[test]
	fn a_limb_follows_its_width_profile() {
		let limb = Shape::limb(pt(0.0, 0.0), pt(0.0, 90.0), pt(0.0, 180.0), &LEG);
		assert!(limb.closed);
		// Straight down, the outline's half width at each station is the profile's.
		let width_at = |y: f32| {
			let near: Vec<f32> = limb
				.points
				.iter()
				.step_by(3)
				.filter(|p| (p.y - y).abs() < 0.5)
				.map(|p| p.x.abs())
				.collect();
			near.iter().copied().fold(0.0, f32::max) * 2.0
		};
		assert!((width_at(30.0) - 36.0).abs() < 0.01, "a full thigh");
		assert!((width_at(90.0) - 24.0).abs() < 0.01, "a narrow knee");
		assert!((width_at(120.0) - 30.0).abs() < 0.01, "the calf swells");
		assert!(
			(width_at(180.0) - 16.0).abs() < 0.01,
			"then narrows to the ankle"
		);
		// Round ends reach past the root and the end by half their width.
		let (lo, hi) = limb.bounds();
		assert!(lo.y < -19.0 && hi.y > 187.0, "{lo:?} {hi:?}");
	}

	#[test]
	fn a_bent_limb_is_one_continuous_outline_with_no_gap_or_ball_at_the_joint() {
		let (root, joint) = (pt(0.0, 0.0), pt(0.0, 90.0));

		// From straight, through a right angle, to folded flat back along itself.
		for degrees in [0.0_f32, 30.0, 90.0, 135.0, 170.0, -90.0] {
			let end = joint + pt(0.0, 90.0).rotate(degrees);
			let limb = Shape::limb(root, joint, end, &LEG);
			assert!(limb.closed && limb.curves() >= 8, "{degrees}°");

			// Every point of the bones is inside the outline: no gaps.
			for i in 1..20 {
				let s = i as f32 / 20.0;
				for p in [root.lerp(joint, s), joint.lerp(end, s)] {
					assert!(inside(&limb, p), "{degrees}°: {p:?} is outside");
				}
			}

			// No outline point strays further from the bones than the widest station: no
			// balloon over the joint, no slivers.
			for &p in limb.points.iter().step_by(3) {
				let off = distance(p, root, joint).min(distance(p, joint, end));
				assert!(off <= 20.01, "{degrees}°: {p:?} is {off} off the bones");
			}
		}
	}

	#[test]
	fn shapes_measure_what_is_inside_and_how_far_apart_they_are() {
		let disc = Shape::ellipse(pt(0.0, 0.0), 50.0, 50.0);
		assert!(disc.contains(pt(30.0, 30.0)) && !disc.contains(pt(40.0, 40.0)));
		assert!(disc.distance(pt(10.0, -5.0)).abs() < f32::EPSILON);
		assert!((disc.distance(pt(80.0, 0.0)) - 30.0).abs() < 0.2);

		// Apart, touching and overlapping.
		let other = |x: f32| Shape::ellipse(pt(x, 0.0), 20.0, 20.0);
		assert!((disc.gap(&other(100.0)) - 30.0).abs() < 0.2);
		assert!(disc.gap(&other(70.0)) < 0.2);
		assert!(disc.gap(&other(10.0)).abs() < f32::EPSILON);

		// An open line has no inside, but is measured to.
		let line = Shape::parse("M0 0 C0 10 0 20 0 30");
		assert!(!line.contains(pt(0.0, 15.0)));
		assert!((line.distance(pt(12.0, 15.0)) - 12.0).abs() < 0.01);
	}

	#[test]
	fn tapers_close_round_their_centre_line() {
		let limb = Shape::taper(&[pt(0.0, 0.0), pt(0.0, 50.0), pt(0.0, 100.0)], 20.0, 10.0);
		assert!(limb.closed);
		// The outline passes beyond both ends by half the width there (the round ends).
		let anchors = || limb.points.iter().step_by(3).map(|p| p.y);
		let top = anchors().fold(f32::MAX, f32::min);
		let bottom = anchors().fold(f32::MIN, f32::max);
		assert!((top + 10.0).abs() < 1.5 && (bottom - 105.0).abs() < 1.5);
	}
}
