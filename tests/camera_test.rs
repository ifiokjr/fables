//! End-to-end checks on the camera-test story: the automatic director's shot plan, the
//! storyboard's panels and labels, and a still framed by a page heading's shot. Narration is
//! estimated from word counts, so nothing here records or needs a voice.

use std::path::Path;

use fables::cast::Cast;
use fables::press;
use fables::stage;
use fables::story::Story;
use fables::video;
use fables::video::voice::SAMPLE_RATE;
use fables::video::voice::Speech;
use fables::video::voice::Utterance;
use fables::world::World;

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
