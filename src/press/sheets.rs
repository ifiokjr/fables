//! Reference sheets: every cast member with their name and habit, a character's model sheet and
//! every mood, and every setting. They are the model sheets an illustrator would pin above the desk, kept
//! in sync with the cast file automatically.

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use resvg::tiny_skia::Pixmap;

use crate::art::Bounds;
use crate::art::Color;
use crate::art::Path;
use crate::art::Pt;
use crate::art::STAGE;
use crate::art::Svg;
use crate::art::critters::CritterState;
use crate::art::escape;
use crate::art::figure;
use crate::art::figure::FigureState;
use crate::art::figure::Look;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::figure::Viseme;
use crate::art::light::Light;
use crate::art::pack::StylePack;
use crate::art::palette;
use crate::art::parts::FIGURE_PARTS;
use crate::art::parts::Level;
use crate::art::parts::Need;
use crate::art::parts::Parts;
use crate::art::parts::Turn;
use crate::art::parts::Views;
use crate::art::pt;
use crate::art::rig::Stance;
use crate::art::settings;
use crate::art::settings::Library;
use crate::art::settings::Weather;
use crate::art::shape::Shape;
use crate::art::style::Style;
use crate::book;
use crate::book::BookPage;
use crate::cast::Cast;
use crate::cast::Kind;
use crate::cast::Member;
use crate::compose;
use crate::compose::Body;
use crate::compose::Puppet;
use crate::raster;
use crate::story;
use crate::text;
use crate::text::Anchor;
use crate::text::Area;
use crate::text::Ink;

const CARD_W: f32 = 900.0;
const CARD_H: f32 = 1300.0;
const COLUMNS: usize = 4;
/// Where every card's character stands, down the card.
const CARD_GROUND: f32 = 1190.0;
/// The margin kept clear inside a card's edges, and below its words.
const CARD_MARGIN: f32 = 60.0;
/// Where a card's name, birth name and habit are set. Words too long for it at their smallest
/// run on over the character rather than shrink the character away.
const CARD_WORDS: Area = Area {
	x: 70.0,
	y: 70.0,
	w: CARD_W - 140.0,
	h: 430.0,
};

/// Every cast member of the world called `world` on a card with their name, birth name and habit.
///
/// The characters fill the room the longest card's words leave, standing on one line, every
/// person drawn the same height. Each is measured as drawn, waving hand and hair and all, so the
/// tallest reaches the words and the widest the card's edges.
pub fn cast_sheet(cast: &Cast, world: &str, style: Style) -> Result<Pixmap> {
	let members: Vec<&Member> = cast.members.values().collect();
	let words: Vec<text::Block> = members.iter().map(|member| card_words(member)).collect();
	let top = words
		.iter()
		.map(|block| block.bounds.y + block.bounds.h)
		.fold(0.0, f32::max)
		.min(CARD_WORDS.y + CARD_WORDS.h)
		+ CARD_MARGIN;
	let mut puppets: Vec<Puppet> = members
		.iter()
		.map(|member| {
			let mut puppet = sheet_puppet(member, pt(CARD_W / 2.0, CARD_GROUND));

			if let Body::Figure(state) = &mut puppet.body {
				state.mood = Mood::Grin;
			}

			puppet
		})
		.collect();
	// Everyone comes 600 pixels tall, so they grow together: by as much as the one that
	// reaches highest can.
	let grow = puppets
		.iter()
		.filter(|puppet| matches!(puppet.body, Body::Figure(_)))
		.map(|puppet| card_fit(puppet, top))
		.fold(f32::INFINITY, f32::min);

	for puppet in &mut puppets {
		let by = card_fit(puppet, top).min(grow);
		puppet.scale *= by;
		puppet.at.y = CARD_GROUND - (CARD_GROUND - puppet.at.y) * by;
	}

	let rows = members.len().div_ceil(COLUMNS);
	let width = CARD_W * COLUMNS as f32;
	let height = CARD_H * rows as f32 + 260.0;
	let mut svg = Svg::styled(style);
	svg.fill(&Path::rect(0.0, 0.0, width, height), palette::INK);

	let heading = format!("**The folk of {world}**");
	let heading = text::layout(
		&[heading.as_str()],
		Area {
			x: 0.0,
			y: 50.0,
			w: width,
			h: 180.0,
		},
		110.0,
		Anchor::Top,
	);
	svg.raw(&text::svg(
		&heading,
		Ink {
			body: palette::PAPER,
			shout: palette::SUNFLOWER,
		},
	));

	for (index, ((member, words), puppet)) in members.iter().zip(&words).zip(&puppets).enumerate() {
		let x = (index % COLUMNS) as f32 * CARD_W;
		let y = 260.0 + (index / COLUMNS) as f32 * CARD_H;
		svg.begin(&format!("translate({x} {y})"));
		card(&mut svg, member, words, puppet, index, top);
		svg.end();
	}

	let doc = svg.finish(
		(0.0, 0.0, width, height),
		(width / 2.0) as u32,
		(height / 2.0) as u32,
	);
	let mut pixmap = raster::render(&doc)?;
	raster::finish(&mut pixmap, &style, 0.05, 0);

	Ok(pixmap)
}

/// How many times bigger a card's puppet, standing on the card's ground, can be drawn and still
/// keep below `top` and inside the card's margins.
fn card_fit(puppet: &Puppet, top: f32) -> f32 {
	let (x0, y0, x1, _) = puppet.extents().subject;
	let middle = CARD_W / 2.0;
	let up = (CARD_GROUND - y0).max(1.0);
	let across = (middle - x0).max(x1 - middle).max(1.0);

	((CARD_GROUND - top) / up).min((middle - CARD_MARGIN) / across)
}

/// The colour a character mostly reads as (skin for people).
fn main_colour(member: &Member) -> Color {
	match &member.kind {
		Kind::Figure(look) => look.skin,
		Kind::Bird(color)
		| Kind::Dog(color)
		| Kind::Tree(color)
		| Kind::Toothbrush(color)
		| Kind::Flower(_, color) => *color,
		Kind::Worm => palette::FLAMINGO,
	}
}

/// The backdrop that stands furthest from `colour`, so nobody disappears into their card.
fn contrasting(tints: &[Color], colour: Color) -> Color {
	let distance = |a: Color| {
		let (r1, g1, b1) = a.rgb();
		let (r2, g2, b2) = colour.rgb();
		let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
		d(r1, r2) + d(g1, g2) + d(b1, b2)
	};

	tints
		.iter()
		.copied()
		.max_by_key(|tint| distance(*tint))
		.unwrap_or(palette::MINT)
}

/// A member's name, birth name and habit, laid out at the top of their card.
fn card_words(member: &Member) -> text::Block {
	let mut lines = vec![format!("**{}**", member.name)];

	if let Some(born) = &member.born {
		lines.push(format!("*Born {born}*"));
	}

	if let Some(about) = &member.about {
		lines.push(about.clone());
	}

	let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
	text::layout(&refs, CARD_WORDS, 40.0, Anchor::Top)
}

/// One member's card: their words at the top, and their puppet below, in the room from `top`
/// down to the ground.
fn card(
	svg: &mut Svg,
	member: &Member,
	words: &text::Block,
	puppet: &Puppet,
	index: usize,
	top: f32,
) {
	let tints = [
		palette::MINT,
		palette::SUNFLOWER,
		palette::LILAC,
		palette::SKY,
		palette::FLAMINGO,
		palette::MEADOW,
	];
	let inner = Path::wobbly_poly(
		&[
			pt(30.0, 30.0),
			pt(CARD_W - 30.0, 30.0),
			pt(CARD_W - 30.0, CARD_H - 30.0),
			pt(30.0, CARD_H - 30.0),
		],
		40.0,
		4.0,
		index as u64,
	);
	svg.inked(&inner, palette::PAPER);
	// The backdrop sits behind the character, in the room below the words.
	let room = CARD_GROUND - top;
	let blob = Path::wobbly_ellipse(
		pt(CARD_W / 2.0, top + room * 0.56),
		(room * 0.42).min(CARD_W / 2.0 - 90.0),
		room * 0.42,
		0.06,
		index as u64 + 50,
	);
	svg.fill(&blob, contrasting(&tints, main_colour(member)));

	svg.begin_layer();
	compose::draw_puppet(svg, puppet);
	svg.end();
	svg.raw(&text::svg(words, Ink::default()));
}

/// A puppet sized to fit a sheet card, standing at `at`: a person 600 pixels tall.
fn sheet_puppet(member: &Member, at: Pt) -> Puppet {
	let (body, scale, lift) = match &member.kind {
		Kind::Figure(look) => {
			let frame = look.frame();
			let mut state = FigureState::new(&frame);
			state.hands = frame.hands(Pose::Wave, 0.2);
			state.facing = 0.2;
			(Body::Figure(state), 600.0 / frame.height(), 0.0)
		}
		Kind::Bird(_) => (Body::Critter(CritterState::default()), 2.4, 140.0),
		Kind::Dog(_) => {
			(
				Body::Critter(CritterState {
					lively: 1.0,
					..CritterState::default()
				}),
				1.5,
				0.0,
			)
		}
		Kind::Worm => (Body::Critter(CritterState::default()), 3.0, 0.0),
		Kind::Toothbrush(_) => (Body::Critter(CritterState::default()), 0.9, 0.0),
		Kind::Flower(..) | Kind::Tree(_) => (Body::Critter(CritterState::default()), 0.6, 0.0),
	};

	Puppet {
		id: member.id.clone(),
		kind: member.kind.clone(),
		at: at - pt(0.0, lift),
		scale,
		z: 0,
		opacity: 1.0,
		body,
		stink: 0.0,
		mint: 0.0,
		heading: 1.0,
		breath: 0.0,
		lift: 0.0,
		smear: 0.0,
		exerted: 0.0,
	}
}

/// One character in every mood, for checking expressions stay on-model.
pub fn mood_sheet(member: &Member, style: Style) -> Result<Option<Pixmap>> {
	let Kind::Figure(look) = &member.kind else {
		return Ok(None);
	};

	let frame = look.frame();
	let cell = 700.0;
	let columns = 5;
	let rows = Mood::ALL.len().div_ceil(columns);
	let width = cell * columns as f32;
	let height = cell * 1.25 * rows as f32;
	let mut svg = Svg::styled(style);
	svg.fill(&Path::rect(0.0, 0.0, width, height), palette::PAPER);

	for (index, (name, mood)) in Mood::ALL.iter().enumerate() {
		let x = (index % columns) as f32 * cell + cell / 2.0;
		let y = (index / columns) as f32 * cell * 1.25;
		let mut state = FigureState::new(&frame);
		state.mood = *mood;
		state.hands = frame.hands(Pose::Idle, 0.0);

		let shift = if *mood == Mood::Fainted {
			state.lie = 1.0;
			cell * 0.3
		} else {
			0.0
		};

		svg.begin(&format!(
			"translate({} {}) scale({:.3})",
			x + shift,
			y + cell * 1.0,
			560.0 / frame.height()
		));
		figure::draw(&mut svg, look, &state);
		svg.end();

		let label = text::layout(
			&[name],
			Area {
				x: x - cell / 2.0,
				y: y + cell * 1.05,
				w: cell,
				h: 90.0,
			},
			54.0,
			Anchor::Top,
		);
		svg.raw(&text::svg(&label, Ink::default()));
	}

	let doc = svg.finish(
		(0.0, 0.0, width, height),
		(width / 2.0) as u32,
		(height / 2.0) as u32,
	);

	let mut pixmap = raster::render(&doc)?;
	raster::finish(&mut pixmap, &style, 0.05, 0);

	Ok(Some(pixmap))
}

/// The `facing` the stage gives a figure turned towards the middle of the page.
const THREE_QUARTER: f32 = 0.35;

/// The beats a model sheet shows: a mood, a pose, a mouth and a label.
const MODEL_BEATS: [(Mood, Pose, Option<Viseme>, &str); 5] = [
	(Mood::Happy, Pose::Idle, None, "happy"),
	(Mood::Laugh, Pose::Wave, None, "laughing, waving"),
	(
		Mood::Cross,
		Pose::ArmsCrossed,
		Some(Viseme::Open),
		"cross, talking",
	),
	(Mood::Sad, Pose::Droop, None, "sad"),
	(Mood::Shocked, Pose::Cheer, Some(Viseme::Round), "shocked"),
];

/// A character's model sheet, the one every design is signed off on: five beats (moods, poses
/// and a talking mouth), each front-on and three-quarters, `px` pixels wide.
pub fn model_sheet(title: &str, look: &Look, px: u32) -> Result<Pixmap> {
	let cell = 520.0;
	let (width, height) = (cell * 5.0, 140.0 + cell * 2.0 + 40.0);
	let mut svg = Svg::new();
	svg.fill(&Path::rect(0.0, 0.0, width, height), palette::PAPER);
	let label = |svg: &mut Svg, x: f32, y: f32, size: f32, words: &str| {
		let block = text::layout(
			&[words],
			Area {
				x: x - cell,
				y,
				w: cell * 2.0,
				h: size * 1.6,
			},
			size,
			Anchor::Top,
		);
		svg.raw(&text::svg(&block, Ink::default()));
	};
	label(&mut svg, width / 2.0, 40.0, 46.0, &format!("**{title}**"));

	// Everyone fills their own sheet: a grown-up is drawn smaller to fit.
	let frame = look.frame();
	let fit = (cell * 0.82 / frame.height()).min(0.88);

	for (column, (mood, pose, viseme, name)) in MODEL_BEATS.into_iter().enumerate() {
		for (row, facing) in [0.0, THREE_QUARTER].into_iter().enumerate() {
			let x = cell * (column as f32 + 0.5);
			let ground = 140.0 + cell * (row as f32 + 1.0) - 60.0;
			let mut state = FigureState::new(&frame);
			state.mood = mood;
			state.facing = facing;
			state.hands = frame.hands(pose, facing);
			state.hand_shapes = pose.hand_shapes();
			state.legs = frame.limbs().stance(Stance::Stand, facing);
			state.viseme = viseme;

			svg.begin(&format!("translate({x:.1} {ground:.1}) scale({fit:.3})"));
			figure::shadow(&mut svg, look, &state, 0.0);
			figure::draw(&mut svg, look, &state);
			svg.end();

			let view = if row == 0 { "front" } else { "three-quarter" };
			label(
				&mut svg,
				x,
				ground + 30.0,
				26.0,
				&format!("{name} · {view}"),
			);
		}
	}

	let px_height = (px as f32 * height / width).round() as u32;
	raster::render(&svg.finish((0.0, 0.0, width, height), px, px_height))
}

/// Every setting in `settings`, by day, plus a few in other weather, labelled with their
/// script names. With `zones`, each setting is outlined with what a story page puts on it.
pub fn places_sheet(settings: &Library, style: Style, zones: Option<&Zones>) -> Result<Pixmap> {
	let mut pixmap = raster::render(&places_document(settings, style, zones))?;
	raster::finish(&mut pixmap, &style, 0.05, 0);

	Ok(pixmap)
}

/// The places sheet as an SVG document.
fn places_document(settings: &Library, style: Style, zones: Option<&Zones>) -> String {
	let cell = 600.0;
	let columns = 4;
	let mut list: Vec<(&str, Weather)> = Vec::new();

	for name in settings.names() {
		list.push((name, Weather::default()));
	}

	list.push((
		"bedroom night",
		Weather {
			night: true,
			..Weather::default()
		},
	));
	list.push((
		"garden wilt=1",
		Weather {
			wilt: 1.0,
			..Weather::default()
		},
	));
	list.push((
		"park stink=1",
		Weather {
			stink: 1.0,
			..Weather::default()
		},
	));
	list.push((
		"park mint=1",
		Weather {
			mint: 1.0,
			..Weather::default()
		},
	));

	let rows = list.len().div_ceil(columns);
	let width = cell * columns as f32;
	let height = (cell + 90.0) * rows as f32;
	let mut svg = Svg::styled(style);
	svg.fill(&Path::rect(0.0, 0.0, width, height), palette::INK);

	for (index, (label, weather)) in list.iter().enumerate() {
		let name = label.split_whitespace().next().unwrap_or_default();
		let Some(setting) = settings.get(name) else {
			continue;
		};
		let x = (index % columns) as f32 * cell;
		let y = (index / columns) as f32 * (cell + 90.0);
		let clip = svg.next_id("place");
		svg.define(&format!(
			r#"<clipPath id="{clip}"><rect x="0" y="0" width="{STAGE}" height="{STAGE}"/></clipPath>"#
		));
		svg.begin(&format!(
			"translate({} {}) scale({:.4})",
			x + 10.0,
			y + 10.0,
			(cell - 20.0) / STAGE
		));
		svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
		svg.begin_layer();
		settings::backdrop(&mut svg, &setting, weather);
		settings::foreground(&mut svg, &setting, weather);
		svg.end();

		if let Some(zones) = zones {
			draw_zones(&mut svg, zones, setting.ground());
		}

		svg.raw("</g>");
		svg.end();

		let caption = text::layout(
			&[label],
			Area {
				x,
				y: y + cell,
				w: cell,
				h: 80.0,
			},
			40.0,
			Anchor::Top,
		);
		svg.raw(&text::svg(
			&caption,
			Ink {
				body: palette::PAPER,
				shout: palette::SUNFLOWER,
			},
		));
	}

	svg.finish((0.0, 0.0, width, height), width as u32, height as u32)
}

/// The words of a typical page: two paragraphs, about twenty words, like the median page of
/// the example books.
const TYPICAL_PAGE: &str = "Ada looked up at the hill. The wind was busy again, pulling at every \
                            kite in town.\n\n\"Hold on tight,\" said Gran.";

/// What every story page puts on top of a setting, measured from the book's own layout and
/// from template characters in a drawing style, so a setting can be composed around it.
/// Everything is in stage units.
#[derive(Clone, Debug, PartialEq)]
pub struct Zones {
	/// The paper card under a typical page's words at the top of the page.
	pub top_card: Bounds,
	/// The same card with `text=bottom`.
	pub bottom_card: Bounds,
	/// How far above the ground a standing kid's drawing reaches, hair and all.
	pub kid_height: f32,
	/// How far above the ground a standing grown-up's drawing reaches.
	pub grown_up_height: f32,
	/// How far either of them reaches to the side of the spot they stand on.
	pub half_width: f32,
	/// Where two actors without an `x=` stand, as fractions of the stage's width.
	pub pair: [f32; 2],
	/// Where one actor without an `x=` stands.
	pub lone: f32,
}

impl Zones {
	/// Lays out a two-page book in `style`, a typical page with a kid and a grown-up and the
	/// same page with `text=bottom` and the kid alone, and measures its cards and actors.
	pub fn measure(style: &Arc<StylePack>) -> Result<Zones> {
		let cast = Cast::parse_in(
			"[kid]\nname = \"Kid\"\nkind = \"kid\"\n[adult]\nname = \"Adult\"\nkind = \
			 \"grown-up\"\n",
			std::path::Path::new(""),
			style,
		)?;
		let story = story::parse(&format!(
			"+++\ntitle = \"Zones\"\nauthor = \"fables\"\nstar = \"kid\"\n+++\n# \
			 plain\n@kid\n@adult\n{TYPICAL_PAGE}\n# plain text=bottom\n@kid\n{TYPICAL_PAGE}\n"
		))?;
		let pages = book::assemble(&story, &cast)?;
		let page = |label: &str| {
			pages
				.iter()
				.find(|page| page.label == label)
				.with_context(|| format!("the zones book has no {label} page"))
		};
		let card = |page: &BookPage| {
			book::card_rects(page)
				.into_iter()
				.next()
				.with_context(|| format!("{} lays no card", page.label))
		};
		let (both, alone) = (page("story-01")?, page("story-02")?);
		let ground = both.scene.setting.ground();
		let reach = |id: &str, scene_page: &BookPage| {
			compose::puppets(&scene_page.scene, 0.0, true)
				.into_iter()
				.find(|puppet| puppet.id == id)
				.map(|puppet| (puppet.at.x, puppet.extents().subject))
				.with_context(|| format!("{id} is not on {}", scene_page.label))
		};
		let (kid_x, kid) = reach("kid", both)?;
		let (adult_x, adult) = reach("adult", both)?;
		let (lone_x, _) = reach("kid", alone)?;
		let half_width = [(kid_x, kid), (adult_x, adult)]
			.into_iter()
			.map(|(x, (left, _, right, _))| (x - left).max(right - x))
			.fold(0.0, f32::max);

		Ok(Zones {
			top_card: card(both)?,
			bottom_card: card(alone)?,
			kid_height: ground - kid.1,
			grown_up_height: ground - adult.1,
			half_width,
			pair: [kid_x / STAGE, adult_x / STAGE],
			lone: lone_x / STAGE,
		})
	}
}

/// Outlines `zones` on a setting whose actors stand at `ground`: the typical top and bottom
/// cards, how high a kid's and a grown-up's heads reach, and the spots actors stand on when
/// the script gives no `x=`.
fn draw_zones(svg: &mut Svg, zones: &Zones, ground: f32) {
	let card = palette::TOMATO;
	let heights = palette::SUNFLOWER;
	let spots = palette::WHITE;

	zone_box(svg, "card", zones.top_card, card, false);
	zone_label(
		svg,
		pt(STAGE / 2.0, zones.top_card.1 + 40.0),
		card,
		"typical card",
	);
	zone_box(svg, "card", zones.bottom_card, card, true);
	zone_label(
		svg,
		pt(STAGE / 2.0, zones.bottom_card.3 - 110.0),
		card,
		"text=bottom card",
	);

	// The grown-up's label sits above its line and the others below theirs, so the two head
	// heights, which are close, keep their labels apart.
	for (name, height, dashed, label_y) in [
		("grown-up", zones.grown_up_height, false, -150.0),
		("kid", zones.kid_height, true, 24.0),
		("ground", 0.0, false, 24.0),
	] {
		let y = ground - height;
		zone_stroke(
			svg,
			"height",
			"line",
			&format!(r#"x1="0" y1="{y:.0}" x2="{STAGE}" y2="{y:.0}""#),
			heights,
			dashed,
		);
		zone_label(svg, pt(230.0, y + label_y), heights, name);
	}

	let top = ground - zones.grown_up_height;

	for (name, x, dashed) in [
		("1/3", zones.pair[0], false),
		("2/3", zones.pair[1], false),
		("1/2", zones.lone, true),
	] {
		let x = x * STAGE;
		let spot = (x - zones.half_width, top, x + zones.half_width, ground);
		zone_box(svg, "spot", spot, spots, dashed);
		zone_label(svg, pt(x, ground - 130.0), spots, name);
	}
}

/// A thin rectangle round `bounds`.
fn zone_box(
	svg: &mut Svg,
	zone: &str,
	(left, top, right, bottom): Bounds,
	colour: Color,
	dashed: bool,
) {
	zone_stroke(
		svg,
		zone,
		"rect",
		&format!(
			r#"x="{left:.0}" y="{top:.0}" width="{:.0}" height="{:.0}" rx="24""#,
			right - left,
			bottom - top
		),
		colour,
		dashed,
	);
}

/// One outline of the `zone`, drawn twice, a dark line under a thin coloured one, so it reads
/// over any setting by day or night.
fn zone_stroke(svg: &mut Svg, zone: &str, element: &str, shape: &str, colour: Color, dashed: bool) {
	let dash = if dashed {
		r#" stroke-dasharray="46 30""#
	} else {
		""
	};
	svg.raw(&format!(
		r#"<g data-zone="{zone}"><{element} {shape} fill="none" stroke="{}" stroke-opacity="0.55" stroke-width="22"{dash}/><{element} {shape} fill="none" stroke="{colour}" stroke-width="9"{dash}/></g>"#,
		palette::INK
	));
}

/// A label centred on `at`, in `colour` on a dark tab.
fn zone_label(svg: &mut Svg, at: Pt, colour: Color, words: &str) {
	let size = 72.0;
	let block = text::layout(
		&[words],
		Area {
			x: at.x - 600.0,
			y: at.y,
			w: 1200.0,
			h: size * 1.6,
		},
		size,
		Anchor::Top,
	);
	let Area { x, y, w, h } = block.bounds;
	svg.raw(&format!(
		r#"<rect x="{:.0}" y="{:.0}" width="{:.0}" height="{:.0}" rx="20" fill="{}" fill-opacity="0.72"/>"#,
		x - 22.0,
		y - 14.0,
		w + 44.0,
		h + 28.0,
		palette::INK
	));
	svg.raw(&text::svg(
		&block,
		Ink {
			body: colour,
			shout: colour,
		},
	));
}

/// One square on a parts sheet.
struct Tile {
	label: String,
	note: String,
	shape: Option<Shape>,
	/// The box the drawing is fitted to, shared by a turning part's three tiles so the turn
	/// reads as movement rather than rescaling.
	fit: (Pt, Pt),
}

/// The world's colour script: one setting with one cast member standing in it, drawn once per
/// light preset, labelled — the sky, the grade, the cast shadow's lean and the atmosphere in
/// each. Reference sheet, so nothing here is graded but the scene.
pub fn lights_sheet(
	about: &str,
	setting: &settings::Setting,
	member: &Member,
	style: Style,
) -> Result<Pixmap> {
	let cell = 600.0;
	let columns = 5;
	let presets = Light::ALL.len();
	let rows = presets.div_ceil(columns);
	let width = cell * columns as f32;
	let height = (cell + 90.0) * rows as f32 + 46.0;
	let mut svg = Svg::styled(style);
	svg.fill(&Path::rect(0.0, 0.0, width, height), palette::INK);

	for (index, light) in Light::ALL.iter().enumerate() {
		let x = (index % columns) as f32 * cell;
		let y = (index / columns) as f32 * (cell + 90.0);
		let clip = svg.next_id("lit");
		svg.define(&format!(
			r#"<clipPath id="{clip}"><rect x="0" y="0" width="{STAGE}" height="{STAGE}"/></clipPath>"#
		));
		svg.begin(&format!(
			"translate({} {}) scale({:.4})",
			x + 10.0,
			y + 10.0,
			(cell - 20.0) / STAGE
		));
		svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
		svg.set_light(*light);
		svg.begin_layer();

		let mut weather = Weather {
			light: *light,
			..Weather::default()
		};
		settings::backdrop(&mut svg, setting, &weather);
		let mut puppet = sheet_puppet(member, pt(STAGE / 2.0, setting.ground()));

		if let Body::Figure(state) = &mut puppet.body {
			state.mood = Mood::Grin;
		}

		compose::draw_puppet(&mut svg, &puppet);
		weather.time = 2.0;
		settings::foreground(&mut svg, setting, &weather);
		svg.end();
		svg.clear_light();
		svg.raw("</g>");
		svg.end();

		let caption = text::layout(
			&[light.name()],
			Area {
				x,
				y: y + cell,
				w: cell,
				h: 80.0,
			},
			40.0,
			Anchor::Top,
		);
		svg.raw(&text::svg(
			&caption,
			Ink {
				body: palette::PAPER,
				shout: palette::SUNFLOWER,
			},
		));
	}

	let title_text = format!("**{about}** under every light · {}", setting.name());
	let title = text::layout(
		&[title_text.as_str()],
		Area {
			x: 10.0,
			y: height - 42.0,
			w: width,
			h: 40.0,
		},
		28.0,
		Anchor::Top,
	);
	svg.raw(&text::svg(
		&title,
		Ink {
			body: palette::PAPER,
			shout: palette::SUNFLOWER,
		},
	));

	let doc = svg.finish((0.0, 0.0, width, height), width as u32, height as u32);

	let mut pixmap = raster::render(&doc)?;
	raster::finish(&mut pixmap, &style, 0.05, 0);

	Ok(pixmap)
}

/// Every part of an authored puppet. A turning part is shown front-on, half-turned and
/// three-quarters turned, so a pair that won't blend shows itself; missing parts are marked,
/// and the part check's errors and warnings head the sheet.
pub fn parts_sheet(title: &str, parts: &Parts) -> Result<Pixmap> {
	const TILE: f32 = 300.0;
	const COLUMNS: usize = 6;
	let ink = Color(0x2B_1D_2E);
	let mut groups: Vec<Vec<Tile>> = Vec::new();

	for spec in FIGURE_PARTS {
		let names: Vec<String> = if spec.need == Need::Numbered {
			(1..=parts.numbered(spec.name).len())
				.map(|n| format!("{}-{n}", spec.name))
				.collect()
		} else {
			vec![spec.name.to_owned()]
		};

		for name in names {
			let turn = parts.turn(&name);
			let tile = |label: String, note: &str, shape: Option<Shape>, fit| {
				Tile {
					label,
					note: note.to_owned(),
					shape,
					fit,
				}
			};

			let group = match (turn, spec.views) {
				(Some(turn @ (Turn::Blend { front, three } | Turn::Swap { front, three })), _) => {
					let fit = union(front.bounds(), three.bounds());
					let half = if matches!(turn, Turn::Blend { .. }) {
						"blended"
					} else {
						"swaps here"
					};
					vec![
						tile(format!("{name}.front"), "", Some(front.clone()), fit),
						tile(
							format!("{name}, half-turned"),
							half,
							Some(turn.at(0.5)),
							fit,
						),
						tile(format!("{name}.3q"), "", Some(three.clone()), fit),
					]
				}
				(Some(Turn::Fixed(shape)), _) => {
					vec![tile(name.clone(), "", Some(shape.clone()), shape.bounds())]
				}
				(None, _) if spec.need == Need::Required => {
					let missing = if spec.views == Views::Turning {
						"missing: .front and .3q"
					} else {
						"missing"
					};
					vec![tile(
						name.clone(),
						missing,
						None,
						(pt(0.0, 0.0), pt(1.0, 1.0)),
					)]
				}
				(None, _) => Vec::new(),
			};

			if !group.is_empty() {
				groups.push(group);
			}
		}
	}

	let findings: Vec<String> = parts
		.check()
		.into_iter()
		.filter(|finding| finding.level != Level::Note)
		.map(|finding| finding.to_string())
		.collect();
	let mut placed: Vec<(f32, f32, Tile)> = Vec::new();
	let top = 170.0 + findings.len() as f32 * 34.0;
	let (mut column, mut row) = (0, 0);

	for group in groups {
		if column + group.len() > COLUMNS {
			(column, row) = (0, row + 1);
		}

		for tile in group {
			placed.push((column as f32 * TILE, top + row as f32 * TILE, tile));
			column += 1;
		}
	}

	let width = TILE * COLUMNS as f32;
	let height = top + (row + 1) as f32 * TILE + 40.0;
	let mut svg = Svg::new();
	svg.fill(&Path::rect(0.0, 0.0, width, height), Color(0xFB_F3_E4));
	let label = |svg: &mut Svg, x: f32, y: f32, size: f32, color: Color, text: &str| {
		svg.raw(&format!(
			r#"<text x="{x:.1}" y="{y:.1}" font-family="{}" font-size="{size}" fill="{color}">{}</text>"#,
			raster::BODY_FAMILY,
			escape(text)
		));
	};
	label(&mut svg, 40.0, 80.0, 52.0, ink, title);

	for (index, finding) in findings.iter().enumerate() {
		let color = if finding.starts_with("error") {
			palette::TOMATO
		} else {
			Color(0x8A_6A_2A)
		};
		label(
			&mut svg,
			40.0,
			140.0 + index as f32 * 34.0,
			26.0,
			color,
			finding,
		);
	}

	for (x, y, tile) in &placed {
		let card = Path::rect(x + 12.0, y + 12.0, TILE - 24.0, TILE - 24.0);
		svg.fill(&card, palette::WHITE.mix(Color(0xFB_F3_E4), 0.4));
		let (lo, hi) = tile.fit;
		let size = hi - lo;
		let scale = ((TILE - 110.0) / size.x.max(size.y).max(1.0)).min(4.0);
		let centre = lo.lerp(hi, 0.5);

		match &tile.shape {
			Some(shape) => {
				svg.begin(&format!(
					"translate({:.1} {:.1}) scale({scale:.4}) translate({:.1} {:.1})",
					x + TILE / 2.0,
					y + TILE / 2.0 - 18.0,
					-centre.x,
					-centre.y
				));
				let weight = 3.0 / scale;

				if shape.is_closed() {
					svg.brushed(&shape.path(), Color(0xF3_E7_D3), ink, weight);
				} else {
					svg.brush_line(&shape.path(), ink, weight * 1.4);
				}

				svg.end();
			}
			None => {
				svg.raw(&format!(
					r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="none" stroke="{}" stroke-width="4" stroke-dasharray="14 10"/>"#,
					x + 40.0,
					y + 40.0,
					TILE - 80.0,
					TILE - 130.0,
					palette::TOMATO
				));
			}
		}

		label(&mut svg, x + 28.0, y + TILE - 50.0, 22.0, ink, &tile.label);

		if !tile.note.is_empty() {
			let color = if tile.shape.is_none() || tile.note == "swaps here" {
				palette::TOMATO
			} else {
				Color(0x7A_6A_80)
			};
			label(&mut svg, x + 28.0, y + TILE - 24.0, 20.0, color, &tile.note);
		}
	}

	raster::render(&svg.finish((0.0, 0.0, width, height), width as u32, height as u32))
}

fn union(a: (Pt, Pt), b: (Pt, Pt)) -> (Pt, Pt) {
	(
		pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
		pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn zones() -> Zones {
		Zones::measure(&StylePack::named("cartoon").unwrap()).unwrap()
	}

	#[test]
	fn zones_are_measured_from_the_book_layout() {
		let zones = zones();
		let (_, top, _, bottom) = zones.top_card;

		assert!(
			top < STAGE * 0.1 && (STAGE * 0.25..STAGE * 0.45).contains(&bottom),
			"a typical top card covers about the top third: {:?}",
			zones.top_card
		);
		assert!(
			zones.bottom_card.1 > STAGE * 0.55 && zones.bottom_card.3 > STAGE * 0.9,
			"a bottom card sits low: {:?}",
			zones.bottom_card
		);
		assert!(
			zones.grown_up_height > zones.kid_height && zones.kid_height > STAGE * 0.35,
			"a grown-up stands taller than a kid: {zones:?}"
		);
		assert!(
			(zones.pair[0] - 1.0 / 3.0).abs() < 1e-4 && (zones.pair[1] - 2.0 / 3.0).abs() < 1e-4,
			"two actors share the page in thirds: {:?}",
			zones.pair
		);
		assert!(
			(zones.lone - 0.5).abs() < 1e-4,
			"one actor stands in the middle"
		);
		assert!(
			(150.0..400.0).contains(&zones.half_width),
			"{}",
			zones.half_width
		);
	}

	#[test]
	fn zones_draw_only_when_asked() {
		let library = Library::built_in();
		let style = Style::default();
		let places = library.names().count() + 4;

		let plain = places_document(&library, style, None);
		assert!(!plain.contains("data-zone"), "no zones unless asked");

		let zones = zones();
		let outlined = places_document(&library, style, Some(&zones));
		assert_eq!(
			outlined.matches(r#"data-zone="card""#).count(),
			places * 2,
			"every place shows the top and bottom cards"
		);
		assert_eq!(
			outlined.matches(r#"data-zone="spot""#).count(),
			places * 3,
			"every place shows the three default spots"
		);
		let before_zones = outlined.split("<g data-zone").next().unwrap_or_default();
		assert!(
			plain.starts_with(before_zones),
			"the first setting is drawn exactly as on the plain sheet, with the zones over it"
		);
	}
}
