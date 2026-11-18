//! What fables asks of ffmpeg, and a check that the installed build can do it.
//!
//! Videos and animatics are encoded with `libx264` and `aac` and filtered only with filters
//! that live in libavfilter itself. `aac` is ffmpeg's own encoder and always present;
//! `libx264` is an external library some builds leave out (Fedora's `ffmpeg-free`, for one).
//! Rather than fail deep inside an encode with ffmpeg's own words, [`require`] reads the
//! encoders and filters an argument list names and checks each against the installed build
//! before any frame is drawn, naming what is missing.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

/// The options whose value is a filter graph.
const GRAPH_OPTIONS: &[&str] = &[
	"-filter_complex",
	"-lavfi",
	"-vf",
	"-af",
	"-filter:v",
	"-filter:a",
];

/// The options whose value is an encoder.
const CODEC_OPTIONS: &[&str] = &["-c:v", "-c:a", "-codec:v", "-codec:a", "-vcodec", "-acodec"];

/// The encoders and filters an ffmpeg build offers.
struct Build {
	encoders: BTreeSet<String>,
	filters: BTreeSet<String>,
}

/// Fails, naming the missing piece, unless the installed ffmpeg has every encoder and filter
/// that `args` name. The build is asked once per process.
pub fn require(args: &[OsString]) -> Result<()> {
	static BUILD: OnceLock<Result<Build, String>> = OnceLock::new();

	let build = match BUILD.get_or_init(probe) {
		Ok(build) => build,
		Err(error) => bail!("{error}"),
	};

	for encoder in encoders_in(args) {
		if !build.encoders.contains(&encoder) {
			bail!(
				"this ffmpeg has no `{encoder}` encoder; fables needs an ffmpeg built with \
				 libx264 (Homebrew's, Debian's and Ubuntu's are)"
			);
		}
	}

	for filter in filters_in(args) {
		if !build.filters.contains(&filter) {
			bail!("this ffmpeg has no `{filter}` filter, which fables needs to cut video");
		}
	}

	Ok(())
}

/// Asks the installed ffmpeg for its encoders and filters.
fn probe() -> Result<Build, String> {
	let list = |what: &str| -> Result<BTreeSet<String>, String> {
		let output = Command::new("ffmpeg")
			.args(["-hide_banner", what])
			.stdin(Stdio::null())
			.output()
			.map_err(|_| "could not run ffmpeg; install it to make videos".to_owned())?;

		if !output.status.success() {
			return Err(format!("`ffmpeg {what}` failed"));
		}

		Ok(names(&String::from_utf8_lossy(&output.stdout)))
	};

	Ok(Build {
		encoders: list("-encoders")?,
		filters: list("-filters")?,
	})
}

/// The second column of each line of `ffmpeg -encoders` or `ffmpeg -filters`: the name, after
/// the flags. Header lines add a few stray words, which no real name matches.
fn names(listing: &str) -> BTreeSet<String> {
	listing
		.lines()
		.filter_map(|line| line.split_whitespace().nth(1))
		.map(str::to_owned)
		.collect()
}

/// The encoders `args` choose.
pub fn encoders_in(args: &[OsString]) -> Vec<String> {
	values_after(args, CODEC_OPTIONS)
		.filter(|codec| codec != "copy")
		.collect()
}

/// Every filter named in the graphs `args` carry, in order, repeats included.
pub fn filters_in(args: &[OsString]) -> Vec<String> {
	values_after(args, GRAPH_OPTIONS)
		.flat_map(|graph| filter_names(&graph))
		.collect()
}

fn values_after<'a>(
	args: &'a [OsString],
	options: &'a [&str],
) -> impl Iterator<Item = String> + 'a {
	args.windows(2)
		.filter(|pair| options.iter().any(|option| pair[0] == *option))
		.map(|pair| pair[1].to_string_lossy().into_owned())
}

/// The filter names in a filter graph: it splits on `;` and `,` outside quotes, drops the
/// `[link]` labels around each filter and keeps the name before its options.
pub fn filter_names(graph: &str) -> Vec<String> {
	let mut names = Vec::new();
	let mut current = String::new();
	let mut quoted = false;
	let mut chars = graph.chars();

	while let Some(character) = chars.next() {
		match character {
			'\\' => {
				current.push(character);

				if let Some(escaped) = chars.next() {
					current.push(escaped);
				}
			}
			'\'' => {
				quoted = !quoted;
				current.push(character);
			}
			';' | ',' if !quoted => {
				names.extend(name_of(&current));
				current.clear();
			}
			_ => current.push(character),
		}
	}

	names.extend(name_of(&current));

	names
}

fn name_of(filter: &str) -> Option<String> {
	let mut rest = filter.trim();

	while let Some(label) = rest.strip_prefix('[') {
		rest = label.split_once(']')?.1.trim_start();
	}

	let name: String = rest
		.chars()
		.take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
		.collect();

	(!name.is_empty()).then_some(name)
}

/// Where an encoder writes while it works: beside the target, under a name of this process's
/// own that keeps the extension ffmpeg picks its container by. The target's own name only ever
/// holds a finished file, even when a second render of the same book runs at the same time.
pub fn partial(path: &Path) -> PathBuf {
	let stem = path.file_stem().unwrap_or_default().to_string_lossy();
	let extension = path.extension().map_or(String::new(), |extension| {
		format!(".{}", extension.to_string_lossy())
	});

	path.with_file_name(format!("{stem}.partial-{}{extension}", std::process::id()))
}

/// Moves a finished encode into place, or clears a failed one away.
pub fn settle(partial: &Path, path: &Path, finished: bool) -> Result<()> {
	if finished {
		std::fs::rename(partial, path)
			.with_context(|| format!("could not move the finished file to {}", path.display()))
	} else {
		let _ = std::fs::remove_file(partial);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_partial_file_sits_beside_its_target_and_keeps_the_extension() {
		let target = Path::new("out/book/video/book-16x9.mp4");
		let partial = partial(target);
		assert_eq!(partial.parent(), target.parent());
		assert_eq!(partial.extension(), target.extension());
		assert_ne!(partial, target);
		assert!(
			partial
				.file_name()
				.unwrap()
				.to_string_lossy()
				.starts_with("book-16x9.partial-"),
			"{partial:?}"
		);
	}

	#[test]
	fn only_a_finished_encode_takes_the_target_name() {
		let dir = tempfile::tempdir().unwrap();
		let target = dir.path().join("a.mp4");
		let working = partial(&target);

		std::fs::write(&working, b"half").unwrap();
		settle(&working, &target, false).unwrap();
		assert!(!working.exists() && !target.exists());

		std::fs::write(&target, b"old").unwrap();
		std::fs::write(&working, b"whole").unwrap();
		settle(&working, &target, true).unwrap();
		assert!(!working.exists());
		assert_eq!(std::fs::read(&target).unwrap(), b"whole");
	}

	#[test]
	fn a_graph_names_its_filters_whatever_their_options_hold() {
		let graph = "[0:v]zoompan=z='min(1,(on-0)/24)':x='a,b;c':d=24,setsar=1[z0];[z0][1:\
		             v]overlay=x=0:y=H-h,format=yuv420p[v0];[v0][v1]concat=n=2:v=1:a=0[cut];\
		             color=c=0xffc93c:s=960x6:r=24:d=3[bar];[cut][bar]overlay@bar=x='-w+w*t/2':\
		             y=0[out]";

		assert_eq!(
			filter_names(graph),
			[
				"zoompan", "setsar", "overlay", "format", "concat", "color", "overlay"
			]
		);
	}

	#[test]
	fn encoders_and_graphs_are_read_from_their_options() {
		let args: Vec<OsString> = [
			"-i",
			"in.png",
			"-af",
			"loudnorm=I=-16",
			"-c:v",
			"libx264",
			"-c:a",
			"aac",
			"out.mp4",
		]
		.into_iter()
		.map(Into::into)
		.collect();

		assert_eq!(encoders_in(&args), ["libx264", "aac"]);
		assert_eq!(filters_in(&args), ["loudnorm"]);
	}

	#[test]
	fn a_listing_yields_its_names() {
		let listing = "Encoders:\n V..... = Video\n ------\n V....D libx264    libx264 H.264\n \
		               A....D aac       AAC (Advanced Audio Coding)\n";
		let found = names(listing);

		assert!(found.contains("libx264"));
		assert!(found.contains("aac"));
	}
}
