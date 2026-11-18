//! The five-minute guide, run for real: every command in its fenced `bash` blocks, in order, in
//! a fresh folder. The guide cannot rot without this test failing.

use std::fmt::Write as _;
use std::process::Command;

const GUIDE: &str = include_str!("../docs/first-cartoon.md");
const BIN: &str = env!("CARGO_BIN_EXE_fables");

/// The guide's `bash` blocks, in order.
fn bash_blocks(source: &str) -> Vec<Vec<String>> {
	let lines: Vec<&str> = source.lines().collect();
	let mut blocks = Vec::new();
	let mut index = 0;

	while index < lines.len() {
		if lines[index].trim() == "```bash" {
			index += 1;
			let start = index;

			while index < lines.len() && lines[index].trim() != "```" {
				index += 1;
			}

			assert!(index < lines.len(), "the guide has an unclosed bash block");
			blocks.push(
				lines[start..index]
					.iter()
					.map(|line| (*line).to_owned())
					.collect(),
			);
		}

		index += 1;
	}

	blocks
}

/// A line with a leading `fables` command pointed at the binary under test.
fn shell(line: &str) -> String {
	let indent = line.len() - line.trim_start().len();
	let (indent, command) = line.split_at(indent);

	if let Some(rest) = command.strip_prefix("fables ") {
		format!("{indent}{} {rest}", quote(BIN))
	} else if command == "fables" {
		format!("{indent}{}", quote(BIN))
	} else {
		line.to_owned()
	}
}

fn quote(path: &str) -> String {
	format!("'{}'", path.replace('\'', "'\\''"))
}

#[test]
fn the_guide_runs_from_an_empty_folder() {
	let blocks = bash_blocks(GUIDE);
	assert!(
		blocks.len() >= 4,
		"the guide has {} bash blocks",
		blocks.len()
	);
	// The preview video records a voice with macOS `say` and encodes with ffmpeg. Both are
	// required for `fables video`; elsewhere the guide's other commands are still all run.
	let can_video = cfg!(target_os = "macos")
		&& Command::new("ffmpeg")
			.arg("-version")
			.output()
			.is_ok_and(|output| output.status.success());
	let mut script = String::from("set -e\n");

	for (index, block) in blocks.iter().enumerate() {
		writeln!(script, "echo '▶ block {}'", index + 1).expect("writing to a String cannot fail");

		for line in block {
			if !can_video && line.trim_start().starts_with("fables video ") {
				script.push_str("echo 'skipped: fables video (needs macOS say and ffmpeg)'\n");
				continue;
			}

			script.push_str(&shell(line));
			script.push('\n');
		}
	}

	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join("guide.sh");
	std::fs::write(&path, &script).unwrap();
	let output = Command::new("sh")
		.arg("-e")
		.arg(&path)
		.current_dir(dir.path())
		.output()
		.unwrap();
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		output.status.success(),
		"the guide failed:\n{stdout}\n{stderr}\n--- script ---\n{script}"
	);
	assert!(
		stdout.contains("drawing: chibi · skin: riso"),
		"the guide's check reports the drawing style and page skin:\n{stdout}"
	);

	// What the guide says you will find is there.
	let cartoon = dir.path().join("my-cartoon");
	for file in [
		"worlds/teacup-lane/world.toml",
		"worlds/teacup-lane/cast.toml",
		"worlds/teacup-lane/puppets/mum.svg",
		"worlds/teacup-lane/stories/pip-and-the-puddle/story.md",
		"out/pip-and-the-puddle/riso/proofs/contact-sheet.png",
		"out/pip-and-the-puddle/riso/storyboard/storyboard.png",
	] {
		let path = cartoon.join(file);
		assert!(
			path.is_file(),
			"the guide promises {file}; {}",
			path.display()
		);
	}

	assert!(
		!cartoon.join("worlds/teacup-lane/puppets/pip.svg").exists(),
		"Pip is drawn from the template, without a file"
	);

	if can_video {
		let videos: Vec<_> = std::fs::read_dir(cartoon.join("out/pip-and-the-puddle/riso/video"))
			.unwrap()
			.filter_map(|entry| Some(entry.ok()?.path()))
			.filter(|path| path.extension().is_some_and(|extension| extension == "mp4"))
			.collect();
		assert!(!videos.is_empty(), "the guide's preview video was written");
	}
}
