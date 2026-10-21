//! Authored shapes: drawings kept as cubic Bézier points, so they can be blended and warped.
//!
//! [`Path`] is write-only SVG text; a [`Shape`] keeps its points. That is what cut-out rigs in
//! Moho and Toon Boom rely on: two drawings of the same part with the same commands (a front
//! view and a three-quarter view) blend point by point, so a head turns without a separate
//! drawing for every angle, and every point can be pushed through a warp (wrapping a face
//! round a head) before it is drawn.

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
			points.len() >= 4 && (points.len() - 1) % 3 == 0,
			"a shape is a start point plus whole curves: {data:?}"
		);

		Shape { points, closed }
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
