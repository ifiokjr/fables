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
use crate::art::figure::HandShape;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::fx;
use crate::art::lerp;
use crate::art::palette;
use crate::art::props;
use crate::art::pt;
use crate::art::seed_of;
use crate::art::settings;
use crate::art::settings::Weather;
use crate::cast::Kind;
use crate::stage::Actor;
use crate::stage::Scene;
use crate::stage::See;
use crate::video::acting;

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
	/// How out of breath the puppet is (0..1): high just after running, so breathing stays
	/// fast for a while.
	pub exerted: f32,
}

#[derive(Clone, Debug)]
pub enum Body {
	Figure(FigureState),
	Critter(CritterState),
	/// A prop standing on the stage: no limbs, just its drawing.
	Prop,
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

	/// The point a `see=` target is found by: the middle of a figure's face, a prop's centre, or
	/// a critter's body.
	pub fn focus(&self) -> Pt {
		match (&self.kind, &self.body) {
			(Kind::Figure(look), Body::Figure(_)) => {
				let frame = look.frame();
				self.at + frame.head * self.scale
			}
			(Kind::Prop(prop), Body::Prop) => {
				let (lo, hi) = prop.bounds();
				let base = pt(f32::midpoint(lo.x, hi.x), hi.y);
				let centre = pt(f32::midpoint(lo.x, hi.x), f32::midpoint(lo.y, hi.y));
				self.at + (centre - base) * self.scale
			}
			_ => self.at + pt(0.0, -80.0 * self.scale),
		}
	}

	/// The hand nearest `near`, in stage units: where a tether is tied.
	pub fn hand_near(&self, near: Pt) -> Option<Pt> {
		let Body::Figure(state) = &self.body else {
			return None;
		};
		let mirror = if state.facing < 0.0 { -1.0 } else { 1.0 };
		let hands = [state.hands.0, state.hands.1]
			.map(|hand| self.at + pt(hand.x * mirror, hand.y) * self.scale);

		hands
			.into_iter()
			.min_by(|a, b| (*a - near).len().total_cmp(&(*b - near).len()))
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

/// The bounds of the thing a figure holds, drawn where its hand draws it and measured from
/// the drawing itself — the fist end of a tether, or a prop a scene is framed on.
pub fn held_prop_bounds(puppet: &Puppet) -> Option<Bounds> {
	let Body::Figure(state) = &puppet.body else {
		return None;
	};
	state.prop.as_ref()?;
	let Kind::Figure(look) = &puppet.kind else {
		return None;
	};
	let mut svg = Svg::new();
	svg.measuring(true);
	svg.begin(&subject_transform(puppet));
	svg.begin_measure();
	figure::draw_held(&mut svg, look, state);
	let bounds = svg.end_measure();
	svg.end();

	bounds
}

/// Resolves every visible actor in a scene to a puppet at `time` seconds.
///
/// A `still` (book page) draws anyone who spoke on the page mid-sentence; in motion, mouths
/// are driven by the narration instead.
pub fn puppets(scene: &Scene, time: f32, still: bool) -> Vec<Puppet> {
	let ground = scene.setting.ground();
	let placed = scene.placed();
	let mut puppets: Vec<Puppet> = placed
		.iter()
		.map(|(actor, x)| puppet(actor, *x, ground, time, still))
		.collect();
	let focus: Vec<(String, Pt)> = puppets
		.iter()
		.map(|puppet| (puppet.id.clone(), puppet.focus()))
		.collect();

	for (puppet, (actor, _)) in puppets.iter_mut().zip(&placed) {
		aim(puppet, actor, &focus);
	}

	puppets
}

/// Aims a figure's eyes (and, through the eyes-lead-the-head rule, its head) at its `see=`
/// target. The look holds until the script changes it; `see=front` releases it.
fn aim(puppet: &mut Puppet, actor: &Actor, focus: &[(String, Pt)]) {
	let Some(see) = &actor.eyes.see else {
		return;
	};
	let (at, scale, heading) = (puppet.at, puppet.scale, puppet.heading);
	let Kind::Figure(look) = &puppet.kind else {
		return;
	};
	let frame = look.frame();
	let head = at + frame.head * scale;
	let Body::Figure(state) = &mut puppet.body else {
		return;
	};
	let mirror = if state.facing < 0.0 { -1.0 } else { 1.0 };
	let mine = at + pt(state.hands.1.x * mirror, state.hands.1.y) * scale;
	let found = match see {
		See::Actor(id) => {
			focus
				.iter()
				.find(|(key, _)| key == id)
				.map(|(_, point)| acting::aim(head, *point))
		}
		See::Mine => Some(acting::aim(head, mine)),
		other => Some(acting::word_aim(other, heading)),
	};
	let Some(aim) = found else {
		return;
	};

	state.gaze = aim.gaze;
	state.gaze_y = aim.gaze_y;
	state.facing = aim.turn * 0.7;

	match see {
		See::Up => state.head_tilt -= 9.0,
		See::Down => state.head_tilt += 7.0,
		// Front lets the eyes go: a speaker nearby pulls them again, as they always have.
		See::Front => {}
		_ => state.gaze_locked = true,
	}
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

			// A two-handed prop brings the other hand to the grip too.
			if let Some(prop) = &actor.prop
				&& prop.hands == props::Hands::Two
			{
				state.hands.0 = state.hands.1 + pt(-12.0, 8.0);
				state.hand_shapes.0 = HandShape::Fist;
			}

			state.facing = facing;
			state.gaze = heading * 0.6;
			state.lie = actor.knocked;
			state.teeth = actor.teeth;
			state.prop.clone_from(&actor.prop);
			figure::carry(look, actor.pose, &mut state);
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
		Kind::Prop(_) => (Body::Prop, 0.0),
		_ => (Body::Critter(critter_state(actor, heading, time)), 0.0),
	};

	// An open mouth (a yawn, a shout) lets breath out even before video drives it from speech.
	let breath = match &body {
		Body::Figure(state) => (state.mouth_open * 1.8).clamp(0.0, 1.0),
		Body::Critter(_) | Body::Prop => 0.0,
	};
	let at = match actor.y {
		Some(y) => pt(x * STAGE, y * STAGE),
		None => pt(x * STAGE, ground - lift * actor.size),
	};

	// A stage prop's size is relative to the size a kid holds it at.
	let scale = match actor.kind {
		Kind::Prop(_) => actor.size * props::STAGE_SIZE,
		_ => actor.size,
	};

	Puppet {
		id: actor.id.clone(),
		kind: actor.kind.clone(),
		at,
		scale,
		z: actor.z,
		opacity: 1.0,
		body,
		stink: actor.stink,
		mint: actor.mint,
		heading,
		breath,
		lift: 0.0,
		smear: 0.0,
		exerted: 0.0,
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
		exerted: lerp(a.exerted, b.exerted, t),
	}
}

/// Draws a whole scene: backdrop, puppets, breath effects, foreground.
pub fn draw_scene(svg: &mut Svg, scene: &Scene, puppets: &[Puppet], time: f32) {
	let mut weather = scene.weather;
	weather.time = time;

	settings::backdrop(svg, &scene.setting, &weather);
	draw_effects(svg, &weather, puppets);
	draw_tethers(svg, scene, puppets, time);

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
	draw_tethers(svg, scene, puppets, time);

	for puppet in puppets {
		draw_puppet(svg, puppet);
	}

	for puppet in puppets {
		draw_breath(svg, puppet, time);
	}

	settings::foreground(svg, &scene.setting, &weather);
}

/// The two ends of a stage prop's tether: at the prop's grip, and in the hand it is tied to.
/// `None` when `id` is not a tethered prop, or the holder is not on stage.
pub fn tether_points(scene: &Scene, puppets: &[Puppet], id: &str) -> Option<(Pt, Pt)> {
	let actor = scene.actors.iter().find(|actor| actor.id == id)?;
	let target = actor.tether.as_deref()?;
	let prop = puppets.iter().find(|puppet| puppet.id == id)?;
	let Kind::Prop(kind) = &prop.kind else {
		return None;
	};
	let holder = puppets.iter().find(|puppet| puppet.id == target)?;
	let start = kind.base_grip(prop.at, prop.scale);
	let end = holder.hand_near(start)?;

	Some((start, end))
}

/// Strings from stage props to the hands that hold them: they sag under their own weight and
/// sway gently in video, where `time` moves.
fn draw_tethers(svg: &mut Svg, scene: &Scene, puppets: &[Puppet], time: f32) {
	for actor in &scene.actors {
		let Some((start, end)) = tether_points(scene, puppets, &actor.id) else {
			continue;
		};
		let length = (end - start).len();
		let sway = (time * 1.4 + seed_of(&actor.id) as f32 % 6.3).sin() * length * 0.02;
		let mid = start.lerp(end, 0.5) + pt(sway, length * 0.12 + 8.0);
		let path = Path::new().move_to(start).quad_to(mid, end);
		svg.stroke_alpha(&path, palette::INK, 2.6, 0.75);
	}
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
		(Kind::Prop(prop), Body::Prop) => {
			prop.draw_standing(svg, 1.0);
			None
		}
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
	use crate::art::figure::Mood;
	use crate::art::figure::Pose;
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
			exerted: 0.0,
		}
	}

	/// A kid from a cast line — the parametric puppet, no authored parts — with a named
	/// hairdo and pose, facing `heading`.
	fn kid_of(hair: &str, pose: Pose, heading: f32) -> Puppet {
		let cast = crate::cast::Cast::parse(&format!(
			"[kid]\nname = \"Kid\"\nhair = \"{hair}\"\npuppet = \"parametric\"\n"
		))
		.unwrap();
		let Kind::Figure(look) = cast.members["kid"].kind.clone() else {
			panic!("a cast kid is a figure");
		};
		let frame = look.frame();
		let facing = heading * 0.7;
		let mut state = FigureState::new(&frame);
		state.hands = frame.hands(pose, facing);
		state.hand_shapes = pose.hand_shapes();
		state.facing = facing;

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
			heading,
			breath: 0.0,
			lift: 0.0,
			smear: 0.0,
			exerted: 0.0,
		}
	}

	/// The sausage dog, facing `heading`.
	fn dog(heading: f32) -> Puppet {
		Puppet {
			id: "dog".into(),
			kind: Kind::Dog(palette::TOMATO),
			at: pt(1200.0, 2060.0),
			scale: 1.0,
			z: 0,
			opacity: 1.0,
			body: Body::Critter(CritterState {
				facing: heading,
				..CritterState::default()
			}),
			stink: 0.0,
			mint: 0.0,
			heading,
			breath: 0.0,
			lift: 0.0,
			smear: 0.0,
			exerted: 0.0,
		}
	}

	/// Draws the puppet's subject alone on transparent paper — what [`Puppet::extents`]
	/// measures, ground shadow and effects and all left out — and returns the box of its
	/// visible pixels in stage units.
	fn pixel_bounds(puppet: &Puppet) -> Bounds {
		let view = (puppet.at.x - 1500.0, puppet.at.y - 1700.0, 3000.0, 3200.0);
		let mut svg = Svg::new();
		svg.begin(&subject_transform(puppet));
		draw_subject(&mut svg, puppet);
		svg.end();
		let pixmap = crate::raster::render(&svg.finish(view, 600, 640)).unwrap();
		let (vx, vy, vw, _) = view;
		let unit = vw / pixmap.width() as f32;
		let (mut left, mut top, mut right, mut bottom) = (
			f32::INFINITY,
			f32::INFINITY,
			f32::NEG_INFINITY,
			f32::NEG_INFINITY,
		);

		for y in 0..pixmap.height() {
			for x in 0..pixmap.width() {
				if pixmap.pixel(x, y).unwrap().alpha() > 0 {
					left = left.min(vx + x as f32 * unit);
					top = top.min(vy + y as f32 * unit);
					right = right.max(vx + (x + 1) as f32 * unit);
					bottom = bottom.max(vy + (y + 1) as f32 * unit);
				}
			}
		}

		assert!(left <= right, "the puppet drew nothing in {view:?}");
		(left, top, right, bottom)
	}

	/// The measured bounds hold every drawn pixel, and sit within a small tolerance of
	/// them: the measurement is the drawing, give or take control points, stroke bleed
	/// and pixel granularity.
	fn holds_every_pixel(puppet: &Puppet, what: &str) {
		let Extents { subject, head } = puppet.extents();
		let pixels = pixel_bounds(puppet);
		// One pixel (5 units) of slack for antialiased edges.
		let slack = 6.0;

		assert!(
			pixels.0 >= subject.0 - slack
				&& pixels.1 >= subject.1 - slack
				&& pixels.2 <= subject.2 + slack
				&& pixels.3 <= subject.3 + slack,
			"{what}: drawn pixels {pixels:?} escape the measured bounds {subject:?}"
		);

		assert!(
			(subject.0 - pixels.0).abs() < 40.0
				&& (subject.1 - pixels.1).abs() < 40.0
				&& (pixels.2 - subject.2).abs() < 40.0
				&& (pixels.3 - subject.3).abs() < 40.0,
			"{what}: measured bounds {subject:?} stray from the drawn pixels {pixels:?}"
		);

		assert!(
			head.0 >= subject.0 - 0.01
				&& head.1 >= subject.1 - 0.01
				&& head.2 <= subject.2 + 0.01
				&& head.3 <= subject.3 + 0.01,
			"{what}: the head {head:?} escapes its own subject {subject:?}"
		);
	}

	#[test]
	fn measured_bounds_hold_every_drawn_pixel() {
		// A tall hairdo waving, both ways; the dog, both ways; and an authored puppet.
		holds_every_pixel(&kid_of("puff", Pose::Wave, 1.0), "puff kid waving right");
		holds_every_pixel(&kid_of("puff", Pose::Wave, -1.0), "puff kid waving left");
		holds_every_pixel(&dog(1.0), "dog facing right");
		holds_every_pixel(&dog(-1.0), "dog facing left");
		holds_every_pixel(&kid(), "authored template kid");
	}

	#[test]
	fn the_bounds_mirror_when_the_heading_flips() {
		// The dog mirrors exactly (its drawing is `scale(-1 1)`); an authored kid's
		// three-quarter face only leans, so its flip is held loosely.
		let (right, left) = (dog(1.0), dog(-1.0));
		let (a, b) = (right.extents().subject, left.extents().subject);
		let x = right.at.x;

		assert!(
			((a.0 - x) + (b.2 - x)).abs() < 1.0 && ((a.2 - x) + (b.0 - x)).abs() < 1.0,
			"dog {a:?} vs mirrored {b:?}"
		);
		assert!(
			(a.1 - b.1).abs() < 1.0 && (a.3 - b.3).abs() < 1.0,
			"{a:?} vs {b:?}"
		);

		let mut turned = kid();
		turned.heading = -1.0;

		if let Body::Figure(state) = &mut turned.body {
			state.facing = -0.7;
		}

		let (a, b) = (kid().extents().subject, turned.extents().subject);
		let x = turned.at.x;

		assert!(
			((a.0 - x) + (b.2 - x)).abs() < 50.0 && ((a.2 - x) + (b.0 - x)).abs() < 50.0,
			"authored kid {a:?} vs turned {b:?}"
		);
	}

	#[test]
	fn a_waving_figure_measures_the_raised_hand() {
		let idle = kid_of("puff", Pose::Idle, 1.0).extents().subject;
		let wave = kid_of("puff", Pose::Wave, 1.0).extents().subject;

		assert!(
			wave.2 > idle.2 + 15.0,
			"the waving hand reaches further right: idle {idle:?} vs wave {wave:?}"
		);
	}

	#[test]
	fn effects_do_not_enlarge_the_bounds() {
		let clean = kid().extents().subject;

		let mut breathing = kid();
		breathing.stink = 1.0;
		breathing.mint = 1.0;
		breathing.breath = 1.0;
		breathing.smear = -1.0;

		assert_eq!(
			breathing.extents().subject,
			clean,
			"breath, stink, mint and speed lines must not be part of the subject"
		);

		// Dizzy stars orbit well above the head; the dizzy face may draw differently,
		// but the subject's top must not rise to the stars.
		let mut dizzy = kid();

		if let Body::Figure(state) = &mut dizzy.body {
			state.mood = Mood::Dizzy;
		}

		assert!(
			dizzy.extents().subject.1 >= clean.1 - 40.0,
			"dizzy stars lifted the subject: {:?} against {clean:?}",
			dizzy.extents().subject
		);
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

	/// A scene rehearsed from a script and a small cast.
	fn staged(script: &str) -> Scene {
		let story = crate::story::parse(&format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n{script}"
		))
		.unwrap();
		let cast = crate::cast::Cast::parse(
			"[hana]\nname = \"Hana\"\n[mo]\nname = \"Mo\"\nkind = \"grown-up\"\n",
		)
		.unwrap();
		let rehearsals = crate::stage::rehearse(&story, &cast).unwrap();

		rehearsals[0].finale().clone()
	}

	fn body(puppets: &[Puppet], id: &str) -> FigureState {
		let Body::Figure(state) = &puppets.iter().find(|p| p.id == id).unwrap().body else {
			panic!("{id} is a figure");
		};

		state.clone()
	}

	#[test]
	fn a_stage_prop_stands_at_its_place_and_tethers_to_the_hand() {
		let scene = staged("# park\n@hana x=0.3\n@kite x=0.7 y=sky tether=hana\n");
		let puppets = puppets(&scene, 0.0, true);
		let kite = puppets.iter().find(|p| p.id == "kite").unwrap();

		assert!(matches!(kite.kind, Kind::Prop(_)));
		assert!(matches!(kite.body, Body::Prop));
		assert!(
			(kite.at.y - 0.32 * STAGE).abs() < 0.01,
			"the kite's base floats where `y=sky` puts it: {:?}",
			kite.at
		);

		let extents = kite.extents();
		assert!(
			(extents.subject.3 - kite.at.y).abs() < 8.0,
			"its lowest drawing stands on its place (within the ink's bleed): {extents:?} at {:?}",
			kite.at
		);
		assert!(
			extents.subject.1 < kite.at.y - 200.0,
			"the kite is drawn above it: {extents:?}"
		);

		// The tether runs from the prop to a hand of the actor it is tied to.
		let (start, end) = tether_points(&scene, &puppets, "kite").unwrap();
		let hana = puppets.iter().find(|p| p.id == "hana").unwrap();
		let hand = hana.hand_near(start).unwrap();

		assert!(
			start.x >= extents.subject.0 - 1.0
				&& start.y >= extents.subject.1 - 1.0
				&& start.x <= extents.subject.2 + 1.0
				&& start.y <= extents.subject.3 + 1.0,
			"the string starts on the prop at {start:?}: {extents:?}"
		);
		assert!((end - hand).len() < 0.01, "and ends in the hand {hand:?}");

		let subject = hana.extents().subject;
		assert!(
			end.x >= subject.0 - 1.0
				&& end.y >= subject.1 - 1.0
				&& end.x <= subject.2 + 1.0
				&& end.y <= subject.3 + 1.0,
			"the hand {end:?} is part of the holder {subject:?}"
		);
	}

	#[test]
	fn a_two_handed_prop_brings_both_hands_to_the_grip() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::create_dir(dir.path().join("props")).unwrap();
		std::fs::write(
			dir.path().join("props/two-hand.svg"),
			r#"<svg xmlns="http://www.w3.org/2000/svg" data-grip="0 0" data-hands="two" data-about="Two hands"><path id="bar" d="M-60 -20 C-20 -24 20 -24 60 -20 C62 -6 62 6 60 20 C20 24 -20 24 -60 20 C-62 6 -62 -6 -60 -20 Z" fill="toffee" data-shade="1"/></svg>"#,
		)
		.unwrap();

		let mut story = crate::story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\n+++\n# park\n@hana x=0.5 hold holds=two-hand\n",
		)
		.unwrap();
		story.props = props::Library::for_world(dir.path(), &StylePack::default_pack()).unwrap();
		let cast = crate::cast::Cast::parse("[hana]\nname = \"Hana\"\n").unwrap();
		let rehearsals = crate::stage::rehearse(&story, &cast).unwrap();
		let drawn = puppets(rehearsals[0].finale(), 0.0, true);
		let state = body(&drawn, "hana");

		assert_eq!(
			state.hand_shapes.0,
			HandShape::Fist,
			"the second hand fists"
		);
		assert!(
			(state.hands.0 - state.hands.1).len() < 30.0,
			"both hands meet at the grip: {:?} and {:?}",
			state.hands.0,
			state.hands.1
		);
	}

	#[test]
	fn a_see_follows_an_actor_and_tracks_a_prop_that_moves() {
		let right = staged("# park\n@hana x=0.3 see=kite\n@kite x=0.85\n");
		let drawn = puppets(&right, 0.0, true);
		let state = body(&drawn, "hana");

		assert!(state.gaze > 0.8, "looking right at the kite: {state:?}");
		assert!(state.facing > 0.4, "the head follows: {state:?}");
		assert!(state.gaze_locked);

		let left = staged("# park\n@hana x=0.3 see=kite\n@kite x=0.1\n");
		let drawn = puppets(&left, 0.0, true);
		let moved = body(&drawn, "hana");

		assert!(
			moved.gaze < -0.8,
			"the eyes swing to the kite's new place: {}",
			moved.gaze
		);
		assert!(moved.facing < -0.4, "and the head with them: {moved:?}");
	}

	#[test]
	fn see_words_and_the_held_prop_aim_the_eyes() {
		let up = staged("# park\n@hana x=0.5 see=up\n");
		let state = body(&puppets(&up, 0.0, true), "hana");
		assert!(state.gaze_y < -0.9, "eyes up: {state:?}");
		assert!(state.head_tilt < -5.0, "the chin lifts: {state:?}");

		let front = staged("# park\n@hana x=0.5 see=front\n");
		let state = body(&puppets(&front, 0.0, true), "hana");
		assert_eq!(state.gaze, 0.0);
		assert!(!state.gaze_locked, "front releases the look");

		let mine = staged("# park\n@hana x=0.5 hold holds=jam see=mine\n");
		let state = body(&puppets(&mine, 0.0, true), "hana");
		assert!(
			state.gaze.abs() > 0.2 || state.gaze_y.abs() > 0.2,
			"the eyes find the thing in hand: {state:?}"
		);
	}

	#[test]
	fn a_held_prop_measures_in_the_hand() {
		let scene = staged("# park\n@hana x=0.5 hold holds=jam\n");
		let puppets = puppets(&scene, 0.0, true);
		let hana = puppets.iter().find(|p| p.id == "hana").unwrap();
		let bounds = held_prop_bounds(hana).expect("the jam is drawn");

		let middle = pt(
			f32::midpoint(bounds.0, bounds.2),
			f32::midpoint(bounds.1, bounds.3),
		);
		let hand = hana.hand_near(middle).expect("a figure has hands");
		assert!(
			hand.x >= bounds.0 - 1.0
				&& hand.y >= bounds.1 - 1.0
				&& hand.x <= bounds.2 + 1.0
				&& hand.y <= bounds.3 + 1.0,
			"the hand {hand:?} holds the prop's measured bounds {bounds:?}"
		);

		let subject = hana.extents().subject;
		assert!(
			bounds.0 >= subject.0 - 1.0
				&& bounds.1 >= subject.1 - 1.0
				&& bounds.2 <= subject.2 + 1.0
				&& bounds.3 <= subject.3 + 1.0,
			"and the measured prop is part of the figure's subject {subject:?}"
		);
	}
}
