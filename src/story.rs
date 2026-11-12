//! The story script: one plain-text file that drives the book, the print files and the videos.
//!
//! A script is TOML front matter between `+++` fences, followed by pages. Each page starts with
//! a `# setting` heading, and its body interleaves narration paragraphs with `@actor` stage
//! directions. Directions take effect at the point in the narration where they are written, so
//! the same script gives a still illustration (the state at the end of the page) and a moving
//! scene (the state changing as the narrator reads).
//!
//! ```text
//! # bathroom text=bottom
//! @mum x=0.7 pose=point mood=cross talk
//! "Brush your teeth, Stanley!" said Mum.
//! @stinky pose=arms-crossed mood=defiant talk
//! "**NOPE**," said Stanley.
//! ```

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use crate::art::dials::Fill;
use crate::art::dials::Motion;
use crate::art::dials::Palette;
use crate::art::dials::Tone;
use crate::art::light::Light;
use crate::art::settings::Library;
use crate::art::style::Style;
use crate::art::style::StyleName;
pub use crate::video::voice::Voice;

/// Book-level metadata from the front matter.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Meta {
	pub title: String,
	#[serde(default)]
	pub subtitle: Option<String>,
	pub author: String,
	#[serde(default)]
	pub illustrator: Option<String>,
	#[serde(default)]
	pub series: Option<String>,
	#[serde(default)]
	pub number: Option<u32>,
	#[serde(default = "default_language")]
	pub language: String,
	#[serde(default)]
	pub year: Option<u32>,
	#[serde(default)]
	pub publisher: Option<String>,
	#[serde(default)]
	pub isbn: Option<String>,
	#[serde(default)]
	pub dedication: Option<String>,
	/// Back-cover and store description.
	#[serde(default)]
	pub blurb: Option<String>,
	/// The one-line moral, spoken at the end of the video.
	#[serde(default)]
	pub moral: Option<String>,
	/// The cast member drawn on the cover.
	#[serde(default)]
	pub star: Option<String>,
	/// The world the book belongs to, as a folder relative to the script. Defaults to the
	/// nearest `world.toml` above the script (see [`world`](crate::world)).
	#[serde(default)]
	pub world: Option<String>,
	/// Characters invited from other worlds under the same root, as `world/id` (e.g.
	/// `"kiteharbour/hana"`). A guest is directed by its id, or by `world.id` when its id
	/// clashes with a local cast member. See [`world::World::cast_for`].
	#[serde(default)]
	pub guests: Vec<String>,
	/// How guests are inked: with their own style's `[line]` and `[shade]` (the default), so
	/// they keep looking like themselves, or with the host world's.
	#[serde(default)]
	pub guest_style: GuestStyle,
	/// The light every page starts in, unless its heading names `light=` (`"golden"`). A
	/// world's default applies unless the book names its own.
	#[serde(default)]
	pub light: Option<String>,
	#[serde(default)]
	pub cover: Option<CoverSpec>,
	/// The art style: riso, tv, bold, cut-paper, crayon, storybook or clear (default riso).
	#[serde(default)]
	pub style: Option<String>,
	/// The narrator; without one, the world's narrator reads (see [`Story::narrator`]).
	#[serde(default)]
	pub voice: Option<Voice>,
	#[serde(default)]
	pub video: VideoOptions,
}

fn default_language() -> String {
	"en-GB".to_owned()
}

/// How a book's guests from other worlds are inked and shaded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestStyle {
	/// Each guest is inked with its own style's `[line]` and `[shade]` dials, so a cartoon kid
	/// and an anime kid can share a page looking like themselves.
	#[default]
	Own,
	/// Guests are inked and shaded with the host world's `[line]` and `[shade]` dials; their
	/// construction never changes.
	Host,
}

/// What the front cover shows.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct CoverSpec {
	#[serde(default)]
	pub setting: Option<String>,
	/// Directives applied to the star on the cover, e.g. `"mood=grin stink=1 talk"`.
	#[serde(default)]
	pub pose: Option<String>,
	#[serde(default)]
	pub tint: Option<String>,
}

/// How the video is performed.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct VideoOptions {
	/// Characters speak the words inside quotes in their own cast voices; the narrator reads the
	/// rest. Off, the narrator reads everything, like a parent reading aloud.
	#[serde(default)]
	pub cast_voices: bool,
	/// With cast voices, skip short dialogue tags ("said Mum.") so scenes play like a show.
	#[serde(default)]
	pub drop_tags: bool,
	/// Who directs the camera: `auto` (default) shots pages by film rules when the script says
	/// nothing; `locked` keeps the older always-gliding camera exactly as it was.
	#[serde(default)]
	pub direction: Direction,
	/// Soften the setting behind close shots (depth of field). On by default; video only.
	#[serde(default = "default_true")]
	pub depth_of_field: bool,
	/// How every page is entered, unless its heading says `transition=` (default wipe).
	#[serde(default)]
	pub transition: Option<Transition>,
	/// The performance layer: `full` (the default) animates by the acting rules in
	/// [`acting`](crate::video::acting); `plain` turns the whole layer off and reproduces the
	/// plain tween exactly.
	#[serde(default)]
	pub acting: Acting,
}

impl Default for VideoOptions {
	fn default() -> Self {
		Self {
			cast_voices: false,
			drop_tags: false,
			direction: Direction::default(),
			depth_of_field: true,
			transition: None,
			acting: Acting::default(),
		}
	}
}

/// Whether characters perform by the acting rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Acting {
	/// The acting rules are on, tuned by the style's `[motion]` dials.
	#[default]
	Full,
	/// The whole layer is off: parts blend with one easing curve and slide.
	Plain,
}

fn default_true() -> bool {
	true
}

/// Who directs the camera.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
	/// An automatic director shots pages without explicit `@camera` directions by film rules.
	#[default]
	Auto,
	/// The older camera, kept byte-for-byte: one auto-framed view per page that glides.
	Locked,
}

/// How a page is entered from the previous one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
	/// A paper sheet sweeps across (the classic page turn).
	#[default]
	Wipe,
	/// No effect: the page begins on the cut.
	Cut,
	/// The previous page dissolves into this one.
	Dissolve,
	/// The page goes through the paper colour.
	Fade,
	/// A circle closes on the previous page's subject and opens on this page's.
	Iris,
	/// The new page slides in from the right.
	Push,
}

impl Transition {
	pub const ALL: [&'static str; 6] = ["wipe", "cut", "dissolve", "fade", "iris", "push"];

	pub fn parse(value: &str) -> Option<Transition> {
		match value {
			"wipe" => Some(Transition::Wipe),
			"cut" => Some(Transition::Cut),
			"dissolve" => Some(Transition::Dissolve),
			"fade" => Some(Transition::Fade),
			"iris" => Some(Transition::Iris),
			"push" => Some(Transition::Push),
			_ => None,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Transition::Wipe => "wipe",
			Transition::Cut => "cut",
			Transition::Dissolve => "dissolve",
			Transition::Fade => "fade",
			Transition::Iris => "iris",
			Transition::Push => "push",
		}
	}
}

/// Where narration text appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Medium {
	Both,
	BookOnly,
	VideoOnly,
}

/// Where a page's text block sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPlace {
	Top,
	Bottom,
	None,
}

/// One `key=value` or bare-word token from a directive or page heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
	pub key: String,
	pub value: Option<String>,
}

impl fmt::Display for Token {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.value {
			Some(value) => write!(f, "{}={value}", self.key),
			None => write!(f, "{}", self.key),
		}
	}
}

/// A stage direction: `@actor tokens…`.
#[derive(Clone, Debug, PartialEq)]
pub struct Directive {
	pub actor: String,
	pub tokens: Vec<Token>,
	pub line: usize,
}

/// One step through a page, in reading order.
#[derive(Clone, Debug, PartialEq)]
pub enum Beat {
	Direct(Directive),
	Say {
		text: String,
		medium: Medium,
		line: usize,
	},
	Pause {
		seconds: f32,
		line: usize,
	},
	Sfx {
		name: String,
		line: usize,
	},
}

#[derive(Clone, Debug)]
pub struct Page {
	/// The setting's name, found in the story's [`Library`] when the page is staged.
	pub setting: String,
	pub options: Vec<Token>,
	pub beats: Vec<Beat>,
	pub line: usize,
}

impl Page {
	pub fn option(&self, key: &str) -> Option<&Token> {
		self.options.iter().find(|token| token.key == key)
	}

	pub fn flag(&self, key: &str) -> bool {
		self.option(key)
			.is_some_and(|token| token.value.as_deref().is_none_or(|value| value == "true"))
	}

	pub fn text_place(&self) -> TextPlace {
		match self.option("text").and_then(|token| token.value.as_deref()) {
			Some("bottom") => TextPlace::Bottom,
			Some("none") => TextPlace::None,
			_ => TextPlace::Top,
		}
	}

	/// How the page is entered from the previous one, from its heading
	/// (`transition=wipe|cut|dissolve|fade|iris|push`).
	pub fn transition(&self) -> Option<Transition> {
		self.option("transition")
			.and_then(|token| token.value.as_deref())
			.and_then(Transition::parse)
	}

	/// Whether a stage direction directs the camera, which makes the page shot exactly as
	/// written instead of by the automatic director's rules.
	pub fn directed_by_camera(&self) -> bool {
		self.beats
			.iter()
			.any(|beat| matches!(beat, Beat::Direct(directive) if directive.actor == "camera"))
	}

	/// Paragraphs printed in the book.
	pub fn book_text(&self) -> Vec<&str> {
		self.beats
			.iter()
			.filter_map(|beat| {
				match beat {
					Beat::Say { text, medium, .. } if *medium != Medium::VideoOnly => {
						Some(text.as_str())
					}
					_ => None,
				}
			})
			.collect()
	}

	pub fn in_video(&self) -> bool {
		self.option("video")
			.and_then(|token| token.value.as_deref())
			!= Some("skip")
	}

	pub fn in_book(&self) -> bool {
		self.option("book").and_then(|token| token.value.as_deref()) != Some("skip")
	}
}

#[derive(Clone, Debug)]
pub struct Story {
	pub meta: Meta,
	pub pages: Vec<Page>,
	/// Directory containing the script, for resolving relative paths.
	pub dir: std::path::PathBuf,
	/// File-name-safe identifier derived from the title.
	pub slug: String,
	/// The words its world writes into the front and back matter.
	pub matter: crate::world::Matter,
	/// How its world's drawing style prints and moves.
	pub drawing: Drawing,
	/// The settings its pages can use: the built-in ones and its world's.
	pub settings: Arc<Library>,
	/// The light pages start in when their heading says nothing (`light = "dawn"` in front
	/// matter, or the world's default).
	pub light: Light,
}

/// How a world's drawing style prints and moves (see [`StylePack`](crate::art::pack::StylePack)).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drawing {
	/// How colour is laid down on the page: flat, snapped to a palette, or screentone.
	pub fill: Fill,
	pub tone: Tone,
	pub palette: Palette,
	pub motion: Motion,
}

impl Story {
	/// Who reads the book aloud: the script's own `[voice]`, else the default voice.
	pub fn narrator(&self) -> &Voice {
		static DEFAULT: std::sync::LazyLock<Voice> = std::sync::LazyLock::new(Voice::default);
		self.meta.voice.as_ref().unwrap_or(&DEFAULT)
	}

	/// The book's art style.
	/// The skin pages are drawn in, printed the way the world's drawing style prints.
	pub fn style(&self) -> Style {
		let mut style = self
			.meta
			.style
			.as_deref()
			.and_then(Style::parse)
			.unwrap_or_default();
		style.finish.tone = (self.drawing.fill == Fill::Screentone).then_some(self.drawing.tone);
		style.finish.palette = (self.drawing.fill == Fill::Palette).then_some(self.drawing.palette);
		style
	}

	pub fn load(path: &Path) -> Result<Story> {
		let source = std::fs::read_to_string(path)
			.with_context(|| format!("could not read {}", path.display()))?;
		let mut story = parse(&source).with_context(|| format!("in {}", path.display()))?;
		story.dir = path.parent().map(Path::to_path_buf).unwrap_or_default();

		Ok(story)
	}
}

/// Parses a script from source text.
pub fn parse(source: &str) -> Result<Story> {
	let (meta_src, body, body_start) = split_front_matter(source)?;
	let meta: Meta = toml::from_str(meta_src).context("front matter is not valid")?;
	let pages = parse_pages(body, body_start)?;

	if pages.is_empty() {
		bail!("the story has no pages; start one with a `# setting` heading");
	}

	if let Some(name) = &meta.style {
		let names: Vec<&str> = StyleName::ALL.iter().map(|(name, _)| *name).collect();

		if StyleName::parse(name).is_none() {
			bail!("unknown style `{name}`; use one of: {}", names.join(", "));
		}
	}

	let light = meta
		.light
		.as_deref()
		.map(|name| {
			Light::parse(name).ok_or_else(|| {
				anyhow::anyhow!(
					"unknown light `{name}`; use one of: {}",
					crate::art::light::Preset::names()
				)
			})
		})
		.transpose()?
		.unwrap_or_else(Light::day);

	let slug = slugify(&meta.title);

	Ok(Story {
		meta,
		pages,
		dir: std::path::PathBuf::new(),
		slug,
		matter: crate::world::Matter::default(),
		drawing: Drawing::default(),
		settings: Library::built_in(),
		light,
	})
}

fn split_front_matter(source: &str) -> Result<(&str, &str, usize)> {
	let Some(rest) = source.trim_start().strip_prefix("+++") else {
		bail!("a story must begin with TOML front matter between `+++` lines");
	};

	let Some(end) = rest.find("\n+++") else {
		bail!("the front matter is never closed with a `+++` line");
	};

	let meta = &rest[..end];
	let after = &rest[end + 4..];
	let consumed = source.len() - after.len();
	// The body begins on the closing `+++` line itself (its remainder is empty).
	let body_start = source[..consumed].lines().count();

	Ok((meta, after, body_start))
}

fn parse_pages(body: &str, first_line: usize) -> Result<Vec<Page>> {
	let mut pages: Vec<Page> = Vec::new();
	let mut paragraph: Vec<&str> = Vec::new();
	let mut paragraph_line = 0;

	for (offset, raw) in body.lines().enumerate() {
		let line_no = first_line + offset;
		let line = raw.trim();

		let is_text = !line.is_empty()
			&& !line.starts_with('#')
			&& !line.starts_with('@')
			&& !line.starts_with("//");
		let starts_paragraph = line.starts_with('~') || line.starts_with('>');

		if (!is_text || starts_paragraph) && !paragraph.is_empty() {
			push_paragraph(&mut pages, &paragraph, paragraph_line)?;
			paragraph.clear();
		}

		if line.is_empty() || line.starts_with("//") {
			continue;
		}

		if let Some(heading) = line.strip_prefix('#') {
			pages.push(parse_heading(heading, line_no)?);
			continue;
		}

		if let Some(direction) = line.strip_prefix('@') {
			let page = pages.last_mut().with_context(|| {
				format!("line {line_no}: stage direction before the first `# setting` heading")
			})?;
			page.beats.push(parse_direction(direction, line_no)?);
			continue;
		}

		if paragraph.is_empty() {
			paragraph_line = line_no;
		}

		paragraph.push(line);
	}

	if !paragraph.is_empty() {
		push_paragraph(&mut pages, &paragraph, paragraph_line)?;
	}

	Ok(pages)
}

fn push_paragraph(pages: &mut [Page], lines: &[&str], line: usize) -> Result<()> {
	let page = pages
		.last_mut()
		.with_context(|| format!("line {line}: text before the first `# setting` heading"))?;
	let joined = lines.join(" ");
	let (medium, text) = if let Some(rest) = joined.strip_prefix('~') {
		(Medium::BookOnly, rest.trim())
	} else if let Some(rest) = joined.strip_prefix('>') {
		(Medium::VideoOnly, rest.trim())
	} else {
		(Medium::Both, joined.trim())
	};

	page.beats.push(Beat::Say {
		text: text.to_owned(),
		medium,
		line,
	});

	Ok(())
}

fn parse_heading(heading: &str, line: usize) -> Result<Page> {
	let mut words = heading.split_whitespace();
	let Some(name) = words.next() else {
		bail!("line {line}: a page heading needs a setting, e.g. `# park`");
	};

	let options = words.map(parse_token).collect();

	Ok(Page {
		setting: name.to_owned(),
		options,
		beats: Vec::new(),
		line,
	})
}

fn parse_direction(direction: &str, line: usize) -> Result<Beat> {
	let mut words = direction.split_whitespace();
	let Some(actor) = words.next() else {
		bail!("line {line}: `@` must be followed by an actor name");
	};

	let tokens: Vec<Token> = words.map(parse_token).collect();

	match actor {
		"pause" => {
			let seconds = tokens
				.first()
				.and_then(|token| token.key.parse::<f32>().ok())
				.with_context(|| {
					format!("line {line}: `@pause` needs a number of seconds, e.g. `@pause 1.5`")
				})?;

			Ok(Beat::Pause { seconds, line })
		}
		"sfx" => {
			let name = tokens
				.first()
				.map(|token| token.key.clone())
				.with_context(|| format!("line {line}: `@sfx` needs a sound name"))?;

			Ok(Beat::Sfx { name, line })
		}
		_ => {
			Ok(Beat::Direct(Directive {
				actor: actor.to_owned(),
				tokens,
				line,
			}))
		}
	}
}

/// Splits `key=value` and bare-word tokens out of a line of text.
pub fn tokens(text: &str) -> Vec<Token> {
	text.split_whitespace().map(parse_token).collect()
}

fn parse_token(word: &str) -> Token {
	match word.split_once('=') {
		Some((key, value)) => {
			Token {
				key: key.to_owned(),
				value: Some(value.to_owned()),
			}
		}
		None => {
			Token {
				key: word.to_owned(),
				value: None,
			}
		}
	}
}

/// A lowercase, hyphenated, ASCII identifier.
pub fn slugify(title: &str) -> String {
	let mut slug = String::new();

	for ch in title.chars() {
		if ch.is_ascii_alphanumeric() {
			slug.push(ch.to_ascii_lowercase());
		} else if !slug.ends_with('-') && !slug.is_empty() {
			slug.push('-');
		}
	}

	slug.trim_end_matches('-').to_owned()
}

/// A run of text with one style, produced from `*bold*` and `**shout**` markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
	pub text: String,
	pub style: RunStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStyle {
	Plain,
	Bold,
	Shout,
}

/// Splits a paragraph into styled runs.
pub fn runs(text: &str) -> Vec<Run> {
	let mut out: Vec<Run> = Vec::new();
	let mut rest = text;

	while !rest.is_empty() {
		let (marker, style) = if rest.starts_with("**") {
			("**", RunStyle::Shout)
		} else if rest.starts_with('*') {
			("*", RunStyle::Bold)
		} else {
			let next = rest.find('*').unwrap_or(rest.len());
			push_run(&mut out, &rest[..next], RunStyle::Plain);
			rest = &rest[next..];
			continue;
		};

		let inner = &rest[marker.len()..];

		if let Some(end) = inner.find(marker) {
			push_run(&mut out, &inner[..end], style);
			rest = &inner[end + marker.len()..];
		} else {
			// An unmatched marker is just a literal asterisk.
			push_run(&mut out, marker, RunStyle::Plain);
			rest = inner;
		}
	}

	out
}

fn push_run(out: &mut Vec<Run>, text: &str, style: RunStyle) {
	if text.is_empty() {
		return;
	}

	if let Some(last) = out.last_mut()
		&& last.style == style
	{
		last.text.push_str(text);
		return;
	}

	out.push(Run {
		text: text.to_owned(),
		style,
	});
}

/// The paragraph with markup removed (for TTS, captions and alt text).
pub fn plain_text(text: &str) -> String {
	runs(text).into_iter().map(|run| run.text).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	const SCRIPT: &str = r#"+++
title = "Stinky McStinkbreath"
author = "Ifiok Jr."
+++

// A comment.
# bathroom text=bottom night
@mum x=0.7 pose=point talk
"Brush your teeth!"
said Mum.

@stinky mood=defiant
~ This line is only in the book.
> This line is only in the video.
@pause 1.5
@sfx boing

# park
Hello.
"#;

	#[test]
	fn parses_pages_and_beats() {
		let story = parse(SCRIPT).unwrap();
		assert_eq!(story.slug, "stinky-mcstinkbreath");
		assert_eq!(story.pages.len(), 2);

		let page = &story.pages[0];
		assert_eq!(page.setting, "bathroom");
		assert_eq!(page.text_place(), TextPlace::Bottom);
		assert!(page.flag("night"));
		assert_eq!(page.beats.len(), 7);
		assert!(matches!(&page.beats[0], Beat::Direct(d) if d.actor == "mum" && d.line == 8));
		assert!(
			matches!(&page.beats[1], Beat::Say { text, medium: Medium::Both, line: 9 } if text == "\"Brush your teeth!\" said Mum.")
		);
		assert!(matches!(
			&page.beats[3],
			Beat::Say {
				medium: Medium::BookOnly,
				..
			}
		));
		assert!(matches!(
			&page.beats[4],
			Beat::Say {
				medium: Medium::VideoOnly,
				..
			}
		));
		assert!(
			matches!(page.beats[5], Beat::Pause { seconds, .. } if (seconds - 1.5).abs() < f32::EPSILON)
		);
		assert!(matches!(&page.beats[6], Beat::Sfx { name, .. } if name == "boing"));
		assert_eq!(page.book_text().len(), 2);
	}

	#[test]
	fn rejects_text_before_first_page() {
		let error = parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\nHello\n# park\n").unwrap_err();
		assert!(format!("{error:#}").contains("before the first"));
	}

	#[test]
	fn requires_front_matter() {
		assert!(parse("# park\nHi").is_err());
		assert!(parse("+++\ntitle = \"T\"\n").is_err());
	}

	#[test]
	fn splits_markup_into_runs() {
		let parsed = runs("\"**NOPE**,\" said *Stanley* with a 5* grin");
		assert_eq!(
			parsed,
			vec![
				Run {
					text: "\"".into(),
					style: RunStyle::Plain
				},
				Run {
					text: "NOPE".into(),
					style: RunStyle::Shout
				},
				Run {
					text: ",\" said ".into(),
					style: RunStyle::Plain
				},
				Run {
					text: "Stanley".into(),
					style: RunStyle::Bold
				},
				Run {
					text: " with a 5* grin".into(),
					style: RunStyle::Plain
				},
			]
		);
		assert_eq!(plain_text("**Hi** *there*"), "Hi there");
	}

	#[test]
	fn slugs_are_file_safe() {
		assert_eq!(slugify("Stinky McStinkbreath!"), "stinky-mcstinkbreath");
		assert_eq!(slugify("  The  End  "), "the-end");
	}

	#[test]
	fn guests_are_named_in_the_front_matter() {
		let story = parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nguests = [\"kiteharbour/hana\", \
			 \"kerfuffle/stinky\"]\nguest-style = \"host\"\n+++\n# plain\nHi.\n",
		)
		.unwrap();
		assert_eq!(
			story.meta.guests,
			vec!["kiteharbour/hana", "kerfuffle/stinky"]
		);
		assert_eq!(story.meta.guest_style, GuestStyle::Host);

		let plain = parse("+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# plain\nHi.\n").unwrap();
		assert_eq!(plain.meta.guests, Vec::<String>::new());
		assert_eq!(plain.meta.guest_style, GuestStyle::Own);

		let error = parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nguest-style = \"guests\"\n+++\n# plain\nHi.\n",
		)
		.unwrap_err();
		assert!(
			format!("{error:#}").contains("expected `own` or `host`"),
			"{error:#}"
		);
	}
}
