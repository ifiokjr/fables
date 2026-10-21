//! Fixed-layout EPUB 3 for Kindle (KDP accepts EPUB directly) and Apple Books.
//!
//! Each page is an XHTML document the size of the page: the illustration is a JPEG behind, and
//! the words are live HTML text positioned exactly where the print layout put them. Apple
//! rejects picture books whose text is baked into images, and live text also gives screen
//! readers and text-to-speech something to read.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use rayon::prelude::*;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::art::STAGE;
use crate::art::escape;
use crate::book::BookPage;
use crate::book::Side;
use crate::book::page_svg;
use crate::book::page_words;
use crate::raster;
use crate::story::RunStyle;
use crate::story::Story;
use crate::text::Ink;
use crate::text::font;

/// Page images and the reading viewport are this many pixels square.
pub const PAGE_PX: u32 = 1600;
/// Lighter grain than print: noise is expensive in JPEG bytes, and KDP charges per megabyte.
const GRAIN: f32 = 0.02;
const QUALITY: u8 = 80;

/// Writes the ePub to `path`. `cover` is the front cover image (JPEG).
pub fn write(story: &Story, pages: &[BookPage], cover: &[u8], path: &Path) -> Result<()> {
	let images: Vec<Vec<u8>> = pages
		.par_iter()
		.map(|page| {
			let mut pixmap =
				raster::render(&page_svg(page, Side::Screen, PAGE_PX, PAGE_PX, false))?;
			raster::finish(&mut pixmap, &page.style, GRAIN, 0);
			raster::jpeg(&pixmap, QUALITY)
		})
		.collect::<Result<_>>()?;

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.with_context(|| format!("could not create {}", parent.display()))?;
	}

	let file = std::fs::File::create(path)
		.with_context(|| format!("could not create {}", path.display()))?;
	let mut zip = ZipWriter::new(file);
	let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
	let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

	// The OCF spec requires `mimetype` to be the first entry, uncompressed.
	zip.start_file("mimetype", stored)?;
	zip.write_all(b"application/epub+zip")?;

	zip.start_file("META-INF/container.xml", deflated)?;
	zip.write_all(CONTAINER.as_bytes())?;

	zip.start_file("OEBPS/css/book.css", deflated)?;
	zip.write_all(css().as_bytes())?;

	for (name, data) in FONTS {
		zip.start_file(format!("OEBPS/fonts/{name}"), deflated)?;
		zip.write_all(data)?;
	}

	zip.start_file("OEBPS/images/cover.jpg", stored)?;
	zip.write_all(cover)?;

	zip.start_file("OEBPS/pages/cover.xhtml", deflated)?;
	zip.write_all(cover_page(story).as_bytes())?;

	for (index, (page, image)) in pages.iter().zip(&images).enumerate() {
		zip.start_file(
			format!("OEBPS/images/{}.jpg", file_stem(index, page)),
			stored,
		)?;
		zip.write_all(image)?;
		zip.start_file(
			format!("OEBPS/pages/{}.xhtml", file_stem(index, page)),
			deflated,
		)?;
		zip.write_all(page_xhtml(story, index, page).as_bytes())?;
	}

	zip.start_file("OEBPS/nav.xhtml", deflated)?;
	zip.write_all(nav(story, pages).as_bytes())?;

	zip.start_file("OEBPS/content.opf", deflated)?;
	zip.write_all(package(story, pages).as_bytes())?;

	zip.finish().context("could not finish the ePub archive")?;

	Ok(())
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const FONTS: [(&str, &[u8]); 3] = [
	("Andika-Regular.ttf", raster::BODY_FONT),
	("Andika-Bold.ttf", raster::BODY_BOLD_FONT),
	("LilitaOne-Regular.ttf", raster::DISPLAY_FONT),
];

fn file_stem(index: usize, page: &BookPage) -> String {
	format!("p{:02}-{}", index + 1, page.label)
}

fn css() -> String {
	let ink = Ink::default();

	format!(
		r#"@font-face {{ font-family: "Andika"; font-weight: 400; src: url("../fonts/Andika-Regular.ttf"); }}
@font-face {{ font-family: "Andika"; font-weight: 700; src: url("../fonts/Andika-Bold.ttf"); }}
@font-face {{ font-family: "Lilita One"; font-weight: 400; src: url("../fonts/LilitaOne-Regular.ttf"); }}
html, body {{ margin: 0; padding: 0; width: {PAGE_PX}px; height: {PAGE_PX}px; overflow: hidden; }}
.page {{ position: relative; width: {PAGE_PX}px; height: {PAGE_PX}px; }}
.art {{ position: absolute; left: 0; top: 0; width: {PAGE_PX}px; height: {PAGE_PX}px; }}
.t {{ position: absolute; margin: 0; padding: 0; white-space: pre; line-height: 1; color: {}; font-family: "Andika", sans-serif; }}
.b {{ font-weight: 700; }}
.s {{ font-family: "Lilita One", sans-serif; color: {}; }}
"#,
		ink.body, ink.shout
	)
}

/// Converts stage units to page pixels.
fn px(units: f32) -> f32 {
	units * PAGE_PX as f32 / STAGE
}

/// Where the baseline sits below the top of a `line-height: 1` box, as a fraction of the size.
///
/// CSS centres the font's ascent+descent in the line box, so the baseline lands at
/// `(1 + ascent - descent) / 2` em from the top.
fn baseline_ratio(style: RunStyle) -> f32 {
	let data = match style {
		RunStyle::Plain => raster::BODY_FONT,
		RunStyle::Bold => raster::BODY_BOLD_FONT,
		RunStyle::Shout => raster::DISPLAY_FONT,
	};

	let Ok(face) = ttf_parser::Face::parse(data, 0) else {
		return 0.8;
	};

	let per_em = f32::from(face.units_per_em());
	let ascent = f32::from(face.ascender()) / per_em;
	let descent = f32::from(face.descender()).abs() / per_em;

	(1.0 + ascent - descent) / 2.0
}

fn page_xhtml(story: &Story, index: usize, page: &BookPage) -> String {
	let mut text = String::new();

	for spec in &page.texts {
		for piece in &spec.block.pieces {
			let (_, _, color) = font(piece.style, spec.ink);
			let class = match piece.style {
				RunStyle::Plain => "t",
				RunStyle::Bold => "t b",
				RunStyle::Shout => "t s",
			};
			let top = piece.baseline - piece.size * baseline_ratio(piece.style);
			let _ = write!(
				text,
				r#"<span class="{class}" style="left:{:.1}px;top:{:.1}px;font-size:{:.1}px;color:{color}">{}</span>"#,
				px(piece.x),
				px(top),
				px(piece.size),
				escape(&piece.text)
			);
		}
	}

	let words = page_words(page);
	let title = if words.is_empty() {
		format!("{} — page {}", story.meta.title, index + 1)
	} else {
		words.clone()
	};

	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width={PAGE_PX}, height={PAGE_PX}"/>
<title>{title}</title>
<link rel="stylesheet" type="text/css" href="../css/book.css"/>
</head>
<body>
<div class="page" id="page-{n}" epub:type="pagebreak" role="doc-pagebreak" aria-label="{n}">
<img class="art" src="../images/{stem}.jpg" alt="{alt}"/>
{text}
</div>
</body>
</html>
"#,
		lang = escape(&story.meta.language),
		title = escape(&title),
		n = index + 1,
		stem = file_stem(index, page),
		alt = escape(&page.alt),
	)
}

fn cover_page(story: &Story) -> String {
	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width={PAGE_PX}, height={PAGE_PX}"/>
<title>{title}</title>
<link rel="stylesheet" type="text/css" href="../css/book.css"/>
</head>
<body epub:type="cover">
<div class="page"><img class="art" src="../images/cover.jpg" alt="Cover of {title} by {author}"/></div>
</body>
</html>
"#,
		lang = escape(&story.meta.language),
		title = escape(&story.meta.title),
		author = escape(&story.meta.author),
	)
}

fn nav(story: &Story, pages: &[BookPage]) -> String {
	let first_story = pages
		.iter()
		.position(|page| page.label.starts_with("story-"))
		.unwrap_or(0);
	let mut page_list = String::new();

	for (index, page) in pages.iter().enumerate() {
		let _ = writeln!(
			page_list,
			r#"<li><a href="pages/{}.xhtml">{}</a></li>"#,
			file_stem(index, page),
			index + 1
		);
	}

	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head><meta charset="UTF-8"/><title>{title}</title></head>
<body>
<nav epub:type="toc" id="toc"><h1>Contents</h1><ol>
<li><a href="pages/cover.xhtml">Cover</a></li>
<li><a href="pages/{title_page}.xhtml">Title page</a></li>
<li><a href="pages/{story_page}.xhtml">{title}</a></li>
</ol></nav>
<nav epub:type="landmarks" id="landmarks" hidden="hidden"><ol>
<li><a epub:type="cover" href="pages/cover.xhtml">Cover</a></li>
<li><a epub:type="bodymatter" href="pages/{story_page}.xhtml">Start of story</a></li>
</ol></nav>
<nav epub:type="page-list" id="page-list" hidden="hidden"><ol>
{page_list}</ol></nav>
</body>
</html>
"#,
		lang = escape(&story.meta.language),
		title = escape(&story.meta.title),
		title_page = file_stem(0, &pages[0]),
		story_page = file_stem(first_story, &pages[first_story]),
	)
}

fn package(story: &Story, pages: &[BookPage]) -> String {
	let meta = &story.meta;
	let identifier = match &meta.isbn {
		Some(isbn) => format!("urn:isbn:{}", isbn.replace('-', "")),
		None => {
			format!(
				"urn:uuid:{}",
				stable_uuid(&format!("{}|{}", meta.title, meta.author))
			)
		}
	};
	let mut manifest = String::new();
	let mut spine = String::new();

	for (name, _) in FONTS {
		let _ = writeln!(
			manifest,
			r#"<item id="font-{}" href="fonts/{name}" media-type="font/ttf"/>"#,
			name.trim_end_matches(".ttf")
		);
	}

	let _ = writeln!(
		manifest,
		r#"<item id="cover-image" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>"#
	);
	let _ = writeln!(
		manifest,
		r#"<item id="cover" href="pages/cover.xhtml" media-type="application/xhtml+xml"/>"#
	);
	let _ = writeln!(spine, r#"<itemref idref="cover" linear="yes"/>"#);

	for (index, page) in pages.iter().enumerate() {
		let stem = file_stem(index, page);
		let _ = writeln!(
			manifest,
			r#"<item id="img-{stem}" href="images/{stem}.jpg" media-type="image/jpeg"/>"#
		);
		let _ = writeln!(
			manifest,
			r#"<item id="{stem}" href="pages/{stem}.xhtml" media-type="application/xhtml+xml"/>"#
		);
		let _ = writeln!(spine, r#"<itemref idref="{stem}"/>"#);
	}

	let description = meta
		.blurb
		.as_deref()
		.map(|blurb| format!("<dc:description>{}</dc:description>\n", escape(blurb)))
		.unwrap_or_default();
	let publisher = meta
		.publisher
		.as_deref()
		.map(|publisher| format!("<dc:publisher>{}</dc:publisher>\n", escape(publisher)))
		.unwrap_or_default();
	let series = meta
		.series
		.as_deref()
		.map(|series| {
			let position = meta.number.map(|n| format!(r##"<meta refines="#series" property="group-position">{n}</meta>"##)).unwrap_or_default();
			format!(
				r##"<meta property="belongs-to-collection" id="series">{}</meta><meta refines="#series" property="collection-type">series</meta>{position}
"##,
				escape(series)
			)
		})
		.unwrap_or_default();

	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="{lang}" prefix="rendition: http://www.idpf.org/vocab/rendition/# ibooks: http://vocabulary.itunes.apple.com/rdf/ibooks/vocabulary-extensions-1.0/">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="book-id">{identifier}</dc:identifier>
<dc:title>{title}</dc:title>
<dc:creator>{author}</dc:creator>
<dc:language>{lang}</dc:language>
{publisher}{description}{series}<meta property="dcterms:modified">{modified}</meta>
<meta property="rendition:layout">pre-paginated</meta>
<meta property="rendition:spread">none</meta>
<meta property="rendition:orientation">auto</meta>
<meta property="ibooks:specified-fonts">true</meta>
<meta name="original-resolution" content="{PAGE_PX}x{PAGE_PX}"/>
<meta name="fixed-layout" content="true"/>
<meta name="book-type" content="children"/>
<meta name="cover" content="cover-image"/>
<meta property="schema:accessMode">textual</meta>
<meta property="schema:accessMode">visual</meta>
<meta property="schema:accessModeSufficient">textual</meta>
<meta property="schema:accessibilityFeature">alternativeText</meta>
<meta property="schema:accessibilityFeature">readingOrder</meta>
<meta property="schema:accessibilityFeature">pageNavigation</meta>
<meta property="schema:accessibilityHazard">none</meta>
<meta property="schema:accessibilitySummary">A fixed-layout picture book. Every page's story text is real text, and every illustration has a description.</meta>
</metadata>
<manifest>
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="css" href="css/book.css" media-type="text/css"/>
{manifest}</manifest>
<spine>
{spine}</spine>
</package>
"#,
		lang = escape(&meta.language),
		title = escape(&meta.title),
		author = escape(&meta.author),
		modified = utc_now(),
	)
}

/// A UUID (version 5 style layout) derived from a name, so rebuilding a book keeps its identity.
fn stable_uuid(name: &str) -> String {
	let a = crate::art::seed_of(name);
	let b = crate::art::seed_of(&format!("{name}#fables"));
	let hex = format!("{a:016x}{b:016x}");

	format!(
		"{}-{}-5{}-{}{}-{}",
		&hex[0..8],
		&hex[8..12],
		&hex[13..16],
		["8", "9", "a", "b"][(b % 4) as usize],
		&hex[17..20],
		&hex[20..32]
	)
}

/// The current UTC time as `CCYY-MM-DDThh:mm:ssZ` (required by `dcterms:modified`).
fn utc_now() -> String {
	let seconds = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_secs());
	format_utc(seconds)
}

/// Converts Unix seconds to an ISO 8601 UTC timestamp (Howard Hinnant's civil-from-days).
fn format_utc(seconds: u64) -> String {
	let days = (seconds / 86_400) as i64;
	let rem = seconds % 86_400;
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z - era * 146_097;
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let day = doy - (153 * mp + 2) / 5 + 1;
	let month = if mp < 10 { mp + 3 } else { mp - 9 };
	let year = yoe + era * 400 + i64::from(month <= 2);

	format!(
		"{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
		rem / 3600,
		rem % 3600 / 60,
		rem % 60
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn formats_utc_timestamps() {
		assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
		assert_eq!(format_utc(1_792_154_096), "2026-10-16T12:34:56Z");
		assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
	}

	#[test]
	fn uuids_are_stable_and_well_formed() {
		let a = stable_uuid("Stinky");
		assert_eq!(a, stable_uuid("Stinky"));
		assert_ne!(a, stable_uuid("Minty"));
		let parts: Vec<usize> = a.split('-').map(str::len).collect();
		assert_eq!(parts, vec![8, 4, 4, 4, 12]);
		assert_eq!(&a[14..15], "5");
	}
}
