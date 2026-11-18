//! "Did you mean …?": the nearest valid name to one a script or a flag got wrong.

/// The option nearest to `name`: the same word with a part after a dot (`bird` for `bird.a`), a
/// longer or shorter spelling of it, or one a slip of the keys away. `None` when nothing is close.
pub fn nearest<'a>(name: &str, options: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
	let name = name.trim().to_lowercase();

	if name.is_empty() {
		return None;
	}

	// A third of the word may be wrong, and always at least one letter.
	let slack = (name.chars().count() / 3).max(1);

	options
		.into_iter()
		.filter_map(|option| {
			let lower = option.to_lowercase();
			let base = lower.split('.').next().unwrap_or(&lower);
			let distance = if base == name || lower.split('.').next_back() == Some(&name) {
				0
			} else if lower.starts_with(&name) || name.starts_with(&lower) {
				1
			} else {
				edits(&name, &lower)
			};

			(distance <= slack).then_some((distance, option))
		})
		.min_by_key(|(distance, _)| *distance)
		.map(|(_, option)| option)
}

/// ` (did you mean `x`?)` for the nearest option, or nothing: ready to end an error with.
pub fn did_you_mean<'a>(name: &str, options: impl IntoIterator<Item = &'a str>) -> String {
	nearest(name, options).map_or(String::new(), |option| {
		format!(" (did you mean `{option}`?)")
	})
}

/// How many letters must be added, dropped or swapped to turn one word into the other.
fn edits(a: &str, b: &str) -> usize {
	let b: Vec<char> = b.chars().collect();
	let mut row: Vec<usize> = (0..=b.len()).collect();

	for (i, x) in a.chars().enumerate() {
		let mut diagonal = row[0];
		row[0] = i + 1;

		for (j, y) in b.iter().enumerate() {
			let swap = diagonal + usize::from(x != *y);
			diagonal = row[j + 1];
			row[j + 1] = swap.min(row[j] + 1).min(row[j + 1] + 1);
		}
	}

	row[b.len()]
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_bare_name_finds_its_numbered_actor() {
		assert_eq!(nearest("bird", ["bird.a", "momo"]), Some("bird.a"));
	}

	#[test]
	fn a_slip_of_the_keys_finds_the_word() {
		assert_eq!(
			nearest("tomatoe", ["teal", "tomato", "toffee"]),
			Some("tomato")
		);
		assert_eq!(nearest("wrem", ["wren", "gus"]), Some("wren"));
	}

	#[test]
	fn nothing_close_suggests_nothing() {
		assert_eq!(nearest("pip", ["nell"]), None);
		assert_eq!(nearest("", ["nell"]), None);
		assert_eq!(did_you_mean("pip", ["nell"]), "");
	}

	#[test]
	fn the_hint_ends_a_sentence() {
		assert_eq!(
			did_you_mean("bird", ["bird.a"]),
			" (did you mean `bird.a`?)"
		);
	}
}
