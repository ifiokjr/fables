//! Props and eyelines end to end, in a temporary world the test makes itself: a held prop, a
//! world prop that overrides a built-in, a two-handed prop, a stage prop on a tether, a give, a
//! catch out of a jump, every `see=` target and a notice. It checks, renders proofs, and
//! measures what it drew.

use fables::art::props;
use fables::cast::Cast;
use fables::cast::Kind;
use fables::compose;
use fables::compose::Body;
use fables::compose::Puppet;
use fables::publish;
use fables::stage;
use fables::stage::See;
use fables::story::Story;
use fables::video;
use fables::video::frames::Format;
use fables::video::frames::Renderer;
use fables::video::voice::SAMPLE_RATE;
use fables::video::voice::Speech;
use fables::video::voice::Utterance;
use fables::world::World;

const CAST: &str = r#"
[hana]
name = "Hana"
skin = "tangerine"

[mo]
name = "Mo"
kind = "grown-up"
skin = "toffee"
"#;

/// The world's own ball, repainting the built-in one under the same name.
const BALL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" data-grip="0 0" data-hands="one" data-about="The town ball, painted blue"><path id="ball" d="M40 -40 C40 -17.9 22.1 0 0 0 C-22.1 0 -40 -17.9 -40 -40 C-40 -62.1 -22.1 -80 0 -80 C22.1 -80 40 -62.1 40 -40 Z" fill="cornflower" data-shade="1"/><path id="band" d="M-36 -54 C-20 -48 20 -48 36 -54 L34 -36 C20 -30 -20 -30 -34 -36 Z" fill="cream" data-line="detail"/></svg>"#;

/// A long box two hands hold.
const TWO_HAND: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" data-grip="0 0" data-hands="two" data-about="A long crate, held with both hands"><path id="crate" d="M-70 -30 C-20 -34 20 -34 70 -30 C72 -10 72 10 70 30 C20 34 -20 34 -70 30 C-72 10 -72 -10 -70 -30 Z" fill="toffee" data-shade="1"/><path id="band" d="M-70 -6 L70 -6 L70 6 L-70 6 Z" fill="cocoa" data-line="none"/></svg>"#;

const SCRIPT: &str = r#"+++
title = "The Props Test"
author = "Ifiok Jr."
star = "hana"

[video]
direction = "locked"
+++

# park
@hana x=0.28 hold holds=lantern
@mo x=0.72
Hana has a lantern, and Mo is here too.
@hana x=0.3 see=mo happy
@mo x=0.7 see=hana
They look at each other.

# garden
@hana x=0.3 jump catch
@mo x=0.72 reach-out
Hana leaps and catches it.

# hilltop
@hana x=0.28 hold holds=kite-spool
@kite x=0.72 y=sky tether=hana
Hana flies her kite.
@hana notice see=kite
She notices it climb.

# beach
@hana x=0.3 hold holds=ball
@mo x=0.7
The town ball is ours.
@hana see=mine
Hana looks at it.
@hana holds=nothing
@mo hold holds=ball
Hana gives the ball to Mo.

# town
@mo x=0.6 hold holds=two-hand
@hana x=0.3 see=up
Mo carries the long crate.
@hana see=front
Hana stops looking up.
"#;

struct Fixture {
	_dir: tempfile::TempDir,
	story: Story,
	cast: Cast,
}

fn fixture() -> Fixture {
	let dir = tempfile::tempdir().unwrap();
	let world = dir.path().join("worlds/propville");
	std::fs::create_dir_all(world.join("props")).unwrap();
	std::fs::create_dir_all(world.join("stories/the-props-test")).unwrap();
	std::fs::write(
		world.join("world.toml"),
		"name = \"Propville\"\nstyle = \"cartoon\"\n",
	)
	.unwrap();
	std::fs::write(world.join("cast.toml"), CAST).unwrap();
	std::fs::write(world.join("props/ball.svg"), BALL).unwrap();
	std::fs::write(world.join("props/two-hand.svg"), TWO_HAND).unwrap();

	let script = world.join("stories/the-props-test/story.md");
	std::fs::write(&script, SCRIPT).unwrap();

	let mut story = Story::load(&script).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast().unwrap();

	Fixture {
		_dir: dir,
		story,
		cast,
	}
}

fn page(fixture: &Fixture, index: usize) -> Vec<Puppet> {
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();

	compose::puppets(rehearsals[index].finale(), 0.0, true)
}

fn puppet<'a>(puppets: &'a [Puppet], id: &str) -> &'a Puppet {
	puppets
		.iter()
		.find(|puppet| puppet.id == id)
		.unwrap_or_else(|| panic!("there is no `{id}` on stage"))
}

fn figure(puppet: &Puppet) -> &fables::art::figure::FigureState {
	let Body::Figure(state) = &puppet.body else {
		panic!("{} is a figure", puppet.id);
	};

	state
}

#[test]
fn the_props_story_checks_and_renders_proofs() {
	let fixture = fixture();
	publish::check(&fixture.story, &fixture.cast).unwrap();

	let out = tempfile::tempdir().unwrap();
	let dir = publish::proofs(&fixture.story, &fixture.cast, out.path()).unwrap();
	let pages = std::fs::read_dir(&dir)
		.unwrap()
		.filter_map(|entry| Some(entry.ok()?.path()))
		.filter(|path| {
			path.extension()
				.is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
		})
		.count();

	assert!(pages >= 6, "the proofs are written: {pages} images");
	assert!(dir.join("contact-sheet.png").is_file());
}

#[test]
fn every_new_word_reaches_the_stage() {
	let fixture = fixture();

	// A built-in held in one hand.
	let first = page(&fixture, 0);
	let hana = first.iter().find(|p| p.id == "hana").unwrap();
	let Kind::Figure(_) = &hana.kind else {
		panic!("hana is a figure");
	};
	assert_eq!(
		figure(hana).prop.as_ref().map(|prop| prop.name.as_str()),
		Some("lantern")
	);
	assert!(figure(hana).gaze > 0.0, "see=mo aims her eyes right");

	// The catch out of a jump.
	let second = page(&fixture, 1);
	let hana = puppet(&second, "hana");
	assert_eq!(
		figure(hana).hand_shapes.1,
		fables::art::figure::HandShape::Open
	);
	assert!(
		figure(hana).hands.1.y < figure(hana).hands.0.y - 60.0,
		"the catching hand reaches high: {:?}",
		figure(hana).hands
	);

	// The kite stands on stage, tethered, and the notice looks at it.
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();
	let kite_page = rehearsals[2].finale();
	let kite = kite_page
		.actors
		.iter()
		.find(|actor| actor.id == "kite")
		.unwrap();
	assert_eq!(kite.tether.as_deref(), Some("hana"));
	assert!(
		matches!(&kite.kind, Kind::Prop(prop) if prop.name == "kite"),
		"@kite is the built-in prop"
	);
	let hana = kite_page
		.actors
		.iter()
		.find(|actor| actor.id == "hana")
		.unwrap();
	assert!(hana.eyes.notice, "the look is a notice");
	assert_eq!(hana.eyes.see, Some(See::Actor("kite".into())));

	// The world's own ball, then the give: Hana lets go, Mo takes it.
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();
	let after_give = rehearsals[3].finale();
	let hana = after_give.actors.iter().find(|a| a.id == "hana").unwrap();
	let mo = after_give.actors.iter().find(|a| a.id == "mo").unwrap();
	assert!(hana.prop.is_none(), "Hana gave it away");
	let ball = mo.prop.as_ref().expect("Mo holds the ball");
	assert_eq!(ball.name, "ball");
	assert_eq!(
		ball.about, "The town ball, painted blue",
		"the world's own ball replaced the built-in"
	);

	// Two hands meet at the long crate.
	let last = page(&fixture, 4);
	let mo = puppet(&last, "mo");
	let state = figure(mo);
	assert_eq!(
		state.prop.as_ref().map(|prop| prop.hands),
		Some(props::Hands::Two)
	);
	assert!(
		(state.hands.0 - state.hands.1).len() < 30.0,
		"both hands are on it: {:?}",
		state.hands
	);
}

#[test]
fn a_held_prop_sits_in_the_hand_and_a_tether_reaches_both_ends() {
	let fixture = fixture();
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();
	let scene = rehearsals[2].finale();
	let drawn = compose::puppets(scene, 0.0, true);
	let hana = puppet(&drawn, "hana");
	let kite = puppet(&drawn, "kite");

	let bounds = compose::held_prop_bounds(hana).expect("the spool is drawn");
	let middle = fables::art::pt(
		f32::midpoint(bounds.0, bounds.2),
		f32::midpoint(bounds.1, bounds.3),
	);
	let hand = hana.hand_near(middle).expect("a figure has hands");
	assert!(
		hand.x >= bounds.0 - 1.0
			&& hand.y >= bounds.1 - 1.0
			&& hand.x <= bounds.2 + 1.0
			&& hand.y <= bounds.3 + 1.0,
		"the hand {hand:?} holds the spool's measured bounds {bounds:?}"
	);

	let (start, end) = compose::tether_points(scene, &drawn, "kite").expect("the kite is tethered");
	let extents = kite.extents().subject;
	assert!(
		start.x >= extents.0 - 1.0
			&& start.y >= extents.1 - 1.0
			&& start.x <= extents.2 + 1.0
			&& start.y <= extents.3 + 1.0,
		"the string starts on the kite at {start:?}: {extents:?}"
	);
	let subject = hana.extents().subject;
	assert!(
		end.x >= subject.0 - 1.0
			&& end.y >= subject.1 - 1.0
			&& end.x <= subject.2 + 1.0
			&& end.y <= subject.3 + 1.0,
		"and ends in Hana's hand {end:?}: {subject:?}"
	);
}

#[test]
fn a_grown_up_holds_the_same_prop_bigger_than_a_kid() {
	let fixture = fixture();
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();

	// The kid holds it before the give on the beach page; the grown-up holds it after.
	let kid = compose::held_prop_bounds(puppet(
		&compose::puppets(&rehearsals[3].after[0], 0.0, true),
		"hana",
	));
	let grown = compose::held_prop_bounds(puppet(
		&compose::puppets(rehearsals[3].finale(), 0.0, true),
		"mo",
	));

	let (Some(kid), Some(grown)) = (kid, grown) else {
		panic!("both hold the ball");
	};
	// The page draws a grown-up smaller than a kid, so compare each prop with its holder's
	// drawing scale: the bigger hand holds a bigger ball.
	let (kid_scale, grown_scale) = (2.25, 1.85);

	assert!(
		(grown.3 - grown.1) / grown_scale > (kid.3 - kid.1) / kid_scale * 1.05,
		"the grown-up's hand holds it bigger: {kid:?} against {grown:?}"
	);
}

#[test]
fn a_preview_frame_draws_the_props_on_stage() {
	let fixture = fixture();
	let rehearsals = stage::rehearse(&fixture.story, &fixture.cast).unwrap();
	let timeline = video::timeline::build(
		&fixture.story,
		&fixture.cast,
		&rehearsals,
		|line: &Utterance<'_>| {
			let seconds = line.text.split_whitespace().count() as f32 * 0.3 + 0.25;
			let samples = (seconds * SAMPLE_RATE as f32) as usize;
			let envelope = vec![0.5; (seconds * 60.0) as usize];

			Ok(Speech {
				text: line.text.to_owned(),
				samples: vec![0.0; samples],
				track: video::lipsync::from_envelope(line.text, &envelope, 60.0),
				envelope,
			})
		},
	)
	.unwrap();

	let (width, height) = Format::Landscape.size(true);
	let renderer = Renderer::new(
		&timeline,
		&fixture.story,
		Format::Landscape,
		width / 4,
		height / 4,
	);

	for shot in timeline.shots.iter().take(6) {
		let middle = shot.start + shot.duration() / 2.0;
		let pixmap = renderer.render(middle, 0).unwrap();
		assert_eq!((pixmap.width(), pixmap.height()), (width / 4, height / 4));
	}
}

/// The props sheet, written where a person can look at it: every prop alone and in the hand of
/// the world's first figure. Run through the CLI for the evidence; here, only that it draws.
#[test]
fn the_props_sheet_draws_every_prop() {
	let fixture = fixture();
	let world = World::for_story(&fixture.story, None).unwrap();
	let sheet = fables::press::sheets::props_sheet(
		&world.props().unwrap(),
		&fixture.cast,
		&world.name,
		fables::art::style::Style::default(),
	)
	.unwrap();

	assert!(sheet.width() > 1000);
	assert!(sheet.height() > 1000);
}
