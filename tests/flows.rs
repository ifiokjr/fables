//! Story flows: every built-in shape starts a book that checks, a world's own flow is found
//! before the built-in of the same name, and every worked example stays inside its flow's page
//! budget.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use fables::flow;
use fables::story::Story;
use fables::world::World;

const BIN: &str = env!("CARGO_BIN_EXE_fables");

/// Every built-in flow carries a worked example in an example world, so the shapes are shown
/// as books and not only as scaffolds.
const WORKED: [&str; 9] = [
	"three-tries",
	"pile-up",
	"there-and-back",
	"round-we-go",
	"odd-couple",
	"wrong-end-of-the-stick",
	"goodnight-tour",
	"gag-short",
	"habit-tale",
];

/// A world with two cast members and a `stories/` folder, under a fresh root.
fn scratch() -> (tempfile::TempDir, PathBuf) {
	let root = tempfile::tempdir().unwrap();
	let home = root.path().join("worlds/home");
	std::fs::create_dir_all(home.join("stories")).unwrap();
	std::fs::write(
		home.join(fables::world::FILE),
		"name = \"Home\"\nauthor = \"A. Writer\"\n",
	)
	.unwrap();
	std::fs::write(
		home.join("cast.toml"),
		"[mo]\nname = \"Mo\"\n[nan]\nname = \"Nan\"\nkind = \"grown-up\"\n",
	)
	.unwrap();

	(root, home)
}

/// Runs `fables` with the root's `worlds/`.
fn fables(root: &Path, args: &[&str]) -> std::process::Output {
	Command::new(BIN)
		.arg("--root")
		.arg(root)
		.args(args)
		.output()
		.expect("fables runs")
}

fn stdout(output: &std::process::Output) -> String {
	String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
	String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Whether a flow stages a second character and so needs `--friend`.
fn needs_friend(flow: &str) -> bool {
	!matches!(
		flow,
		"three-tries" | "there-and-back" | "round-we-go" | "goodnight-tour" | "gag-short"
	)
}

#[test]
fn every_built_in_flow_scaffold_has_exactly_one_age() {
	let (_root, home) = scratch();
	let world = World::load(&home).unwrap();

	for flow in flow::built_in_flows().unwrap() {
		let friend = needs_friend(&flow.slug).then_some("nan");
		let script = flow
			.scaffold(&world, "A Tale", "mo", friend, "A. Writer")
			.unwrap();

		assert_eq!(
			script.matches("\nage").count(),
			1,
			"{}: {script}",
			flow.slug
		);
		assert!(script.contains("age = \"3-5\""), "{}: {script}", flow.slug);
	}
}

#[test]
fn a_worlds_flow_without_an_age_gets_the_default() {
	let (root, home) = scratch();
	std::fs::create_dir_all(home.join("flows")).unwrap();
	std::fs::write(home.join("flows/plain-shape.md"), flow_file("Plain Shape")).unwrap();

	let output = fables(
		root.path(),
		&[
			"new",
			"A Plain Tale",
			"--star",
			"mo",
			"--flow",
			"plain-shape",
		],
	);
	assert!(output.status.success(), "{}", stderr(&output));

	let script = std::fs::read_to_string(home.join("stories/a-plain-tale/story.md")).unwrap();
	assert_eq!(script.matches("\nage").count(), 1, "{script}");
	assert!(script.contains("age = \"3-5\""), "{script}");
}

#[test]
fn every_built_in_flow_lays_out_the_pages_its_beats_promise() {
	let flows = flow::built_in_flows().unwrap();
	assert_eq!(flows.len(), 9, "the nine built-in flows are all here");

	for flow in &flows {
		let laid_out: usize = flow.beats.iter().map(|beat| beat.pages).sum();

		assert!(
			flow.pages.0 <= laid_out && laid_out <= flow.pages.1,
			"{}: {laid_out} pages, budget {}–{}",
			flow.slug,
			flow.pages.0,
			flow.pages.1
		);
		assert!(
			!flow.about.is_empty() && !flow.good_for.is_empty(),
			"{}: a flow has an about and a good-for",
			flow.slug
		);
	}
}

#[test]
fn every_flow_starts_a_book_that_checks() {
	let (root, _home) = scratch();

	for flow in flow::built_in_flows().unwrap() {
		let title = format!("A {} Tale", flow.name);
		let slug = fables::story::slugify(&title);
		let mut args = vec!["new", &title, "--star", "mo", "--flow", &flow.slug];

		if needs_friend(&flow.slug) {
			args.extend(["--friend", "nan"]);
		}

		let output = fables(root.path(), &args);
		assert!(
			output.status.success(),
			"{}: {}{}",
			flow.slug,
			stdout(&output),
			stderr(&output)
		);

		let story = root
			.path()
			.join(format!("worlds/home/stories/{slug}/story.md"));
		let output = fables(root.path(), &["check", story.to_str().unwrap()]);
		let text = stdout(&output);

		assert!(
			output.status.success(),
			"{} did not check: {}{}",
			flow.slug,
			text,
			stderr(&output)
		);
		assert!(
			text.contains(&format!(
				"{} script pages",
				flow.beats.iter().map(|b| b.pages).sum::<usize>()
			)),
			"{}: {text}",
			flow.slug
		);
	}
}

#[test]
fn a_worlds_own_flow_is_found_before_the_built_in() {
	let (_root, home) = scratch();
	std::fs::create_dir_all(home.join("flows")).unwrap();
	std::fs::write(
		home.join("flows/three-tries.md"),
		flow_file("Home Three Tries"),
	)
	.unwrap();
	std::fs::write(home.join("flows/our-shape.md"), flow_file("Our Shape")).unwrap();
	let world = World::load(&home).unwrap();

	let found = flow::find(Some(&world), "three-tries").unwrap();
	assert_eq!(found.name, "Home Three Tries");
	assert_eq!(
		found.path.as_deref(),
		Some(home.join("flows/three-tries.md").as_path())
	);

	let built_in = flow::find(None, "three-tries").unwrap();
	assert_eq!(built_in.name, "Three Tries");

	let listed = flow::list(Some(&world)).unwrap();
	let names: Vec<&str> = listed.iter().map(|flow| flow.slug.as_str()).collect();
	assert_eq!(
		names[0], "our-shape",
		"a world's own flows come first: {names:?}"
	);
	assert_eq!(names[1], "three-tries");
	assert_eq!(
		names.len(),
		10,
		"the world's two flows plus the eight built-ins it does not replace: {names:?}"
	);
	assert_eq!(
		names.iter().filter(|name| **name == "three-tries").count(),
		1,
		"the world's three-tries replaces the built-in: {names:?}"
	);
}

#[test]
fn a_flow_that_needs_a_friend_names_the_flow_and_the_cast() {
	let (root, _home) = scratch();
	let output = fables(
		root.path(),
		&["new", "No Friend", "--star", "mo", "--flow", "odd-couple"],
	);
	let error = stderr(&output);

	assert!(!output.status.success(), "{error}");
	assert!(error.contains("two characters"), "{error}");
	assert!(error.contains("mo, nan"), "{error}");
}

#[test]
fn a_world_without_a_recipe_starts_from_three_tries() {
	let (root, home) = scratch();
	let output = fables(root.path(), &["new", "A Default Tale", "--star", "mo"]);
	assert!(output.status.success(), "{}", stderr(&output));

	let script = std::fs::read_to_string(home.join("stories/a-default-tale/story.md")).unwrap();
	assert!(script.contains("flow = \"three-tries\""), "{script}");
	assert_eq!(script.matches("\n# ").count(), 15, "{script}");
}

#[test]
fn every_worked_example_stays_inside_its_flows_page_budget() {
	let mut used: Vec<String> = Vec::new();

	for script in examples() {
		let story = Story::load(&script).unwrap();
		let Some(name) = &story.meta.flow else {
			continue;
		};

		let shape = flow::find(None, name).unwrap();
		let pages = story.pages.len();
		assert!(
			shape.pages.0 <= pages && pages <= shape.pages.1,
			"{}: {pages} pages, {} wants {}–{}",
			script.display(),
			shape.slug,
			shape.pages.0,
			shape.pages.1
		);
		used.push(shape.slug);
	}

	for slug in WORKED {
		assert!(
			used.iter().any(|name| name == slug),
			"{slug} has no worked example: {used:?}"
		);
	}
}

#[test]
fn every_worked_example_plans_a_storyboard_panel_for_its_pages() {
	for script in examples() {
		let mut story = Story::load(&script).unwrap();
		let world = World::for_story(&story, None).unwrap();
		world.furnish(&mut story).unwrap();
		let cast = world.cast_for(&story).unwrap();
		let out = tempfile::tempdir().unwrap();
		let timeline = fables::publish::estimated_timeline(&story, &cast, out.path()).unwrap();
		let panels: usize = timeline
			.shots
			.iter()
			.map(|shot| shot.setups_for(false).len())
			.sum();

		println!(
			"{}: {} pages, {panels} storyboard panels",
			story.meta.title,
			story.pages.len()
		);
		assert!(
			panels >= story.pages.len(),
			"{}: {panels} panels for {} pages",
			script.display(),
			story.pages.len()
		);
	}
}

#[test]
fn flow_list_and_flow_show_print_the_shape() {
	let root = tempfile::tempdir().unwrap();
	let list = fables(root.path(), &["flow", "list"]);
	let text = stdout(&list);

	assert!(list.status.success(), "{}", stderr(&list));

	for flow in flow::built_in_flows().unwrap() {
		assert!(text.contains(&flow.slug), "{}: {text}", flow.slug);
		assert!(text.contains(&flow.about), "{}: {text}", flow.slug);
		assert!(text.contains(&flow.good_for), "{}: {text}", flow.slug);
	}

	let show = fables(root.path(), &["flow", "show", "three-tries"]);
	let text = stdout(&show);
	assert!(show.status.success(), "{}", stderr(&show));
	assert!(text.contains("good for:"), "{text}");
	assert!(text.contains("Home and the want, said aloud"), "{text}");
	assert!(text.contains("Try 4 succeeds"), "{text}");
	assert!(text.contains("pages"), "{text}");

	for flow in flow::built_in_flows().unwrap() {
		let output = fables(root.path(), &["flow", "show", &flow.slug]);
		let text = stdout(&output);

		assert!(output.status.success(), "{}", stderr(&output));
		assert!(text.contains(&flow.beats[0].name), "{}: {text}", flow.slug);
		assert!(
			text.contains(&flow.beats[flow.beats.len() - 1].name),
			"{}: {text}",
			flow.slug
		);
	}
}

/// Every example world's story that names a flow, in a stable order.
fn examples() -> Vec<PathBuf> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut scripts = Vec::new();
	let mut worlds: Vec<PathBuf> = std::fs::read_dir(root.join("worlds"))
		.unwrap()
		.filter_map(|entry| Some(entry.ok()?.path()))
		.collect();
	worlds.sort();

	for world in worlds {
		let Ok(entries) = std::fs::read_dir(world.join("stories")) else {
			continue;
		};
		let mut found: Vec<PathBuf> = entries
			.filter_map(|entry| Some(entry.ok()?.path().join("story.md")))
			.filter(|script| script.is_file())
			.collect();
		found.sort();
		scripts.extend(found);
	}

	scripts
}

/// A one-page flow a world can own, for the lookup tests.
fn flow_file(name: &str) -> String {
	format!(
		"+++\ntitle = \"{{{{title}}}}\"\nauthor = \"{{{{author}}}}\"\nstar = \
		 \"{{{{star}}}}\"\n\n[flow]\nname = \"{name}\"\nabout = \"The world's own \
		 shape.\"\ngood-for = \"When the world wants its own way.\"\npages = [1, 1]\nwords = [1, \
		 20]\nneeds = \"1\"\n\n[[flow.beats]]\nname = \"One page\"\npages = 1\nwords = [1, \
		 20]\nnote = \"One page is enough here.\"\n+++\n# plain\n@{{{{star}}}} happy\n[One \
		 line.]\n"
	)
}
