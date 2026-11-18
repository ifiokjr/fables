//! Starting a cartoon from the command line: a world and two characters per drawing style,
//! one character with a part file of their own and one drawn from the style's template, then a
//! checked book and its proofs. Neither character may fall back to the old parametric puppet.

use std::path::Path;
use std::process::Command;

use fables::art::figure::Build;
use fables::art::pack::StylePack;
use fables::art::parts::Parts;
use fables::art::puppet::joins;
use fables::art::shape::Shape;
use fables::cast::Kind;
use fables::world::World;

const BIN: &str = env!("CARGO_BIN_EXE_fables");

/// A part set's names, sorted.
fn names(parts: &Parts) -> Vec<String> {
	let mut names: Vec<String> = parts.iter().map(|(name, _)| name.to_owned()).collect();
	names.sort();
	names
}

/// Whether two outlines are the same drawing, allowing for the rounding a written file does.
fn same_drawing(a: &Shape, b: &Shape) -> bool {
	let (a, b) = (a.polygon(1), b.polygon(1));
	a.len() == b.len()
		&& a.iter()
			.zip(&b)
			.all(|(p, q)| (p.x - q.x).abs() < 0.05 && (p.y - q.y).abs() < 0.05)
}

/// Runs the CLI in `dir`, with its output in the failure message.
fn run(dir: &Path, args: &[&str]) -> String {
	let output = Command::new(BIN)
		.args(args)
		.current_dir(dir)
		.output()
		.unwrap_or_else(|error| panic!("could not run fables {args:?}: {error}"));
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		output.status.success(),
		"fables {args:?} failed:\n{stdout}\n{stderr}"
	);
	stdout.into_owned()
}

#[test]
fn a_scratch_world_in_each_drawing_style_draws_both_characters_in_it() {
	for (style, skin) in [
		("chibi", "riso"),
		("clear-line", "clear"),
		("rubber-hose", "riso"),
	] {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path();

		run(
			root,
			&[
				"world",
				"new",
				"Puddle Lane",
				"--style",
				style,
				"--author",
				"An Author",
			],
		);
		run(
			root,
			&[
				"character",
				"new",
				"pip",
				"--name",
				"Pip Pocket",
				"--skin",
				"tangerine",
				"--hair-colour",
				"plum",
				"--top",
				"teal",
				"--trim",
				"cream",
				"--bottom",
				"night",
				"--shoes",
				"ink",
				"--eyes",
				"cocoa",
			],
		);
		run(
			root,
			&[
				"character",
				"new",
				"mum",
				"--name",
				"Mum",
				"--kind",
				"grown-up",
				"--skin",
				"cocoa",
				"--hair-colour",
				"ink",
				"--top",
				"tomato",
				"--trim",
				"cream",
				"--bottom",
				"night",
				"--shoes",
				"cocoa",
				"--eyes",
				"cocoa",
				"--draw",
			],
		);

		// Both are drawn from parts in the world's own style: only `--draw` has a file, and
		// neither is the parametric puppet.
		let world = World::load(&root.join("worlds/puddle-lane")).unwrap();
		assert_eq!(world.style, style);
		let cast = world.cast().unwrap();
		for id in ["pip", "mum"] {
			let Kind::Figure(look) = cast.resolve(id).unwrap() else {
				panic!("{id} is a person");
			};
			let puppet = look
				.puppet
				.as_ref()
				.unwrap_or_else(|| panic!("{id} fell back to the parametric puppet"));
			assert_eq!(puppet.style.name, style, "{id} is drawn in {style}");
			assert_eq!(puppet.template, id == "pip", "{id}'s own drawing");
		}
		assert!(root.join("worlds/puddle-lane/puppets/mum.svg").is_file());
		assert!(
			!root.join("worlds/puddle-lane/puppets/pip.svg").exists(),
			"a character without --draw needs no file"
		);

		// Mum is a grown-up, so `--draw` wrote the grown-up's template, not a kid's.
		let pack = StylePack::named(style).unwrap();
		let mum = Parts::load(&root.join("worlds/puddle-lane/puppets/mum.svg")).unwrap();
		assert_eq!(
			names(&mum),
			names(&pack.template_for(Build::Grownup)),
			"mum's parts in {style}"
		);

		// A book with both of them, a bare light and a bare scene preset, checks and renders.
		let story = root.join("worlds/puddle-lane/stories/pip-and-the-puddle/story.md");
		std::fs::create_dir_all(story.parent().unwrap()).unwrap();
		std::fs::write(
			&story,
			"+++\ntitle = \"Pip and the Puddle\"\nstar = \"pip\"\n+++\n\n# garden golden\n@pip \
			 x=0.32 worried\n@mum x=0.7 happy\nPip looked at the puddle. It looked very \
			 deep.\n\n# garden\n@pip x=0.5 grin jump\nPip jumped. **SPLASH!**\n@scene dusk\nA \
			 very wet, very happy Pip.\n",
		)
		.unwrap();

		let checked = run(
			root,
			&[
				"check",
				"worlds/puddle-lane/stories/pip-and-the-puddle/story.md",
			],
		);
		assert!(
			checked.contains(&format!("drawing: {style} · skin: {skin}")),
			"{checked}"
		);

		let proofs = run(
			root,
			&[
				"proofs",
				"worlds/puddle-lane/stories/pip-and-the-puddle/story.md",
				"--out",
				"out",
			],
		);
		let sheet = root.join(format!(
			"out/pip-and-the-puddle/{skin}/proofs/contact-sheet.png"
		));
		assert!(sheet.is_file(), "{proofs}");
		let png = std::fs::read(&sheet).unwrap();
		assert!(png.starts_with(b"\x89PNG"), "{} is a PNG", sheet.display());
	}
}

#[test]
fn a_grown_up_is_drawn_from_the_grown_up_template_on_every_path() {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path();
	run(
		root,
		&[
			"world",
			"new",
			"Hoop Lane",
			"--style",
			"rubber-hose",
			"--author",
			"An Author",
		],
	);

	// Gran has no part file, Pop is drawn with `--draw`, Nan is given one by `puppet template`,
	// and Ada gets hers from her first redraw.
	for (id, extra) in [
		("gran", None),
		("pop", Some("--draw")),
		("nan", None),
		("ada", None),
	] {
		let mut args = vec!["character", "new", id, "--name", id, "--kind", "grown-up"];
		args.extend(extra);
		run(root, &args);
	}
	run(root, &["puppet", "template", "nan"]);

	let pack = StylePack::named("rubber-hose").unwrap();
	let grown_up = pack.template_for(Build::Grownup);
	let torso = grown_up.get("torso.front").unwrap();
	run(
		root,
		&[
			"puppet",
			"set",
			"ada",
			"torso.front",
			"--d",
			torso.path().as_str(),
		],
	);

	// Gran resolves to the grown-up template, and joins up on the grown-up body.
	let world = World::load(&root.join("worlds/hoop-lane")).unwrap();
	let cast = world.cast().unwrap();
	let Kind::Figure(look) = cast.resolve("gran").unwrap() else {
		panic!("gran is a person");
	};
	let puppet = look.puppet.as_ref().expect("gran is drawn from parts");
	assert!(puppet.template, "gran has no part file of her own");
	assert_eq!(look.build, Build::Grownup);
	assert_eq!(
		names(&puppet.parts),
		names(&pack.template_figure(Build::Grownup)),
		"gran is the grown-up template over the kit"
	);
	assert_eq!(joins(puppet, look.build), Vec::new());

	let checked = run(root, &["puppet", "check", "gran"]);
	assert!(
		checked.contains("from the rubber-hose grown-up template"),
		"{checked}"
	);
	let parts = run(root, &["puppet", "parts", "gran", "torso.front"]);
	assert!(parts.contains(torso.path().as_str()), "{parts}");

	// Every file written for a grown-up holds the grown-up's parts: no kid's tuft, and the
	// grown-up's torso.
	for id in ["pop", "nan", "ada"] {
		let file = root.join(format!("worlds/hoop-lane/puppets/{id}.svg"));
		let parts = Parts::load(&file).unwrap();
		assert_eq!(names(&parts), names(&grown_up), "{id}'s parts");
		assert!(
			same_drawing(parts.get("torso.front").unwrap(), torso),
			"{id} has the grown-up's torso"
		);
		run(root, &["puppet", "check", id]);
	}
}
