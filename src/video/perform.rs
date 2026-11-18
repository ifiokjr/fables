//! The performance pass: what a plain script means, worked out once over a page's scene keys.
//!
//! [`acting`](super::acting) decides how a figure moves between two keys. This pass comes
//! before it and decides what the keys should say, so a first-time writer gets a performance
//! from a script that only names moods and places:
//!
//! - **A run goes somewhere.** `run` or `walk` with no `x` of its own runs to the `x` the
//!   actor is given next on the page, instead of jogging on the spot until then.
//! - **A reaction waits for its cause.** A strong mood directed before the thing it reacts to
//!   has arrived (an entrance later on the page) is held back: the actor stays mildly
//!   concerned, looks towards the arrival when it starts, and takes when it is on stage.
//! - **A reaction looks at its cause.** A strong mood that lands as somebody arrives looks at
//!   them first, and takes once they are on stage.
//! - **Talking to each other.** Two figures alone on a page where one of them speaks turn
//!   towards each other, unless the script faces them itself.
//! - **A pose that fits the mood.** A figure with no pose directed takes its mood's
//!   ([`mood_pose`]).
//!
//! All of it is video-only: a book's page is drawn from the script's own scene and never from
//! these keys. `[video] acting = "plain"` skips the pass.

use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::rig::Gait;
use crate::art::rig::Stance;
use crate::stage::Actor;
use crate::stage::See;
use crate::stage::Side;
use crate::video::timeline::Key;

/// Two stage positions closer than this are the same place.
const SAME_PLACE: f32 = 0.02;
/// How long after an entrance starts the newcomer is far enough on stage to react to.
const ARRIVAL: f32 = 0.9;
/// Two moments closer than this are one beat.
const BEAT: f32 = 0.3;
/// How far two talkers turn towards each other (1 is a full profile).
const TOWARDS: f32 = 0.85;

/// The pose a mood stands in when the script directs none. `None` keeps the relaxed `idle`.
/// Neighbouring feelings never share a pose, so a page of mixed moods never lines up alike.
pub fn mood_pose(mood: Mood) -> Option<Pose> {
	match mood {
		Mood::Sad | Mood::Sleepy => Some(Pose::Droop),
		Mood::Grumpy | Mood::Defiant => Some(Pose::ArmsCrossed),
		Mood::Cross | Mood::Proud => Some(Pose::Hips),
		Mood::Shocked => Some(Pose::Shrug),
		Mood::Worried => Some(Pose::Think),
		Mood::Laugh => Some(Pose::Cheer),
		Mood::Happy | Mood::Grin | Mood::Disgusted | Mood::Fainted | Mood::Dizzy | Mood::Dreamy => {
			None
		}
	}
}

/// The moods that play as a take, and so want their cause on stage.
fn takes(mood: Mood) -> bool {
	matches!(mood, Mood::Shocked | Mood::Disgusted | Mood::Laugh)
}

/// The mild version of a strong mood, held while its cause has not arrived yet.
fn mild(mood: Mood) -> Mood {
	match mood {
		Mood::Laugh => Mood::Happy,
		Mood::Disgusted => Mood::Grumpy,
		_ => Mood::Worried,
	}
}

fn actor<'k>(key: &'k Key, id: &str) -> Option<&'k Actor> {
	key.scene
		.actors
		.iter()
		.find(|actor| actor.id == id && !actor.hidden)
}

fn actor_mut<'k>(key: &'k mut Key, id: &str) -> Option<&'k mut Actor> {
	key.scene
		.actors
		.iter_mut()
		.find(|actor| actor.id == id && !actor.hidden)
}

/// Every figure that is on stage at some point on the page.
fn figures(keys: &[Key]) -> Vec<String> {
	let mut ids: Vec<String> = Vec::new();

	for key in keys {
		for actor in &key.scene.actors {
			if actor.kind.is_figure() && !actor.hidden && !ids.contains(&actor.id) {
				ids.push(actor.id.clone());
			}
		}
	}

	ids
}

/// A copy of the state in force at `time`, as a key of its own at `time`: the same stage,
/// with no camera move of its own. Returns its index.
fn split(keys: &mut Vec<Key>, time: f32) -> usize {
	let index = keys.iter().rposition(|key| key.time <= time).unwrap_or(0);

	if (keys[index].time - time).abs() < 1e-3 {
		return index;
	}

	let mut key = keys[index].clone();
	key.time = time;
	key.after = None;
	key.scene.camera.travel = None;
	key.scene.camera.over = 0.0;
	key.scene.camera.shake = 0.0;

	for actor in &mut key.scene.actors {
		actor.eyes.notice = false;
	}

	keys.insert(index + 1, key);
	index + 1
}

/// Runs the whole pass over one page's keys. `first_line` is when the page's first words
/// start, or the page's end when it has none.
pub fn perform(keys: &mut Vec<Key>, first_line: f32) {
	if keys.is_empty() {
		return;
	}

	let ids = figures(keys);

	for id in &ids {
		run_somewhere(keys, id, first_line);
	}

	for id in &ids {
		wait_for_cause(keys, id);
	}

	face_each_other(keys, &ids);

	for key in keys.iter_mut() {
		for actor in &mut key.scene.actors {
			pose_for_mood(actor);
		}
	}
}

/// A `run` or `walk` with no destination runs to the `x` the actor is given next, and a gait
/// that has carried its actor somewhere is spent: the actor stands when it gets there.
fn run_somewhere(keys: &mut Vec<Key>, id: &str, first_line: f32) {
	// A run directed before the page's first words has no key to run in: it sets off as the
	// words start, from where the page opened.
	if let Some(opening) = actor(&keys[0], id)
		&& opening.gait.is_some()
		&& let Some(from) = opening.x
		&& destination(keys, 0, id, from).is_some()
	{
		let limit = keys.get(1).map_or(f32::INFINITY, |key| key.time);

		if first_line > keys[0].time + 1e-3 && first_line < limit - 1e-3 {
			split(keys, first_line);

			if let Some(opening) = actor_mut(&mut keys[0], id) {
				opening.gait = None;
			}
		}
	}

	let directed: Vec<Option<Gait>> = keys
		.iter()
		.map(|key| actor(key, id).and_then(|actor| actor.gait))
		.collect();
	let mut spent = false;
	let mut index = 0;

	while index < keys.len() {
		let gait = actor(&keys[index], id).and_then(|actor| actor.gait);
		let Some(here) = actor(&keys[index], id).and_then(|actor| actor.x) else {
			index += 1;
			continue;
		};

		if gait.is_none() {
			spent = false;
			index += 1;
			continue;
		}

		let before = index.checked_sub(1).and_then(|i| actor(&keys[i], id));

		if index > 0 && directed[index - 1] != directed[index] {
			spent = false;
		}

		// It travels on this key: an entrance, or an `x` of its own.
		let travels = index > 0
			&& before.is_none_or(|before| before.x.is_none_or(|x| (x - here).abs() > SAME_PLACE));

		if travels {
			spent = true;
		} else if spent {
			if let Some(actor) = actor_mut(&mut keys[index], id) {
				actor.gait = None;
			}
		} else if index > 0
			&& let Some((arrives, x)) = destination(keys, index, id, here)
		{
			for key in &mut keys[index..arrives] {
				if let Some(actor) = actor_mut(key, id) {
					actor.x = Some(x);
				}
			}

			spent = true;
		}

		index += 1;
	}
}

/// The next place on the page the actor is put after key `from`, and the key that puts it
/// there, while it keeps its gait.
fn destination(keys: &[Key], from: usize, id: &str, here: f32) -> Option<(usize, f32)> {
	let gait = actor(&keys[from], id)?.gait;

	for (index, key) in keys.iter().enumerate().skip(from + 1) {
		let later = actor(key, id)?;

		if let Some(x) = later.x
			&& (x - here).abs() > SAME_PLACE
		{
			return Some((index, x));
		}

		if later.gait != gait {
			return None;
		}
	}

	None
}

/// Somebody arriving on key `index`: the first actor on stage there who was not on the key
/// before, and the side they come from.
fn arrival(keys: &[Key], index: usize, not: &str) -> Option<(String, Option<Side>)> {
	let before = &keys[index.checked_sub(1)?];

	keys[index]
		.scene
		.actors
		.iter()
		.filter(|actor| !actor.hidden && actor.id != not)
		.find(|actor| self::actor(before, &actor.id).is_none())
		.map(|actor| (actor.id.clone(), actor.enter))
}

/// Looks at whoever is arriving, eyes first, unless the script already holds the look.
fn look_at(actor: &mut Actor, cause: &str, side: Option<Side>) {
	if actor.eyes.see.is_some() {
		return;
	}

	actor.eyes.see = Some(match side {
		Some(Side::Left) => See::Left,
		Some(Side::Right) => See::Right,
		None => See::Actor(cause.to_owned()),
	});
	actor.eyes.notice = true;
}

/// A strong mood waits for its cause, and looks at it.
fn wait_for_cause(keys: &mut Vec<Key>, id: &str) {
	let mut index = 0;

	while index < keys.len() {
		let Some(mood) = actor(&keys[index], id).map(|actor| actor.mood) else {
			index += 1;
			continue;
		};
		let fresh =
			index == 0 || actor(&keys[index - 1], id).is_none_or(|before| before.mood != mood);

		if !takes(mood) || !fresh {
			index += 1;
			continue;
		}

		// The cause arrives on this key, or later on the page. Until it is on stage the actor
		// keeps the mood it had, or a mild version of the one directed.
		let here = arrival(keys, index, id).map(|(cause, side)| (index, cause, side));
		let later = || {
			(index + 1..keys.len()).find_map(|later| {
				let still = actor(&keys[later], id).is_some_and(|actor| actor.mood == mood);

				arrival(keys, later, id)
					.filter(|_| still)
					.map(|(cause, side)| (later, cause, side))
			})
		};
		let Some((arrives, cause, side)) = here.or_else(later) else {
			index += 1;
			continue;
		};
		let waiting = if arrives == index {
			actor(&keys[index - 1], id).map_or(mild(mood), |before| before.mood)
		} else {
			mild(mood)
		};
		let at = keys[arrives].time + ARRIVAL;
		// The take lands on a direction of its own, or shares one that is a beat away.
		let take = keys
			.iter()
			.position(|key| (key.time - at).abs() < BEAT && key.time > keys[arrives].time)
			.unwrap_or_else(|| split(keys, at));

		for key in &mut keys[index..take] {
			if let Some(actor) = actor_mut(key, id)
				&& actor.mood == mood
			{
				actor.mood = waiting;
			}
		}

		if let Some(actor) = actor_mut(&mut keys[arrives], id) {
			look_at(actor, &cause, side);
		}

		// The look holds through the take, eyes already there.
		let see = actor(&keys[arrives], id).and_then(|actor| actor.eyes.see.clone());

		if let Some(actor) = actor_mut(&mut keys[take], id)
			&& actor.eyes.see.is_none()
		{
			actor.eyes.see = see;
		}

		index = take + 1;
	}
}

/// Two figures alone on a page where one of them speaks turn towards each other, on every key
/// where the script has not faced them itself.
fn face_each_other(keys: &mut [Key], ids: &[String]) {
	let [a, b] = ids else {
		return;
	};
	let talks = keys.iter().any(|key| {
		[a, b]
			.iter()
			.any(|id| actor(key, id).is_some_and(|actor| actor.talking))
	});

	if !talks {
		return;
	}

	for key in keys.iter_mut() {
		let (Some(xa), Some(xb)) = (
			actor(key, a).and_then(|actor| actor.x),
			actor(key, b).and_then(|actor| actor.x),
		) else {
			continue;
		};

		if (xa - xb).abs() < SAME_PLACE {
			continue;
		}

		for (id, towards) in [(a, (xb - xa).signum()), (b, (xa - xb).signum())] {
			if let Some(actor) = actor_mut(key, id)
				&& actor.facing.is_none()
				&& actor.knocked < 0.5
			{
				actor.facing = Some(towards * TOWARDS);
			}
		}
	}
}

/// A standing figure with no pose directed and nothing in its hands takes its mood's pose.
fn pose_for_mood(actor: &mut Actor) {
	if actor.kind.is_figure()
		&& actor.pose == Pose::Idle
		&& actor.stance == Stance::Stand
		&& actor.prop.is_none()
		&& actor.gait.is_none()
		&& actor.knocked < 0.5
		&& let Some(pose) = mood_pose(actor.mood)
	{
		actor.pose = pose;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cast::Cast;
	use crate::stage::rehearse;
	use crate::story;
	use crate::video::lipsync::Track;
	use crate::video::timeline::Shot;
	use crate::video::timeline::Timeline;
	use crate::video::voice::SAMPLE_RATE;
	use crate::video::voice::Speech;

	/// The timeline of a one-page script, half a second a word, and the still's own scenes.
	fn built(script: &str, front: &str) -> (Timeline, Vec<crate::stage::Rehearsal>) {
		let source =
			format!("+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n{front}\n+++\n{script}");
		let story = story::parse(&source).unwrap();
		let cast = Cast::parse(
			"[kid]\nname = \"Kid\"\n[mum]\nname = \"Mum\"\nkind = \"grown-up\"\n[rex]\nname = \
			 \"Rex\"\nkind = \"dog\"\n",
		)
		.unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let timeline = crate::video::timeline::build(&story, &cast, &rehearsals, |line| {
			let seconds = line.text.split_whitespace().count() as f32 * 0.5;

			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap();

		(timeline, rehearsals)
	}

	fn page(script: &str) -> Shot {
		built(script, "").0.shots[1].clone()
	}

	fn at<'s>(shot: &'s Shot, key: usize, id: &str) -> &'s Actor {
		actor(&shot.keys[key], id).unwrap_or_else(|| panic!("{id} is on key {key}"))
	}

	#[test]
	fn every_mood_has_a_pose_or_stands_at_ease_and_a_directed_pose_wins() {
		for (name, mood) in Mood::ALL {
			if let Some(pose) = mood_pose(mood) {
				assert_ne!(pose, Pose::Idle, "{name} names a pose of its own");
			}
		}

		assert_eq!(mood_pose(Mood::Sad), Some(Pose::Droop));
		assert_eq!(mood_pose(Mood::Proud), Some(Pose::Hips));
		assert_eq!(mood_pose(Mood::Happy), None, "happy stands at ease");

		let shot = page("# park\n@kid x=0.3 sad\n@mum x=0.7 sad wave\nOne two.\n");
		assert_eq!(at(&shot, 0, "kid").pose, Pose::Droop, "the mood's pose");
		assert_eq!(at(&shot, 0, "mum").pose, Pose::Wave, "a directed pose wins");

		let sitting = page("# park\n@kid x=0.3 sad sit\nOne two.\n");
		assert_eq!(
			at(&sitting, 0, "kid").pose,
			Pose::Idle,
			"a stance keeps its arms"
		);
	}

	#[test]
	fn the_pass_is_video_only_and_plain_acting_skips_it() {
		let script = "# park\n@kid x=0.3 sad talk\n@mum x=0.7\nOne two.\n";
		let (timeline, rehearsals) = built(script, "");
		let still = rehearsals[0]
			.finale()
			.actors
			.iter()
			.find(|actor| actor.id == "kid")
			.unwrap();

		assert_eq!(
			still.pose,
			Pose::Idle,
			"the book's page keeps the script's own pose"
		);
		assert_eq!(still.facing, None, "and its own facing");
		assert_eq!(at(&timeline.shots[1], 0, "kid").pose, Pose::Droop);

		let (plain, _) = built(script, "[video]\nacting = \"plain\"");
		assert_eq!(at(&plain.shots[1], 0, "kid").pose, Pose::Idle);
		assert_eq!(at(&plain.shots[1], 0, "kid").facing, None);
	}

	#[test]
	fn a_take_waits_for_a_cause_that_arrives_later_and_looks_its_way_first() {
		let shot = page(
			"# park\n@kid x=0.3 shocked\nOne two three four.\n@mum enter=right x=0.7\nFive six \
			 seven eight.\n",
		);

		assert_eq!(
			at(&shot, 0, "kid").mood,
			Mood::Worried,
			"mild until she comes"
		);

		let arrives = shot
			.keys
			.iter()
			.position(|key| actor(key, "mum").is_some())
			.unwrap();
		let kid = at(&shot, arrives, "kid");
		assert_eq!(kid.mood, Mood::Worried, "not yet: she is still walking on");
		assert!(matches!(kid.eyes.see, Some(See::Right)), "eyes to her side");
		assert!(kid.eyes.notice, "eyes first");

		let take = &shot.keys[arrives + 1];
		assert!(
			(take.time - shot.keys[arrives].time - ARRIVAL).abs() < 1e-3,
			"the take lands when she is on stage"
		);
		assert_eq!(at(&shot, arrives + 1, "kid").mood, Mood::Shocked);
	}

	#[test]
	fn a_take_whose_cause_never_arrives_plays_where_it_is_directed() {
		let shot = page("# park\n@kid x=0.3 shocked\n@mum x=0.7\nOne two three four.\n");

		assert_eq!(at(&shot, 0, "kid").mood, Mood::Shocked);
		assert_eq!(shot.keys.len(), 1, "no beat is added");
	}

	#[test]
	fn a_reaction_directed_as_somebody_arrives_looks_at_them_then_takes() {
		let shot = page(
			"# park\n@kid x=0.6\nOne two three.\n@mum enter=left x=0.2\n@kid laugh\nFour five six \
			 seven.\n",
		);
		let kid = at(&shot, 1, "kid");

		assert_eq!(kid.mood, Mood::Happy, "the mood it had, until she is on");
		assert!(matches!(kid.eyes.see, Some(See::Left)));
		assert_eq!(at(&shot, 2, "kid").mood, Mood::Laugh);
		assert!(matches!(at(&shot, 2, "kid").eyes.see, Some(See::Left)));
	}

	#[test]
	fn a_look_the_script_directs_is_kept() {
		let shot = page(
			"# park\n@kid x=0.6\nOne two three.\n@mum enter=left x=0.2\n@kid shocked see=up\nFour \
			 five six seven.\n",
		);

		assert!(matches!(at(&shot, 1, "kid").eyes.see, Some(See::Up)));
	}

	#[test]
	fn a_run_with_no_destination_runs_to_the_next_x() {
		let shot = page(
			"# park\n@kid x=0.2 run\nOne two three four five six.\n@kid x=0.7 proud\nSeven \
			 eight.\n",
		);

		// It sets off as the words start, from where the page opened.
		assert_eq!(
			at(&shot, 0, "kid").gait,
			None,
			"it stands until the words start"
		);
		assert_eq!(at(&shot, 0, "kid").x, Some(0.2));
		assert!((shot.keys[1].time - shot.lines[0].start).abs() < 1e-3);
		assert_eq!(at(&shot, 1, "kid").gait, Some(Gait::Run));
		assert_eq!(at(&shot, 1, "kid").x, Some(0.7), "the whole distance");

		// It is there when the next direction lands, and it has stopped running.
		assert_eq!(at(&shot, 2, "kid").x, Some(0.7));
		assert_eq!(
			at(&shot, 2, "kid").gait,
			None,
			"a run that arrived is spent"
		);
	}

	#[test]
	fn a_run_that_covers_the_ground_is_a_run_on_planted_feet() {
		use crate::compose;
		use crate::video::acting;

		let shot = page(
			"# park\n@kid x=0.2 run\nOne two three four five six.\n@kid x=0.7 proud\nSeven \
			 eight.\n",
		);
		let from = compose::puppets(&shot.keys[0].scene, 0.0, false);
		let to = compose::puppets(&shot.keys[1].scene, 0.0, false);
		let cx = acting::Cx {
			on: true,
			motion: crate::art::dials::Motion::default(),
			lines: Vec::new(),
			key_time: 0.0,
			hold: false,
		};
		let plan = acting::plan_between(&from[0], &to[0], &cx).expect("it travels on foot");

		assert_eq!(plan.gait, Gait::Run);
		assert!((plan.x1 - plan.x0 - 0.5 * crate::art::STAGE).abs() < 1.0);
		// Whole steps, so it starts and ends with both feet planted.
		let steps = plan.omega * plan.duration / std::f32::consts::PI;
		assert!((steps - steps.round()).abs() < 1e-3, "{steps} steps");
	}

	#[test]
	fn a_run_with_nowhere_to_go_stays_a_treadmill() {
		let shot = page("# park\n@kid x=0.2 run\nOne two three four.\n@kid proud\nFive.\n");

		assert_eq!(at(&shot, 0, "kid").gait, Some(Gait::Run));
		assert_eq!(at(&shot, 1, "kid").gait, Some(Gait::Run));
		assert_eq!(at(&shot, 1, "kid").x, Some(0.2));
	}

	#[test]
	fn two_talkers_face_each_other_and_a_lone_speaker_does_not_turn() {
		let shot = page("# park\n@kid x=0.3 talk\n@mum x=0.7\nHello there.\n");
		assert_eq!(at(&shot, 0, "kid").facing, Some(TOWARDS));
		assert_eq!(at(&shot, 0, "mum").facing, Some(-TOWARDS));

		// Standing on the "wrong" side of each other still turns them the right way.
		let close = page("# park\n@kid x=0.2 talk\n@mum x=0.3\nHello there.\n");
		assert_eq!(at(&close, 0, "mum").facing, Some(-TOWARDS));

		let alone = page("# park\n@kid x=0.3 talk\nHello there.\n");
		assert_eq!(at(&alone, 0, "kid").facing, None);

		let silent = page("# park\n@kid x=0.3\n@mum x=0.7\nNobody spoke.\n");
		assert_eq!(at(&silent, 0, "kid").facing, None, "nobody is talking");

		let directed = page("# park\n@kid x=0.3 talk face=left\n@mum x=0.7\nHello there.\n");
		assert_eq!(
			at(&directed, 0, "kid").facing,
			Some(-1.0),
			"the script's facing"
		);

		let crowd = page("# park\n@kid x=0.3 talk\n@mum x=0.7\n@rex x=0.5\nHello there.\n");
		assert_eq!(
			at(&crowd, 0, "kid").facing,
			Some(TOWARDS),
			"a dog is not a third"
		);
	}
}
