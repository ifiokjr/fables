//! The command line's own promises: a script's `world = "…"` is a path relative to the script,
//! an absolute one is refused, every build says which folder it resolved, and a reader that
//! goes away (`| head -1`) ends any command quietly instead of panicking.

use std::process::Command;
use std::process::Stdio;

/// A world with an empty cast and one book that names its world with a relative path.
fn scratch() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
	let root = tempfile::tempdir().unwrap();
	let world = root.path().join("worlds/home");
	let book = world.join("stories/book");
	std::fs::create_dir_all(&book).unwrap();
	std::fs::write(world.join("world.toml"), "name = \"Home\"\n").unwrap();
	std::fs::write(world.join("cast.toml"), "").unwrap();
	let script = book.join("story.md");

	(root, world, script)
}

fn check(script: &std::path::Path) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_fables"))
		.arg("check")
		.arg(script)
		.output()
		.expect("fables runs")
}

#[test]
fn a_script_names_its_world_and_the_build_says_which_folder_it_read() {
	let (_root, world, script) = scratch();
	std::fs::write(
		&script,
		"+++\ntitle = \"Home\"\nauthor = \"A\"\nworld = \"../..\"\n+++\n# plain\nHi.\n",
	)
	.unwrap();

	let output = check(&script);
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		output.status.success(),
		"stdout: {stdout}\nstderr: {stderr}"
	);
	assert!(stdout.contains("world: "), "{stdout}");
	assert!(
		stdout.contains(&world.display().to_string()),
		"the resolved world folder is printed: {stdout}"
	);
	assert!(stdout.contains("skin: "), "{stdout}");

	// An absolute path is refused, and the error states the contract.
	std::fs::write(
		&script,
		format!(
			"+++\ntitle = \"Home\"\nauthor = \"A\"\nworld = \"{}\"\n+++\n# plain\nHi.\n",
			world.display()
		),
	)
	.unwrap();

	let output = check(&script);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(!output.status.success(), "{stderr}");
	assert!(
		stderr.contains("relative") && stderr.contains("world"),
		"{stderr}"
	);
}

/// Runs the binary with its stdout pipe closed before it writes, as `| head -1` does.
fn closed_stdout(args: &[&str]) -> std::process::Output {
	let mut child = Command::new(env!("CARGO_BIN_EXE_fables"))
		.args(args)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("fables runs");

	// The reader takes what it needs and closes the pipe.
	drop(child.stdout.take());
	child.wait_with_output().expect("fables exits")
}

#[test]
fn a_reader_that_goes_away_does_not_panic_the_command() {
	let story = concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md"
	);
	let commands: [&[&str]; 3] = [
		&["style", "list"],
		&["style", "describe", "cartoon"],
		&["check", story],
	];

	for args in commands {
		let output = closed_stdout(args);
		let stderr = String::from_utf8_lossy(&output.stderr);
		assert!(
			output.status.success(),
			"{args:?} failed on a closed pipe: {:?}\n{stderr}",
			output.status
		);
		assert!(
			!stderr.contains("panicked") && !stderr.contains("Broken pipe"),
			"{args:?}: {stderr}"
		);
	}
}
