//! The directing tools over the shipped crossover story: the guest carries her own world's
//! style into the host book, the story checks, and the review has nothing to fix.

use std::path::Path;

use fables::cast::Kind;
use fables::publish;
use fables::review::Severity;
use fables::story::Story;
use fables::world::World;

#[test]
fn the_visitor_crosses_worlds_and_reviews_clean() {
	let script =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/mumbleton/stories/the-visitor/story.md");
	let mut story = Story::load(&script).unwrap();
	let world = World::for_story(&story, None).unwrap();
	world.furnish(&mut story).unwrap();
	let cast = world.cast_for(&story).unwrap();

	// The guest keeps her own world's construction and inks beside the host's cast.
	let Kind::Figure(look) = cast.resolve("hana").unwrap() else {
		panic!("hana is a person");
	};
	let puppet = look.puppet.as_ref().expect("hana has a puppet");
	assert_eq!(puppet.style.name, "anime", "her construction is her own");
	assert_eq!(
		puppet.inker().name,
		"anime",
		"and so are her line and shading"
	);

	let Kind::Figure(look) = cast.resolve("mo").unwrap() else {
		panic!("mo is a person");
	};
	assert_eq!(
		look.puppet.as_ref().expect("mo has a puppet").style.name,
		"cartoon"
	);

	publish::check(&story, &cast).unwrap();

	let out = tempfile::tempdir().unwrap();
	let findings = publish::review(&story, &cast, out.path()).unwrap();
	let fixes: Vec<&fables::review::Finding> = findings
		.iter()
		.filter(|finding| finding.severity == Severity::Fix)
		.collect();

	assert!(fixes.is_empty(), "nothing to fix: {fixes:#?}");
}
