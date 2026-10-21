//! Places in Kerfuffle.
//!
//! Every setting is drawn on the square stage with generous overdraw past the edges (for print
//! bleed and camera moves). The top third is kept calm so story text can sit there, and the
//! ground line is where actors stand by default.

use super::Color;
use super::Path;
use super::Pt;
use super::STAGE;
use super::Svg;
use super::critters;
use super::critters::CritterState;
use super::critters::FlowerKind;
use super::noise;
use super::palette;
use super::pt;

/// How far drawings extend past the stage edge.
const OVER: f32 = 160.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
	Plain,
	Town,
	Street,
	Garden,
	Park,
	Bathroom,
	Bedroom,
}

impl Setting {
	pub const ALL: [(&'static str, Setting); 7] = [
		("plain", Setting::Plain),
		("town", Setting::Town),
		("street", Setting::Street),
		("garden", Setting::Garden),
		("park", Setting::Park),
		("bathroom", Setting::Bathroom),
		("bedroom", Setting::Bedroom),
	];

	pub fn parse(name: &str) -> Option<Setting> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, setting)| *setting)
	}

	/// Stage y where actors' feet rest.
	pub fn ground(self) -> f32 {
		match self {
			Setting::Plain => 2080.0,
			Setting::Town => 2090.0,
			Setting::Street | Setting::Garden | Setting::Park => 2060.0,
			Setting::Bathroom | Setting::Bedroom => 2100.0,
		}
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
	pub time: f32,
}

impl Default for Weather {
	fn default() -> Self {
		Self {
			wilt: 0.0,
			stink: 0.0,
			mint: 0.0,
			night: false,
			tint: palette::SUNFLOWER,
			time: 0.0,
		}
	}
}

/// Depth layers of a setting. Video moves the far layer less than the near one as the camera
/// travels (parallax), which gives flat art real depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
	/// Sky, sun, clouds and distant hills (or the whole of a plain backdrop).
	Far,
	/// Everything else behind the actors.
	Near,
}

/// Draws everything behind the actors.
pub fn backdrop(svg: &mut Svg, setting: Setting, weather: &Weather) {
	backdrop_layer(svg, setting, weather, Layer::Far);
	backdrop_layer(svg, setting, weather, Layer::Near);
}

/// Draws one depth layer of a setting.
pub fn backdrop_layer(svg: &mut Svg, setting: Setting, weather: &Weather, layer: Layer) {
	if layer == Layer::Far {
		svg.fill(&full_rect(-OVER, STAGE + OVER * 2.0), palette::PAPER);
	}

	let (horizon, hill_base) = match setting {
		Setting::Town => (1500.0, 1560.0),
		Setting::Street => (1400.0, 1500.0),
		Setting::Garden => (1350.0, 1450.0),
		Setting::Park => (1500.0, 1600.0),
		Setting::Plain | Setting::Bathroom | Setting::Bedroom => (0.0, 0.0),
	};

	match (setting, layer) {
		(Setting::Plain, Layer::Far) => plain(svg, weather),
		(Setting::Bathroom, Layer::Near) => bathroom(svg, weather),
		(Setting::Bedroom, Layer::Near) => bedroom(svg, weather),
		(Setting::Town | Setting::Street | Setting::Garden | Setting::Park, Layer::Far) => {
			sky(svg, weather, horizon);
			hills(svg, hill_base, weather);
		}
		(Setting::Town, Layer::Near) => town(svg, weather),
		(Setting::Street, Layer::Near) => street(svg, weather),
		(Setting::Garden, Layer::Near) => garden(svg, weather),
		(Setting::Park, Layer::Near) => park(svg, weather),
		(Setting::Plain, Layer::Near) | (Setting::Bathroom | Setting::Bedroom, Layer::Far) => {}
	}
}

/// Draws anything that sits in front of the actors, plus scene-wide air effects.
pub fn foreground(svg: &mut Svg, setting: Setting, weather: &Weather) {
	match setting {
		Setting::Garden => {
			for (i, x) in [-40.0, 260.0, 2160.0, 2440.0].iter().enumerate() {
				flower_at(svg, pt(*x, 2420.0), i as u64 + 90, 560.0, weather);
			}
		}
		Setting::Park | Setting::Town => grass_tufts(svg, setting.ground() + 150.0, weather),
		_ => {}
	}

	let bounds = (-OVER, -OVER, STAGE + OVER * 2.0, STAGE + OVER * 2.0);
	super::fx::haze(svg, weather.stink, palette::MOSS, bounds);
	super::fx::haze(svg, weather.mint * 0.6, palette::ICE, bounds);
}

fn full_rect(y: f32, h: f32) -> Path {
	Path::rect(-OVER, y, STAGE + OVER * 2.0, h)
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

fn sky(svg: &mut Svg, weather: &Weather, horizon: f32) {
	let top = if weather.night {
		palette::NIGHT
	} else {
		palette::SKY
	};
	svg.fill(&full_rect(-OVER, horizon + OVER), top);

	let band = full_rect(horizon - 520.0, 520.0);
	let dots = halftone(
		svg,
		if weather.night {
			palette::GRAPE
		} else {
			palette::WHITE
		},
		34.0,
		7.0,
	);
	fill_pattern(svg, &band, &dots, 0.55);

	if weather.night {
		svg.inked(&Path::circle(pt(1980.0, 760.0), 120.0), palette::CREAM);
		svg.fill(&Path::circle(pt(1930.0, 730.0), 110.0), palette::NIGHT);

		for i in 0..18 {
			let p = pt(
				100.0 + (noise(5, i) + 1.0) * 1100.0,
				120.0 + (noise(6, i) + 1.0) * 400.0,
			);
			super::fx::sparkle(
				svg,
				p,
				18.0 + noise(7, i).abs() * 14.0,
				palette::CREAM,
				weather.time + i as f32,
			);
		}

		return;
	}

	let sun = pt(2080.0, horizon - 260.0);
	svg.inked(
		&Path::wobbly_ellipse(sun, 150.0, 150.0, 0.03, svg.seed(3)),
		palette::SUNFLOWER,
	);

	for (i, x) in [260.0, 1120.0].iter().enumerate() {
		let drift = (weather.time * 6.0) % 400.0;
		cloud(
			svg,
			pt(*x + drift, horizon - 380.0 + i as f32 * 90.0),
			1.0 - i as f32 * 0.25,
			i as u64,
		);
	}
}

fn cloud(svg: &mut Svg, c: Pt, scale: f32, seed: u64) {
	let puffs = [
		(-110.0, 20.0, 80.0),
		(-20.0, -30.0, 110.0),
		(100.0, 10.0, 85.0),
		(10.0, 40.0, 90.0),
	];
	let mut shape = Path::new();

	for (i, (dx, dy, r)) in puffs.iter().enumerate() {
		shape = shape.append(&Path::wobbly_ellipse(
			c + pt(*dx, *dy) * scale,
			r * scale,
			r * scale * 0.8,
			0.06,
			seed * 10 + i as u64,
		));
	}

	svg.fill(&shape, palette::WHITE);
}

fn hills(svg: &mut Svg, base: f32, weather: &Weather) {
	let back = palette::MEADOW.mix(palette::MOSS, weather.wilt * 0.8);
	let front = palette::LEAF.mix(palette::MOSS, weather.wilt * 0.8);
	let far = Path::spline(&[
		pt(-OVER, base - 120.0),
		pt(400.0, base - 300.0),
		pt(900.0, base - 180.0),
		pt(1500.0, base - 340.0),
		pt(2100.0, base - 200.0),
		pt(STAGE + OVER, base - 260.0),
	])
	.line_to(pt(STAGE + OVER, STAGE + OVER))
	.line_to(pt(-OVER, STAGE + OVER))
	.close();
	svg.fill(&far, back);

	let near = Path::spline(&[
		pt(-OVER, base - 40.0),
		pt(700.0, base - 140.0),
		pt(1600.0, base - 20.0),
		pt(STAGE + OVER, base - 110.0),
	])
	.line_to(pt(STAGE + OVER, STAGE + OVER))
	.line_to(pt(-OVER, STAGE + OVER))
	.close();
	svg.fill(&near, front);

	let dots = halftone(svg, front.darken(0.25), 30.0, 6.0);
	fill_pattern(svg, &near, &dots, 0.35);
}

fn grass_tufts(svg: &mut Svg, y: f32, weather: &Weather) {
	let color = palette::LEAF.mix(palette::MOSS, weather.wilt).darken(0.15);

	for i in 0..14 {
		let x = -60.0 + i as f32 * 190.0 + noise(31, i) * 50.0;
		let base = pt(x, y + noise(32, i) * 60.0);
		let tuft = Path::new()
			.move_to(base + pt(-24.0, 0.0))
			.quad_to(base + pt(-30.0, -40.0), base + pt(-38.0, -62.0))
			.move_to(base)
			.quad_to(base + pt(4.0, -50.0), base + pt(2.0, -80.0))
			.move_to(base + pt(22.0, 0.0))
			.quad_to(base + pt(30.0, -40.0), base + pt(42.0, -58.0));
		svg.stroke(&tuft, color, svg.line * 1.4);
	}
}

fn flower_at(svg: &mut Svg, base: Pt, seed: u64, height: f32, weather: &Weather) {
	let kinds = [FlowerKind::Daisy, FlowerKind::Tulip, FlowerKind::Sunflower];
	let colors = [
		palette::BUBBLEGUM,
		palette::TOMATO,
		palette::SUNFLOWER,
		palette::LILAC,
		palette::WHITE,
	];
	let kind = kinds[(seed % 3) as usize];
	let color = colors[(seed % 5) as usize];
	let state = CritterState {
		wilt: weather.wilt,
		time: weather.time,
		..CritterState::default()
	};

	svg.begin(&format!("translate({:.1} {:.1})", base.x, base.y));
	critters::flower(svg, kind, color, height, state, seed);
	svg.end();
}

fn plain(svg: &mut Svg, weather: &Weather) {
	svg.fill(&full_rect(-OVER, STAGE + OVER * 2.0), palette::PAPER);
	let blob = Path::wobbly_ellipse(
		pt(STAGE / 2.0, STAGE * 0.6),
		STAGE * 0.42,
		STAGE * 0.36,
		0.06,
		svg.seed(77),
	);
	svg.fill(&blob, weather.tint);
	let dots = halftone(svg, weather.tint.darken(0.2), 36.0, 7.0);
	fill_pattern(svg, &blob, &dots, 0.35);

	for i in 0..10 {
		let angle = i as f32 / 10.0 * std::f32::consts::TAU + 0.3;
		let p = pt(STAGE / 2.0, STAGE * 0.6)
			+ pt(angle.cos() * STAGE * 0.46, angle.sin() * STAGE * 0.4);
		super::fx::sparkle(svg, p, 26.0, weather.tint, weather.time + i as f32);
	}
}

/// A crooked Kerfuffle house. `base` is the middle of its front step.
#[allow(clippy::too_many_arguments)]
fn house(
	svg: &mut Svg,
	base: Pt,
	w: f32,
	h: f32,
	wall: Color,
	roof: Color,
	lean: f32,
	seed: u64,
	night: bool,
) {
	let half = w / 2.0;
	let top_shift = lean * h * 0.08;
	let body = Path::wobbly_poly(
		&[
			base + pt(-half, 0.0),
			base + pt(-half + top_shift, -h),
			base + pt(half + top_shift, -h),
			base + pt(half, 0.0),
		],
		10.0,
		4.0,
		svg.seed(seed),
	);
	svg.inked(&body, wall);

	let roof_shape = Path::wobbly_poly(
		&[
			base + pt(-half - 40.0 + top_shift, -h + 10.0),
			base + pt(top_shift * 1.6 + lean * 30.0, -h - w * 0.55),
			base + pt(half + 40.0 + top_shift, -h + 10.0),
		],
		18.0,
		5.0,
		svg.seed(seed + 1),
	);
	let chimney = Path::wobbly_poly(
		&[
			base + pt(half * 0.45 + top_shift, -h - w * 0.2),
			base + pt(half * 0.45 + top_shift, -h - w * 0.5),
			base + pt(half * 0.75 + top_shift, -h - w * 0.5),
			base + pt(half * 0.75 + top_shift, -h - w * 0.05),
		],
		6.0,
		2.0,
		svg.seed(seed + 2),
	);
	svg.inked(&chimney, palette::TOMATO.darken(0.1));
	svg.inked(&roof_shape, roof);

	let glass = if night {
		palette::SUNFLOWER
	} else {
		palette::SKY.lighten(0.3)
	};
	let window_w = w * 0.22;

	for (i, dx) in [-half * 0.5, half * 0.5].iter().enumerate() {
		let c = base + pt(*dx + top_shift * 0.7, -h * 0.68);
		let pane = Path::wobbly_poly(
			&[
				c + pt(-window_w / 2.0, -window_w * 0.6),
				c + pt(window_w / 2.0, -window_w * 0.6),
				c + pt(window_w / 2.0, window_w * 0.6),
				c + pt(-window_w / 2.0, window_w * 0.6),
			],
			8.0,
			2.0,
			svg.seed(seed + 3 + i as u64),
		);
		svg.inked(&pane, glass);
		svg.stroke(
			&Path::new()
				.move_to(c + pt(0.0, -window_w * 0.6))
				.line_to(c + pt(0.0, window_w * 0.6)),
			palette::INK,
			svg.line * 0.8,
		);
	}

	let door_w = w * 0.24;
	let door = Path::wobbly_poly(
		&[
			base + pt(-door_w / 2.0, 0.0),
			base + pt(-door_w / 2.0, -h * 0.38),
			base + pt(0.0, -h * 0.38 - door_w * 0.35),
			base + pt(door_w / 2.0, -h * 0.38),
			base + pt(door_w / 2.0, 0.0),
		],
		12.0,
		2.0,
		svg.seed(seed + 6),
	);
	svg.inked(&door, roof.darken(0.15));
	svg.fill(
		&Path::circle(base + pt(door_w * 0.25, -h * 0.2), 7.0),
		palette::SUNFLOWER,
	);
}

fn town(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Town.ground();

	let tower_base = pt(1180.0, 1430.0);
	let tower = Path::wobbly_poly(
		&[
			tower_base + pt(-90.0, 0.0),
			tower_base + pt(-70.0, -720.0),
			tower_base + pt(70.0, -740.0),
			tower_base + pt(90.0, 0.0),
		],
		12.0,
		4.0,
		svg.seed(200),
	);
	svg.inked(&tower, palette::SAND);
	let spire = Path::wobbly_poly(
		&[
			tower_base + pt(-110.0, -700.0),
			tower_base + pt(20.0, -980.0),
			tower_base + pt(110.0, -720.0),
		],
		14.0,
		4.0,
		svg.seed(201),
	);
	svg.inked(&spire, palette::GRAPE);
	let clock_c = tower_base + pt(0.0, -560.0);
	svg.inked(&Path::circle(clock_c, 62.0), palette::CREAM);
	let hands = Path::new()
		.move_to(clock_c + pt(0.0, -42.0))
		.line_to(clock_c)
		.line_to(clock_c + pt(30.0, 12.0));
	svg.stroke(&hands, palette::INK, svg.line);

	let row = [
		(
			130.0,
			400.0,
			470.0,
			palette::BUBBLEGUM,
			palette::NIGHT,
			-1.0,
		),
		(
			560.0,
			380.0,
			560.0,
			palette::SUNFLOWER,
			palette::TOMATO,
			1.0,
		),
		(930.0, 330.0, 420.0, palette::TEAL, palette::PLUM, -1.0),
		(1480.0, 370.0, 520.0, palette::LILAC, palette::TEAL, 1.0),
		(
			1880.0,
			400.0,
			450.0,
			palette::TANGERINE,
			palette::CORNFLOWER,
			-1.0,
		),
		(2290.0, 380.0, 560.0, palette::MINT, palette::BUBBLEGUM, 1.0),
	];

	for (i, (x, w, h, wall, roof, lean)) in row.iter().enumerate() {
		house(
			svg,
			pt(*x, 1660.0 + noise(9, i as u64) * 30.0),
			*w,
			*h,
			*wall,
			*roof,
			*lean,
			300 + i as u64 * 10,
			weather.night,
		);
	}

	let path = Path::spline(&[
		pt(-OVER, ground + 40.0),
		pt(800.0, ground - 40.0),
		pt(1700.0, ground + 30.0),
		pt(STAGE + OVER, ground - 20.0),
	])
	.line_to(pt(STAGE + OVER, ground + 220.0))
	.line_to(pt(-OVER, ground + 240.0))
	.close();
	svg.fill(&path, palette::SAND);
	let dots = halftone(svg, palette::TOFFEE, 26.0, 5.0);
	fill_pattern(svg, &path, &dots, 0.4);
}

fn bunting(svg: &mut Svg, from: Pt, to: Pt, sag: f32) {
	let mid = from.lerp(to, 0.5) + pt(0.0, sag);
	let line = Path::new().move_to(from).quad_to(mid, to);
	svg.stroke(&line, palette::INK, svg.line * 0.7);
	let colors = [
		palette::TOMATO,
		palette::SUNFLOWER,
		palette::TEAL,
		palette::BUBBLEGUM,
		palette::CORNFLOWER,
	];
	let flags = 16;

	for i in 1..flags {
		let t = i as f32 / flags as f32;
		let a = from.lerp(mid, t);
		let b = mid.lerp(to, t);
		let p = a.lerp(b, t);
		let flag = Path::new()
			.move_to(p + pt(-34.0, 0.0))
			.line_to(p + pt(34.0, 0.0))
			.line_to(p + pt(0.0, 70.0))
			.close();
		svg.inked(&flag, colors[i % colors.len()]);
	}
}

fn street(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Street.ground();

	let fronts = [
		(
			260.0,
			760.0,
			900.0,
			palette::SUNFLOWER,
			palette::TOMATO,
			-1.0,
		),
		(1200.0, 700.0, 780.0, palette::LILAC, palette::NIGHT, 1.0),
		(
			2120.0,
			740.0,
			860.0,
			palette::MINT,
			palette::CORNFLOWER,
			-1.0,
		),
	];

	for (i, (x, w, h, wall, roof, lean)) in fronts.iter().enumerate() {
		house(
			svg,
			pt(*x, ground - 330.0),
			*w,
			*h,
			*wall,
			*roof,
			*lean,
			400 + i as u64 * 10,
			weather.night,
		);
	}

	bunting(svg, pt(560.0, 1000.0), pt(1860.0, 1000.0), 160.0);

	let pavement = full_rect(ground - 330.0, 600.0);
	svg.fill(&pavement, palette::SAND);

	for i in 0..9 {
		let x = -100.0 + i as f32 * 320.0;
		let crack = Path::new()
			.move_to(pt(x, ground - 330.0))
			.line_to(pt(x - 60.0, ground + 270.0));
		svg.stroke(&crack, palette::TOFFEE, svg.line * 0.8);
	}

	let kerb = full_rect(ground + 250.0, 400.0);
	svg.fill(&kerb, palette::SLATE);

	let lamp_base = pt(1720.0, ground - 290.0);
	let post = Path::new()
		.move_to(lamp_base)
		.line_to(lamp_base + pt(0.0, -980.0));
	svg.inked_noodle(&post, palette::NIGHT, 26.0);
	let lamp = Path::wobbly_poly(
		&[
			lamp_base + pt(-60.0, -1080.0),
			lamp_base + pt(60.0, -1080.0),
			lamp_base + pt(40.0, -980.0),
			lamp_base + pt(-40.0, -980.0),
		],
		10.0,
		2.0,
		svg.seed(410),
	);
	svg.inked(
		&lamp,
		if weather.night {
			palette::SUNFLOWER
		} else {
			palette::CREAM
		},
	);

	let box_base = pt(700.0, ground - 300.0);
	let postbox = Path::wobbly_poly(
		&[
			box_base + pt(-70.0, 0.0),
			box_base + pt(-70.0, -300.0),
			box_base + pt(70.0, -300.0),
			box_base + pt(70.0, 0.0),
		],
		50.0,
		2.0,
		svg.seed(411),
	);
	svg.inked(&postbox, palette::TOMATO);
	svg.inked(
		&Path::rect(box_base.x - 45.0, box_base.y - 220.0, 90.0, 18.0),
		palette::INK,
	);

	for (i, x) in [1000.0, 1440.0].iter().enumerate() {
		hedge(svg, pt(*x, ground - 300.0), 200.0, weather, 420 + i as u64);
	}
}

fn hedge(svg: &mut Svg, base: Pt, w: f32, weather: &Weather, seed: u64) {
	let color = palette::LEAF
		.mix(palette::MOSS, weather.wilt)
		.mix(palette::TOFFEE, weather.wilt * 0.4);
	let bush = Path::wobbly_ellipse(base + pt(0.0, -w * 0.45), w, w * 0.55, 0.12, svg.seed(seed));
	svg.inked(&bush, color);
}

fn garden(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Garden.ground();

	let fence_top = ground - 640.0;
	let rail_color = palette::TOFFEE;

	for i in 0..14 {
		let x = -100.0 + i as f32 * 190.0;
		let picket = Path::wobbly_poly(
			&[
				pt(x - 48.0, ground - 200.0),
				pt(x - 48.0, fence_top + 50.0),
				pt(x, fence_top),
				pt(x + 48.0, fence_top + 50.0),
				pt(x + 48.0, ground - 200.0),
			],
			6.0,
			3.0,
			svg.seed(500 + i),
		);
		svg.inked(&picket, palette::CREAM);
	}

	for y in [fence_top + 120.0, ground - 330.0] {
		svg.inked(&full_rect(y, 36.0), rail_color);
	}

	let lawn = full_rect(ground - 220.0, 700.0);
	svg.fill(&lawn, palette::LEAF.mix(palette::MOSS, weather.wilt * 0.8));
	let soil = Path::wobbly_poly(
		&[
			pt(-OVER, ground - 250.0),
			pt(STAGE + OVER, ground - 250.0),
			pt(STAGE + OVER, ground - 120.0),
			pt(-OVER, ground - 120.0),
		],
		30.0,
		8.0,
		svg.seed(520),
	);
	svg.fill(&soil, palette::COCOA);

	for i in 0..9 {
		let x = 80.0 + i as f32 * 280.0 + noise(521, i) * 40.0;
		let height = 420.0 + noise(522, i) * 90.0;
		flower_at(svg, pt(x, ground - 170.0), 530 + i, height, weather);
	}
}

fn park(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Park.ground();

	let tree_state = CritterState {
		wilt: weather.wilt,
		time: weather.time,
		..CritterState::default()
	};

	for (i, (x, h, color)) in [
		(120.0, 1100.0, palette::LEAF),
		(560.0, 820.0, palette::TEAL),
		(2280.0, 1050.0, palette::LEAF),
	]
	.iter()
	.enumerate()
	{
		svg.begin(&format!("translate({x} {})", ground - 280.0));
		critters::tree(svg, *color, *h, tree_state, 600 + i as u64);
		svg.end();
	}

	slide(svg, pt(1780.0, ground - 260.0));
	swing(svg, pt(980.0, ground - 260.0), weather.time);

	let path = Path::spline(&[
		pt(-OVER, ground + 180.0),
		pt(900.0, ground + 80.0),
		pt(1700.0, ground + 160.0),
		pt(STAGE + OVER, ground + 60.0),
	])
	.line_to(pt(STAGE + OVER, ground + 300.0))
	.line_to(pt(-OVER, ground + 320.0))
	.close();
	svg.fill(&path, palette::SAND);
}

fn slide(svg: &mut Svg, base: Pt) {
	let ladder = Path::new()
		.move_to(base + pt(150.0, 0.0))
		.line_to(base + pt(150.0, -620.0));
	svg.inked_noodle(&ladder, palette::CORNFLOWER, 24.0);
	let ladder2 = Path::new()
		.move_to(base + pt(270.0, 0.0))
		.line_to(base + pt(270.0, -620.0));
	svg.inked_noodle(&ladder2, palette::CORNFLOWER, 24.0);

	for i in 1..6 {
		let y = -i as f32 * 105.0;
		svg.stroke(
			&Path::new()
				.move_to(base + pt(150.0, y))
				.line_to(base + pt(270.0, y)),
			palette::INK,
			svg.line * 1.6,
		);
	}

	let chute = Path::new()
		.move_to(base + pt(150.0, -620.0))
		.cubic_to(
			base + pt(-40.0, -560.0),
			base + pt(-150.0, -60.0),
			base + pt(-330.0, -30.0),
		)
		.line_to(base + pt(-330.0, 30.0))
		.cubic_to(
			base + pt(-120.0, 10.0),
			base + pt(0.0, -520.0),
			base + pt(170.0, -560.0),
		)
		.close();
	svg.inked(&chute, palette::TOMATO);
	svg.inked(
		&Path::rect(base.x + 120.0, base.y - 660.0, 180.0, 50.0),
		palette::SUNFLOWER,
	);
}

fn swing(svg: &mut Svg, base: Pt, time: f32) {
	let frame = Path::new()
		.move_to(base + pt(-260.0, 0.0))
		.line_to(base + pt(-200.0, -760.0))
		.line_to(base + pt(200.0, -760.0))
		.line_to(base + pt(260.0, 0.0));
	svg.stroke(&frame, palette::INK, 36.0);
	svg.stroke(&frame, palette::SUNFLOWER, 22.0);

	let sway = (time * 1.4).sin() * 40.0;

	for dx in [-60.0, 60.0] {
		let rope = Path::new()
			.move_to(base + pt(dx, -760.0))
			.line_to(base + pt(dx + sway, -250.0));
		svg.stroke(&rope, palette::INK, svg.line);
	}

	let seat = Path::wobbly_poly(
		&[
			base + pt(-95.0 + sway, -265.0),
			base + pt(95.0 + sway, -265.0),
			base + pt(95.0 + sway, -225.0),
			base + pt(-95.0 + sway, -225.0),
		],
		8.0,
		2.0,
		svg.seed(620),
	);
	svg.inked(&seat, palette::TEAL);
}

fn tiles(svg: &mut Svg, top: f32, bottom: f32, size: f32, grout: Color) {
	let mut y = top;

	while y < bottom {
		svg.stroke(
			&Path::new()
				.move_to(pt(-OVER, y))
				.line_to(pt(STAGE + OVER, y)),
			grout,
			5.0,
		);
		y += size;
	}

	let mut x = -OVER;

	while x < STAGE + OVER {
		svg.stroke(
			&Path::new().move_to(pt(x, top)).line_to(pt(x, bottom)),
			grout,
			5.0,
		);
		x += size;
	}
}

fn bathroom(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Bathroom.ground();
	let wall = if weather.night {
		palette::SKY.darken(0.25)
	} else {
		palette::ICE
	};
	svg.fill(&full_rect(-OVER, ground + OVER), wall);
	tiles(svg, 1150.0, ground - 120.0, 150.0, palette::SKY);
	svg.inked(&full_rect(1120.0, 40.0), palette::TEAL);

	let floor = full_rect(ground - 120.0, 800.0);
	svg.fill(&floor, palette::CORNFLOWER);
	let dots = halftone(svg, palette::WHITE, 40.0, 8.0);
	fill_pattern(svg, &floor, &dots, 0.35);

	let tub = Path::wobbly_poly(
		&[
			pt(-OVER, 1480.0),
			pt(780.0, 1480.0),
			pt(740.0, 1960.0),
			pt(-OVER, 1960.0),
		],
		90.0,
		4.0,
		svg.seed(700),
	);
	svg.inked(&tub, palette::WHITE);

	for i in 0..8 {
		let p = pt(
			40.0 + i as f32 * 90.0 + noise(701, i) * 30.0,
			1430.0 + noise(702, i) * 40.0,
		);
		svg.inked(
			&Path::circle(p, 40.0 + noise(703, i).abs() * 30.0),
			palette::WHITE,
		);
	}

	duck(svg, pt(560.0, 1380.0));

	let mirror_c = pt(1700.0, 760.0);
	svg.inked(
		&Path::wobbly_ellipse(mirror_c, 290.0, 330.0, 0.02, svg.seed(710)),
		palette::SUNFLOWER,
	);
	svg.inked(
		&Path::wobbly_ellipse(mirror_c, 245.0, 285.0, 0.02, svg.seed(711)),
		palette::ICE.mix(palette::SKY, 0.3),
	);
	let shine = Path::new()
		.move_to(mirror_c + pt(-150.0, -120.0))
		.line_to(mirror_c + pt(-60.0, -210.0));
	svg.stroke(&shine, palette::WHITE, 22.0);

	let sink_top = 1340.0;
	let pedestal = Path::wobbly_poly(
		&[
			pt(1630.0, sink_top),
			pt(1770.0, sink_top),
			pt(1740.0, ground - 120.0),
			pt(1660.0, ground - 120.0),
		],
		12.0,
		3.0,
		svg.seed(712),
	);
	svg.inked(&pedestal, palette::WHITE);
	let basin = Path::wobbly_poly(
		&[
			pt(1420.0, sink_top - 120.0),
			pt(1980.0, sink_top - 120.0),
			pt(1900.0, sink_top + 40.0),
			pt(1500.0, sink_top + 40.0),
		],
		50.0,
		3.0,
		svg.seed(713),
	);
	svg.inked(&basin, palette::WHITE);
	let tap = Path::new()
		.move_to(pt(1700.0, sink_top - 120.0))
		.line_to(pt(1700.0, sink_top - 200.0))
		.line_to(pt(1760.0, sink_top - 200.0));
	svg.inked_noodle(&tap, palette::SLATE, 22.0);

	let cup_base = pt(1910.0, sink_top - 120.0);
	let cup = Path::wobbly_poly(
		&[
			cup_base + pt(-45.0, 0.0),
			cup_base + pt(-52.0, -130.0),
			cup_base + pt(52.0, -130.0),
			cup_base + pt(45.0, 0.0),
		],
		10.0,
		2.0,
		svg.seed(714),
	);
	let brush = Path::new()
		.move_to(cup_base + pt(-10.0, -60.0))
		.line_to(cup_base + pt(-40.0, -250.0));
	svg.inked_noodle(&brush, palette::BUBBLEGUM, 14.0);
	let brush2 = Path::new()
		.move_to(cup_base + pt(10.0, -60.0))
		.line_to(cup_base + pt(30.0, -240.0));
	svg.inked_noodle(&brush2, palette::SUNFLOWER, 14.0);
	svg.inked(&cup, palette::BUBBLEGUM);

	let mat = Path::wobbly_poly(
		&[
			pt(1180.0, ground - 40.0),
			pt(1980.0, ground - 40.0),
			pt(2060.0, ground + 120.0),
			pt(1100.0, ground + 120.0),
		],
		30.0,
		4.0,
		svg.seed(715),
	);
	svg.inked(&mat, palette::BUBBLEGUM);
	let mat_dots = halftone(svg, palette::WHITE, 30.0, 6.0);
	fill_pattern(svg, &mat, &mat_dots, 0.6);
}

fn duck(svg: &mut Svg, c: Pt) {
	let body = Path::wobbly_ellipse(c, 70.0, 46.0, 0.05, 720);
	let head = Path::wobbly_ellipse(c + pt(46.0, -56.0), 38.0, 36.0, 0.05, 721);
	svg.inked(&body, palette::SUNFLOWER);
	svg.inked(&head, palette::SUNFLOWER);
	let beak = Path::new()
		.move_to(c + pt(76.0, -64.0))
		.line_to(c + pt(118.0, -52.0))
		.line_to(c + pt(76.0, -44.0))
		.close();
	svg.inked(&beak, palette::TANGERINE);
	svg.fill(&Path::circle(c + pt(56.0, -64.0), 7.0), palette::INK);
}

fn bedroom(svg: &mut Svg, weather: &Weather) {
	let ground = Setting::Bedroom.ground();
	let wall = if weather.night {
		palette::GRAPE.darken(0.35)
	} else {
		palette::LILAC
	};
	svg.fill(&full_rect(-OVER, ground + OVER), wall);

	let stripe = if weather.night {
		palette::GRAPE.darken(0.2)
	} else {
		palette::LILAC.lighten(0.3)
	};
	let mut x = -OVER;

	while x < STAGE + OVER {
		svg.fill(&Path::rect(x, -OVER, 60.0, ground + OVER), stripe);
		x += 180.0;
	}

	window(svg, pt(1720.0, 820.0), weather);

	let floor = full_rect(ground - 140.0, 800.0);
	svg.fill(&floor, palette::TOFFEE);

	for i in 0..6 {
		let y = ground - 140.0 + i as f32 * 110.0;
		svg.stroke(
			&Path::new()
				.move_to(pt(-OVER, y))
				.line_to(pt(STAGE + OVER, y)),
			palette::COCOA,
			5.0,
		);
	}

	let rug = Path::wobbly_ellipse(pt(1250.0, ground + 40.0), 620.0, 130.0, 0.03, svg.seed(800));
	svg.inked(&rug, palette::TEAL);
	svg.stroke(
		&Path::wobbly_ellipse(pt(1250.0, ground + 40.0), 520.0, 90.0, 0.03, svg.seed(801)),
		palette::CREAM,
		14.0,
	);

	let bed_top = 1500.0;
	let headboard = Path::wobbly_poly(
		&[
			pt(-OVER, ground - 120.0),
			pt(-OVER, bed_top - 360.0),
			pt(120.0, bed_top - 420.0),
			pt(160.0, ground - 120.0),
		],
		30.0,
		4.0,
		svg.seed(802),
	);
	svg.inked(&headboard, palette::CORNFLOWER);
	let mattress = Path::wobbly_poly(
		&[
			pt(-OVER, bed_top),
			pt(820.0, bed_top),
			pt(820.0, ground - 160.0),
			pt(-OVER, ground - 160.0),
		],
		40.0,
		4.0,
		svg.seed(803),
	);
	svg.inked(&mattress, palette::WHITE);
	let duvet = Path::wobbly_poly(
		&[
			pt(160.0, bed_top - 30.0),
			pt(860.0, bed_top - 10.0),
			pt(880.0, ground - 120.0),
			pt(140.0, ground - 120.0),
		],
		60.0,
		6.0,
		svg.seed(804),
	);
	svg.inked(&duvet, palette::SUNFLOWER);
	let duvet_dots = halftone(svg, palette::TANGERINE, 46.0, 10.0);
	fill_pattern(svg, &duvet, &duvet_dots, 0.7);
	let pillow = Path::wobbly_ellipse(pt(30.0, bed_top - 50.0), 150.0, 70.0, 0.06, svg.seed(805));
	svg.inked(&pillow, palette::WHITE);

	let lamp_base = pt(1020.0, 1640.0);
	let table = Path::wobbly_poly(
		&[
			lamp_base + pt(-110.0, 0.0),
			lamp_base + pt(110.0, 0.0),
			lamp_base + pt(100.0, 320.0),
			lamp_base + pt(-100.0, 320.0),
		],
		12.0,
		3.0,
		svg.seed(806),
	);
	svg.inked(&table, palette::TOMATO);
	let stem = Path::new()
		.move_to(lamp_base)
		.line_to(lamp_base + pt(0.0, -190.0));
	svg.inked_noodle(&stem, palette::INK, 14.0);
	let shade = Path::wobbly_poly(
		&[
			lamp_base + pt(-90.0, -180.0),
			lamp_base + pt(90.0, -180.0),
			lamp_base + pt(55.0, -320.0),
			lamp_base + pt(-55.0, -320.0),
		],
		10.0,
		2.0,
		svg.seed(807),
	);
	svg.inked(
		&shade,
		if weather.night {
			palette::SUNFLOWER
		} else {
			palette::MINT
		},
	);

	let shelf_y = 1180.0;
	svg.inked(&Path::rect(2050.0, shelf_y, 420.0, 34.0), palette::COCOA);
	let rocket = Path::wobbly_poly(
		&[
			pt(2120.0, shelf_y),
			pt(2120.0, shelf_y - 140.0),
			pt(2160.0, shelf_y - 220.0),
			pt(2200.0, shelf_y - 140.0),
			pt(2200.0, shelf_y),
		],
		10.0,
		2.0,
		svg.seed(808),
	);
	svg.inked(&rocket, palette::TOMATO);
	svg.inked(
		&Path::circle(pt(2340.0, shelf_y - 70.0), 70.0),
		palette::CORNFLOWER,
	);
	svg.fill(
		&super::figure::star_path(pt(2340.0, shelf_y - 70.0), 40.0, 18.0, 5),
		palette::SUNFLOWER,
	);
}

fn window(svg: &mut Svg, c: Pt, weather: &Weather) {
	let w = 360.0;
	let h = 420.0;
	let frame = Path::wobbly_poly(
		&[c + pt(-w, -h), c + pt(w, -h), c + pt(w, h), c + pt(-w, h)],
		20.0,
		3.0,
		svg.seed(820),
	);
	svg.inked(&frame, palette::CREAM);
	let glass_color = if weather.night {
		palette::NIGHT
	} else {
		palette::SKY
	};
	let glass = Path::rect(
		c.x - w + 40.0,
		c.y - h + 40.0,
		w * 2.0 - 80.0,
		h * 2.0 - 80.0,
	);
	svg.fill(&glass, glass_color);

	if weather.night {
		svg.inked(&Path::circle(c + pt(120.0, -150.0), 90.0), palette::CREAM);
		svg.fill(&Path::circle(c + pt(85.0, -175.0), 80.0), glass_color);

		for i in 0..6 {
			let p = c + pt(noise(821, i) * w * 0.8, noise(822, i) * h * 0.8);
			super::fx::sparkle(svg, p, 20.0, palette::CREAM, weather.time + i as f32);
		}
	} else {
		cloud(svg, c + pt(-60.0, -120.0), 0.7, 8);
		svg.inked(
			&Path::circle(c + pt(170.0, 190.0), 70.0),
			palette::SUNFLOWER,
		);
	}

	svg.stroke(
		&Path::new()
			.move_to(c + pt(0.0, -h + 40.0))
			.line_to(c + pt(0.0, h - 40.0)),
		palette::CREAM,
		34.0,
	);
	svg.stroke(
		&Path::new()
			.move_to(c + pt(-w + 40.0, 0.0))
			.line_to(c + pt(w - 40.0, 0.0)),
		palette::CREAM,
		34.0,
	);

	for side in [-1.0, 1.0] {
		let curtain = Path::wobbly_poly(
			&[
				c + pt(side * (w + 80.0), -h - 60.0),
				c + pt(side * (w - 120.0), -h - 60.0),
				c + pt(side * (w - 40.0), h + 60.0),
				c + pt(side * (w + 120.0), h + 120.0),
			],
			30.0,
			5.0,
			svg.seed(830 + side as u64),
		);
		svg.inked(&curtain, palette::BUBBLEGUM);
	}

	svg.inked(
		&Path::rect(c.x - w - 140.0, c.y - h - 90.0, w * 2.0 + 280.0, 40.0),
		palette::COCOA,
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn settings_round_trip_by_name() {
		for (name, setting) in Setting::ALL {
			assert_eq!(Setting::parse(name), Some(setting));
		}
	}

	#[test]
	fn every_setting_renders() {
		for (_, setting) in Setting::ALL {
			for night in [false, true] {
				let mut svg = Svg::new();
				let weather = Weather {
					night,
					wilt: 0.5,
					..Weather::default()
				};
				backdrop(&mut svg, setting, &weather);
				foreground(&mut svg, setting, &weather);
				let doc = svg.finish((0.0, 0.0, STAGE, STAGE), 64, 64);
				crate::raster::render(&doc).unwrap();
			}
		}
	}
}
