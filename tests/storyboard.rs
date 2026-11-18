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

/// Pins `source` in one shape's plan, checks `--unpin` restores it byte for byte, and checks
/// the pinned script shoots the same setups and shots as before. `replan` loads a script into
/// a story, its cast and its estimated timeline.
fn assert_pins_exactly(
	name: &str,
	source: &str,
	portrait: bool,
	replan: impl Fn(&str) -> (Story, Cast, Timeline),
) {
	let (story, cast, before) = replan(source);
	let pinned = storyboard::pin(source, &story, &before, None, portrait).unwrap();

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
	let (after_story, after_cast, after) = replan(&pinned.script);

	assert!(
		(after.duration - before.duration).abs() < 1e-3,
		"{name}: the film runs {:.3}s before pinning and {:.3}s after",
		before.duration,
		after.duration
	);
	assert_eq!(before.holds, after.holds, "{name}: dead stops");

	for (before_shot, after_shot) in before.shots.iter().zip(&after.shots) {
		let (x_setups, y_setups) = (
			before_shot.setups_for(portrait),
			after_shot.setups_for(portrait),
		);
		assert_eq!(before_shot.kind, after_shot.kind, "{name}");
		assert_eq!(
			x_setups.len(),
			y_setups.len(),
			"{name} {:?}: setups before {x_setups:#?} after {y_setups:#?}",
			before_shot.kind,
		);

		for (x, y) in x_setups.iter().zip(y_setups) {
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

	let before_rows = storyboard::plan(&story, &before, &cast, portrait);
	let after_rows = storyboard::plan(&after_story, &after, &after_cast, portrait);
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
		assert_eq!(x.music, y.music, "{name} #{}", x.number);
		assert!((x.stop - y.stop).abs() < 0.01, "{name} #{}", x.number);
	}
}

/// The three books the task names, pinned in memory in each shape's plan and compared shot
/// for shot.
#[test]
fn pinning_reproduces_the_plan_and_unpins_byte_for_byte() {
	for name in [
		"worlds/mumbleton/stories/the-visitor/story.md",
		"worlds/mumbleton/stories/camera-test/story.md",
		"worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md",
	] {
		let path = script(name);
		let (world, ..) = load(&path);
		let out = tempfile::tempdir().unwrap();
		let source = std::fs::read_to_string(&path).unwrap();
		let replan = |script: &str| {
			let mut story = fables::story::parse(script).unwrap();
			world.furnish(&mut story).unwrap();
			let cast = world.cast_for(&story).unwrap();
			let timeline = timed(&story, &cast, out.path());
			(story, cast, timeline)
		};

		for portrait in [false, true] {
			assert_pins_exactly(
				&format!("{name} (portrait {portrait})"),
				&source,
				portrait,
				replan,
			);
		}
	}
}

/// A book with an age band (its own lead-in, gaps and tail), a heading's `hold=` and an
/// `@hold` dead stop, music cues and a slow push: a pinned `after=` is measured the way the
/// timeline measures time, so it still lands where the director put it.
#[test]
fn pinning_stays_exact_with_an_age_band_and_dead_stops() {
	let source = "+++\ntitle = \"Holding On\"\nauthor = \"A\"\nage = \"3-5\"\nstar = \
	              \"kid\"\n[video]\ncast-voices = true\n+++\n# park hold=1.5\n@kid x=0.3\n@mum \
	              x=0.7\nThe kite went up and up and up, past the trees and the tall red chimney \
	              and the little round clouds, and Kid watched it go all the way.\n@kid \
	              talk\n\"Look at it fly,\" said Kid.\n@hold 2\n@mum talk\n\"Hold on tight,\" \
	              said Mum.\n@music tense\n@kid shocked talk\n\"It is pulling me!\" said \
	              Kid.\n\n# garden hold=0.8\n@kid x=0.4\n@hold 1.2\nThe wind dropped, and the \
	              kite came down softly onto the grass, and everything was quiet again.\n";
	let cast =
		Cast::parse("[kid]\nname = \"Kid\"\n[mum]\nname = \"Mum\"\nkind = \"grown-up\"\n").unwrap();
	let out = tempfile::tempdir().unwrap();
	let replan = |script: &str| {
		let story = fables::story::parse(script).unwrap();
		let timeline = timed(&story, &cast, out.path());
		(story, cast.clone(), timeline)
	};

	let (story, _, timeline) = replan(source);
	assert!(story.age.is_some(), "the book has an age band");
	assert!(
		timeline.holds.len() >= 3,
		"both headings' holds and the @hold dead stops are in the timeline: {:?}",
		timeline.holds
	);
	assert!(
		timeline
			.shots
			.iter()
			.any(|shot| { shot.setups.iter().any(|setup| setup.travel.is_move()) }),
		"the long opening hold rides a slow push, so a pin needs `after=`"
	);

	for portrait in [false, true] {
		assert_pins_exactly(
			&format!("holding on (portrait {portrait})"),
			source,
			portrait,
			replan,
		);
	}

	// The pinned script's `after=` lines are what make this hard; check one is written.
	let pinned = storyboard::pin(source, &story, &timeline, None, false).unwrap();
	assert!(pinned.script.contains("after="), "{}", pinned.script);
}

/// Whether two timelines shoot the same setups. Both come from one script with or without a
/// single word, so a word that matters moves a setup by more than a millisecond.
fn same_plan(a: &Timeline, b: &Timeline) -> bool {
	a.shots.len() == b.shots.len()
		&& a.shots.iter().zip(&b.shots).all(|(x, y)| {
			x.setups.len() == y.setups.len()
				&& x.setups.iter().zip(&y.setups).all(|(p, q)| {
					p.framing == q.framing
						&& p.travel == q.travel
						&& (p.time - q.time).abs() < 1e-3
						&& (p.over - q.over).abs() < 1e-3
						&& (p.shake - q.shake).abs() < 1e-3
				})
		})
}

/// A pinned line is meant to be edited, so it writes only what differs from what the camera
/// would do without it: leaving out any one of its options changes the plan.
#[test]
fn a_pinned_line_writes_no_option_at_its_default() {
	// The framing words name the shot; every other option is one a writer could leave out.
	const OPTIONAL: [&str; 6] = ["cut", "ease", "move=", "over=", "shake=", "after="];

	for name in [
		"worlds/mumbleton/stories/the-visitor/story.md",
		"worlds/mumbleton/stories/camera-test/story.md",
		"worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md",
	] {
		let path = script(name);
		let (world, story, cast) = load(&path);
		let out = tempfile::tempdir().unwrap();
		let source = std::fs::read_to_string(&path).unwrap();
		let pinned = storyboard::pin(
			&source,
			&story,
			&timed(&story, &cast, out.path()),
			None,
			false,
		)
		.unwrap()
		.script;
		let replan = |script: &str| {
			let mut story = fables::story::parse(script).unwrap();
			world.furnish(&mut story).unwrap();
			let cast = world.cast_for(&story).unwrap();
			timed(&story, &cast, out.path())
		};
		let plan = replan(&pinned);
		let lines: Vec<&str> = pinned.lines().collect();

		for (index, line) in lines.iter().enumerate() {
			let Some(body) = line
				.strip_prefix("@camera ")
				.and_then(|line| line.strip_suffix(" // pinned"))
			else {
				continue;
			};
			let words: Vec<&str> = body.split_whitespace().collect();

			for (at, word) in words.iter().enumerate() {
				if let Some(after) = word.strip_prefix("after=") {
					assert!(
						after
							.split_once('.')
							.is_none_or(|(_, decimals)| decimals.len() <= 2),
						"{name}:{}: `{word}` has more than two decimals",
						index + 1
					);
				}

				if !OPTIONAL.iter().any(|option| {
					word == option || (option.ends_with('=') && word.starts_with(option))
				}) {
					continue;
				}

				let mut fewer = words.clone();
				fewer.remove(at);
				let mut edited = lines.clone();
				let shorter = format!("@camera {} // pinned", fewer.join(" "));
				edited[index] = &shorter;

				assert!(
					!same_plan(&plan, &replan(&edited.join("\n"))),
					"{name}:{}: `{word}` in `{line}` is the default; leaving it out changes \
					 nothing",
					index + 1
				);
			}
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
		false,
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
	let pinned = storyboard::pin(
		source,
		&story,
		&timed(&story, &cast, out.path()),
		None,
		false,
	)
	.unwrap();

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
	let pinned = storyboard::pin(
		source,
		&story,
		&timed(&story, &cast, out.path()),
		None,
		false,
	)
	.unwrap();

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
	assert!(text.contains(
		"| # | page | line | at | length | shot | subjects | move | in | by | music | stop | \
		 spoken |"
	));
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

/// Where the two shapes plan other shots, one note names those pages and the way the other
/// shape leans, so a writer knows `--format portrait` would pin something else.
#[test]
fn one_note_names_the_pages_the_other_shape_shoots_differently() {
	let path = script("worlds/mumbleton/stories/the-visitor/story.md");
	let (_, story, cast) = load(&path);
	let out = tempfile::tempdir().unwrap();
	let source = std::fs::read_to_string(&path).unwrap();
	let timeline = timed(&story, &cast, out.path());

	for (portrait, expected) in [
		(false, "portrait plans closer shots on pages 1, "),
		(true, "landscape plans wider shots on pages 1, "),
	] {
		let pinned = storyboard::pin(&source, &story, &timeline, None, portrait).unwrap();
		let notes: Vec<&String> = pinned
			.notes
			.iter()
			.filter(|note| note.contains("plans"))
			.collect();

		assert_eq!(notes.len(), 1, "one note per pin run: {:?}", pinned.notes);
		assert!(notes[0].starts_with(expected), "{}", notes[0]);
		assert!(notes[0].contains("--format portrait"), "{}", notes[0]);
	}
}
