//! End-to-end: the pages a heading frames with `shot=` keep their subjects' heads inside the
//! part of the page the text card leaves clear — measured on the book's own layout, card and
//! all, the way [`fables::book`] draws it.

use std::path::Path;

use fables::art::Bounds;
use fables::art::STAGE;
use fables::book;
use fables::cast::Cast;
use fables::compose;
use fables::story::Story;
use fables::video::director;
use fables::world::World;

fn the_visitor() -> (Story, Cast) {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut story =
		Story::load(&root.join("worlds/mumbleton/stories/the-visitor/story.md")).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast_for(&story).unwrap();

	(story, cast)
}

/// Whether the boxes share no area: `a` lies wholly to one side of `b`.
fn apart(a: Bounds, b: Bounds) -> bool {
	a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1
}

/// The two pages the review saw crop — the close on Hana and the two-shot under the moon —
/// keep every subject's measured head bounds out of the text card and on the page.
#[test]
fn framed_pages_keep_heads_out_of_the_text_card() {
	let (story, cast) = the_visitor();
	let pages = book::assemble(&story, &cast).unwrap();

	for label in ["story-03", "story-08"] {
		let page = pages
			.iter()
			.find(|page| page.label == label)
			.unwrap_or_else(|| panic!("{label} is in the book"));
		let size = page
			.scene
			.camera
			.shot
			.unwrap_or_else(|| panic!("{label}'s heading names a shot"));
		let card = book::card_rects(page)
			.into_iter()
			.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
			.unwrap_or_else(|| panic!("{label} lays its text on a card"));
		let puppets = compose::puppets(&page.scene, 0.0, true);
		let view = director::still_view(
			&page.scene,
			&puppets,
			size,
			&page.scene.camera.on,
			Some(card),
		)
		.unwrap_or_else(|| panic!("{label}'s shot finds its subjects"));

		// The page draws the view onto the page one to one: stage units become page units
		// through the same transform `draw_art` applies.
		let scale = STAGE / view.height;
		let to_page = |x: f32, y: f32| {
			(
				STAGE / 2.0 + (x - view.centre.x) * scale,
				STAGE / 2.0 + (y - view.centre.y) * scale,
			)
		};

		for puppet in puppets
			.iter()
			.filter(|puppet| page.scene.camera.on.contains(&puppet.id))
		{
			let (hl, ht, hr, hb) = puppet.extents().head;
			let (left, top) = to_page(hl, ht);
			let (right, bottom) = to_page(hr, hb);
			let head = (left, top, right, bottom);
			let what = format!("{label}: {}'s head", puppet.id);

			assert!(
				apart(head, card),
				"{what} at {head:?} runs under the text card at {card:?}"
			);
			assert!(
				left >= 0.0 && top >= 0.0 && right <= STAGE && bottom <= STAGE,
				"{what} at {head:?} leaves the page"
			);
		}
	}
}
