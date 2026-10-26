//! Effects: the visible consequences that make Kerfuffle's habits show on the outside, and the
//! manga page's own effects (focus lines and impact bursts).
//!
//! Every effect takes a `time` in seconds so the same call draws a still for the book and a
//! moving frame for video.

use std::f32::consts::TAU;

use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::noise;
use super::palette;
use super::pt;

/// Four-point twinkle.
pub fn sparkle(svg: &mut Svg, c: Pt, size: f32, color: Color, time: f32) {
	let pulse = 0.8 + (time * 5.0 + c.x * 0.01).sin() * 0.2;
	let s = size * pulse;
	let k = s * 0.22;
	let star = Path::new()
		.move_to(c + pt(0.0, -s))
		.quad_to(c + pt(k, -k), c + pt(s, 0.0))
		.quad_to(c + pt(k, k), c + pt(0.0, s))
		.quad_to(c + pt(-k, k), c + pt(-s, 0.0))
		.quad_to(c + pt(-k, -k), c + pt(0.0, -s))
		.close();
	svg.fill(&star, color);
	svg.stroke(&star, palette::INK, svg.line * 0.45);
}

/// Little stars circling a knocked-out head.
pub fn dizzy_stars(svg: &mut Svg, c: Pt, r: f32, time: f32) {
	for i in 0..3 {
		let angle = time * 2.6 + i as f32 * TAU / 3.0;
		let p = c + pt(angle.cos() * r * 0.75, angle.sin() * r * 0.22);
		let star = super::figure::star_path(p, r * 0.16, r * 0.07, 5);
		svg.inked(&star, palette::SUNFLOWER);
	}
}

/// The billowing shape shared by stinky and minty breath: puffs that grow and zigzag away from
/// the mouth. Sharing the shape is deliberate; the book's turn is the same cloud changing colour.
fn breath_puffs(origin: Pt, dir: f32, unit: f32, time: f32, seed: u64) -> Vec<(Pt, f32)> {
	let count = 7;

	(0..count)
		.map(|i| {
			let t = i as f32 / (count - 1) as f32;
			let zig = if i % 2 == 0 { -1.0 } else { 1.0 } * unit * 0.32 * t;
			let drift = (time * 1.1 + i as f32 * 0.9).sin() * unit * 0.1;
			let along = unit * 0.7 + t * unit * 5.0 + (time * 0.6 + i as f32).sin() * unit * 0.08;
			let c = origin + pt(dir * along, -t * t * unit * 1.5 + zig + drift);
			let r = unit * (0.42 + t * 0.95) * (0.9 + noise(seed, 50 + i as u64).abs() * 0.25);
			(c, r)
		})
		.collect()
}

/// Draws overlapping puffs as one merged cloud: every outline first, then every fill on top,
/// so only the outer silhouette keeps its ink line.
fn merged_cloud(svg: &mut Svg, puffs: &[(Pt, f32)], fill: Color, opacity: f32, seed: u64) {
	let shapes: Vec<Path> = puffs
		.iter()
		.enumerate()
		.map(|(i, (c, r))| Path::wobbly_ellipse(*c, *r, *r * 0.84, 0.1, svg.seed(seed + i as u64)))
		.collect();
	let offset = svg.registration;
	let line = svg.line;

	svg.begin_opacity(opacity);

	for shape in &shapes {
		svg.stroke(shape, palette::INK, line * 2.0);
	}

	svg.begin(&format!("translate({:.1} {:.1})", offset.x, offset.y));

	for shape in &shapes {
		svg.fill(shape, fill);
	}

	svg.end();
	svg.end();
}

fn swirl(svg: &mut Svg, c: Pt, r: f32, spin: f32, color: Color, width: f32) {
	let points: Vec<Pt> = (0..10)
		.map(|k| {
			let t = k as f32 / 9.0;
			let angle = spin + t * 4.8;
			c + pt(angle.cos(), angle.sin()) * (r * t)
		})
		.collect();
	svg.stroke(&Path::spline(&points), color, width);
}

/// A drifting green breath cloud with flies and stink lines.
///
/// `amount` scales the whole effect (0 hides it). `dir` is +1 for rightwards, -1 for leftwards.
pub fn stink_cloud(svg: &mut Svg, origin: Pt, dir: f32, amount: f32, time: f32, seed: u64) {
	if amount <= 0.01 {
		return;
	}

	let unit = 70.0 * amount;
	let puffs = breath_puffs(origin, dir, unit, time, seed);
	merged_cloud(svg, &puffs, palette::MOSS, 0.96, seed);

	for (i, (c, r)) in puffs.iter().enumerate().skip(2).step_by(2) {
		swirl(
			svg,
			*c,
			r * 0.5,
			time * 0.8 * dir + i as f32,
			palette::BOG,
			svg.line * 0.9,
		);
	}

	for i in 0..3 {
		let (c, r) = puffs[puffs.len() - 1 - i * 2];
		let base = c + pt(noise(seed, 20 + i as u64) * r * 0.3, -r * 0.95);
		let wiggle = (time * 4.0 + i as f32).sin() * unit * 0.12;
		let line = Path::spline(&[
			base,
			base + pt(unit * 0.18 + wiggle, -unit * 0.45),
			base + pt(-unit * 0.18 - wiggle, -unit * 0.9),
			base + pt(unit * 0.12, -unit * 1.3),
		]);
		svg.stroke(&line, palette::BOG, svg.line * 1.1);
	}

	let (tip, tip_r) = puffs[puffs.len() - 1];

	for i in 0..4 {
		let angle = time * (3.0 + i as f32 * 0.7) + i as f32 * 1.7;
		let p = tip + pt(angle.cos() * tip_r * 1.25, angle.sin() * tip_r * 0.9);
		fly(svg, p, 7.0 + amount * 3.0, time + i as f32);
	}
}

/// A buzzing fly.
pub fn fly(svg: &mut Svg, c: Pt, size: f32, time: f32) {
	let flap = (time * 40.0).sin() * 0.3 + 1.0;
	let wings = Path::ellipse(
		c + pt(-size * 0.6, -size * 0.9),
		size * 0.6,
		size * 0.9 * flap,
	)
	.append(&Path::ellipse(
		c + pt(size * 0.6, -size * 0.9),
		size * 0.6,
		size * 0.9 * flap,
	));
	svg.fill_alpha(&wings, palette::WHITE, 0.9);
	svg.stroke(&wings, palette::INK, svg.line * 0.4);
	svg.fill(&Path::ellipse(c, size * 0.75, size * 0.62), palette::INK);
}

/// The cool, swirling, sparkling mist of freshly brushed teeth: the stink cloud's shape,
/// reborn in ice and mint.
pub fn mint_mist(svg: &mut Svg, origin: Pt, dir: f32, amount: f32, time: f32, seed: u64) {
	if amount <= 0.01 {
		return;
	}

	let unit = 70.0 * amount;
	let puffs = breath_puffs(origin, dir, unit, time, seed);
	merged_cloud(svg, &puffs, palette::ICE, 0.88, seed);

	for (i, (c, r)) in puffs.iter().enumerate().skip(1) {
		swirl(
			svg,
			*c,
			r * 0.6,
			-time * 1.2 * dir + i as f32 * 1.3,
			palette::MINT,
			svg.line * 1.6,
		);
	}

	for ribbon in 0..2 {
		let phase = time * 1.6 + ribbon as f32 * 2.1;
		let points: Vec<Pt> = (0..9)
			.map(|k| {
				let t = k as f32 / 8.0;
				let wave = (phase + t * 7.0).sin() * unit * 0.5 * t;
				origin
					+ pt(
						dir * t * unit * 6.4,
						wave - t * t * unit * 1.6 + (ribbon as f32 - 0.5) * unit * 0.9 * t,
					)
			})
			.collect();
		svg.stroke_alpha(&Path::spline(&points), palette::WHITE, unit * 0.16, 0.9);
	}

	for i in 0..14 {
		let t = (i as f32 / 14.0 + time * 0.12).fract();
		let spread = noise(seed, i) * unit * 2.2;
		let p = origin
			+ pt(
				dir * (unit * 0.6 + t * unit * 6.4),
				spread * t - t * t * unit * 1.4,
			);
		let size = unit * (0.16 + 0.22 * (1.0 - t)) * (0.7 + noise(seed, 100 + i).abs() * 0.6);
		let color = if i % 3 == 0 {
			palette::MINT
		} else {
			palette::WHITE
		};
		sparkle(svg, p, size, color, time + i as f32);
	}
}

/// A soft green haze over a whole scene while someone stinky is about.
pub fn haze(svg: &mut Svg, amount: f32, color: Color, bounds: (f32, f32, f32, f32)) {
	if amount <= 0.01 {
		return;
	}

	let (x, y, w, h) = bounds;
	svg.fill_alpha(&Path::rect(x, y, w, h), color, amount * 0.22);
}

/// Floating "Z"s above a sleeper.
pub fn snooze(svg: &mut Svg, c: Pt, size: f32, time: f32) {
	for i in 0..3 {
		let t = (time * 0.4 + i as f32 / 3.0).fract();
		let p = c + pt(t * size * 1.6, -t * size * 3.0);
		let s = size * (0.4 + t * 0.6);
		let z = Path::new()
			.move_to(p + pt(-s * 0.5, -s * 0.5))
			.line_to(p + pt(s * 0.5, -s * 0.5))
			.line_to(p + pt(-s * 0.5, s * 0.5))
			.line_to(p + pt(s * 0.5, s * 0.5));
		svg.stroke(&z, palette::INK, svg.line * 1.2);
	}
}

/// Little hearts that float up from an admirer.
pub fn hearts(svg: &mut Svg, c: Pt, size: f32, time: f32) {
	for i in 0..3 {
		let t = (time * 0.5 + i as f32 / 3.0).fract();
		let p = c + pt((t * 7.0 + i as f32).sin() * size * 0.6, -t * size * 3.0);
		let s = size * (0.5 + t * 0.4);
		let heart = Path::new()
			.move_to(p + pt(0.0, s * 0.9))
			.cubic_to(
				p + pt(-s * 1.4, -s * 0.1),
				p + pt(-s * 0.6, -s * 1.1),
				p + pt(0.0, -s * 0.35),
			)
			.cubic_to(
				p + pt(s * 0.6, -s * 1.1),
				p + pt(s * 1.4, -s * 0.1),
				p + pt(0.0, s * 0.9),
			)
			.close();
		svg.inked(&heart, palette::BUBBLEGUM);
	}
}

/// Speed lines behind something on the move.
pub fn whoosh(svg: &mut Svg, c: Pt, dir: f32, size: f32) {
	for i in 0..3 {
		let y = c.y + (i as f32 - 1.0) * size * 0.45;
		let start = pt(c.x - dir * size * 0.6, y);
		let end = pt(c.x - dir * size * (1.4 + i as f32 * 0.25), y);
		svg.stroke(
			&Path::new().move_to(start).line_to(end),
			palette::INK,
			svg.line,
		);
	}
}

/// Manga focus lines: thin wedges rushing in from the edges of the page towards `focus`,
/// leaving a clear space round it. `strength` (0..1) sets how many there are and how far in
/// they reach.
pub fn focus_lines(svg: &mut Svg, focus: Pt, extent: f32, strength: f32, seed: u64, time: f32) {
	if strength <= 0.0 {
		return;
	}

	let count = (60.0 + 90.0 * strength) as u64;
	// Lines flicker between drawings in video, like a hand redrawing them.
	let frame = (time * 12.0) as u64;

	for i in 0..count {
		let jitter = noise(seed ^ frame, i);
		let angle = (i as f32 + jitter * 0.8) / count as f32 * TAU;
		let direction = pt(angle.cos(), angle.sin());
		let clear =
			extent * (0.32 + 0.14 * noise(seed ^ frame, i + 999).abs()) * (1.2 - strength * 0.4);
		let width = extent * (0.006 + 0.01 * noise(seed, i + 77).abs());
		let tip = focus + direction * clear;
		let base = focus + direction * extent * 1.5;
		let side = direction.perp() * width;
		svg.fill(
			&Path::new()
				.move_to(tip)
				.line_to(base + side)
				.line_to(base - side)
				.close(),
			palette::INK,
		);
	}
}

/// A manga impact burst: a jagged white star with an inked edge, behind the action.
pub fn impact_burst(svg: &mut Svg, focus: Pt, size: f32, strength: f32, seed: u64) {
	if strength <= 0.0 {
		return;
	}

	let spikes = 18;
	let points: Vec<Pt> = (0..spikes * 2)
		.map(|i| {
			let angle = i as f32 / (spikes * 2) as f32 * TAU;
			let reach = if i % 2 == 0 {
				size * strength * (1.0 + 0.35 * noise(seed, i as u64).abs())
			} else {
				size * strength * 0.55
			};
			focus + pt(angle.cos(), angle.sin()) * reach
		})
		.collect();
	let mut burst = Path::new().move_to(points[0]);

	for point in &points[1..] {
		burst = burst.line_to(*point);
	}

	let burst = burst.close();
	svg.fill(&burst, palette::WHITE);
	svg.stroke(&burst, palette::INK, svg.line * 1.2);
}
