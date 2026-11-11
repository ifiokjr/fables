//! Turns a [`Scene`] into drawing instructions.
//!
//! The stage model talks in story terms ("Stinky, arms crossed, on the left"); this module turns
//! that into puppets with concrete positions and puppet parameters. Puppets are plain numbers,
//! so video can blend two of them to animate between stage directions.

use crate::art::Bounds;
use crate::art::Path;
use crate::art::Pt;
use crate::art::STAGE;
use crate::art::Svg;
use crate::art::critters;
use crate::art::critters::CritterState;
use crate::art::figure;
use crate::art::figure::FigureState;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::fx;
use crate::art::lerp;
use crate::art::palette;
use crate::art::pt;
use crate::art::seed_of;
use crate::art::settings;
use crate::art::settings::Weather;
use crate::cast::Kind;
use crate::stage::Actor;
use crate::stage::Scene;

/// A drawable actor with every parameter resolved to a number.
#[derive(Clone, Debug)]
pub struct Puppet {
	pub id: String,
	pub kind: Kind,
	/// Stage position of the puppet's origin (feet for people, body centre for birds).
	pub at: Pt,
	pub scale: f32,
	pub z: i32,
	pub opacity: f32,
	pub body: Body,
	pub stink: f32,
	pub mint: f32,
	/// Which way breath and gaze go: -1 left, 1 right.
	pub heading: f32,
	/// How much breath cloud is out (0..1). Stills derive it from the mouth; video drives it
	/// from the narration so clouds billow for a whole sentence instead of flickering.
	pub breath: f32,
	/// Height above the ground (hops). The puppet rises; its contact shadow stays put.
	pub lift: f32,
	/// Speed lines trailing a fast move: their strength, signed by the way it is going.
	pub smear: f32,
}

#[derive(Clone, Debug)]
pub enum Body {
	Figure(FigureState),
	Critter(CritterState),
}

/// What a puppet's own drawing covers, in stage units, measured from the drawing itself.
#[derive(Clone, Copy, Debug)]
pub struct Extents {
	/// Everything the puppet draws for itself: its head's hair, raised arms, held props and
	/// all — but not its ground shadow, breath or speed lines, which are not the subject.
	pub subject: Bounds,
	/// The head with its hair and anything riding on it; a critter's whole outline where
	/// its drawing has no separate head.
	pub head: Bounds,
}

impl Puppet {
	/// Where the mouth is on stage (for breath effects), if this puppet has one worth using.
	pub fn mouth(&self) -> Option<Pt> {
		let Body::Figure(state) = &self.body else {
			return None;
		};

		let Kind::Figure(look) = &self.kind else {
			return None;
		};

		if state.lie > 0.3 {
			return None;
		}

		let local = look.frame().mouth_at(state.facing);

		Some(self.at + local * self.scale)
	}

	/// The head's centre and radius on stage, for things that circle it (flies, sparkles).
	pub fn head(&self) -> (Pt, f32) {
		match (&self.kind, &self.body) {
			(Kind::Figure(look), Body::Figure(_)) => {
				let frame = look.frame();
				(self.at + frame.head * self.scale, frame.head_r * self.scale)
			}
			_ => (self.at + pt(0.0, -80.0 * self.scale), 60.0 * self.scale),
		}
	}

	/// Measures what this puppet draws, in stage units, by drawing it into a scratch
	/// document exactly the way [`draw_puppet`] draws it into a frame — heading, scale,
	/// lift, lie, pose, hair, head extras, held prop and a critter's own outline all in
	/// force — and taking the bounding box of the path data it emits. The measurement
	/// cannot drift from the drawing, because it is the drawing.
	///
	/// Framing reads this several times per camera setup (and every frame of a tracking
	/// move), so callers that measure the same setup again and again cache the view, not
	/// the puppet.
	pub fn extents(&self) -> Extents {
		let mut svg = Svg::new();
		svg.measuring(true);
		svg.begin(&subject_transform(self));
		svg.begin_measure();
		let head = draw_subject(&mut svg, self);
		let subject = svg.end_measure().unwrap_or_else(|| missing(self));
		svg.end();

		Extents {
			subject,
			head: head.unwrap_or(subject),
		}
	}
}

/// What to say about a puppet whose drawing emitted nothing. Every drawing emits paths, so
/// this is a bug in the drawing, and the box it gives is the puppet's own origin.
fn missing(puppet: &Puppet) -> Bounds {
	(puppet.at.x, puppet.at.y, puppet.at.x, puppet.at.y)
}

/// Resolves every visible actor in a scene to a puppet at `time` seconds.
///
/// A `still` (book page) draws anyone who spoke on the page mid-sentence; in motion, mouths
/// are driven by the narration instead.
pub fn puppets(scene: &Scene, time: f32, still: bool) -> Vec<Puppet> {
	let ground = scene.setting.ground();

	scene
		.placed()
		.into_iter()
		.map(|(actor, x)| puppet(actor, x, ground, time, still))
		.collect()
}

fn puppet(actor: &Actor, x: f32, ground: f32, time: f32, still: bool) -> Puppet {
	// Unless told otherwise, everyone turns towards the middle of the page.
	let toward_middle = if x <= 0.5 { 1.0 } else { -1.0 };
	let heading = match actor.facing {
		Some(facing) if facing.abs() > 0.01 => facing.signum(),
		_ => toward_middle,
	};
	let facing = actor.facing.unwrap_or(heading * 0.5).clamp(-1.0, 1.0) * 0.7;
	let (body, lift) = match &actor.kind {
		Kind::Figure(look) => {
			let frame = look.frame();
			let mut state = FigureState::new(&frame);
			state.mood = actor.mood;
			state.hands = frame.hands(actor.pose, facing);
			state.hand_shapes = actor.pose.hand_shapes();
			state.legs = frame.limbs().stance(actor.stance, facing);
			state.gait = actor
				.gait
				.map(|gait| (gait, if still { 1.1 } else { time * gait.rate() }));

			if !still {
				state.hands = gesture(
					state.hands,
					actor.pose,
					time + (seed_of(&actor.id) % 97) as f32 * 0.13,
					frame.head_r,
				);
			}

			state.facing = facing;
			state.gaze = heading * 0.6;
			state.lie = actor.knocked;
			state.teeth = actor.teeth;
			state.prop = actor.prop;
			state.time = time;
			state.mouth_open =
				actor
					.mouth
					.unwrap_or(if still && actor.talked { 0.55 } else { 0.0 });
			state.tilt = match actor.mood {
				Mood::Defiant => -4.0 * heading,
				Mood::Sad => 3.0 * heading,
				Mood::Laugh => -3.0 * heading,
				_ => 0.0,
			};

			(Body::Figure(state), 0.0)
		}
		Kind::Bird(_) => {
			let lift = if actor.knocked > 0.5 { 46.0 } else { 58.0 };
			(Body::Critter(critter_state(actor, heading, time)), lift)
		}
		_ => (Body::Critter(critter_state(actor, heading, time)), 0.0),
	};

	// An open mouth (a yawn, a shout) lets breath out even before video drives it from speech.
	let breath = match &body {
		Body::Figure(state) => (state.mouth_open * 1.8).clamp(0.0, 1.0),
		Body::Critter(_) => 0.0,
	};
	let at = match actor.y {
		Some(y) => pt(x * STAGE, y * STAGE),
		None => pt(x * STAGE, ground - lift * actor.size),
	};

	Puppet {
		id: actor.id.clone(),
		kind: actor.kind.clone(),
		at,
		scale: actor.size,
		z: actor.z,
		opacity: 1.0,
		body,
		stink: actor.stink,
		mint: actor.mint,
		heading,
		breath,
		lift: 0.0,
		smear: 0.0,
	}
}

fn critter_state(actor: &Actor, heading: f32, time: f32) -> CritterState {
	CritterState {
		facing: heading,
		knocked: actor.knocked,
		wilt: actor.wilt,
		lively: actor.lively,
		time,
	}
}

/// Small looping motions that make a held pose look alive on video.
fn gesture(hands: (Pt, Pt), pose: Pose, time: f32, r: f32) -> (Pt, Pt) {
	let (left, right) = hands;

	match pose {
		Pose::Wave => {
			(
				left,
				right
					+ pt(
						(time * 9.0).sin() * r * 0.28,
						-(time * 9.0).cos().abs() * r * 0.06,
					),
			)
		}
		Pose::Cheer => {
			let bob = (time * 7.0).sin() * r * 0.14;
			(left + pt(0.0, bob), right + pt(0.0, -bob))
		}
		Pose::Brush => (left, right + pt((time * 18.0).sin() * r * 0.1, 0.0)),
		Pose::Point => {
			(
				left,
				right + pt((time * 3.0).sin().max(0.0) * r * 0.14, 0.0),
			)
		}
		Pose::Shrug | Pose::ReachOut => {
			let bob = (time * 2.5).sin() * r * 0.06;
			(left + pt(0.0, bob), right + pt(0.0, bob))
		}
		_ => hands,
	}
}

/// Blends two puppets of the same actor; `t` = 0 gives `a`, 1 gives `b`.
pub fn blend(a: &Puppet, b: &Puppet, t: f32) -> Puppet {
	let body = match (&a.body, &b.body) {
		(Body::Figure(fa), Body::Figure(fb)) => Body::Figure(fa.blend(fb, t)),
		(Body::Critter(ca), Body::Critter(cb)) => {
			Body::Critter(CritterState {
				facing: if t < 0.5 { ca.facing } else { cb.facing },
				knocked: lerp(ca.knocked, cb.knocked, t),
				wilt: lerp(ca.wilt, cb.wilt, t),
				lively: lerp(ca.lively, cb.lively, t),
				time: cb.time,
			})
		}
		_ => b.body.clone(),
	};

	Puppet {
		id: b.id.clone(),
		kind: b.kind.clone(),
		at: a.at.lerp(b.at, t),
		scale: lerp(a.scale, b.scale, t),
		z: b.z,
		opacity: lerp(a.opacity, b.opacity, t),
		body,
		stink: lerp(a.stink, b.stink, t),
		mint: lerp(a.mint, b.mint, t),
		heading: if t < 0.5 { a.heading } else { b.heading },
		breath: lerp(a.breath, b.breath, t),
		lift: lerp(a.lift, b.lift, t),
		smear: b.smear,
	}
}

/// Draws a whole scene: backdrop, puppets, breath effects, foreground.
pub fn draw_scene(svg: &mut Svg, scene: &Scene, puppets: &[Puppet], time: f32) {
	let mut weather = scene.weather;
	weather.time = time;

	settings::backdrop(svg, &scene.setting, &weather);
	draw_effects(svg, &weather, puppets);

	for puppet in puppets {
		draw_puppet(svg, puppet);
	}

	for puppet in puppets {
		draw_breath(svg, puppet, time);
	}

	settings::foreground(svg, &scene.setting, &weather);
}

/// Draws everything but the far backdrop layer, for video frames that move the far layer
/// separately (parallax).
pub fn draw_near(svg: &mut Svg, scene: &Scene, puppets: &[Puppet], time: f32) {
	let mut weather = scene.weather;
	weather.time = time;

	settings::backdrop_layer(svg, &scene.setting, &weather, settings::Layer::Near);
	draw_effects(svg, &weather, puppets);

	for puppet in puppets {
		draw_puppet(svg, puppet);
	}

	for puppet in puppets {
		draw_breath(svg, puppet, time);
	}

	settings::foreground(svg, &scene.setting, &weather);
}

/// Focus lines and impact bursts, aimed at the first person on stage (or the middle of it).
pub(crate) fn draw_effects(svg: &mut Svg, weather: &Weather, puppets: &[Puppet]) {
	if weather.speed <= 0.0 && weather.impact <= 0.0 {
		return;
	}

	let focus = puppets
		.iter()
		.find_map(|puppet| {
			match &puppet.kind {
				Kind::Figure(look) => {
					Some(puppet.at - pt(0.0, look.frame().height() * puppet.scale * 0.6))
				}
				_ => None,
			}
		})
		.unwrap_or(pt(STAGE / 2.0, STAGE * 0.45));
	fx::impact_burst(svg, focus, STAGE * 0.42, weather.impact, 41);
	fx::focus_lines(svg, focus, STAGE, weather.speed, 17, weather.time);
}

/// Draws one puppet at its stage position.
pub fn draw_puppet(svg: &mut Svg, puppet: &Puppet) {
	if puppet.opacity <= 0.01 {
		return;
	}

	if puppet.opacity < 0.99 {
		svg.begin_opacity(puppet.opacity);
	}

	if puppet.smear != 0.0 {
		let height = match &puppet.kind {
			Kind::Figure(look) => look.frame().height() * puppet.scale,
			_ => 300.0 * puppet.scale,
		};
		fx::smear_lines(svg, puppet.at, height, puppet.smear);
	}

	draw_shadow(svg, puppet);
	svg.begin(&subject_transform(puppet));
	draw_subject(svg, puppet);
	svg.end();

	if puppet.opacity < 0.99 {
		svg.end();
	}
}

/// The group transform that places a puppet's drawing on stage: where it stands, lifted
/// for a hop, at its scale. Mirroring happens inside the drawing (the heading turns the
/// parts), so this is the same however the puppet faces.
fn subject_transform(puppet: &Puppet) -> String {
	format!(
		"translate({:.1} {:.1}) scale({:.3})",
		puppet.at.x,
		puppet.at.y - puppet.lift,
		puppet.scale
	)
}

/// Draws the puppet itself — the subject: a figure or critter in full, inside the group
/// transform that places it. The ground shadow, breath, stink and speed lines a frame adds
/// around it are drawn by [`draw_shadow`] and [`draw_breath`], and count for nothing here.
/// Returns the bounds of the subject's head (whole outline where the drawing has no
/// separate head), for whoever is measuring.
fn draw_subject(svg: &mut Svg, puppet: &Puppet) -> Option<Bounds> {
	let seed = seed_of(&puppet.id);

	match (&puppet.kind, &puppet.body) {
		(Kind::Figure(look), Body::Figure(state)) => figure::draw(svg, look, state),
		(Kind::Bird(color), Body::Critter(state)) => critters::bird(svg, *color, *state, seed),
		(Kind::Dog(color), Body::Critter(state)) => critters::dog(svg, *color, *state, seed),
		(Kind::Worm, Body::Critter(state)) => critters::worm(svg, *state, seed),
		(Kind::Flower(kind, color), Body::Critter(state)) => {
			critters::flower(svg, *kind, *color, 480.0, *state, seed)
		}
		(Kind::Tree(color), Body::Critter(state)) => {
			critters::tree(svg, *color, 1000.0, *state, seed)
		}
		(Kind::Toothbrush(color), Body::Critter(state)) => {
			critters::toothbrush_buddy(svg, *color, *state, seed)
		}
		_ => None,
	}
}

/// A shadow where a puppet meets the ground, so nobody floats. Under a scene light it becomes
/// a cast shadow: it leans away from the key light, lengthens as the elevation drops and fades
/// when the light is faint (a soft pool under overcast, barely there at night).
fn draw_shadow(svg: &mut Svg, puppet: &Puppet) {
	let airborne = matches!(&puppet.body, Body::Critter(state) if state.lively > 0.5)
		&& matches!(puppet.kind, Kind::Bird(_));

	if airborne {
		return;
	}

	svg.begin(&format!(
		"translate({:.1} {:.1}) scale({:.3})",
		puppet.at.x, puppet.at.y, puppet.scale
	));

	// The scene light leans and stretches the pool; flat daylight draws exactly the old
	// contact shadow.
	let cast = svg.light.map(|light| {
		let (dx, stretch) = light.cast();
		(dx, stretch, light.ground_shadow)
	});

	if let Some((dx, stretch, alpha)) = cast {
		svg.begin(&format!("translate({dx:.1} 0) scale({stretch:.3} 1)"));
		svg.begin_opacity(alpha);
	}

	match (&puppet.kind, &puppet.body) {
		(Kind::Figure(look), Body::Figure(state)) => {
			figure::shadow(svg, look, state, puppet.lift / puppet.scale.max(0.01));
		}
		(Kind::Dog(_), _) => {
			svg.fill_alpha(
				&Path::ellipse(pt(10.0, 0.0), 230.0, 26.0),
				palette::INK,
				0.15,
			);
		}
		(Kind::Toothbrush(_), _) => {
			svg.fill_alpha(
				&Path::ellipse(pt(0.0, 0.0), 120.0, 20.0),
				palette::INK,
				0.15,
			);
		}
		(Kind::Bird(_), _) => {
			svg.fill_alpha(
				&Path::ellipse(pt(0.0, 56.0), 60.0, 12.0),
				palette::INK,
				0.15,
			);
		}
		_ => {}
	}

	if cast.is_some() {
		svg.end();
		svg.end();
	}

	svg.end();
}

/// Breath clouds and mist come out of open mouths; closed mouths still give themselves away
/// with flies (stinky) or glints (minty).
pub(crate) fn draw_breath(svg: &mut Svg, puppet: &Puppet, time: f32) {
	if puppet.opacity < 0.5 {
		return;
	}

	let seed = seed_of(&puppet.id);
	let open = puppet.breath;
	let (head, radius) = puppet.head();

	if puppet.stink > 0.01 {
		if let Some(mouth) = puppet.mouth() {
			fx::stink_cloud(
				svg,
				mouth,
				puppet.heading,
				puppet.stink * open * puppet.scale * 0.8,
				time,
				seed,
			);
		}

		for i in 0..(puppet.stink * 3.0).round() as usize {
			let angle = time * (2.2 + i as f32 * 0.6) + i as f32 * 2.1;
			// The orbit stays outside the head so a fly never lands on a face.
			let p = head + pt(angle.cos() * radius * 1.6, angle.sin() * radius * 1.35);
			fx::fly(svg, p, 9.0 * puppet.scale, time + i as f32);
		}
	}

	if puppet.mint > 0.01 {
		if let Some(mouth) = puppet.mouth() {
			fx::mint_mist(
				svg,
				mouth,
				puppet.heading,
				puppet.mint * open * puppet.scale * 0.8,
				time,
				seed,
			);
		}

		for i in 0..3 {
			let angle = time * 0.9 + i as f32 * 2.1;
			let p = head + pt(angle.cos() * radius * 1.7, angle.sin() * radius * 1.45);
			fx::sparkle(
				svg,
				p,
				22.0 * puppet.scale * puppet.mint.min(1.0),
				palette::WHITE,
				time + i as f32,
			);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure::Build;
	use crate::art::light::Light;
	use crate::art::light::Preset;
	use crate::art::pack::StylePack;

	/// A standing kid at the middle of the stage.
	fn kid() -> Puppet {
		let look = crate::cast::template_look(&StylePack::default_pack(), Build::Kid).unwrap();
		let state = FigureState::new(&look.frame());
		Puppet {
			id: "kid".into(),
			kind: Kind::Figure(look),
			at: pt(1200.0, 2060.0),
			scale: 2.25,
			z: 0,
			opacity: 1.0,
			body: Body::Figure(state),
			stink: 0.0,
			mint: 0.0,
			heading: 1.0,
			breath: 0.0,
			lift: 0.0,
			smear: 0.0,
		}
	}

	#[test]
	fn a_scene_light_turns_the_ground_shadow_into_a_cast_shadow() {
		let golden = Light::of(Preset::Golden);
		let (dx, stretch) = golden.cast();
		let lean = format!("translate({dx:.1} 0) scale({stretch:.3} 1)");

		let mut svg = Svg::new();
		svg.set_light(golden);
		draw_puppet(&mut svg, &kid());
		let lit = svg.finish((0.0, 0.0, STAGE, STAGE), 64, 64);
		assert!(lit.contains(&lean), "the shadow leans from the key: {lit}");

		// Flat daylight keeps the plain contact shadow: no lean group at all.
		let mut svg = Svg::new();
		draw_puppet(&mut svg, &kid());
		let day = svg.finish((0.0, 0.0, STAGE, STAGE), 64, 64);
		assert!(!day.contains(&lean), "no cast lean in daylight: {day}");
	}
}
