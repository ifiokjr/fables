//! A world is just a folder: one with no cast, no puppets and no narrator still makes a book,
//! so a new series starts from nothing but a `world.toml`.

use fables::book;
use fables::book::Side;
use fables::raster;
use fables::story::Story;
use fables::world::World;

#[test]
fn an_empty_world_renders_a_blank_page() {
	let dir = tempfile::tempdir().unwrap();
	let world = dir.path().join("worlds/empty");
	let script = world.join("stories/blank/story.md");
	std::fs::create_dir_all(script.parent().unwrap()).unwrap();
	std::fs::write(world.join("world.toml"), "name = \"Empty\"\n").unwrap();
	std::fs::write(world.join("cast.toml"), "").unwrap();
	std::fs::write(
		&script,
		"+++\ntitle = \"Blank\"\nauthor = \"Nobody\"\n+++\n# plain\nNothing happens.\n",
	)
	.unwrap();

	let found = World::find(dir.path()).unwrap();
	assert_eq!(found.name, "Empty");

	let mut story = Story::load(&script).unwrap();
	let world = World::for_story(&story, None).unwrap();
	assert_eq!(world.dir, found.dir);
	world.furnish(&mut story);
	assert!(story.meta.voice.is_none(), "an empty world has no narrator");

	let cast = world.cast().unwrap();
	let pages = book::assemble(&story, &cast).unwrap();
	let page = pages
		.iter()
		.find(|page| book::page_words(page).contains("Nothing happens."))
		.expect("the story page is in the book");
	let pixmap = raster::render(&book::page_svg(page, Side::Screen, 256, 256, true)).unwrap();
	assert_eq!((pixmap.width(), pixmap.height()), (256, 256));

	for page in &pages {
		let words = book::page_words(page);
		assert!(
			!words.contains("Kerfuffle") && !words.contains("Wormsley"),
			"{} borrows another world's words: {words}",
			page.label
		);
	}
}
