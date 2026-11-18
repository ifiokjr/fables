//! The storyboard's text shot list and the pin round trip on the shipped books.

use std::path::Path;
use std::path::PathBuf;

use fables::cast::Cast;
use fables::press::storyboard;
use fables::publish;
use fables::story::Story;
use fables::video::timeline::Timeline;
use fables::world::World;

fn script(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn load(path: &Path) -> (World, Story, Cast) {
	let mut story = Story::load(path).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast_for(&story).unwrap();

	(world, story, cast)
}

fn timed(story: &Story, cast: &Cast, out: &Path) -> Timeline {
	publish::estimated_timeline(story, cast, out).unwrap()
}

/// The three books the task names, pinned in memory and compared shot for shot.
#[test]
fn pinning_reproduces_the_plan_and_unpins_byte_for_byte() {
	for name in [
		"worlds/mumbleton/stories/the-visitor/story.md",
		"worlds/mumbleton/stories/camera-test/story.md",
		"worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md",
	] {
		let path = script(name);
		let (world, story, cast) = load(&path);
		let out = tempfile::tempdir().unwrap();
		let before = timed(&story, &cast, out.path());
		let source = std::fs::read_to_string(&path).unwrap();
		let pinned = storyboard::pin(&source, &story, &before, None).unwrap();

		assert!(pinned.count > 0, "{name}: pinned nothing");
		assert!(
			!pinned.pages.is_empty(),
			"{name}: pinned no pages ({} notes)",
			pinned.notes.len()
		);

		// The marker lines are exactly what `--unpin` removes.
		let (restored, removed) = storyboard::unpin(&pinned.script);
		assert_eq!(
			removed, pinned.count,
			"{name}: every pinned line is removed"
		);
		assert_eq!(
			restored, source,
			"{name}: unpin restores the script byte for byte"
		);

		// The pinned script is still the same book, with the same cast.
		let mut after_story = fables::story::parse(&pinned.script).unwrap();
		world.furnish(&mut after_story).unwrap();
		let after_cast = world.cast_for(&after_story).unwrap();
		let after = timed(&after_story, &after_cast, out.path());

		assert!(
			(after.duration - before.duration).abs() < 1e-3,
			"{name}: the film runs {:.3}s before pinning and {:.3}s after",
			before.duration,
			after.duration
		);

		for (before_shot, after_shot) in before.shots.iter().zip(&after.shots) {
			assert_eq!(before_shot.kind, after_shot.kind, "{name}");
			assert_eq!(
				before_shot.setups.len(),
				after_shot.setups.len(),
				"{name} {:?}: setups before {:#?} after {:#?}",
				before_shot.kind,
				before_shot.setups,
				after_shot.setups
			);

			for (x, y) in before_shot.setups.iter().zip(&after_shot.setups) {
				assert_eq!(x.framing, y.framing, "{name} {:?}", before_shot.kind);
				assert_eq!(x.travel, y.travel, "{name} {:?}", before_shot.kind);
				assert!(
					(x.time - y.time).abs() < 0.01,
					"{name} {:?}: a setup starts at {:.3}s before and {:.3}s after",
					before_shot.kind,
					x.time,
					y.time
				);
				assert!(
					(x.over - y.over).abs() < 0.01,
					"{name} {:?}: move length {:.3}s before and {:.3}s after",
					before_shot.kind,
					x.over,
					y.over
				);
			}
		}

		let before_rows = storyboard::plan(&story, &before, &cast, false);
		let after_rows = storyboard::plan(&after_story, &after, &after_cast, false);
		assert_eq!(before_rows.len(), after_rows.len(), "{name}: shot rows");

		for (x, y) in before_rows.iter().zip(&after_rows) {
			assert_eq!(x.size, y.size, "{name} #{}", x.number);
			assert_eq!(x.subjects, y.subjects, "{name} #{}", x.number);
			assert_eq!(x.movement, y.movement, "{name} #{}", x.number);
			assert!(
				(x.start - y.start).abs() < 0.01,
				"{name} #{}: {:.3}s before and {:.3}s after",
				x.number,
				x.start,
				y.start
			);
			assert!((x.hold - y.hold).abs() < 0.01, "{name} #{}", x.number);
			assert_eq!(x.spoken.len(), y.spoken.len(), "{name} #{}", x.number);
		}
	}
}

#[test]
fn pinning_named_pages_touches_only_them() {
	let path = script("worlds/mumbleton/stories/the-visitor/story.md");
	let (_, story, cast) = load(&path);
	let out = tempfile::tempdir().unwrap();
	let source = std::fs::read_to_string(&path).unwrap();
	let pinned = storyboard::pin(
		&source,
		&story,
		&timed(&story, &cast, out.path()),
		Some(&[3]),
	)
	.unwrap();

	assert_eq!(pinned.pages, vec![3]);

	let headings: Vec<usize> = pinned
		.script
		.lines()
		.enumerate()
		.filter(|(_, line)| line.starts_with("# "))
		.map(|(index, _)| index)
		.collect();
	let (page_three, page_four) = (headings[2], headings[3]);

	for (index, line) in pinned.script.lines().enumerate() {
		if line.contains("// pinned") {
			assert!(
				(index > page_three) && (index < page_four),
				"a pinned line outside page 3: {line}"
			);
		}
	}

	assert_eq!(storyboard::unpin(&pinned.script).0, source);
}

#[test]
fn pinning_leaves_comments_and_spacing_alone() {
	let source = "+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n\n// a comment\n#   \
	              park    transition=wipe\n\n@kid   x=0.3\ntalk\nHello there today my friend, \
	              said Kid.\n\n// another comment\n@mum x=0.7\nWhy hello to you too.\n\n#   \
	              garden\n@kid x=0.4\nAnd then the wind blew.";
	let story = fables::story::parse(source).unwrap();
	let cast =
		Cast::parse("[kid]\nname = \"Kid\"\n[mum]\nname = \"Mum\"\nkind = \"grown-up\"\n").unwrap();
	let out = tempfile::tempdir().unwrap();
	let pinned = storyboard::pin(source, &story, &timed(&story, &cast, out.path()), None).unwrap();

	assert!(pinned.count > 0);
	assert!(pinned.script.contains("// a comment"));
	assert!(pinned.script.contains("// another comment"));
	assert!(pinned.script.contains("#   garden"));
	assert!(
		pinned.script.ends_with("And then the wind blew."),
		"the file's last line keeps its missing newline"
	);

	let (restored, removed) = storyboard::unpin(&pinned.script);
	assert_eq!(removed, pinned.count);
	assert_eq!(restored, source);
}

#[test]
fn a_page_with_no_body_pins_after_its_heading() {
	let source = "+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park";
	let story = fables::story::parse(source).unwrap();
	let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
	let out = tempfile::tempdir().unwrap();
	let pinned = storyboard::pin(source, &story, &timed(&story, &cast, out.path()), None).unwrap();

	assert!(
		pinned.script.contains("# park\n@camera "),
		"{}",
		pinned.script
	);
	assert_eq!(storyboard::unpin(&pinned.script).0, source);
}

#[test]
fn the_visitor_shot_list_names_every_shot_and_its_line() {
	let path = script("worlds/mumbleton/stories/the-visitor/story.md");
	let (_, story, cast) = load(&path);
	let out = tempfile::tempdir().unwrap();
	let timeline = timed(&story, &cast, out.path());
	let rows = storyboard::plan(&story, &timeline, &cast, false);
	let text = storyboard::shot_list(&story, &timeline, &rows, false);

	assert!(
		text.starts_with("# The Visitor — shot list (landscape)"),
		"{text}"
	);
	assert!(
		text.contains(
			"| # | page | line | at | hold | shot | subjects | move | in | by | spoken |"
		)
	);
	assert!(text.contains("**Length.** 16:9"), "{text}");
	assert!(text.contains("**Shots.**"), "{text}");
	assert!(text.contains("**Short.**"), "{text}");
	assert!(text.contains("Mo Mumble"), "{text}");
	assert!(
		rows.iter()
			.any(|row| row.size == "close" && row.subjects.contains("Hana")),
		"a close-up on Hana is in the plan"
	);

	for row in &rows {
		if row.page.is_some() {
			assert!(row.line > 0, "row {} has a script line", row.number);
		}

		assert!(
			text.contains(&format!("| {} | ", row.number)),
			"row {} is in the table",
			row.number
		);
	}
}

#[test]
fn animatic_frames_land_on_the_timelines_boundaries() {
	for name in [
		"worlds/mumbleton/stories/the-visitor/story.md",
		"worlds/mumbleton/stories/camera-test/story.md",
	] {
		let (_, story, cast) = load(&script(name));
		let out = tempfile::tempdir().unwrap();
		let timeline = timed(&story, &cast, out.path());
		let rows = storyboard::plan(&story, &timeline, &cast, false);
		let bounds = storyboard::frame_bounds(&rows, 24);

		assert_eq!(bounds.len(), rows.len(), "{name}");
		assert_eq!(bounds[0].0, 0, "{name}: the animatic starts on frame 0");

		for pair in bounds.windows(2) {
			assert_eq!(pair[0].1, pair[1].0, "{name}: shots share a boundary");
		}

		for (row, (first, last)) in rows.iter().zip(&bounds) {
			assert_eq!(
				(*first, *last),
				(
					(row.start * 24.0).round() as usize,
					((row.start + row.hold) * 24.0).round() as usize
				),
				"{name} #{}: the shot's frames are its timeline seconds",
				row.number
			);
		}

		assert_eq!(
			bounds.last().unwrap().1,
			(timeline.duration * 24.0).round() as usize,
			"{name}: the last shot ends where the timeline does"
		);
	}
}
