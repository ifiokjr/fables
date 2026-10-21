//! Kerfuffle park: one setting with real depth.
//!
//! The scene is built back to front on the 2400-unit square stage the books use. Depth comes
//! from the painter's old tricks rather than detail: colours fade towards the sky colour as
//! they recede (aerial perspective), things overlap, the path narrows to a vanishing point,
//! sizes shrink up the page, and ink lines only appear in the foreground. Far shapes have no
//! line at all, the middle distance gets a thin tinted line, and only the nearest things (and
//! the characters) get the full dark ink.

use fables::art::Color;
use fables::art::Path;
use fables::art::Pt;
use fables::art::Svg;
use fables::art::noise;
use fables::art::pt;
use fables::art::shape::Shape;

pub const SIZE: f32 = 2400.0;
/// Where the land meets the sky.
pub const HORIZON: f32 = 1280.0;
/// Where the path vanishes.
const VANISH: Pt = pt(1380.0, 1330.0);

const SKY_TOP: Color = Color(0xB9_DC_DA);
const SKY_LOW: Color = Color(0xF6_EC_D6);
const CLOUD: Color = Color(0xFF_F8_EC);
const HILL_FAR: Color = Color(0x9F_C1_B6);
const HILL_MID: Color = Color(0x94_BE_8A);
const GRASS: Color = Color(0x83_B4_62);
const GRASS_NEAR: Color = Color(0x73_A6_55);
const LEAF: Color = Color(0x6E_A3_5E);
const LEAF_DARK: Color = Color(0x4F_7E_4A);
const BARK: Color = Color(0x8A_68_55);
const PATH: Color = Color(0xEE_DD_BA);
const PATH_EDGE: Color = Color(0xD6_BF_93);
const ROOF: Color = Color(0xD9_8C_74);
const WALL: Color = Color(0xF3_E3_C8);
const INK: Color = Color(0x2B_1D_2E);

/// Fades a colour into the haze of the sky by `depth` (0 near, 1 at the horizon).
fn haze(color: Color, depth: f32) -> Color {
	color.mix(SKY_LOW, depth * 0.62)
}

/// Builds a closed shape from a ridge line (left to right) down to `floor`.
fn ridge(points: &[Pt], floor: f32) -> Path {
	let mut path = Path::spline(points);
	let (first, last) = (points[0], points[points.len() - 1]);
	path = path
		.line_to(pt(last.x, floor))
		.line_to(pt(first.x, floor))
		.close();
	path
}

pub fn draw(c: &mut Svg) {
	sky(c);
	far_hills(c);
	mid_hills(c);
	ground(c);
	path(c);
	mid_trees(c);
	bench(c, pt(1900.0, 1610.0), 1.25);
	for (x, y, s) in [
		(1090.0, 1470.0, 0.45),
		(1180.0, 1420.0, 0.32),
		(1530.0, 1450.0, 0.4),
	] {
		bush(c, pt(x, y), s);
	}
	near_tree(c);
	tufts(c);
}

/// The foreground layer, drawn after the characters so the nearest flowers overlap their feet.
pub fn draw_front(c: &mut Svg) {
	flowers(c);
}

fn sky(c: &mut Svg) {
	let sky = c.next_id("sky");
	c.define(&format!(
		r#"<linearGradient id="{sky}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{SKY_TOP}"/><stop offset="0.55" stop-color="{}"/><stop offset="1" stop-color="{SKY_LOW}"/></linearGradient>"#,
		SKY_TOP.mix(SKY_LOW, 0.5)
	));
	c.fill_paint(&Path::rect(0.0, 0.0, SIZE, HORIZON + 40.0), &sky);

	// A low sun at the top left, where the light on every character comes from.
	let glow = c.next_id("sun");
	c.define(&format!(
		r#"<radialGradient id="{glow}"><stop offset="0" stop-color="{}" stop-opacity="0.9"/><stop offset="1" stop-color="{}" stop-opacity="0"/></radialGradient>"#,
		Color(0xFF_F6_DC),
		Color(0xFF_F6_DC)
	));
	c.fill_paint(&Path::circle(pt(420.0, 380.0), 520.0), &glow);

	for (x, y, s) in [
		(1650.0, 360.0, 1.0),
		(520.0, 860.0, 0.7),
		(2150.0, 900.0, 0.55),
	] {
		cloud(c, pt(x, y), s);
	}
}

fn cloud(c: &mut Svg, at: Pt, scale: f32) {
	let puffs = [
		(-170.0, 20.0, 90.0),
		(-70.0, -30.0, 120.0),
		(60.0, -50.0, 135.0),
		(170.0, 0.0, 95.0),
		(0.0, 30.0, 110.0),
	];
	let mut body = Path::new();

	for (x, y, r) in puffs {
		body = body.append(&Path::ellipse(
			at + pt(x, y) * scale,
			r * scale,
			r * scale * 0.82,
		));
	}

	c.fill_alpha(&body, CLOUD, 0.9);
	// A flat, slightly cooler base where the cloud is in its own shadow.
	c.begin_clip(&body);
	c.fill_alpha(
		&Path::rect(
			at.x - 400.0 * scale,
			at.y + 40.0 * scale,
			800.0 * scale,
			200.0 * scale,
		),
		Color(0xDD_E4_E8),
		0.8,
	);
	c.end();
}

fn far_hills(c: &mut Svg) {
	// The far town of Kerfuffle on its hill, nearly lost in the haze.
	let hill = ridge(
		&[
			pt(-40.0, 1130.0),
			pt(400.0, 1060.0),
			pt(820.0, 1120.0),
			pt(1300.0, 1150.0),
			pt(1800.0, 1050.0),
			pt(2440.0, 1110.0),
		],
		HORIZON + 60.0,
	);
	c.fill(&hill, haze(HILL_FAR, 0.75));

	let town = [
		(150.0, 1100.0, 52.0, 58.0, 0),
		(205.0, 1092.0, 44.0, 76.0, 1),
		(262.0, 1090.0, 60.0, 50.0, 2),
		(330.0, 1084.0, 48.0, 88.0, 0),
		(392.0, 1086.0, 58.0, 62.0, 1),
		(455.0, 1092.0, 46.0, 52.0, 2),
		(1660.0, 1092.0, 50.0, 60.0, 2),
		(1716.0, 1080.0, 46.0, 84.0, 0),
		(1778.0, 1072.0, 62.0, 66.0, 1),
		(1846.0, 1074.0, 48.0, 96.0, 2),
		(1906.0, 1082.0, 56.0, 58.0, 0),
		(1966.0, 1092.0, 44.0, 70.0, 1),
	];
	let roofs = [ROOF, Color(0x9C_7E_A8), Color(0x6F_A8_A0)];

	for (index, (x, base, w, h, roof)) in town.into_iter().enumerate() {
		let depth = 0.55 + noise(7, index as u64).abs() * 0.1;
		let wall = Path::rect(x - w / 2.0, base - h, w, h + 40.0);
		let peak = base - h - w * 0.55;
		let roof_path = Path::new()
			.move_to(pt(x - w / 2.0 - 6.0, base - h + 4.0))
			.quad_to(pt(x - w * 0.2, peak + 6.0), pt(x, peak))
			.quad_to(
				pt(x + w * 0.2, peak + 6.0),
				pt(x + w / 2.0 + 6.0, base - h + 4.0),
			)
			.close();
		c.fill(&wall, haze(WALL, depth));
		c.fill(
			&Path::rect(x + w * 0.1, base - h, w * 0.4, h + 40.0),
			haze(WALL.darken(0.12), depth),
		);
		c.fill(&roof_path, haze(roofs[roof], depth));
		c.fill(
			&Path::rect(x - w * 0.22, base - h * 0.62, w * 0.16, w * 0.2),
			haze(Color(0x5A_5E_78), depth),
		);
	}

	let nearer = ridge(
		&[
			pt(-40.0, 1210.0),
			pt(300.0, 1170.0),
			pt(700.0, 1205.0),
			pt(1150.0, 1190.0),
			pt(1600.0, 1225.0),
			pt(2100.0, 1170.0),
			pt(2440.0, 1200.0),
		],
		HORIZON + 80.0,
	);
	c.fill(&nearer, haze(HILL_FAR, 0.45));

	// Far trees: just soft dots of colour on the ridge.
	for i in 0..14 {
		let x = 80.0 + i as f32 * 170.0 + noise(21, i) * 50.0;
		let y = 1196.0 + noise(22, i) * 18.0;
		let r = 26.0 + noise(23, i) * 6.0;
		c.fill(
			&Path::ellipse(pt(x, y - r * 0.6), r, r * 1.15),
			haze(LEAF, 0.5),
		);
	}
}

fn mid_hills(c: &mut Svg) {
	let hill = ridge(
		&[
			pt(-40.0, 1300.0),
			pt(400.0, 1240.0),
			pt(900.0, 1290.0),
			pt(1500.0, 1300.0),
			pt(2000.0, 1250.0),
			pt(2440.0, 1290.0),
		],
		1500.0,
	);
	c.fill(&hill, haze(HILL_MID, 0.25));
}

fn ground(c: &mut Svg) {
	let field = c.next_id("field");
	c.define(&format!(
		r#"<linearGradient id="{field}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{GRASS_NEAR}"/></linearGradient>"#,
		haze(GRASS, 0.15)
	));
	let land = ridge(
		&[
			pt(-40.0, 1360.0),
			pt(600.0, 1330.0),
			pt(1300.0, 1345.0),
			pt(1900.0, 1320.0),
			pt(2440.0, 1350.0),
		],
		SIZE + 40.0,
	);
	c.fill_paint(&land, &field);

	// Shadows of the clouds lying across the field, flattened by the perspective.
	for (x, y, rx, ry) in [(600.0, 1520.0, 520.0, 70.0), (1900.0, 1880.0, 700.0, 110.0)] {
		c.fill_alpha(&Path::ellipse(pt(x, y), rx, ry), LEAF_DARK, 0.12);
	}
}

fn path(c: &mut Svg) {
	// Both edges run from wide at the bottom of the page to the vanishing point, with a bend.
	let left = [
		pt(560.0, 2440.0),
		pt(900.0, 2050.0),
		pt(1180.0, 1720.0),
		pt(1240.0, 1500.0),
		pt(VANISH.x - 14.0, VANISH.y),
	];
	let right = [
		pt(VANISH.x + 14.0, VANISH.y),
		pt(1420.0, 1500.0),
		pt(1560.0, 1720.0),
		pt(1760.0, 2050.0),
		pt(2050.0, 2440.0),
	];
	// One spline up the left edge, over the vanishing point and down the right edge; the
	// closing line runs along the bottom, off the page.
	let edges: Vec<Pt> = left.iter().chain(&right).copied().collect();
	let path = Path::spline(&edges).close();
	let worn = c.next_id("worn");
	c.define(&format!(
		r#"<linearGradient id="{worn}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{PATH}"/></linearGradient>"#,
		haze(PATH, 0.4)
	));
	c.fill_paint(&path, &worn);
	c.stroke(&Path::spline(&left), PATH_EDGE, 7.0);
	c.stroke(&Path::spline(&right), PATH_EDGE, 7.0);

	// Pebbles that shrink as the path recedes.
	for i in 0..40 {
		let depth = (i as f32 / 40.0).powf(0.6);
		let y = 2380.0 - depth * 1000.0;
		let spread = 520.0 * (1.0 - depth * 0.9);
		let x = 1310.0 + noise(31, i) * spread + (1.0 - depth) * -60.0;
		let r = 9.0 * (1.0 - depth * 0.85);
		c.fill(&Path::ellipse(pt(x, y), r * 1.6, r), PATH_EDGE);
	}
}

fn tree(c: &mut Svg, base: Pt, scale: f32, depth: f32, line: f32) {
	let ink = haze(LEAF_DARK.darken(0.35), depth);
	let trunk = Shape::parse(&format!(
		"M{} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} Z",
		base.x - 22.0 * scale,
		base.y,
		base.x - 16.0 * scale,
		base.y - 80.0 * scale,
		base.x - 14.0 * scale,
		base.y - 160.0 * scale,
		base.x - 10.0 * scale,
		base.y - 230.0 * scale,
		base.x - 4.0 * scale,
		base.y - 240.0 * scale,
		base.x + 4.0 * scale,
		base.y - 240.0 * scale,
		base.x + 10.0 * scale,
		base.y - 230.0 * scale,
		base.x + 14.0 * scale,
		base.y - 160.0 * scale,
		base.x + 16.0 * scale,
		base.y - 80.0 * scale,
		base.x + 22.0 * scale,
		base.y,
		base.x + 8.0 * scale,
		base.y + 6.0 * scale,
		base.x - 8.0 * scale,
		base.y + 6.0 * scale,
		base.x - 22.0 * scale,
		base.y,
	));
	c.fill(&trunk.path(), haze(BARK, depth));
	c.cel_shade(&trunk, pt(-8.0 * scale, 0.0));

	if line > 0.0 {
		c.stroke(&trunk.path(), ink, line);
	}

	let crown = base + pt(0.0, -300.0 * scale);
	let puffs = [
		(-120.0, 40.0, 70.0),
		(-90.0, -40.0, 80.0),
		(-30.0, -110.0, 85.0),
		(60.0, -90.0, 85.0),
		(120.0, -10.0, 75.0),
		(110.0, 70.0, 70.0),
		(30.0, 100.0, 75.0),
		(-60.0, 90.0, 70.0),
		(0.0, 0.0, 120.0),
	];
	let mut canopy = Path::new();

	for (x, y, r) in puffs {
		canopy = canopy.append(&Path::ellipse(
			crown + pt(x, y) * scale,
			r * scale,
			r * scale * 0.92,
		));
	}

	if line > 0.0 {
		c.stroke(&canopy, ink, line * 2.0);
	}

	c.fill(&canopy, haze(LEAF, depth));
	// Shadow on the lower right of the whole crown, then a lit cap on the top left.
	c.begin_clip(&canopy);
	c.fill_alpha(
		&Path::ellipse(crown + pt(70.0, 90.0) * scale, 190.0 * scale, 140.0 * scale),
		haze(LEAF_DARK, depth),
		0.7,
	);
	c.fill_alpha(
		&Path::ellipse(
			crown + pt(-60.0, -90.0) * scale,
			110.0 * scale,
			80.0 * scale,
		),
		haze(LEAF.lighten(0.25), depth),
		0.6,
	);

	for i in 0..7 {
		let at = crown + pt(noise(61, i) * 110.0, noise(62, i) * 80.0) * scale;
		let mark = Path::new()
			.move_to(at + pt(-12.0, 4.0) * scale)
			.quad_to(at + pt(0.0, -10.0) * scale, at + pt(12.0, 4.0) * scale);
		c.stroke(&mark, haze(LEAF_DARK, depth), 3.0 * scale.sqrt());
	}

	c.end();
}

fn mid_trees(c: &mut Svg) {
	// Furthest first. Lines thin out and colours fade with distance.
	for (x, y, s) in [
		(2240.0_f32, 1360.0_f32, 0.55),
		(1630.0, 1350.0, 0.5),
		(300.0, 1400.0, 0.75),
	] {
		let depth = ((1700.0 - y) / 400.0).clamp(0.0, 1.0) * 0.55;
		c.fill_alpha(
			&Path::ellipse(pt(x + 40.0 * s, y), 110.0 * s, 18.0 * s),
			LEAF_DARK,
			0.25,
		);
		tree(c, pt(x, y), s, depth, 2.5);
	}

	// A low hedge behind the bench.
	let hedge = Shape::parse(
		"M1700 1560 C1700 1500 1760 1480 1800 1500 C1830 1460 1900 1460 1930 1495 C1970 1470 2040 \
		 1480 2060 1520 C2100 1510 2160 1530 2160 1560 C2160 1580 2140 1590 2120 1590 C2000 1595 \
		 1820 1595 1720 1590 C1705 1588 1700 1575 1700 1560 Z",
	);
	c.fill(&hedge.path(), haze(LEAF, 0.15));
	c.cel_shade(&hedge, pt(-10.0, -12.0));
	c.stroke(&hedge.path(), LEAF_DARK.darken(0.3), 3.0);
}

fn bench(c: &mut Svg, at: Pt, s: f32) {
	let wood = Color(0xC0_7A_52);
	let iron = Color(0x4A_40_52);
	let line = INK.mix(LEAF_DARK, 0.3);
	c.fill_alpha(
		&Path::ellipse(at + pt(30.0, 6.0) * s, 210.0 * s, 20.0 * s),
		LEAF_DARK,
		0.3,
	);

	for x in [-150.0, 150.0] {
		let leg = Path::rect(at.x + (x - 8.0) * s, at.y - 70.0 * s, 16.0 * s, 70.0 * s);
		c.fill(&leg, iron);
	}

	for (y, h) in [(-150.0, 26.0), (-112.0, 26.0)] {
		let slat = Path::rect(at.x - 180.0 * s, at.y + y * s, 360.0 * s, h * s);
		c.fill(&slat, wood.mix(Color(0xF6_EC_D6), 0.08));
		c.stroke(&slat, line, 3.0);
	}

	let seat = Path::rect(at.x - 190.0 * s, at.y - 78.0 * s, 380.0 * s, 22.0 * s);
	c.fill(&seat, wood);
	c.fill(
		&Path::rect(at.x - 190.0 * s, at.y - 62.0 * s, 380.0 * s, 6.0 * s),
		wood.darken(0.25),
	);
	c.stroke(&seat, line, 3.5);
}

fn bush(c: &mut Svg, at: Pt, s: f32) {
	let mut body = Path::new();

	for (x, y, r) in [
		(-90.0, 0.0, 80.0),
		(0.0, -40.0, 100.0),
		(90.0, 0.0, 80.0),
		(0.0, 10.0, 90.0),
	] {
		body = body.append(&Path::ellipse(at + pt(x, y) * s, r * s, r * s * 0.85));
	}

	let depth = 0.2;
	c.fill_alpha(
		&Path::ellipse(at + pt(30.0, 60.0) * s, 200.0 * s, 26.0 * s),
		LEAF_DARK,
		0.25,
	);
	c.stroke(&body, haze(LEAF_DARK.darken(0.3), depth), 5.0);
	c.fill(&body, haze(LEAF.mix(Color(0x9C_B8_5A), 0.3), depth));
	c.begin_clip(&body);
	c.fill_alpha(
		&Path::ellipse(at + pt(60.0, 70.0) * s, 200.0 * s, 110.0 * s),
		haze(LEAF_DARK, depth),
		0.55,
	);
	c.end();
}

fn near_tree(c: &mut Svg) {
	// A big tree at the left edge, cropped by the page, frames the scene and sits nearest.
	c.fill_alpha(
		&Path::ellipse(pt(200.0, 2010.0), 300.0, 46.0),
		LEAF_DARK,
		0.3,
	);
	tree(c, pt(60.0, 2010.0), 2.6, 0.0, 4.0);
}

fn tufts(c: &mut Svg) {
	for i in 0..110 {
		let depth = noise(41, i).abs();
		let y = 2380.0 - depth * 960.0;
		let x = 1200.0 + noise(42, i) * 1300.0;
		let on_path = (x - 1310.0).abs() < 380.0 * (1.0 - depth * 0.9) && y > 1500.0;

		if on_path {
			continue;
		}

		let s = 1.9 * (1.0 - depth * 0.8);
		let color = GRASS_NEAR.darken(0.18).mix(GRASS, depth * 0.6);

		for blade in [-1.0, 0.0, 1.0] {
			let tip = pt(x + blade * 14.0 * s, y - (34.0 - blade.abs() * 10.0) * s);
			c.stroke(
				&Path::new()
					.move_to(pt(x + blade * 4.0 * s, y))
					.quad_to(pt(x + blade * 6.0 * s, y - 20.0 * s), tip),
				color,
				5.0 * s,
			);
		}
	}
}

fn flowers(c: &mut Svg) {
	let petal = Color(0xFF_F4_E4);
	let heart = Color(0xF0_B4_3C);
	let coral = Color(0xEE_8A_78);

	for (i, (x, y, s)) in [
		(310.0, 2330.0, 1.2),
		(420.0, 2390.0, 1.4),
		(2140.0, 2320.0, 1.25),
		(2260.0, 2380.0, 1.5),
		(1990.0, 2400.0, 1.3),
		(180.0, 2250.0, 1.0),
	]
	.into_iter()
	.enumerate()
	{
		let stem = Path::new()
			.move_to(pt(x, y + 120.0))
			.quad_to(pt(x - 10.0 * s, y + 50.0 * s), pt(x, y));
		c.stroke(&stem, LEAF_DARK, 6.0 * s);
		let color = if i % 2 == 0 { petal } else { coral };

		for k in 0..5 {
			let angle = k as f32 * 72.0 + noise(51, i as u64) * 20.0;
			let tip = pt(0.0, -26.0 * s).rotate(angle);
			let leaf = Path::ellipse(pt(x, y) + tip * 0.6, 14.0 * s, 14.0 * s);
			c.fill(&leaf, color);
			c.stroke(&leaf, INK.mix(LEAF_DARK, 0.4), 2.5);
		}

		c.fill(&Path::circle(pt(x, y), 10.0 * s), heart);
		c.stroke(
			&Path::circle(pt(x, y), 10.0 * s),
			INK.mix(LEAF_DARK, 0.4),
			2.5,
		);
	}
}

/// Wormsley, who hides on every page, peeking out of a hole in the grass.
pub fn wormsley(c: &mut Svg, at: Pt, s: f32) {
	let pink = Color(0xF2_9A_A8);
	c.fill(&Path::ellipse(at, 24.0 * s, 7.0 * s), Color(0x004E_3A32));
	let body = Shape::parse(&format!(
		"M{} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} Z",
		at.x - 11.0 * s,
		at.y,
		at.x - 13.0 * s,
		at.y - 30.0 * s,
		at.x - 12.0 * s,
		at.y - 50.0 * s,
		at.x,
		at.y - 52.0 * s,
		at.x + 12.0 * s,
		at.y - 50.0 * s,
		at.x + 13.0 * s,
		at.y - 30.0 * s,
		at.x + 11.0 * s,
		at.y,
		at.x + 4.0 * s,
		at.y + 3.0 * s,
		at.x - 4.0 * s,
		at.y + 3.0 * s,
		at.x - 11.0 * s,
		at.y,
	));
	c.brush_fill(&body.path(), pink, INK, 2.5);
	c.cel_shade(&body, pt(-4.0 * s, -3.0 * s));
	c.brush_line(&body.path(), INK, 2.5);

	for side in [-1.0, 1.0] {
		let eye = pt(at.x + side * 5.0 * s, at.y - 38.0 * s);
		c.fill(&Path::circle(eye, 4.0 * s), Color(0xFF_FF_FF));
		c.stroke(&Path::circle(eye, 4.0 * s), INK, 1.6);
		c.fill(&Path::circle(eye + pt(0.8, 0.5) * s, 1.8 * s), INK);
	}

	let hat = Path::ellipse(pt(at.x, at.y - 52.0 * s), 17.0 * s, 4.0 * s);
	c.fill(&hat, INK);
	c.fill(
		&Path::ellipse(pt(at.x, at.y - 60.0 * s), 10.0 * s, 10.0 * s),
		INK,
	);
	c.fill(
		&Path::rect(at.x - 10.0 * s, at.y - 60.0 * s, 20.0 * s, 8.0 * s),
		INK,
	);
}
