//! End-to-end checks on the camera-test story: the automatic director's shot plan, the
//! storyboard's panels and labels, and a still framed by a page heading's shot. Narration is
//! estimated from word counts, so nothing here records or needs a voice.

use std::path::Path;

use fables::cast::Cast;
use fables::press;
use fables::stage;
use fables::story::Story;
use fables::video;
use fables::video::frames::Format;
use fables::video::frames::Renderer;
use fables::video::timeline::Framing;
use fables::video::timeline::ShotKind;
use fables::video::voice::SAMPLE_RATE;
use fables::video::voice::Speech;
use fables::video::voice::Utterance;
use fables::world::World;

/// The director's minimum setup length, for assertions in this file.
const MIN_SETUP: f32 = 1.2;

/// The storyboard's offline speaker: about 2.6 words a second.
fn estimated(story: &Story, cast: &Cast) -> video::timeline::Timeline {
	let rehearsals = stage::rehearse(story, cast).unwrap();

	video::timeline::build(story, cast, &rehearsals, |line: &Utterance<'_>| {
		let seconds = (line.text.split_whitespace().count() as f32 / 2.6).max(0.3);
		let samples = (seconds * SAMPLE_RATE as f32) as usize;

		Ok(Speech {
			text: String::new(),
			samples: vec![0.0; samples],
			envelope: Vec::new(),
			track: video::lipsync::Track::default(),
		})
	})
	.unwrap()
}

fn camera_test() -> (Story, Cast) {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut story =
		Story::load(&root.join("worlds/mumbleton/stories/camera-test/story.md")).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast().unwrap();

	(story, cast)
}

#[test]
fn depth_of_field_softens_the_backdrop_without_black_holes() {
	let (story, cast) = camera_test();
	let timeline = estimated(&story, &cast);

	// Page 3 cuts to an extreme close-up: its setting arrives softened, from the cache.
	let page = timeline
		.shots
		.iter()
		.find(|shot| shot.kind == ShotKind::Page(2))
		.expect("page 3 is in the video");
	let setups = page.setups_for(false);
	let index = setups
		.iter()
		.position(|setup| {
			matches!(&setup.framing, Framing::Shot { size, .. } if *size == stage::ShotSize::ExtremeClose)
		})
		.expect("page 3 holds an extreme close-up");
	let at = setups[index].time + 0.5;

	let mut sharp_story = camera_test().0;
	sharp_story.meta.video.depth_of_field = false;
	let sharp_timeline = estimated(&sharp_story, &cast);

	let frame = 4;
	let soft = Renderer::new(&timeline, &story, Format::Landscape, 960, 540)
		.render(at, frame)
		.unwrap();
	let crisp = Renderer::new(&sharp_timeline, &sharp_story, Format::Landscape, 960, 540)
		.render(at, frame)
		.unwrap();

	let soft_pixels = soft.data().as_chunks::<4>().0;
	let crisp_pixels = crisp.data().as_chunks::<4>().0;
	assert_eq!(soft_pixels.len(), crisp_pixels.len());

	// Wherever the sharp frame shows something that is not black, the softened frame must
	// show it too: a blurred layer cached without alpha would sit there as pure black.
	let mut differing = 0;

	for (soft, crisp) in soft_pixels.iter().zip(crisp_pixels) {
		let blackened = soft[..3].iter().all(|channel| *channel <= 2);
		let visible = crisp[..3].iter().any(|channel| *channel > 8);

		assert!(
			!(blackened && visible),
			"a pixel that is {crisp:?} in the sharp frame is black in the depth-of-field frame"
		);

		if soft[..3] != crisp[..3] {
			differing += 1;
		}
	}

	assert!(
		differing * 20 > soft.width() * soft.height(),
		"the backdrop is genuinely softened: only {differing} of {} pixels differ",
		soft.width() * soft.height()
	);
}

#[test]
fn no_setup_runs_shorter_than_the_minimum() {
	let (story, cast) = camera_test();
	let timeline = estimated(&story, &cast);

	for portrait in [false, true] {
		for shot in &timeline.shots {
			let setups = shot.setups_for(portrait);
			let page = shot
				.kind
				.page()
				.map_or("title/end".to_owned(), |p| (p + 1).to_string());

			for (index, setup) in setups.iter().enumerate() {
				let end = setups.get(index + 1).map_or(shot.end, |next| next.time);

				assert!(
					end - setup.time >= MIN_SETUP - 1e-3,
					"page {page} (portrait {portrait}) setup {index} holds only {:.2}s: {:?}",
					end - setup.time,
					setups.iter().map(|s| &s.framing).collect::<Vec<_>>()
				);
			}
		}
	}
}

#[test]
fn the_board_shows_different_shots_with_labels() {
	let (story, cast) = camera_test();
	let timeline = estimated(&story, &cast);
	let landscape = press::storyboard::panels(&story, &timeline, &cast, false).unwrap();

	// Every page opens on an establishing shot and gets closer when someone speaks, so the
	// board holds several framings.
	let sizes: Vec<String> = landscape
		.iter()
		.map(|panel| panel.label.split(" · ").nth(1).unwrap_or("").to_owned())
		.collect();

	for shot in ["wide", "medium", "close", "extreme-close", "full"] {
		assert!(
			sizes.iter().any(|size| size.starts_with(shot)),
			"no {shot} panel in {sizes:?}"
		);
	}

	// Every transition is used once, arriving at its page.
	for transition in ["push", "wipe", "cut", "dissolve", "fade", "iris"] {
		assert!(
			landscape
				.iter()
				.any(|panel| panel.label.contains(&format!("{transition} in"))),
			"no panel enters with {transition}: {:?}",
			landscape.iter().map(|p| &p.label).collect::<Vec<_>>()
		);
	}

	// The explicit @camera page is shot as written: an extreme close-up on Mo.
	assert!(
		landscape
			.iter()
			.any(|panel| panel.label.contains("extreme-close on Mo Mumble")),
		"{:?}",
		landscape.iter().map(|p| &p.label).collect::<Vec<_>>()
	);

	// Short lines share a two-shot: both characters in one medium.
	assert!(
		landscape
			.iter()
			.any(|panel| panel.label.contains("medium on Mo Mumble, dog")),
		"{:?}",
		landscape.iter().map(|p| &p.label).collect::<Vec<_>>()
	);

	// The entrance widens so the action stays in frame.
	assert!(
		landscape.iter().any(|panel| panel.label.contains("full ·")),
		"{:?}",
		landscape.iter().map(|p| &p.label).collect::<Vec<_>>()
	);

	// A phone shoots closer: portrait panels favour close singles.
	let portrait = press::storyboard::panels(&story, &timeline, &cast, true).unwrap();
	let portrait_sizes: Vec<String> = portrait
		.iter()
		.map(|panel| panel.label.split(" · ").nth(1).unwrap_or("").to_owned())
		.collect();

	assert!(
		portrait_sizes
			.iter()
			.filter(|size| size.starts_with("close"))
			.count() > sizes
			.iter()
			.filter(|size| size.starts_with("close"))
			.count(),
		"portrait favours close shots: landscape {sizes:?} vs portrait {portrait_sizes:?}"
	);

	// Every hold states its seconds and is at least the minimum, unless the board marks it
	// `· short` — a hold only a script-written cut could leave behind.
	for panel in landscape.iter().chain(portrait.iter()) {
		let stated = panel.label.split(" · ").nth(2).unwrap_or("");
		let seconds = stated.strip_suffix('s').unwrap_or_default();

		if panel.label.ends_with(" · short") {
			let hold: f32 = seconds.parse().unwrap_or(0.0);
			assert!(
				hold < MIN_SETUP,
				"only a short hold is marked: {}",
				panel.label
			);
			continue;
		}

		let hold: f32 = seconds
			.parse()
			.unwrap_or_else(|_| panic!("{} states its hold", panel.label));
		assert!(
			hold >= MIN_SETUP,
			"{} holds only {hold:.1}s, under the minimum",
			panel.label
		);
	}
}

#[test]
fn every_page_opens_on_an_establishing_shot() {
	let (story, cast) = camera_test();
	let timeline = estimated(&story, &cast);

	for shot in &timeline.shots {
		if shot.kind.page().is_none() {
			continue;
		}

		let setups = shot.setups_for(false);
		assert!(
			setups.len() >= 2,
			"page {} gets one setup only: {:?}",
			shot.kind.page().unwrap() + 1,
			setups.iter().map(|s| &s.framing).collect::<Vec<_>>()
		);
		let hold = setups[1].time - setups[0].time;
		assert!(
			hold >= 1.4,
			"page {} opens for only {hold:.2}s",
			shot.kind.page().unwrap() + 1
		);
	}
}

/// The head pixels of a rendered shot's subject sit clear of the frame's edges: at least
/// `HEADROOM` (6% of the height) below the top and `SIDE_MARGIN` (5% of the width) inside
/// each side, measured on the rasterised puppet alone, at the midpoint of the shots the
/// review saw crop — the medium on the left-facing dog, and the medium on Mo, hair and
/// all. Framing measures the puppet's drawn bounds; the pixels are the ground truth.
#[test]
fn measured_framing_keeps_heads_inside_rendered_frames() {
	let (story, cast) = camera_test();
	let timeline = estimated(&story, &cast);
	let format = Format::Landscape;
	let (w, h) = (960, 540);
	let renderer = Renderer::new(&timeline, &story, format, w, h);
	let aspect = format.aspect();

	for (page, subject, what) in [
		(1usize, "dog", "page 2 · medium on dog"),
		(0, "mo", "page 1 · medium on Mo Mumble"),
	] {
		let shot = timeline
			.shots
			.iter()
			.find(|shot| shot.kind.page() == Some(page))
			.unwrap_or_else(|| panic!("{what}: the page is in the video"));
		let setups = shot.setups_for(false);
		let index = setups
			.iter()
			.position(|setup| {
				matches!(&setup.framing,
					Framing::Shot { size: stage::ShotSize::Medium, on }
						if on.len() == 1 && on[0] == subject)
			})
			.unwrap_or_else(|| panic!("{what}: no such setup"));
		let end = setups.get(index + 1).map_or(shot.end, |next| next.time);
		let mid = f32::midpoint(setups[index].time, end);

		// The frame, as the renderer draws it.
		let frame = renderer.render(mid, 4).unwrap();
		assert_eq!((frame.width(), frame.height()), (w, h));
		let view = renderer.view_at(mid);

		// The subject's own pixels, through the same camera, alone on transparent paper.
		let scene = shot.scene_at(mid);
		let puppets = fables::compose::puppets(scene, mid, false);
		let puppet = puppets
			.iter()
			.find(|puppet| puppet.id == subject)
			.unwrap_or_else(|| panic!("{what}: the subject is on stage"));
		let scale = h as f32 / view.height;
		let to_pixel = |x: f32, y: f32| {
			(
				(x - view.centre.x + view.width(aspect) / 2.0) * scale,
				(y - view.centre.y + view.height / 2.0) * scale,
			)
		};
		let mut svg = fables::art::Svg::styled(story.style());
		svg.begin(&format!(
			"scale({scale:.5}) translate({:.2} {:.2})",
			-(view.centre.x - view.width(aspect) / 2.0),
			-(view.centre.y - view.height / 2.0)
		));
		fables::compose::draw_puppet(&mut svg, puppet);
		svg.end();
		let alone =
			fables::raster::render(&svg.finish((0.0, 0.0, w as f32, h as f32), w, h)).unwrap();

		// Head pixels: the puppet's pixels inside its measured head box, opened by a
		// pixel so the window does not cut its own edges.
		let (hl, ht, hr, hb) = puppet.extents().head;
		let (wl, wt) = to_pixel(hl, ht);
		let (wr, wb) = to_pixel(hr, hb);
		let (mut left, mut top, mut right, mut bottom) = (
			f32::INFINITY,
			f32::INFINITY,
			f32::NEG_INFINITY,
			f32::NEG_INFINITY,
		);

		for y in 0..alone.height() {
			for x in 0..alone.width() {
				let inside = x as f32 + 1.0 >= wl
					&& x as f32 <= wr + 1.0
					&& y as f32 + 1.0 >= wt
					&& y as f32 <= wb + 1.0;

				if inside && alone.pixel(x, y).unwrap().alpha() > 0 {
					left = left.min(x as f32);
					top = top.min(y as f32);
					right = right.max(x as f32 + 1.0);
					bottom = bottom.max(y as f32 + 1.0);
				}
			}
		}

		assert!(
			left <= right,
			"{what}: no head pixels in the frame at the head's own box \
			 ({wl:.0},{wt:.0})..({wr:.0},{wb:.0})"
		);

		let headroom = top / h as f32;
		let side = left / w as f32;
		let other_side = (w as f32 - right) / w as f32;

		assert!(
			headroom >= 0.06,
			"{what}: head pixels start {headroom:.1}% of the frame height from the top, under the \
			 6% headroom (box {left:.0},{top:.0})..({right:.0},{bottom:.0}, view {:?})",
			view.centre
		);
		eprintln!(
			"{what}: head {hl:.0},{ht:.0}..{hr:.0},{hb:.0} pixels \
			 {wl:.0},{wt:.0}..{wr:.0},{wb:.0} box {left:.0},{top:.0}..{right:.0},{bottom:.0} \
			 headroom {headroom:.3} sides {side:.3}/{other_side:.3}"
		);
		assert!(
			side >= 0.05 && other_side >= 0.05,
			"{what}: head pixels sit {side:.1}% / {other_side:.1}% of the frame width inside the \
			 side edges, under the 5% margin"
		);
	}
}
