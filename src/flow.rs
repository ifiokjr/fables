//! Story flows: the shapes a book can take, each a recipe compiled into fables.
//!
//! A flow file is a recipe like a world's `recipe.md`, with one addition: a `[flow]` table in
//! its front matter describing the shape, its page and word budgets and its beats, followed by
//! the pages a book starts from. `fables new --flow <name>` writes those pages with the star
//! and friend filled in; `fables flow list` and `fables flow show` read the table.
//!
//! The nine built-in flows live in `flows/`, one file each, and are compiled in like styles
//! and settings. A world may add its own in `flows/<name>.md`; those are found first, so a
//! world can replace a built-in flow of the same name.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;

use crate::world::World;

/// How many characters a flow stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Needs {
	/// The star alone, with generic critters where the story needs them.
	One,
	/// The star and a second cast member, written as `{{friend}}`.
	Two,
	/// The star, a second cast member and as many critters as the pile-up needs.
	Many,
}

impl Needs {
	pub const ALL: [&'static str; 3] = ["1", "2", "many"];

	pub fn parse(value: &str) -> Option<Needs> {
		match value.trim() {
			"1" | "one" => Some(Needs::One),
			"2" | "two" => Some(Needs::Two),
			"many" => Some(Needs::Many),
			_ => None,
		}
	}

	/// The word the table and `fables flow list` use.
	pub fn name(self) -> &'static str {
		match self {
			Needs::One => "1",
			Needs::Two => "2",
			Needs::Many => "many",
		}
	}

	/// How many characters the flow needs, in words.
	pub fn line(self) -> &'static str {
		match self {
			Needs::One => "1 character",
			Needs::Two => "2 characters",
			Needs::Many => "a crowd (the star, a friend and critters)",
		}
	}

	/// Whether a script started from the flow needs a `--friend`.
	pub fn needs_friend(self) -> bool {
		!matches!(self, Needs::One)
	}
}

/// One beat of a flow: what happens, over how many pages and in how many words.
#[derive(Clone, Debug, Serialize)]
pub struct Beat {
	pub name: String,
	pub pages: usize,
	/// The words a page of this beat carries.
	pub words: [usize; 2],
	pub note: String,
}

/// A flow's `[flow]` table, for reports and MCP.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
	/// The name `fables new --flow` and `fables flow show` take.
	pub flow: String,
	pub name: String,
	pub about: String,
	pub good_for: String,
	/// The story pages the finished book should run to.
	pub pages: [usize; 2],
	/// The words the finished book should run to.
	pub words: [usize; 2],
	pub needs: &'static str,
	pub beats: Vec<Beat>,
}

/// A recipe of pages to start a book from.
#[derive(Clone, Debug)]
pub struct Flow {
	/// The name the flow is found by: its file stem, e.g. `three-tries`.
	pub slug: String,
	pub name: String,
	pub about: String,
	pub good_for: String,
	pub pages: (usize, usize),
	pub words: (usize, usize),
	pub needs: Needs,
	pub beats: Vec<Beat>,
	/// The book's front matter, without the `[flow]` table.
	front: String,
	/// The laid-out pages.
	body: String,
	/// The file a world's own flow was read from.
	pub path: Option<PathBuf>,
}

/// A script `fables new` starts, and where it came from.
#[derive(Clone, Debug)]
pub struct Started {
	pub script: String,
	/// How the command names where the book started: `the flow "Three Tries"`, or
	/// `the Kerfuffle recipe`.
	pub source: String,
}

/// The built-in flows, in the order `fables flow list` prints them. The first is the default
/// when a world has no `recipe.md`.
const BUILT_IN: [(&str, &str); 9] = [
	("three-tries", include_str!("../flows/three-tries.md")),
	("pile-up", include_str!("../flows/pile-up.md")),
	("there-and-back", include_str!("../flows/there-and-back.md")),
	("round-we-go", include_str!("../flows/round-we-go.md")),
	("odd-couple", include_str!("../flows/odd-couple.md")),
	(
		"wrong-end-of-the-stick",
		include_str!("../flows/wrong-end-of-the-stick.md"),
	),
	("goodnight-tour", include_str!("../flows/goodnight-tour.md")),
	("gag-short", include_str!("../flows/gag-short.md")),
	("habit-tale", include_str!("../flows/habit-tale.md")),
];

/// The flow a book starts from when its world has no `recipe.md` and no flow is named.
pub const DEFAULT: &str = "three-tries";

/// The table of a flow file's front matter, as the file writes it (`[flow]` and its beats).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
	flow: Table,
}

/// The table of a flow file's front matter.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Table {
	name: String,
	about: String,
	good_for: String,
	pages: Span,
	words: Span,
	needs: NeedsField,
	beats: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Row {
	name: String,
	pages: usize,
	words: Span,
	note: String,
}

/// A page or word budget: one number, or a `[min, max]` pair.
#[derive(Deserialize)]
#[serde(untagged)]
enum Span {
	One(usize),
	Pair([usize; 2]),
}

impl Span {
	fn range(&self) -> (usize, usize) {
		match self {
			Span::One(count) => (*count, *count),
			Span::Pair([min, max]) => (*min, *max),
		}
	}
}

/// `needs = 1`, `needs = "2"` or `needs = "many"`.
#[derive(Deserialize)]
#[serde(untagged)]
enum NeedsField {
	Number(u64),
	Word(String),
}

impl NeedsField {
	fn needs(&self) -> Option<Needs> {
		match self {
			NeedsField::Number(count) => Needs::parse(&count.to_string()),
			NeedsField::Word(word) => Needs::parse(word),
		}
	}
}

impl Flow {
	/// Reads a flow file: front matter with a `[flow]` table, then the laid-out pages.
	pub fn parse(slug: &str, source: &str, path: Option<PathBuf>) -> Result<Flow> {
		let context = || {
			match &path {
				Some(path) => format!("in {}", path.display()),
				None => format!("in the {slug} flow"),
			}
		};
		let (front, body, _) = crate::story::split_front_matter(source).with_context(context)?;
		let at = table_offset(front).with_context(|| {
			format!(
				"{} is not a flow: its front matter has no `[flow]` table",
				describe(slug, path.as_deref())
			)
		})?;
		let table: Table = toml::from_str::<Document>(&front[at..])
			.with_context(|| format!("the [flow] table {} is not valid", context()))?
			.flow;
		// The book's front matter is everything above the flow's own table.
		let front = &front[..at];

		let pages = table.pages.range();
		let words = table.words.range();
		ensure_budget(&table.name, "pages", pages)?;
		ensure_budget(&table.name, "words", words)?;

		let needs = table.needs.needs().with_context(|| {
			format!(
				"the {} flow has `needs` that is not one of {}",
				table.name,
				Needs::ALL.join(", ")
			)
		})?;

		let mut beats = Vec::with_capacity(table.beats.len());
		let mut laid_out = 0;

		for row in table.beats {
			ensure!(row.pages > 0, "a beat of {} has no pages", table.name);
			ensure_budget(&table.name, "words", row.words.range())?;

			let beat = Beat {
				name: row.name,
				pages: row.pages,
				words: [row.words.range().0, row.words.range().1],
				note: row.note,
			};
			laid_out += beat.pages;
			beats.push(beat);
		}

		ensure!(!beats.is_empty(), "the {} flow has no beats", table.name);
		ensure!(
			pages.0 <= laid_out && laid_out <= pages.1,
			"the beats of {} lay out {laid_out} pages, outside its {}–{} page budget",
			table.name,
			pages.0,
			pages.1
		);
		ensure!(
			page_count(body) == laid_out,
			"{} lays out {} pages but its beats add up to {laid_out}",
			describe(slug, path.as_deref()),
			page_count(body)
		);
		ensure!(
			front.contains("{{title}}") && front.contains("{{star}}"),
			"the front matter of {} needs `{{{{title}}}}` and `{{{{star}}}}` placeholders",
			describe(slug, path.as_deref())
		);
		ensure!(
			lines(body).any(|line| line.contains("{{star}}")),
			"the pages of {} never stage `{{{{star}}}}`",
			describe(slug, path.as_deref())
		);
		if needs.needs_friend() {
			ensure!(
				lines(body).any(|line| line.contains("{{friend}}")),
				"{} stages a second character but never writes `{{{{friend}}}}`",
				describe(slug, path.as_deref())
			);
		} else {
			ensure!(
				!lines(body).any(|line| line.contains("{{friend}}")),
				"{} writes `{{{{friend}}}}` but its `needs` says one character",
				describe(slug, path.as_deref())
			);
		}

		Ok(Flow {
			slug: slug.to_owned(),
			name: table.name,
			about: table.about,
			good_for: table.good_for.trim().to_owned(),
			pages,
			words,
			needs,
			beats,
			front: front.to_owned(),
			body: body.to_owned(),
			path,
		})
	}

	pub fn report(&self) -> Report {
		Report {
			flow: self.slug.clone(),
			name: self.name.clone(),
			about: self.about.clone(),
			good_for: self.good_for.clone(),
			pages: [self.pages.0, self.pages.1],
			words: [self.words.0, self.words.1],
			needs: self.needs.name(),
			beats: self.beats.clone(),
		}
	}

	/// The pages a new book starts from, with the title, star, friend and author filled in and
	/// the flow's name recorded in its front matter.
	pub fn scaffold(
		&self,
		world: &World,
		title: &str,
		star: &str,
		friend: Option<&str>,
		author: &str,
	) -> Result<String> {
		let friend = match (self.needs, friend) {
			(Needs::One, None) => "",
			(Needs::One, Some(id)) => {
				bail!(
					"the {} flow stages one character, so `{id}` has nobody to play; leave out \
					 the friend",
					self.name
				)
			}
			(_, Some(id)) => id,
			(Needs::Two, None) => {
				bail!(
					"the {} flow stages two characters: pass the second as `--friend <id>`; {}",
					self.name,
					cast_names(world)?
				)
			}
			(Needs::Many, None) => {
				bail!(
					"the {} flow stages a crowd: pass the second character as `--friend <id>`; {}",
					self.name,
					cast_names(world)?
				)
			}
		};

		let mut script = format!("+++{}+++{}", self.front_with_flow(), self.body);
		script = script.replace("{{title}}", title);
		script = script.replace("{{author}}", author);
		script = script.replace("{{year}}", &year().to_string());
		script = script.replace("{{star}}", star);
		script = script.replace("{{flow}}", &self.slug);
		script = script.replace("{{friend}}", friend);

		Ok(script)
	}

	/// The flow's front matter with `flow = "<slug>"` written in it: on a `{{flow}}`
	/// placeholder, after the `star` line, or before the first table.
	fn front_with_flow(&self) -> String {
		if self.front.contains("{{flow}}") {
			return self.front.replace("{{flow}}", &self.slug);
		}

		let line = format!("flow = \"{}\"", self.slug);
		let mut out = String::with_capacity(self.front.len() + line.len() + 1);
		let mut written = false;

		for line_in in self.front.split_inclusive('\n') {
			let trimmed = line_in.trim_start();

			if !written && trimmed.starts_with('[') {
				out.push_str(&line);
				out.push('\n');
				written = true;
			}

			out.push_str(line_in);

			if !written && trimmed.starts_with("star") && trimmed.contains('=') {
				out.push_str(&line);
				out.push('\n');
				written = true;
			}
		}

		if !written {
			out.push_str(&line);
			out.push('\n');
		}

		out
	}
}

/// The script a new book starts from: a named flow's scaffold, or the world's own `recipe.md`
/// when no flow is named, or [`DEFAULT`] when the world has neither.
pub fn start(
	world: &World,
	title: &str,
	star: &str,
	friend: Option<&str>,
	flow: Option<&str>,
	author: Option<&str>,
) -> Result<Started> {
	let author = author.or(world.author.as_deref()).with_context(|| {
		"no author for the title page: pass --author, or set author in the world's world.toml"
	})?;

	if let Some(name) = flow {
		let flow = find(Some(world), name)?;

		return Ok(Started {
			script: flow.scaffold(world, title, star, friend, author)?,
			source: format!("the flow \"{}\"", flow.name),
		});
	}

	if let Some(recipe) = world.recipe() {
		let recipe = std::fs::read_to_string(&recipe)
			.with_context(|| format!("could not read {}", recipe.display()))?;
		let script = fill(&recipe, title, star, author, friend.unwrap_or(""));

		return Ok(Started {
			script,
			source: format!("the {} recipe", world.name),
		});
	}

	let flow = built_in(DEFAULT)?;

	Ok(Started {
		script: flow.scaffold(world, title, star, friend, author)?,
		source: format!("the flow \"{}\"", flow.name),
	})
}

/// The built-in flows, in file order.
pub fn built_in_flows() -> Result<Vec<Flow>> {
	BUILT_IN
		.iter()
		.map(|(slug, source)| Flow::parse(slug, source, None))
		.collect()
}

/// A built-in flow by its name.
pub fn built_in(slug: &str) -> Result<Flow> {
	let (slug, source) = BUILT_IN
		.iter()
		.find(|(name, _)| *name == slug)
		.with_context(|| format!("there is no built-in flow `{slug}`"))?;

	Flow::parse(slug, source, None)
}

/// The flows a world's books can start from: its own `flows/` first, then the built-ins it
/// does not replace.
pub fn list(world: Option<&World>) -> Result<Vec<Flow>> {
	let own = world.map(own_flows).transpose()?.unwrap_or_default();
	let mut flows = own;

	for (slug, source) in BUILT_IN {
		if flows.iter().any(|flow| flow.slug == slug) {
			continue;
		}

		flows.push(Flow::parse(slug, source, None)?);
	}

	Ok(flows)
}

/// A flow by name: the world's own first, then the built-ins.
pub fn find(world: Option<&World>, name: &str) -> Result<Flow> {
	if let Some(world) = world
		&& let Some(flow) = own_flows(world)?.into_iter().find(|flow| flow.slug == name)
	{
		return Ok(flow);
	}

	if let Some((slug, source)) = BUILT_IN.iter().find(|(slug, _)| *slug == name) {
		return Flow::parse(slug, source, None);
	}

	let known: Vec<String> = list(world)?.into_iter().map(|flow| flow.slug).collect();

	bail!(
		"there is no flow `{name}`; the flows are {}",
		known.join(", ")
	)
}

/// The flow files in a world's `flows/` folder, by slug.
fn own_flows(world: &World) -> Result<Vec<Flow>> {
	let dir = world.dir.join("flows");

	if !dir.is_dir() {
		return Ok(Vec::new());
	}

	let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
		.with_context(|| format!("could not read {}", dir.display()))?
		.filter_map(|entry| Some(entry.ok()?.path()))
		.filter(|path| path.extension().is_some_and(|extension| extension == "md"))
		.collect();
	paths.sort();

	paths
		.iter()
		.map(|path| {
			let slug = path
				.file_stem()
				.and_then(|stem| stem.to_str())
				.with_context(|| format!("{} has no usable name", path.display()))?;
			let source = std::fs::read_to_string(path)
				.with_context(|| format!("could not read {}", path.display()))?;

			Flow::parse(slug, &source, Some(path.clone()))
		})
		.collect()
}

/// Where the `[flow]` table starts in a flow's front matter.
fn table_offset(front: &str) -> Option<usize> {
	let mut offset = 0;

	for line in front.split_inclusive('\n') {
		if line.trim() == "[flow]" {
			return Some(offset);
		}

		offset += line.len();
	}

	None
}

/// The `# setting` headings a flow lays out, ignoring its `//` comments.
fn page_count(body: &str) -> usize {
	lines(body).filter(|line| line.starts_with("# ")).count()
}

/// The script lines that are not `//` comments.
fn lines(body: &str) -> impl Iterator<Item = &str> {
	body.lines()
		.map(str::trim)
		.filter(|line| !line.starts_with("//"))
}

fn ensure_budget(name: &str, what: &str, (min, max): (usize, usize)) -> Result<()> {
	ensure!(
		min <= max && max > 0,
		"{name} has a `{what}` budget of {min}–{max}; write one number or `[min, max]`"
	);
	Ok(())
}

fn describe(slug: &str, path: Option<&Path>) -> String {
	match path {
		Some(path) => path.display().to_string(),
		None => format!("the {slug} flow"),
	}
}

/// The cast a second character can be chosen from, for the error when a flow needs one.
fn cast_names(world: &World) -> Result<String> {
	let cast = world
		.cast()
		.with_context(|| format!("could not read {}'s cast", world.name))?;
	let names: Vec<&str> = cast.members.keys().map(String::as_str).collect();

	if names.is_empty() {
		Ok(format!(
			"{} has no cast yet; add a character with `fables character new <id>`",
			world.name
		))
	} else {
		Ok(format!("{}'s cast is {}", world.name, names.join(", ")))
	}
}

fn fill(recipe: &str, title: &str, star: &str, author: &str, friend: &str) -> String {
	recipe
		.replace("{{title}}", title)
		.replace("{{star}}", star)
		.replace("{{friend}}", friend)
		.replace("{{author}}", author)
		.replace("{{year}}", &year().to_string())
}

/// The year on the title page: whole years since 1970, from the clock.
fn year() -> u64 {
	1970 + std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_secs() / 31_556_952)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A minimal world with two cast members, for the flows that stage a friend.
	fn home() -> (tempfile::TempDir, World) {
		let dir = tempfile::tempdir().unwrap();
		let home = dir.path().join("home");
		std::fs::create_dir_all(&home).unwrap();
		std::fs::write(home.join(crate::world::FILE), "name = \"Home\"\n").unwrap();
		std::fs::write(
			home.join("cast.toml"),
			"[pip]\nname = \"Pip\"\n[mo]\nname = \"Mo\"\n",
		)
		.unwrap();

		let world = World::load(&home).unwrap();
		(dir, world)
	}

	/// A minimal flow file, which every test can vary.
	fn flow_source(pages: &str, body: &str) -> String {
		format!(
			"+++\ntitle = \"{{{{title}}}}\"\nauthor = \"{{{{author}}}}\"\nstar = \
			 \"{{{{star}}}}\"\n\n[flow]\nname = \"Test Flow\"\nabout = \"One \
			 sentence.\"\ngood-for = \"When a test needs one.\"\npages = {pages}\nwords = [10, \
			 20]\nneeds = \"1\"\n\n[[flow.beats]]\nname = \"The only beat\"\npages = 1\nwords = \
			 [10, 20]\nnote = \"Do the thing.\"\n+++\n{body}"
		)
	}

	#[test]
	fn a_flow_reads_its_table_and_counts_its_pages() {
		let source = flow_source("1", "# plain\n@{{star}} happy\n[Words.]\n");
		let flow = Flow::parse("test", &source, None).unwrap();

		assert_eq!(flow.name, "Test Flow");
		assert_eq!(flow.pages, (1, 1));
		assert_eq!(flow.words, (10, 20));
		assert_eq!(flow.needs, Needs::One);
		assert_eq!(flow.beats.len(), 1);
		assert_eq!(flow.beats[0].pages, 1);
	}

	#[test]
	fn a_flow_whose_beats_and_pages_disagree_is_refused() {
		let source = flow_source(
			"[1, 2]",
			"# plain\n@{{star}} happy\nHi.\n# park\n@{{star}} happy\nAgain.\n",
		);
		let error = format!("{:#}", Flow::parse("test", &source, None).unwrap_err());
		assert!(error.contains("beats add up to 1"), "{error}");
	}

	#[test]
	fn a_flow_without_a_table_or_with_an_unknown_key_is_refused() {
		let missing = "+++\ntitle = \"{{title}}\"\nstar = \"{{star}}\"\n+++\n# plain\n@{{star}}\n";
		assert!(Flow::parse("test", missing, None).is_err());

		let unknown = flow_source("1", "# plain\n@{{star}}\nHi.\n").replace(
			"words = [10, 20]\nneeds",
			"words = [10, 20]\npace = \"quick\"\nneeds",
		);
		let error = format!("{:#}", Flow::parse("test", &unknown, None).unwrap_err());
		assert!(error.contains("unknown field `pace`"), "{error}");
	}

	#[test]
	fn a_flow_that_does_not_stage_or_name_its_star_is_refused() {
		let source = flow_source("1", "# plain\nHi.\n");
		let error = format!("{:#}", Flow::parse("test", &source, None).unwrap_err());
		assert!(error.contains("{{star}}"), "{error}");
	}

	#[test]
	fn a_flows_friend_must_match_its_needs() {
		let lonely = flow_source("1", "# plain\n@{{star}} happy\n@{{friend}} happy\nHi.\n");
		let error = format!("{:#}", Flow::parse("test", &lonely, None).unwrap_err());
		assert!(error.contains("{{friend}}"), "{error}");
	}

	#[test]
	fn a_scaffold_fills_the_placeholders_and_records_the_flow() {
		let (_home, world) = home();
		let source = flow_source("1", "# plain\n@{{star}} happy\n[Words.]\n");
		let flow = Flow::parse("test", &source, None).unwrap();
		let script = flow
			.scaffold(&world, "A Tale", "pip", None, "A. Writer")
			.unwrap();
		let story = crate::story::parse(&script).unwrap();

		assert_eq!(story.meta.title, "A Tale");
		assert_eq!(story.meta.author, "A. Writer");
		assert_eq!(story.meta.star.as_deref(), Some("pip"));
		assert_eq!(story.meta.flow.as_deref(), Some("test"));
		assert!(script.contains("@pip happy"), "{script}");
		assert!(!script.contains("[flow]"), "{script}");
	}

	#[test]
	fn a_missing_or_unused_friend_is_an_error() {
		let (_home, world) = home();
		let source = flow_source("1", "# plain\n@{{star}} happy\nHi.\n");
		let flow = Flow::parse("test", &source, None).unwrap();
		let error = format!(
			"{:#}",
			flow.scaffold(&world, "A Tale", "pip", Some("mo"), "A. Writer")
				.unwrap_err()
		);
		assert!(error.contains("one character"), "{error}");
	}

	#[test]
	fn a_flow_that_needs_a_friend_names_the_cast_in_its_error() {
		let (_home, world) = home();
		let source = flow_source("1", "# plain\n@{{star}} happy\nHi.\n")
			.replace("needs = \"1\"", "needs = \"2\"")
			.replace("Hi.", "@{{friend}} happy\nHi.");
		let flow = Flow::parse("test", &source, None).unwrap();
		let error = format!(
			"{:#}",
			flow.scaffold(&world, "A Tale", "pip", None, "A. Writer")
				.unwrap_err()
		);
		assert!(error.contains("two characters"), "{error}");
		assert!(error.contains("mo, pip"), "{error}");
	}

	#[test]
	fn the_default_built_in_flow_parses_and_is_three_tries() {
		assert_eq!(DEFAULT, "three-tries");
		assert_eq!(built_in(DEFAULT).unwrap().name, "Three Tries");
	}
}
