//! The readme's tables and the script parser must not drift apart. Every page-heading option and
//! every stage-direction word, key and value the tables document is fed to a minimal script, so
//! a documented spelling the parser refuses fails here rather than in somebody's book.

use std::collections::HashMap;

use fables::cast::Cast;
use fables::stage::Scene;
use fables::story;
use fables::story::Beat;

const README: &str = include_str!("../readme.md");

/// The rows of the first Markdown table under `heading`, as (first cell, second cell).
fn table(heading: &str) -> Vec<(String, String)> {
	let start = README
		.lines()
		.position(|line| line.trim() == heading)
		.unwrap_or_else(|| panic!("the readme has no `{heading}` heading"));
	let mut rows: Vec<(String, String)> = Vec::new();

	for line in README.lines().skip(start + 1) {
		let line = line.trim();

		if !line.starts_with('|') {
			if rows.is_empty() {
				continue;
			}

			break;
		}

		let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();

		// The alignment rule under the header (`| --- | --- |`).
		if cells
			.iter()
			.all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':'))
		{
			continue;
		}

		rows.push((
			cells[0].to_owned(),
			cells.get(1).copied().unwrap_or_default().to_owned(),
		));
	}

	rows.remove(0); // the header row
	rows
}

/// A cell's code spans, each with the text between it and the span before it.
fn spans(cell: &str) -> Vec<(String, String)> {
	let mut out = Vec::new();
	let mut rest = cell;

	while let Some(open) = rest.find('`') {
		let after = &rest[open + 1..];
		let Some(close) = after.find('`') else {
			break;
		};

		out.push((rest[..open].to_owned(), after[..close].to_owned()));
		rest = &after[close + 1..];
	}

	out
}

/// A value the table writes as a placeholder, replaced with one the parser accepts.
fn value_for(key: &str, value: &str) -> String {
	match value {
		"" | "…" | "<seconds>" | "<actor>" | "<actor>,<actor>" => {
			match key {
				"look" => "0.5,0.5".into(),
				"on" => "stinky".into(),
				"shot" => "close".into(),
				"move" => "push".into(),
				"over" => "2".into(),
				_ => "1".into(),
			}
		}
		value => value.to_owned(),
	}
}

/// The page-heading options and the built-in setting names the table documents.
fn page_options() -> (Vec<String>, Vec<String>) {
	let mut options = Vec::new();
	let mut settings = Vec::new();

	for (option, meaning) in table("### Page headings") {
		if option == "Setting" {
			settings.extend(spans(&meaning).into_iter().map(|(_, span)| span));
			continue;
		}

		for (_, span) in spans(&option) {
			for word in span.split_whitespace() {
				match word.split_once('=') {
					Some((key, value)) => options.push(format!("{key}={}", value_for(key, value))),
					None => options.push(word.to_owned()),
				}
			}
		}
	}

	(options, settings)
}

/// The stage-direction words, keys and values the table documents. A cell like
/// `` `teeth=clean`/`grotty` `` documents `grotty` as another value of `teeth`.
fn stage_directions() -> Vec<String> {
	let mut tokens = Vec::new();

	for (kind, values) in table("### Stage directions") {
		// A row of whole directions (a `@pause`, `@hold`, `@music` or `@sfx`) or of `@sfx`
		// names, fed to the parser exactly as the readme writes them.
		if kind.starts_with("Directions") {
			tokens.extend(spans(&values).into_iter().map(|(_, span)| span));
			continue;
		}

		if kind == "Effects" {
			tokens.extend(
				spans(&values)
					.into_iter()
					.map(|(_, name)| format!("@sfx {name}")),
			);
			continue;
		}

		let words = matches!(kind.as_str(), "Moods" | "Poses" | "Gaits" | "Actions")
			|| kind.starts_with("Stances")
			|| kind == "Camera";
		let mut key = None;

		for (gap, span) in spans(&values) {
			if let Some((name, value)) = span.split_once('=') {
				tokens.push(format!("{name}={}", value_for(name, value)));
				key = Some(name.to_owned());
			} else if gap.trim() == "/" {
				if let Some(name) = &key {
					tokens.push(format!("{name}={span}"));
				}
			} else if words && !span.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
				// A bare number is a range's upper bound (`shake=0`–`1`), not a word.
				tokens.push(span);
			}
		}
	}

	tokens
}

/// Rehearses a minimal script, so a refusal names the line to fix.
fn parses(script: &str) -> Result<(), String> {
	let source = format!("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n{script}");
	let story = story::parse(&source).map_err(|error| format!("{error:#}"))?;
	let page = &story.pages[0];
	let mut scene = Scene::for_page(page, &story.settings).map_err(|error| format!("{error:#}"))?;
	let cast =
		Cast::parse("[stinky]\nname = \"Stinky\"\n").map_err(|error| format!("{error:#}"))?;
	let mut memory = HashMap::new();

	for beat in &page.beats {
		if let Beat::Direct(directive) = beat {
			scene
				.apply(directive, &cast, &mut memory)
				.map_err(|error| format!("{error:#}"))?;
		}
	}

	Ok(())
}

#[test]
fn every_documented_page_heading_option_parses() {
	let (options, settings) = page_options();

	assert!(
		options.len() > 15,
		"the page-heading table should document its options: {options:?}"
	);
	assert!(
		settings.len() >= 15,
		"the setting row should name the built-in settings: {settings:?}"
	);

	for option in &options {
		parses(&format!("# park {option}\n@stinky\n"))
			.unwrap_or_else(|error| panic!("# park {option}: {error}"));
	}

	for setting in &settings {
		parses(&format!("# {setting}\n")).unwrap_or_else(|error| panic!("# {setting}: {error}"));
	}

	assert!(
		options.iter().any(|option| option.starts_with("size=")),
		"{options:?}"
	);
	assert!(
		options.iter().any(|option| option == "no-wormsley"),
		"{options:?}"
	);
}

#[test]
fn every_documented_stage_direction_word_key_and_value_parses() {
	let tokens = stage_directions();

	assert!(
		tokens.len() > 60,
		"the stage-direction table should document its vocabulary: {tokens:?}"
	);

	for token in &tokens {
		// A whole direction parses as written, with no actor in front of it.
		if token.starts_with('@') {
			parses(&format!("# park\n{token}\n"))
				.unwrap_or_else(|error| panic!("{token}: {error}"));
			continue;
		}

		let mut failures = Vec::new();
		let mut accepted = false;

		// A direction is written on an actor, `@scene` or `@camera`; a documented token needs
		// only one of them to understand it.
		for actor in ["stinky", "scene", "camera"] {
			match parses(&format!("# park\n@{actor} {token}\n")) {
				Ok(()) => {
					accepted = true;
					break;
				}
				Err(error) => failures.push(format!("@{actor}: {error}")),
			}
		}

		assert!(
			accepted,
			"`{token}` is documented but refused:\n  {}",
			failures.join("\n  ")
		);
	}
}
