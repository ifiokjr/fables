//! Animals and plants. In Kerfuffle even the flowers have faces, so they can react.

use super::Bounds;
use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::figure::star_path;
use super::lerp;
use super::noise;
use super::palette;
use super::pt;

/// Common state for the small cast: birds, dogs, the worm and flowers.
#[derive(Clone, Copy, Debug)]
pub struct CritterState {
	/// -1 faces left, 1 faces right.
	pub facing: f32,
	/// 0 = fine, 1 = keeled over (legs in the air, eyes crossed).
	pub knocked: f32,
	/// For flowers and trees: 0 = blooming, 1 = completely wilted.
	pub wilt: f32,
	/// 0 = resting, 1 = flying/wagging/excited.
	pub lively: f32,
	pub time: f32,
}

impl Default for CritterState {
	fn default() -> Self {
		Self {
			facing: 1.0,
			knocked: 0.0,
			wilt: 0.0,
			lively: 0.0,
			time: 0.0,
		}
	}
}

fn eye(svg: &mut Svg, c: Pt, r: f32, knocked: f32) {
	if knocked > 0.5 {
		let k = r * 0.9;
		svg.stroke(
			&Path::new().move_to(c + pt(-k, -k)).line_to(c + pt(k, k)),
			palette::INK,
			svg.line,
		);
		svg.stroke(
			&Path::new().move_to(c + pt(k, -k)).line_to(c + pt(-k, k)),
			palette::INK,
			svg.line,
		);
		return;
	}

	svg.fill(&Path::circle(c, r), palette::INK);
	svg.fill(
		&Path::circle(c + pt(-r * 0.3, -r * 0.35), r * 0.35),
		palette::WHITE,
	);
}

/// A round little Kerfuffle bird ("a tweetle"). Origin is the middle of its body. The
/// drawing has no separate head — its round body is its head — so the whole outline is
/// reported as the head bounds, and the return is `None`.
pub fn bird(svg: &mut Svg, color: Color, state: CritterState, seed: u64) -> Option<Bounds> {
	let r = 46.0;
	let flip = state.facing.signum();
	let spin = state.knocked * 180.0;
	let flap = if state.knocked < 0.5 {
		(state.time * 14.0).sin() * state.lively
	} else {
		0.0
	};

	svg.begin(&format!("rotate({spin:.1}) scale({flip} 1)"));

	if state.knocked < 0.5 && state.lively < 0.3 {
		for side in [-0.3, 0.25] {
			let leg = Path::new()
				.move_to(pt(side * r, r * 0.8))
				.line_to(pt(side * r, r * 1.25));
			svg.stroke(&leg, palette::INK, svg.line);
		}
	} else if state.knocked >= 0.5 {
		for side in [-0.3, 0.25] {
			let leg = Path::new()
				.move_to(pt(side * r, r * 0.8))
				.line_to(pt(side * r + 4.0, r * 1.35))
				.line_to(pt(side * r + 16.0, r * 1.42));
			svg.stroke(&leg, palette::INK, svg.line);
		}
	}

	let tail = Path::new()
		.move_to(pt(-r * 0.8, -r * 0.1))
		.line_to(pt(-r * 1.55, -r * 0.55))
		.line_to(pt(-r * 1.45, r * 0.25))
		.close();
	svg.inked(&tail, color.darken(0.15));

	let body = Path::wobbly_ellipse(pt(0.0, 0.0), r * 1.05, r * 0.92, 0.05, svg.seed(seed));
	svg.inked(&body, color);

	let belly = Path::wobbly_ellipse(
		pt(r * 0.15, r * 0.35),
		r * 0.6,
		r * 0.45,
		0.05,
		svg.seed(seed + 1),
	);
	svg.fill(&belly, palette::CREAM);

	let wing_angle = -20.0 - flap * 55.0;
	svg.begin(&format!(
		"translate({:.1} {:.1}) rotate({wing_angle:.1})",
		-r * 0.15,
		-r * 0.05
	));
	let wing = Path::wobbly_ellipse(
		pt(-r * 0.35, 0.0),
		r * 0.62,
		r * 0.35,
		0.06,
		svg.seed(seed + 2),
	);
	svg.inked(&wing, color.darken(0.2));
	svg.end();

	let beak = Path::new()
		.move_to(pt(r * 0.88, -r * 0.3))
		.line_to(pt(r * 1.45, -r * 0.08))
		.line_to(pt(r * 0.88, r * 0.12))
		.close();
	svg.inked(&beak, palette::SUNFLOWER);
	eye(svg, pt(r * 0.42, -r * 0.35), r * 0.15, state.knocked);

	if state.knocked >= 0.5 {
		let tongue = Path::wobbly_ellipse(pt(r * 1.2, r * 0.2), 8.0, 12.0, 0.1, seed + 3);
		svg.inked(&tongue, palette::FLAMINGO);
	}

	svg.end();
	None
}

/// Biscuit, Kerfuffle's sausage dog. Origin is between its feet. Returns the bounds of its
/// head — the skull, snout, ear and eye — in the document's units, for whoever is measuring.
pub fn dog(svg: &mut Svg, color: Color, state: CritterState, seed: u64) -> Option<Bounds> {
	let flip = state.facing.signum();
	let body_y = -78.0;

	svg.begin(&format!(
		"translate(0 {:.1}) rotate({:.1}) scale({flip} 1)",
		-state.knocked * 40.0,
		state.knocked * 180.0
	));

	let wag = (state.time * (6.0 + state.lively * 14.0)).sin() * (10.0 + state.lively * 25.0);
	let tail = Path::new().move_to(pt(-120.0, body_y - 10.0)).quad_to(
		pt(-170.0, body_y - 30.0),
		pt(-185.0 + wag * 0.2, body_y - 80.0 + wag.abs() * 0.2),
	);
	svg.inked_noodle(&tail, color, 16.0);

	for x in [-95.0, -55.0, 55.0, 95.0] {
		let leg = Path::new()
			.move_to(pt(x, body_y + 20.0))
			.line_to(pt(x, -8.0));
		svg.inked_noodle(&leg, color.darken(0.08), 22.0);
	}

	let body = Path::wobbly_poly(
		&[
			pt(-140.0, body_y - 40.0),
			pt(120.0, body_y - 40.0),
			pt(125.0, body_y + 38.0),
			pt(-140.0, body_y + 38.0),
		],
		38.0,
		2.0,
		svg.seed(seed),
	);
	svg.inked(&body, color);

	let collar = Path::new()
		.move_to(pt(98.0, body_y - 44.0))
		.line_to(pt(104.0, body_y + 34.0));
	svg.inked_noodle(&collar, palette::TOMATO, 16.0);
	svg.inked(
		&Path::circle(pt(112.0, body_y + 40.0), 11.0),
		palette::SUNFLOWER,
	);

	svg.begin_measure();
	let head_c = pt(160.0, body_y - 62.0);
	let head = Path::wobbly_ellipse(head_c, 66.0, 54.0, 0.04, svg.seed(seed + 1));
	svg.inked(&head, color);
	let snout = Path::wobbly_ellipse(
		head_c + pt(60.0, 18.0),
		38.0,
		28.0,
		0.05,
		svg.seed(seed + 2),
	);
	svg.inked(&snout, color.lighten(0.3));
	svg.fill(
		&Path::ellipse(head_c + pt(92.0, 8.0), 13.0, 10.0),
		palette::INK,
	);

	let ear = Path::wobbly_poly(
		&[
			head_c + pt(-38.0, -30.0),
			head_c + pt(-10.0, -40.0),
			head_c + pt(-20.0, 50.0 - state.lively * 30.0),
			head_c + pt(-58.0, 40.0),
		],
		16.0,
		2.0,
		svg.seed(seed + 3),
	);
	svg.inked(&ear, color.darken(0.25));
	eye(svg, head_c + pt(22.0, -14.0), 9.0, state.knocked);

	if state.knocked >= 0.5 {
		let tongue = Path::wobbly_ellipse(head_c + pt(70.0, 56.0), 11.0, 20.0, 0.08, seed + 4);
		svg.inked(&tongue, palette::FLAMINGO);
	} else {
		let smile = Path::new()
			.move_to(head_c + pt(48.0, 40.0))
			.quad_to(head_c + pt(70.0, 54.0), head_c + pt(88.0, 38.0));
		svg.stroke(&smile, palette::INK, svg.line);
	}

	let head = svg.end_measure();
	svg.end();
	head
}

/// Wormsley, the worm in the bowler hat who hides on every page. Origin is his hole.
/// Returns the bounds of his head — face, hat and all — for whoever is measuring.
pub fn worm(svg: &mut Svg, state: CritterState, seed: u64) -> Option<Bounds> {
	let hole = Path::wobbly_ellipse(pt(0.0, 0.0), 46.0, 14.0, 0.08, svg.seed(seed));
	svg.fill(&hole, palette::INK);

	let peek = 1.0 - state.knocked * 0.6;
	let sway = (state.time * 2.0).sin() * 8.0;
	let top = pt(sway, -88.0 * peek);
	let body = Path::new()
		.move_to(pt(0.0, 4.0))
		.quad_to(pt(-18.0, -40.0 * peek), top);
	svg.inked_noodle(&body, palette::FLAMINGO, 30.0);

	svg.begin_measure();
	let face = top + pt(0.0, -10.0);
	let head = Path::wobbly_ellipse(face, 24.0, 22.0, 0.05, svg.seed(seed + 1));
	svg.inked(&head, palette::FLAMINGO);

	for side in [-1.0, 1.0] {
		svg.stroke(
			&Path::circle(face + pt(side * 9.0, -2.0), 8.0),
			palette::INK,
			svg.line * 0.6,
		);
		eye(svg, face + pt(side * 9.0, -2.0), 3.0, state.knocked);
	}

	let brim = Path::wobbly_ellipse(face + pt(0.0, -20.0), 30.0, 6.0, 0.05, svg.seed(seed + 2));
	let crown = Path::wobbly_poly(
		&[
			face + pt(-18.0, -22.0),
			face + pt(-15.0, -46.0),
			face + pt(15.0, -46.0),
			face + pt(18.0, -22.0),
		],
		12.0,
		1.0,
		svg.seed(seed + 3),
	);
	svg.inked(&crown, palette::INK);
	svg.inked(&brim, palette::INK);

	svg.end_measure()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowerKind {
	Daisy,
	Tulip,
	Sunflower,
}

/// A flower with a face. Origin is where the stem meets the ground. Returns the bounds of
/// its bloom — the head, for whoever is measuring.
pub fn flower(
	svg: &mut Svg,
	kind: FlowerKind,
	color: Color,
	height: f32,
	state: CritterState,
	seed: u64,
) -> Option<Bounds> {
	let wilt = state.wilt.clamp(0.0, 1.0);
	let sway = (state.time * 1.7 + seed as f32 * 0.37).sin() * 0.04 * (1.0 - wilt);
	let droop = wilt * 1.35 + sway;
	let lean = if noise(seed, 0) > 0.0 { 1.0 } else { -1.0 };

	let mid = pt(lean * droop * height * 0.18, -height * 0.55);
	let head = pt(
		lean * (droop * height * 0.62).min(height * 0.62),
		-height * (1.0 - droop * 0.48),
	);
	let stem = Path::new().move_to(pt(0.0, 0.0)).quad_to(mid, head);
	let green = palette::LEAF
		.mix(palette::MOSS, wilt)
		.mix(palette::COCOA, wilt * 0.4);
	svg.inked_noodle(&stem, green, 13.0);

	for side in [-1.0, 1.0] {
		let base = pt(0.0, -height * 0.28);
		let tip = base + pt(side * 60.0, -30.0 + wilt * 55.0);
		let leaf = Path::blob(&[
			base,
			base.lerp(tip, 0.5) + pt(0.0, -16.0),
			tip,
			base.lerp(tip, 0.5) + pt(0.0, 12.0),
		]);
		svg.inked(&leaf, green);
	}

	let petal_color = color.mix(palette::TOFFEE, wilt * 0.75);
	let r = 34.0 + height * 0.04;
	let tilt = lean * wilt * 110.0;

	svg.begin_measure();
	svg.begin(&format!(
		"translate({:.1} {:.1}) rotate({tilt:.1})",
		head.x, head.y
	));

	match kind {
		FlowerKind::Daisy | FlowerKind::Sunflower => {
			let petals = if kind == FlowerKind::Daisy { 8 } else { 12 };
			let length = if kind == FlowerKind::Daisy {
				r * 1.05
			} else {
				r * 0.95
			};

			for i in 0..petals {
				let angle = i as f32 / petals as f32 * 360.0;
				let droop_petal = wilt
					* 25.0 * if (angle - 90.0).abs() < 90.0 {
					1.0
				} else {
					-1.0
				};
				svg.begin(&format!("rotate({:.1})", angle + droop_petal));
				let petal = Path::wobbly_ellipse(
					pt(r * 0.9 + length * 0.35, 0.0),
					length * 0.55,
					r * 0.36,
					0.08,
					svg.seed(seed + i),
				);
				svg.inked(&petal, petal_color);
				svg.end();
			}

			let centre = if kind == FlowerKind::Daisy {
				palette::SUNFLOWER
			} else {
				palette::COCOA
			};
			svg.inked(
				&Path::circle(pt(0.0, 0.0), r * 0.92),
				centre.mix(palette::TOFFEE, wilt * 0.5),
			);
		}
		FlowerKind::Tulip => {
			let cup = Path::blob(&[
				pt(-r * 0.95, -r * 0.6),
				pt(-r * 0.45, -r * 0.1),
				pt(0.0, -r * 0.9),
				pt(r * 0.45, -r * 0.1),
				pt(r * 0.95, -r * 0.6),
				pt(r * 0.8, r * 0.7),
				pt(0.0, r * 0.95),
				pt(-r * 0.8, r * 0.7),
			]);
			svg.inked(&cup, petal_color);
		}
	}

	face(svg, r * 0.5, wilt, state.time);
	svg.end();
	let bloom = svg.end_measure();
	bloom
}

/// A tiny face for plants: smiling when well, frowning with X eyes when wilted.
fn face(svg: &mut Svg, size: f32, wilt: f32, time: f32) {
	let blink = if (time * 0.7).fract() < 0.05 {
		0.2
	} else {
		1.0
	};
	let knocked = if wilt > 0.6 { 1.0 } else { 0.0 };

	for side in [-1.0, 1.0] {
		let c = pt(side * size * 0.45, -size * 0.2);

		if knocked > 0.5 {
			eye(svg, c, size * 0.18, 1.0);
		} else {
			svg.fill(
				&Path::ellipse(c, size * 0.13, size * 0.17 * blink),
				palette::INK,
			);
		}
	}

	let curve = lerp(size * 0.35, -size * 0.25, wilt);
	let mouth = Path::new()
		.move_to(pt(-size * 0.35, size * 0.3))
		.quad_to(pt(0.0, size * 0.3 + curve), pt(size * 0.35, size * 0.3));
	svg.stroke(&mouth, palette::INK, svg.line * 0.8);
}

/// A lollipop tree. Origin is the bottom of the trunk. The tree has no head, so the whole
/// outline is its head bounds for framing, and the return is `None`.
pub fn tree(svg: &mut Svg, color: Color, height: f32, state: CritterState, seed: u64) -> Option<Bounds> {
	let wilt = state.wilt.clamp(0.0, 1.0);
	let trunk = Path::wobbly_poly(
		&[
			pt(-28.0, 0.0),
			pt(-18.0, -height * 0.62),
			pt(18.0, -height * 0.62),
			pt(28.0, 0.0),
		],
		10.0,
		3.0,
		svg.seed(seed),
	);
	svg.inked(&trunk, palette::COCOA);

	let crown_r = height * 0.36;
	let c = pt(0.0, -height * 0.72 + wilt * crown_r * 0.15);
	let leaves = color
		.mix(palette::MOSS, wilt)
		.mix(palette::TOFFEE, wilt * 0.5);
	let crown = Path::wobbly_ellipse(
		c,
		crown_r,
		crown_r * (0.95 - wilt * 0.12),
		0.1,
		svg.seed(seed + 1),
	);
	svg.inked(&crown, leaves);

	for i in 0..6 {
		let p = c + pt(
			noise(seed, 10 + i) * crown_r * 0.6,
			noise(seed, 20 + i) * crown_r * 0.6,
		);
		let mark = Path::new()
			.move_to(p)
			.quad_to(p + pt(14.0, -12.0), p + pt(28.0, 0.0));
		svg.stroke(&mark, leaves.darken(0.25), svg.line);
	}

	if wilt > 0.4 {
		for i in 0..5 {
			let fall = ((state.time * 0.3 + i as f32 * 0.21).fract()) * height * 0.7;
			let p = c + pt(noise(seed, 30 + i) * crown_r, crown_r * 0.6 + fall * wilt);
			let leaf = Path::wobbly_ellipse(p, 14.0, 8.0, 0.1, seed + 40 + i);
			svg.inked(&leaf, leaves);
		}
	}

	None
}

/// A starry knock-out burst, used when something keels over.
pub fn bonk(svg: &mut Svg, c: Pt, size: f32) {
	let burst = star_path(c, size, size * 0.55, 8);
	svg.inked(&burst, palette::SUNFLOWER);
}

/// A toothbrush standing in its cup, with eyes. It only ever looks hopeful. Origin is the
/// bottom of the cup. The whole buddy is its head for framing, and the return is `None`.
pub fn toothbrush_buddy(svg: &mut Svg, color: Color, state: CritterState, seed: u64) -> Option<Bounds> {
	let lean = (state.time * 1.5).sin() * 3.0 + state.lively * (state.time * 9.0).sin() * 6.0;

	svg.begin(&format!("rotate({lean:.1})"));
	let handle = Path::new().move_to(pt(0.0, -60.0)).line_to(pt(0.0, -420.0));
	svg.inked_noodle(&handle, color, 46.0);
	let head = Path::wobbly_poly(
		&[
			pt(-40.0, -560.0),
			pt(40.0, -560.0),
			pt(40.0, -420.0),
			pt(-40.0, -420.0),
		],
		14.0,
		2.0,
		svg.seed(seed),
	);
	svg.inked(&head, palette::WHITE);

	for i in 0..4 {
		let x = -26.0 + i as f32 * 17.0;
		let bristle = Path::new().move_to(pt(x, -560.0)).line_to(pt(x, -600.0));
		svg.stroke(&bristle, palette::SKY, 10.0);
	}

	let paste = Path::wobbly_ellipse(pt(0.0, -612.0), 46.0, 22.0, 0.1, svg.seed(seed + 1));
	svg.inked(&paste, palette::MINT);

	for side in [-1.0, 1.0] {
		let c = pt(side * 13.0, -330.0);
		svg.inked(&Path::ellipse(c, 15.0, 20.0), palette::WHITE);
		svg.fill(
			&Path::circle(c + pt(state.facing * 4.0, -2.0), 8.0),
			palette::INK,
		);
		let brow = Path::new()
			.move_to(c + pt(-side * 14.0, -26.0))
			.line_to(c + pt(side * 12.0, -34.0));
		svg.stroke(&brow, palette::INK, svg.line);
	}

	svg.end();

	let cup = Path::wobbly_poly(
		&[
			pt(-90.0, 0.0),
			pt(-104.0, -230.0),
			pt(104.0, -230.0),
			pt(90.0, 0.0),
		],
		18.0,
		3.0,
		svg.seed(seed + 2),
	);
	svg.inked(&cup, palette::BUBBLEGUM);
	let stripe = Path::rect(-100.0, -150.0, 200.0, 30.0);
	svg.fill(&stripe, palette::WHITE);

	None
}
