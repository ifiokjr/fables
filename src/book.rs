//! Assembles the book: front matter, story pages and back matter, each as art plus text.
//!
//! Pages are described once here and then pressed into every format: proof PNGs, the ePub
//! (art as an image, text as live HTML) and the print PDF (art as a 300 DPI image, text as
//! vector type).

use std::collections::HashMap;

use anyhow::Result;

use crate::art::Bounds;
use crate::art::Path;
use crate::art::STAGE;
use crate::art::Svg;
use crate::art::palette;
use crate::art::pt;
use crate::art::settings::Library;
use crate::art::style::Style;
use crate::cast::Cast;
use crate::compose;
use crate::stage::Rehearsal;
use crate::stage::Scene;
use crate::stage::Traits;
use crate::stage::rehearse;
use crate::story::Directive;
use crate::story::Story;
use crate::story::TextPlace;
use crate::story::Token;
use crate::text;
use crate::text::Anchor;
use crate::text::Area;
use crate::text::Block;
use crate::text::Ink;

/// KDP's minimum page count for a paperback.
pub const MIN_PAGES: usize = 24;

/// Distance from the trim edge that text must stay inside (KDP: 0.375" for bleed books).
pub const SAFE: f32 = STAGE * 0.375 / 8.5;

/// Body text size in stage units (about 30 pt on an 8.5" page).
const BODY_SIZE: f32 = 116.0;

/// A block of text on a page.
#[derive(Clone, Debug)]
pub struct TextSpec {
	pub block: Block,
	pub ink: Ink,
	/// Sit the text on a paper card so it reads over any art.
	pub card: bool,
}

/// Decorations drawn on top of the scene for special pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decor {
	None,
	DrawingFrame,
	/// Nothing at all: plain paper.
	Blank,
}

/// One printed page.
#[derive(Clone, Debug)]
pub struct BookPage {
	/// Human-readable name for logs and file names ("title", "story-03").
	pub label: String,
	pub scene: Scene,
	pub texts: Vec<TextSpec>,
	pub decor: Decor,
	/// Description of the illustration for screen readers.
	pub alt: String,
	/// The art style the page is drawn in.
	pub style: Style,
}

/// Lays out the whole book.
pub fn assemble(story: &Story, cast: &Cast) -> Result<Vec<BookPage>> {
	let rehearsals = rehearse(story, cast)?;
	let mut pages = vec![title_page(story, cast)?, copyright_page(story)];

	for rehearsal in &rehearsals {
		let page = &story.pages[rehearsal.page];

		if page.in_book() {
			pages.push(story_page(story, cast, rehearsal));
		}
	}

	pages.push(meet_the_folk(story, cast)?);

	// Short books are padded to KDP's minimum: one activity page, then plain endpapers.
	if pages.len() < MIN_PAGES || pages.len() % 2 == 1 {
		pages.push(drawing_page(story, pages.len()));
	}

	while pages.len() < MIN_PAGES || pages.len() % 2 == 1 {
		pages.push(blank_page(pages.len()));
	}

	for page in &mut pages {
		page.style = story.style();
	}

	Ok(pages)
}

fn story_page(story: &Story, cast: &Cast, rehearsal: &Rehearsal) -> BookPage {
	let page = &story.pages[rehearsal.page];
	let scene = rehearsal.finale().clone();
	let paragraphs = page.book_text();
	let mut texts = Vec::new();

	if !paragraphs.is_empty() && scene.text != TextPlace::None {
		let (area, anchor) = match scene.text {
			TextPlace::Bottom => {
				(
					Area {
						x: SAFE + 120.0,
						y: STAGE - SAFE - 760.0,
						w: STAGE - 2.0 * (SAFE + 120.0),
						h: 700.0,
					},
					Anchor::Bottom,
				)
			}
			_ => {
				(
					Area {
						x: SAFE + 120.0,
						y: SAFE + 70.0,
						w: STAGE - 2.0 * (SAFE + 120.0),
						h: 760.0,
					},
					Anchor::Top,
				)
			}
		};
		let size = page
			.option("size")
			.and_then(|token| token.value.as_deref()?.parse::<f32>().ok())
			.map_or(BODY_SIZE, |scale| BODY_SIZE * scale);

		texts.push(TextSpec {
			block: text::layout(&paragraphs, area, size, anchor),
			ink: Ink::default(),
			card: true,
		});
	}

	BookPage {
		// Named after the script page so proofs map straight back to the script.
		label: format!("story-{:02}", rehearsal.page + 1),
		alt: describe(&scene, cast),
		scene,
		texts,
		decor: Decor::None,
		style: Style::default(),
	}
}

/// The cover star on the cover background, posed by `pose` (e.g. `"x=0.5 grin wave"`).
///
/// `memory` supplies traits such as teeth and breath, so the back cover can show the star as
/// they are at the end of the story.
pub fn star_scene(
	story: &Story,
	cast: &Cast,
	pose: &str,
	scale: f32,
	memory: &mut HashMap<String, Traits>,
) -> Result<Scene> {
	let cover = story.meta.cover.clone().unwrap_or_default();
	let setting = cover.setting.clone().unwrap_or_else(|| "plain".into());
	let options = cover
		.tint
		.iter()
		.map(|tint| {
			Token {
				key: "tint".into(),
				value: Some(tint.clone()),
			}
		})
		.collect();
	let page = crate::story::Page {
		setting,
		options,
		beats: Vec::new(),
		line: 0,
	};
	let mut scene = Scene::for_page(&page, &story.settings, &story.props)?;

	if let Some(star) = &story.meta.star {
		let directive = Directive {
			actor: star.clone(),
			tokens: crate::story::tokens(pose),
			line: 0,
		};
		scene.apply(&directive, cast, memory)?;

		if let Some(actor) = scene.actors.last_mut() {
			actor.size *= scale;
		}
	}

	Ok(scene)
}

fn cover_pose(story: &Story) -> String {
	story
		.meta
		.cover
		.as_ref()
		.and_then(|cover| cover.pose.clone())
		.unwrap_or_else(|| "x=0.5 grin wave".into())
}

/// The front cover: the star in their cover pose under the title.
pub fn front_cover(story: &Story, cast: &Cast) -> Result<BookPage> {
	let meta = &story.meta;
	let scene = star_scene(story, cast, &cover_pose(story), 1.3, &mut HashMap::new())?;
	let mut heading = Vec::new();

	if let Some(series) = &meta.series {
		heading.push(match meta.number {
			Some(number) => format!("{series} · {number}"),
			None => series.clone(),
		});
	}

	heading.push(format!("**{}**", meta.title));
	let refs: Vec<&str> = heading.iter().map(String::as_str).collect();
	let title = text::layout(
		&refs,
		Area {
			x: SAFE + 40.0,
			y: SAFE + 30.0,
			w: STAGE - 2.0 * (SAFE + 40.0),
			h: 760.0,
		},
		150.0,
		Anchor::Top,
	);
	let byline = format!("*{}*", meta.author);
	let author = text::layout(
		&[byline.as_str()],
		Area {
			x: SAFE,
			y: STAGE - SAFE - 200.0,
			w: STAGE - 2.0 * SAFE,
			h: 170.0,
		},
		96.0,
		Anchor::Bottom,
	);

	Ok(BookPage {
		label: "front-cover".into(),
		alt: format!("Cover of {} by {}.", meta.title, meta.author),
		scene,
		texts: vec![
			TextSpec {
				block: title,
				ink: Ink::default(),
				card: false,
			},
			TextSpec {
				block: author,
				ink: Ink::default(),
				card: false,
			},
		],
		decor: Decor::None,
		style: story.style(),
	})
}

/// The back cover: the blurb, and the star as they are by the end of the story.
///
/// The lower-right corner is left clear because KDP prints its barcode there (2" × 1.2").
pub fn back_cover(story: &Story, cast: &Cast) -> Result<BookPage> {
	let meta = &story.meta;
	let mut memory = HashMap::new();

	if let Some(star) = &meta.star {
		let finale = rehearse(story, cast)?
			.into_iter()
			.rev()
			.find_map(|rehearsal| {
				rehearsal
					.finale()
					.actors
					.iter()
					.find(|actor| &actor.id == star)
					.cloned()
			});

		if let Some(actor) = finale {
			memory.insert(
				star.clone(),
				Traits {
					teeth: actor.teeth,
					stink: actor.stink,
					mint: actor.mint,
					see: actor.eyes.see.clone(),
				},
			);
		}
	}

	let scene = star_scene(story, cast, "x=0.26 wave grin talk", 0.95, &mut memory)?;
	let mut paragraphs = Vec::new();

	if let Some(blurb) = &meta.blurb {
		paragraphs.push(blurb.clone());
	}

	if let Some(moral) = &meta.moral {
		paragraphs.push(format!("*{moral}*"));
	}

	let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
	let blurb = text::layout(
		&refs,
		Area {
			x: SAFE + 120.0,
			y: SAFE + 90.0,
			w: STAGE - 2.0 * (SAFE + 120.0),
			h: 900.0,
		},
		76.0,
		Anchor::Top,
	);

	Ok(BookPage {
		label: "back-cover".into(),
		alt: String::new(),
		scene,
		texts: vec![TextSpec {
			block: blurb,
			ink: Ink::default(),
			card: true,
		}],
		decor: Decor::None,
		style: story.style(),
	})
}

fn title_page(story: &Story, cast: &Cast) -> Result<BookPage> {
	let scene = star_scene(story, cast, &cover_pose(story), 1.15, &mut HashMap::new())?;
	let meta = &story.meta;
	let area = Area {
		x: SAFE + 60.0,
		y: SAFE + 40.0,
		w: STAGE - 2.0 * (SAFE + 60.0),
		h: 640.0,
	};
	let mut paragraphs = vec![format!("**{}**", meta.title)];

	if let Some(subtitle) = &meta.subtitle {
		paragraphs.push(subtitle.clone());
	}

	let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
	let title = text::layout(&refs, area, 150.0, Anchor::Top);
	let byline = format!("by *{}*", meta.author);
	let byline_block = text::layout(
		&[byline.as_str()],
		Area {
			x: SAFE,
			y: STAGE - SAFE - 200.0,
			w: STAGE - 2.0 * SAFE,
			h: 160.0,
		},
		80.0,
		Anchor::Bottom,
	);

	Ok(BookPage {
		label: "title".into(),
		alt: describe(&scene, cast),
		scene,
		texts: vec![
			TextSpec {
				block: title,
				ink: Ink::default(),
				card: false,
			},
			TextSpec {
				block: byline_block,
				ink: Ink::default(),
				card: false,
			},
		],
		decor: Decor::None,
		style: Style::default(),
	})
}

fn plain_scene(tint: crate::art::Color) -> Scene {
	let page = crate::story::Page {
		setting: "plain".into(),
		options: Vec::new(),
		beats: Vec::new(),
		line: 0,
	};
	let mut scene = Scene::for_page(
		&page,
		&Library::built_in(),
		&crate::art::props::Library::built_in(),
	)
	.unwrap_or_else(|error| unreachable!("a bare plain page always builds: {error}"));
	scene.weather.tint = tint;
	scene
}

fn copyright_page(story: &Story) -> BookPage {
	let meta = &story.meta;
	let year = meta.year.unwrap_or(2026);
	let illustrator = meta.illustrator.as_deref().unwrap_or(&meta.author);
	let mut lines = vec![format!("*{}*", meta.title)];

	if let Some(series) = &meta.series {
		lines.push(match meta.number {
			Some(number) => format!("{series}, book {number}"),
			None => series.clone(),
		});
	}

	lines.push(format!(
		"Text copyright © {year} {}. Illustrations copyright © {year} {illustrator}.",
		meta.author
	));
	lines.push(
		"All rights reserved. No part of this book may be reproduced without written permission \
		 from the publisher."
			.into(),
	);

	if let Some(publisher) = &meta.publisher {
		lines.push(format!("Published by {publisher}."));
	}

	if let Some(isbn) = &meta.isbn {
		lines.push(format!("ISBN {isbn}"));
	}

	lines.push("First edition.".into());

	let mut paragraphs = Vec::new();

	if let Some(dedication) = &meta.dedication {
		paragraphs.push(format!("*{dedication}*"));
	}

	if let Some(note) = &story.matter.note {
		paragraphs.push(note.clone());
	}

	let small = lines.iter().map(String::as_str).collect::<Vec<_>>();
	let fine_print = text::layout(
		&small,
		Area {
			x: SAFE + 200.0,
			y: STAGE * 0.52,
			w: STAGE - 2.0 * (SAFE + 200.0),
			h: STAGE * 0.4,
		},
		44.0,
		Anchor::Bottom,
	);
	let notes = paragraphs.iter().map(String::as_str).collect::<Vec<_>>();
	let dedication = text::layout(
		&notes,
		Area {
			x: SAFE + 200.0,
			y: SAFE + 200.0,
			w: STAGE - 2.0 * (SAFE + 200.0),
			h: STAGE * 0.35,
		},
		70.0,
		Anchor::Top,
	);
	let scene = plain_scene(palette::PAPER);

	BookPage {
		label: "copyright".into(),
		scene,
		texts: vec![
			TextSpec {
				block: dedication,
				ink: Ink::default(),
				card: false,
			},
			TextSpec {
				block: fine_print,
				ink: Ink::default(),
				card: false,
			},
		],
		decor: Decor::None,
		style: Style::default(),
		alt: String::new(),
	}
}

/// A line-up of the book's cast members (excluding the star), naming each one.
fn meet_the_folk(story: &Story, cast: &Cast) -> Result<BookPage> {
	let mut scene = plain_scene(palette::MINT);
	let star = story.meta.star.as_deref();
	let folk: Vec<&crate::cast::Member> = cast
		.members
		.values()
		.filter(|member| {
			member.kind.is_figure() && Some(member.id.as_str()) != star && member.habit.is_some()
		})
		.take(4)
		.collect();
	let count = folk.len().max(1) as f32;
	let mut captions = Vec::new();

	for (i, member) in folk.iter().enumerate() {
		let x = (i as f32 + 1.0) / (count + 1.0);
		let directive = Directive {
			actor: member.id.clone(),
			tokens: vec![
				Token {
					key: "x".into(),
					value: Some(format!("{x:.3}")),
				},
				Token {
					key: "size".into(),
					value: Some("1.55".into()),
				},
				Token {
					key: if i % 2 == 0 { "wave" } else { "grin" }.into(),
					value: None,
				},
			],
			line: 0,
		};
		scene.apply(&directive, cast, &mut HashMap::new())?;
		captions.push((x, member.name.clone()));
	}

	let heading: Vec<&str> = story.matter.meet.iter().map(String::as_str).collect();
	let heading = text::layout(
		&heading,
		Area {
			x: SAFE + 120.0,
			y: SAFE + 70.0,
			w: STAGE - 2.0 * (SAFE + 120.0),
			h: 600.0,
		},
		96.0,
		Anchor::Top,
	);
	let mut texts = vec![TextSpec {
		block: heading,
		ink: Ink::default(),
		card: true,
	}];

	for (x, name) in captions {
		let width = STAGE / (count + 1.0) - 20.0;
		texts.push(TextSpec {
			block: text::layout(
				&[name.as_str()],
				Area {
					x: x * STAGE - width / 2.0,
					y: STAGE - SAFE - 230.0,
					w: width,
					h: 200.0,
				},
				64.0,
				Anchor::Bottom,
			),
			ink: Ink::default(),
			card: false,
		});
	}

	Ok(BookPage {
		label: "meet-the-folk".into(),
		alt: describe(&scene, cast),
		scene,
		texts,
		decor: Decor::None,
		style: Style::default(),
	})
}

/// An empty paper page (an endpaper).
fn blank_page(number: usize) -> BookPage {
	BookPage {
		label: format!("blank-{number:02}"),
		scene: plain_scene(palette::PAPER),
		texts: Vec::new(),
		decor: Decor::Blank,
		style: Style::default(),
		alt: String::new(),
	}
}

fn drawing_page(story: &Story, number: usize) -> BookPage {
	let scene = plain_scene(palette::PAPER);
	let prompt: Vec<&str> = story.matter.draw.iter().map(String::as_str).collect();
	let prompt = text::layout(
		&prompt,
		Area {
			x: SAFE + 120.0,
			y: SAFE + 60.0,
			w: STAGE - 2.0 * (SAFE + 120.0),
			h: 520.0,
		},
		76.0,
		Anchor::Top,
	);

	BookPage {
		label: format!("draw-{number:02}"),
		scene,
		texts: vec![TextSpec {
			block: prompt,
			ink: Ink::default(),
			card: false,
		}],
		decor: Decor::DrawingFrame,
		style: Style::default(),
		alt: "An empty frame to draw in.".into(),
	}
}

/// Plain-language description of a scene for alt text.
pub fn describe(scene: &Scene, cast: &Cast) -> String {
	let place = scene.setting.name();
	let mut who = Vec::new();

	for (actor, _) in scene.placed() {
		if actor.id == "wormsley" {
			continue;
		}

		let name = cast.member(&actor.id).map_or_else(
			|| actor.id.split('.').next().unwrap_or("someone").to_owned(),
			|m| m.name.clone(),
		);
		let mood = crate::art::figure::Mood::ALL
			.iter()
			.find(|(_, mood)| *mood == actor.mood)
			.map_or("", |(name, _)| *name);

		if actor.kind.is_figure() {
			who.push(format!("{name}, looking {mood}"));
		} else if actor.knocked > 0.5 {
			who.push(format!("a {name}, knocked out cold"));
		} else {
			who.push(format!("a {name}"));
		}
	}

	if who.is_empty() {
		return format!("An illustration of the {place}.");
	}

	format!("In the {place}: {}.", who.join("; "))
}

/// Which side of the spread a printed page sits on; bleed only extends past the outer edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
	/// Right-hand page (odd page numbers): bleed on the right.
	Recto,
	/// Left-hand page (even page numbers): bleed on the left.
	Verso,
	/// No bleed at all (screens).
	Screen,
}

/// Print bleed: 0.125" in stage units.
pub const BLEED: f32 = STAGE * 0.125 / 8.5;

/// The view box for a page side: the trim square plus bleed on top, bottom and outer edge.
pub fn view_box(side: Side) -> (f32, f32, f32, f32) {
	match side {
		Side::Recto => (0.0, -BLEED, STAGE + BLEED, STAGE + 2.0 * BLEED),
		Side::Verso => (-BLEED, -BLEED, STAGE + BLEED, STAGE + 2.0 * BLEED),
		Side::Screen => (0.0, 0.0, STAGE, STAGE),
	}
}

/// Draws a page's art (and optionally its text) as an SVG document.
pub fn page_svg(page: &BookPage, side: Side, width: u32, height: u32, with_text: bool) -> String {
	let mut svg = Svg::new();
	draw_art(&mut svg, page);

	if with_text {
		draw_text(&mut svg, page);
	}

	svg.finish(view_box(side), width, height)
}

/// The art layer: scene, text cards and decorations, but no lettering.
pub fn draw_art(svg: &mut Svg, page: &BookPage) {
	svg.set_style(page.style);

	if page.decor == Decor::Blank {
		svg.fill(
			&Path::rect(
				-BLEED * 2.0,
				-BLEED * 2.0,
				STAGE + BLEED * 4.0,
				STAGE + BLEED * 4.0,
			),
			palette::PAPER,
		);
		return;
	}

	let puppets = compose::puppets(&page.scene, 0.0, true);
	let zoom = page.scene.camera.zoom;
	let camera = &page.scene.camera;

	// The scene draws under its light; the text card and drawing frame after it are page
	// furniture and keep their own colours.
	svg.set_light(page.scene.weather.light);
	svg.begin_layer();

	// A heading's `shot=` frames the still like the video would frame it: composed from the
	// puppets, with headroom and lead room, and inside the part of the page the text card
	// leaves clear. It wins over a bare `zoom=`.
	let shot_transform = camera.shot.and_then(|size| {
		let card = card_rects(page)
			.into_iter()
			.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
		crate::video::director::still_view(&page.scene, &puppets, size, &camera.on, card)
	});

	if let Some(view) = shot_transform {
		let centre = STAGE / 2.0;
		svg.begin(&format!(
			"translate({:.1} {:.1}) scale({:.3}) translate({:.1} {:.1})",
			centre,
			centre,
			STAGE / view.height,
			-view.centre.x,
			-view.centre.y
		));
		compose::draw_scene(svg, &page.scene, &puppets, 0.0);
		svg.end();
	} else if (zoom - 1.0).abs() > 0.001 {
		let (lx, ly) = page.scene.camera.look;
		let focus = pt(lx * STAGE, ly * STAGE);
		let centre = STAGE / 2.0;
		svg.begin(&format!(
			"translate({:.1} {:.1}) scale({zoom:.3}) translate({:.1} {:.1})",
			centre, centre, -focus.x, -focus.y
		));
		compose::draw_scene(svg, &page.scene, &puppets, 0.0);
		svg.end();
	} else {
		compose::draw_scene(svg, &page.scene, &puppets, 0.0);
	}

	svg.end();
	svg.clear_light();

	if page.decor == Decor::DrawingFrame {
		let frame = Path::wobbly_poly(
			&[
				pt(SAFE + 80.0, 800.0),
				pt(STAGE - SAFE - 80.0, 800.0),
				pt(STAGE - SAFE - 80.0, STAGE - SAFE - 80.0),
				pt(SAFE + 80.0, STAGE - SAFE - 80.0),
			],
			40.0,
			6.0,
			31,
		);
		svg.raw(&format!(
			r#"<path d="{}" fill="{}" stroke="{}" stroke-width="10" stroke-dasharray="40 26" stroke-linecap="round"/>"#,
			frame.as_str(),
			palette::WHITE,
			palette::INK
		));
	}

	for (x, top, right, bottom) in card_rects(page) {
		let card = Path::wobbly_poly(
			&[pt(x, top), pt(right, top), pt(right, bottom), pt(x, bottom)],
			46.0,
			5.0,
			crate::art::seed_of(&page.label),
		);
		svg.inked(&card, palette::PAPER);
	}
}

/// The paper card behind each text block, as it is drawn: the block's bounds plus the
/// card's padding. Still framing reads this to keep its subject in the part of the page the
/// card leaves clear, so the rectangle here is the rectangle the drawn card covers.
pub fn card_rects(page: &BookPage) -> Vec<Bounds> {
	const PAD_X: f32 = 80.0;
	const PAD_Y: f32 = 60.0;

	page.texts
		.iter()
		.filter(|spec| spec.card && !spec.block.pieces.is_empty())
		.map(|spec| {
			let b = spec.block.bounds;
			(
				b.x - PAD_X,
				b.y - PAD_Y,
				b.x + b.w + PAD_X,
				b.y + b.h + PAD_Y * 0.8,
			)
		})
		.collect()
}

/// The lettering layer.
pub fn draw_text(svg: &mut Svg, page: &BookPage) {
	for spec in &page.texts {
		svg.raw(&text::svg(&spec.block, spec.ink));
	}
}

/// All the page's words as plain text, for accessibility and logs.
pub fn page_words(page: &BookPage) -> String {
	let mut out = String::new();

	for spec in &page.texts {
		for piece in &spec.block.pieces {
			if !out.is_empty()
				&& (piece.space_before || std::ptr::eq(piece, &raw const spec.block.pieces[0]))
			{
				out.push(' ');
			}

			out.push_str(&piece.text);
		}
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::story;

	fn sample() -> (Story, Cast) {
		let story = story::parse(
			"+++\ntitle = \"Test Book\"\nauthor = \"A. Writer\"\nstar = \"kid\"\n+++\n# \
			 park\n@kid wave talk\nHello there.\n# bathroom text=bottom\n@kid brush \
			 holds=toothbrush\nBrush, brush.\n",
		)
		.unwrap();
		let cast = Cast::parse(
			"[kid]\nname = \"Kid McKidface\"\nhabit = \"Testing\"\n[wormsley]\nname = \
			 \"Wormsley\"\nkind = \"worm\"\n",
		)
		.unwrap();
		(story, cast)
	}

	#[test]
	fn books_meet_the_kdp_minimum_and_are_even() {
		let (story, cast) = sample();
		let pages = assemble(&story, &cast).unwrap();
		assert!(pages.len() >= MIN_PAGES);
		assert_eq!(pages.len() % 2, 0);
		assert_eq!(pages[0].label, "title");
		assert_eq!(pages[1].label, "copyright");
	}

	#[test]
	fn story_text_stays_inside_the_safe_area() {
		let (story, cast) = sample();

		for page in assemble(&story, &cast).unwrap() {
			for spec in &page.texts {
				for piece in &spec.block.pieces {
					assert!(
						piece.x >= SAFE && piece.x + piece.width <= STAGE - SAFE,
						"{}: {piece:?}",
						page.label
					);
					assert!(
						piece.baseline - piece.size >= SAFE && piece.baseline <= STAGE - SAFE,
						"{}: {piece:?}",
						page.label
					);
				}
			}
		}
	}

	#[test]
	fn wormsley_hides_on_story_pages() {
		let (story, cast) = sample();
		let pages = assemble(&story, &cast).unwrap();
		assert!(
			pages[2]
				.scene
				.actors
				.iter()
				.any(|actor| actor.id == "wormsley")
		);
	}

	#[test]
	fn alt_text_names_the_cast() {
		let (story, cast) = sample();
		let pages = assemble(&story, &cast).unwrap();
		assert!(pages[2].alt.contains("Kid McKidface"), "{}", pages[2].alt);
		assert!(
			!pages[2].alt.contains("Wormsley"),
			"the hidden worm stays hidden"
		);
	}

	#[test]
	fn words_are_recovered_from_layout() {
		let (story, cast) = sample();
		let pages = assemble(&story, &cast).unwrap();
		assert_eq!(page_words(&pages[2]), "Hello there.");
	}

	#[test]
	fn bleed_is_only_on_the_outer_edge() {
		let (x, _, w, _) = view_box(Side::Recto);
		assert!(x.abs() < f32::EPSILON && (w - (STAGE + BLEED)).abs() < 0.01);
		let (x, ..) = view_box(Side::Verso);
		assert!((x + BLEED).abs() < 0.01);
	}
}
