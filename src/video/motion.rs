//! Rig checks: a sheet of stances and gait phases with the skeleton drawn over the character,
//! and a motion test video that puts one character through its paces.
//!
//! Studios run exactly this kind of test whenever a rig changes: does every pose read, do the
//! elbows and knees bend the right way, do walks and runs plant their feet.

use std::path::Path;

use anyhow::Result;
use anyhow::bail;
use resvg::tiny_skia::Pixmap;

use crate::art::Path as Shape;
use crate::art::Svg;
use crate::art::dials::Motion;
use crate::art::escape;
use crate::art::figure;
use crate::art::figure::FigureState;
use crate::art::figure::Look;
use crate::art::figure::Mood;
use crate::art::figure::Pose;
use crate::art::palette;
use crate::art::rig::Gait;
use crate::art::rig::Stance;
use crate::art::style::Style;
use crate::raster;
use crate::video::Clip;

/// One held moment in the motion test.
#[derive(Clone, Copy)]
struct Beat {
	/// When the move to this beat starts, in seconds.
	at: f32,
	label: &'static str,
	pose: Pose,
	stance: Stance,
	gait: Option<Gait>,
	mood: Mood,
}

const fn beat(
	at: f32,
	label: &'static str,
	pose: Pose,
	stance: Stance,
	gait: Option<Gait>,
	mood: Mood,
) -> Beat {
	Beat {
		at,
		label,
		pose,
		stance,
		gait,
		mood,
	}
}

/// The routine: every stance, the arm poses that pair with them, then a walk and a run.
const ROUTINE: [Beat; 12] = [
	beat(
		0.0,
		"stand · idle",
		Pose::Idle,
		Stance::Stand,
		None,
		Mood::Happy,
	),
	beat(1.2, "wave", Pose::Wave, Stance::Stand, None, Mood::Grin),
	beat(
		2.6,
		"crouch · hands on hips",
		Pose::Hips,
		Stance::Crouch,
		None,
		Mood::Defiant,
	),
	beat(
		3.8,
		"jump · cheer",
		Pose::Cheer,
		Stance::Jump,
		None,
		Mood::Laugh,
	),
	beat(
		4.8,
		"sit · think",
		Pose::Think,
		Stance::Sit,
		None,
		Mood::Dreamy,
	),
	beat(
		6.4,
		"kneel · reach out",
		Pose::ReachOut,
		Stance::Kneel,
		None,
		Mood::Worried,
	),
	beat(7.8, "bow", Pose::Droop, Stance::Bow, None, Mood::Proud),
	beat(
		9.0,
		"tiptoe · hold nose",
		Pose::HoldNose,
		Stance::Tiptoe,
		None,
		Mood::Disgusted,
	),
	beat(
		10.4,
		"walk",
		Pose::Idle,
		Stance::Stand,
		Some(Gait::Walk),
		Mood::Happy,
	),
	beat(
		13.0,
		"run",
		Pose::Idle,
		Stance::Stand,
		Some(Gait::Run),
		Mood::Grin,
	),
	beat(15.6, "point", Pose::Point, Stance::Stand, None, Mood::Cross),
	beat(
		16.8,
		"arms crossed",
		Pose::ArmsCrossed,
		Stance::Stand,
		None,
		Mood::Defiant,
	),
];

const LENGTH: f32 = 18.5;
/// How long a move between beats takes.
const MOVE: f32 = 0.55;

fn state(look: &Look, beat: &Beat, time: f32) -> FigureState {
	let frame = look.frame();
	let facing = 0.35;
	let mut state = FigureState::new(&frame);
	state.mood = beat.mood;
	state.hands = frame.hands(beat.pose, facing);
	state.hand_shapes = beat.pose.hand_shapes();
	state.legs = frame.limbs().stance(beat.stance, facing);
	state.gait = beat.gait.map(|gait| (gait, time * gait.rate()));
	state.facing = facing;
	state.gaze = 0.4;
	state.time = time;
	state
}

/// The character at `time` in the routine, moving between beats the way `motion` says: its
/// timing, easing, squash and idle loop.
fn at(look: &Look, time: f32, motion: &Motion) -> (FigureState, &'static str) {
	let moving = |time: f32| {
		ROUTINE
			.iter()
			.rposition(|beat| beat.at <= time)
			.is_some_and(|index| index > 0 && time - ROUTINE[index].at < MOVE)
	};
	let time = motion.drawing_time(time, 0.0, moving(time));
	let index = ROUTINE
		.iter()
		.rposition(|beat| beat.at <= time)
		.unwrap_or(0);
	let current = &ROUTINE[index];
	let previous = &ROUTINE[index.saturating_sub(1)];
	let t = ((time - current.at) / MOVE).clamp(0.0, 1.0);
	let mut state = state(look, previous, time).blend(&state(look, current, time), motion.ease(t));

	if current.stance == Stance::Jump {
		state.squash = 1.0 + 0.09 * motion.squash * (t * std::f32::consts::PI).sin();
	}

	if current.gait.is_none() {
		let (_, squash) = motion.idle_at(time, 0);
		state.squash *= squash;
	}

	(state, current.label)
}

/// One frame: the character on the left, the same pose with its skeleton on the right.
fn frame_svg(
	look: &Look,
	name: &str,
	(style, motion): (Style, &Motion),
	time: f32,
	(width, height): (u32, u32),
) -> String {
	let (state, label) = at(look, time, motion);
	let lift = match state.gait {
		None => motion.idle_at(time, 0).0,
		Some(_) => 0.0,
	};
	let frame = look.frame();
	let (w, h) = (width as f32, height as f32);
	let scale = h * 0.62 / frame.height();
	let ground = h * 0.86;
	let mut svg = Svg::styled(style);
	svg.fill(&Shape::rect(0.0, 0.0, w, h), palette::PAPER);
	svg.fill(&Shape::rect(0.0, ground, w, h - ground), palette::SAND);

	for (x, skeleton) in [(w * 0.27, false), (w * 0.73, true)] {
		svg.begin(&format!("translate({x:.1} {ground:.1}) scale({scale:.4})"));
		figure::shadow(&mut svg, look, &state, lift);
		svg.begin(&format!("translate(0 {:.1})", -lift));
		svg.begin_layer();
		figure::draw(&mut svg, look, &state);
		svg.end();

		if skeleton {
			figure::draw_skeleton(&mut svg, look, &state);
		}

		svg.end();
		svg.end();
	}

	svg.raw(&format!(
		r##"<text x="{:.1}" y="{:.1}" font-family="{}" font-size="{:.1}" fill="#2a2238">{} — rig test</text><text x="{:.1}" y="{:.1}" font-family="{}" font-size="{:.1}" fill="#f2543d" text-anchor="end">{}</text>"##,
		w * 0.04,
		h * 0.1,
		raster::DISPLAY_FAMILY,
		h * 0.06,
		escape(name),
		w * 0.96,
		h * 0.1,
		raster::DISPLAY_FAMILY,
		h * 0.06,
		escape(label)
	));

	svg.finish((0.0, 0.0, w, h), width, height)
}

/// Renders the motion test as an MP4, moving the way `motion` says.
pub fn motion_test(
	look: &Look,
	name: &str,
	(style, motion): (Style, &Motion),
	path: &Path,
) -> Result<()> {
	let (width, height, fps) = (1280, 720, 30);
	let clip = Clip {
		width,
		height,
		fps,
		frames: (LENGTH * fps as f32) as usize,
		audio: None,
		label: "rig test",
	};

	clip.encode(path, |index| {
		let time = index as f32 / fps as f32;
		let mut pixmap = raster::render(&frame_svg(
			look,
			name,
			(style, motion),
			time,
			(width, height),
		))?;
		raster::finish(&mut pixmap, &style, 0.03, index as u32 / 2);
		Ok(pixmap.take())
	})
}

/// A sheet of every stance and of walk and run phases, each with its skeleton.
pub fn rig_sheet(look: &Look, style: Style) -> Result<Pixmap> {
	let cell = 520.0;
	let mut cells: Vec<(String, FigureState)> = Vec::new();

	for beat in &ROUTINE[..8] {
		cells.push((beat.label.to_owned(), state(look, beat, 0.0)));
	}

	for (gait, name) in [(Gait::Walk, "walk"), (Gait::Run, "run")] {
		for step in 0..4 {
			let beat = beat(0.0, "", Pose::Idle, Stance::Stand, Some(gait), Mood::Happy);
			let phase = step as f32 * std::f32::consts::FRAC_PI_2;
			let mut posed = state(look, &beat, 0.0);
			posed.gait = Some((gait, phase));
			cells.push((format!("{name} {}/4", step + 1), posed));
		}
	}

	let columns = 4;
	let rows = cells.len().div_ceil(columns);
	let (width, height) = (cell * columns as f32, cell * 1.12 * rows as f32);
	let frame = look.frame();
	let scale = cell * 0.72 / frame.height();

	if cells.is_empty() {
		bail!("nothing to draw");
	}

	let mut svg = Svg::styled(style);
	svg.fill(&Shape::rect(0.0, 0.0, width, height), palette::PAPER);

	for (index, (label, state)) in cells.iter().enumerate() {
		let x = (index % columns) as f32 * cell + cell / 2.0;
		let y = (index / columns) as f32 * cell * 1.12;
		let ground = y + cell * 0.92;
		svg.fill(
			&Shape::rect(x - cell / 2.0 + 10.0, ground, cell - 20.0, 6.0),
			palette::SAND.darken(0.2),
		);
		svg.begin(&format!("translate({x:.1} {ground:.1}) scale({scale:.4})"));
		svg.begin_layer();
		figure::draw(&mut svg, look, state);
		svg.end();
		svg.begin_opacity(0.85);
		figure::draw_skeleton(&mut svg, look, state);
		svg.end();
		svg.end();
		svg.raw(&format!(
			r##"<text x="{x:.1}" y="{:.1}" font-family="{}" font-size="34" fill="#2a2238" text-anchor="middle">{}</text>"##,
			ground + cell * 0.1,
			raster::BODY_FAMILY,
			escape(label)
		));
	}

	let mut pixmap =
		raster::render(&svg.finish((0.0, 0.0, width, height), width as u32, height as u32))?;
	raster::finish(&mut pixmap, &style, 0.04, 0);

	Ok(pixmap)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_routine_is_in_time_order_and_fits_the_clip() {
		for pair in ROUTINE.windows(2) {
			assert!(pair[0].at < pair[1].at);
		}

		assert!(ROUTINE.last().is_some_and(|beat| beat.at + MOVE < LENGTH));
	}

	#[test]
	fn moves_blend_between_beats() {
		let cast = crate::cast::Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let crate::cast::Kind::Figure(look) = cast.resolve("kid").unwrap() else {
			panic!("kid is a person");
		};
		let (standing, _) = at(&look, 2.5, &Motion::default());
		let (crouching, label) = at(&look, 3.5, &Motion::default());
		assert_eq!(label, "crouch · hands on hips");
		assert!(crouching.legs.crouch > standing.legs.crouch);
	}
}
