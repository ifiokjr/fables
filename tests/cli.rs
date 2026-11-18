//! The command line as a stranger drives it: reading one line of output and closing the pipe
//! (`fables check … | head -1`) must end the command quietly, not in a panic.

use std::process::Command;
use std::process::Stdio;

#[test]
fn a_reader_that_goes_away_does_not_panic_the_command() {
	// A command that does some work before its first line, so the reader is certainly closed
	// before anything is written.
	let world = concat!(env!("CARGO_MANIFEST_DIR"), "/worlds/kerfuffle");
	let mut child = Command::new(env!("CARGO_BIN_EXE_fables"))
		.args(["check", world])
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("fables runs");

	// The reader takes what it needs and closes the pipe, as `head` does.
	drop(child.stdout.take());

	let output = child.wait_with_output().expect("fables exits");
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		output.status.success(),
		"a closed pipe is not a failure: {:?}\n{stderr}",
		output.status
	);
	assert!(
		!stderr.contains("panicked") && !stderr.contains("Broken pipe"),
		"{stderr}"
	);
}
