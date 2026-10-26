//! End-to-end checks on the example book, so the script, the cast and every output format keep
//! working together. Narration is replaced by silence of a plausible length so the tests need
//! neither a TTS engine nor ffmpeg.

use std::io::Read as _;
use std::path::Path;

use fables::book;
use fables::cast::Cast;
use fables::press;
use fables::stage;
use fables::story::Story;
use fables::video;
use fables::video::frames::Format;
use fables::video::frames::Renderer;
use fables::video::voice::SAMPLE_RATE;
use fables::video::voice::Speech;
use fables::video::voice::Utterance;
use fables::world::World;

fn example() -> (Story, Cast) {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut story =
		Story::load(&root.join("worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md")).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story);
	let cast = world.cast().unwrap();

	(story, cast)
}

fn silent_timeline(story: &Story, cast: &Cast) -> video::timeline::Timeline {
	let rehearsals = stage::rehearse(story, cast).unwrap();

	video::timeline::build(story, cast, &rehearsals, |line: &Utterance<'_>| {
		let seconds = line.text.split_whitespace().count() as f32 * 0.35 + 0.3;
		let samples = (seconds * SAMPLE_RATE as f32) as usize;
		let envelope = vec![0.5; (seconds * 60.0) as usize];

		Ok(Speech {
			text: line.text.to_owned(),
			samples: vec![0.0; samples],
			track: video::lipsync::from_envelope(line.text, &envelope, 60.0),
			envelope,
		})
	})
	.unwrap()
}

#[test]
fn the_example_book_assembles_for_kdp() {
	let (story, cast) = example();
	let pages = book::assemble(&story, &cast).unwrap();

	assert!(pages.len() >= book::MIN_PAGES);
	assert_eq!(pages.len() % 2, 0, "KDP needs an even page count");
	assert!(
		pages
			.iter()
			.any(|page| book::page_words(page).contains("Minty McFreshbreath"))
	);
}

#[test]
fn the_ebook_is_a_valid_container() {
	let (story, cast) = example();
	let pages = book::assemble(&story, &cast).unwrap();
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join("book.epub");
	let cover = fables::raster::jpeg(&resvg::tiny_skia::Pixmap::new(16, 16).unwrap(), 80).unwrap();
	press::epub::write(&story, &pages, &cover, &path).unwrap();

	let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
	let first = archive.by_index(0).unwrap();
	assert_eq!(
		first.name().unwrap(),
		"mimetype",
		"mimetype must come first"
	);
	assert_eq!(first.compression(), zip::CompressionMethod::Stored);
	drop(first);

	let mut opf = String::new();
	archive
		.by_name("OEBPS/content.opf")
		.unwrap()
		.read_to_string(&mut opf)
		.unwrap();
	assert!(opf.contains(r#"<meta property="rendition:layout">pre-paginated</meta>"#));
	assert!(opf.contains(r#"<meta name="original-resolution""#));
	assert!(opf.contains(r#"<meta name="book-type" content="children"/>"#));

	let mut page = String::new();
	archive
		.by_name("OEBPS/pages/p06-story-04.xhtml")
		.unwrap()
		.read_to_string(&mut page)
		.unwrap();
	assert!(page.contains(r#"<meta name="viewport""#));
	assert!(
		page.contains("NOPE"),
		"story text must be live text, not part of the image"
	);
	assert!(page.contains(r#"alt="In the bathroom"#), "{page}");
}

#[test]
fn print_files_have_kdp_page_sizes() {
	let (story, cast) = example();
	let pages = book::assemble(&story, &cast).unwrap();
	let dir = tempfile::tempdir().unwrap();
	let interior = dir.path().join("interior.pdf");
	let cover = dir.path().join("cover.pdf");
	press::print::write_interior(&story, &pages[..2], &interior).unwrap();
	let spec = press::print::Spec::for_pages(pages.len());
	press::print::write_cover(
		&story,
		&book::front_cover(&story, &cast).unwrap(),
		&book::back_cover(&story, &cast).unwrap(),
		spec,
		&cover,
	)
	.unwrap();

	let interior = std::fs::read(&interior).unwrap();
	assert!(interior.starts_with(b"%PDF"));
	let text = String::from_utf8_lossy(&interior);
	assert!(
		text.contains("/MediaBox [0 0 621 630]"),
		"8.625in x 8.75in in points"
	);
	assert!(
		text.contains("/TrimBox [0 9 612 621]"),
		"right-hand pages are trimmed on the right"
	);

	let cover = std::fs::read(&cover).unwrap();
	assert!(String::from_utf8_lossy(&cover).contains("/MediaBox [0 0 1246."));
}

#[test]
fn the_video_fits_a_short_and_every_shot_renders() {
	let (story, cast) = example();
	let timeline = silent_timeline(&story, &cast);
	assert!(timeline.shots.len() > 20);

	for format in [Format::Landscape, Format::Portrait] {
		let (width, height) = format.size(true);
		let renderer = Renderer {
			timeline: &timeline,
			story: &story,
			format,
			width: width / 4,
			height: height / 4,
		};

		for shot in &timeline.shots {
			let middle = shot.start + shot.duration() / 2.0;
			let pixmap = renderer.render(middle, 0).unwrap();
			assert_eq!((pixmap.width(), pixmap.height()), (width / 4, height / 4));
		}
	}

	let srt = video::srt(&timeline);
	assert!(srt.starts_with("1\n00:00:"));
	assert!(srt.contains("ALREADY DONE!"));
}

#[test]
fn the_soundtrack_is_written() {
	let (story, cast) = example();
	let timeline = silent_timeline(&story, &cast);
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join("soundtrack.wav");
	video::audio::write(&timeline, &path).unwrap();

	let reader = hound::WavReader::open(&path).unwrap();
	assert_eq!(reader.spec().channels, 2);
	assert_eq!(reader.spec().sample_rate, 48_000);
	let seconds = reader.duration() as f32 / 48_000.0;
	assert!(
		(seconds - timeline.duration).abs() < 1.0,
		"{seconds} vs {}",
		timeline.duration
	);
}
