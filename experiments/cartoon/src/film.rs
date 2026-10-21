//! A seven-second 16:9 test: Stinky walks into the park, stops, turns to camera, waves and
//! says "Hello!". It proves the authored parts survive the rig in motion: the walk cycle and
//! IK from `fables::art::rig`, the turn blend, and lip-sync from `fables::video::lipsync`.

use std::f32::consts::PI;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use fables::art::Svg;
use fables::art::figure::FigureState;
use fables::art::figure::Mood;
use fables::art::figure::Pose;
use fables::art::figure::Viseme;
use fables::art::lerp;
use fables::art::pt;
use fables::art::rig::Gait;
use fables::art::rig::Legs;
use fables::raster;
use fables::video::Clip;
use fables::video::lipsync;

use crate::park;
use crate::stinky;
use crate::stinky::THREE_QUARTER;

const FPS: u32 = 30;
const LENGTH: f32 = 7.0;
const SCALE: f32 = 2.0;
/// The walk ends here, on the path.
const STOP_AT: f32 = 3.0;
const STOP_X: f32 = 1180.0;
const GROUND: f32 = 2130.0;
const LINE: &str = "Hello!";
const SPEAK: (f32, f32) = (4.35, 5.15);

/// Smooth 0..1 between `a` and `b`.
fn ease(time: f32, a: f32, b: f32) -> f32 {
	let t = ((time - a) / (b - a)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// Body speed that keeps the planted foot still: the rig moves a foot by
/// `sin(phase) · stride · leg`, so on average a planted foot travels back at this rate.
fn walk_speed() -> f32 {
	Gait::Walk.rate() * 0.45 * stinky::look().frame().limbs().leg_length() * 2.0 / PI
}

/// The mouth shape at `time`, from fables' own letter-to-viseme table.
fn mouth(time: f32) -> Option<Viseme> {
	if !(SPEAK.0..SPEAK.1).contains(&time) {
		return None;
	}

	let letters = lipsync::letters(LINE);
	let total: f32 = letters.iter().map(|(_, weight)| weight).sum();
	let mut at = (time - SPEAK.0) / (SPEAK.1 - SPEAK.0) * total;

	for (viseme, weight) in letters {
		if at < weight {
			return Some(viseme);
		}

		at -= weight;
	}

	Some(Viseme::Rest)
}

/// Stinky's pose and where he stands at `time`.
fn performance(time: f32) -> (FigureState, f32) {
	let frame = stinky::look().frame();
	let rate = Gait::Walk.rate();
	let mut state = stinky::posed(Mood::Happy, Pose::Idle, THREE_QUARTER);
	state.head_tilt = 2.0;
	state.gaze = 0.5;
	state.time = time;

	if time < STOP_AT {
		state.gait = Some((Gait::Walk, time * rate));
		let x = STOP_X - (STOP_AT - time) * walk_speed() * SCALE;
		return (state, x);
	}

	// Settle: blend the last stride into the standing pose instead of snapping.
	let stand = state.legs;
	let (walking, swing) = frame.limbs().cycle(Gait::Walk, STOP_AT * rate, 1.0, stand);
	let settle = ease(time, STOP_AT, STOP_AT + 0.35);
	state.legs = Legs {
		feet: (
			walking.feet.0.lerp(stand.feet.0, settle),
			walking.feet.1.lerp(stand.feet.1, settle),
		),
		crouch: lerp(walking.crouch, stand.crouch, settle) + (settle * PI).sin() * 0.05,
		bend: lerp(walking.bend, stand.bend, settle),
	};

	// Turn to the camera, then wave and say hello.
	let turn = ease(time, 3.25, 3.75);
	let raise = ease(time, 3.55, 4.05);
	state.facing = lerp(THREE_QUARTER, 0.05, turn);
	let idle = frame.hands(Pose::Idle, state.facing);
	let idle = (
		idle.0 + swing.0 * (1.0 - settle),
		idle.1 + swing.1 * (1.0 - settle),
	);
	let wave = frame.hands(Pose::Wave, state.facing);
	let wiggle = pt(
		(time * 2.6 * 2.0 * PI).cos() * 12.0,
		(time * 5.2 * PI).sin() * 4.0,
	) * ease(time, 4.0, 4.2);
	state.hands = (
		idle.0.lerp(wave.0, raise),
		idle.1.lerp(wave.1, raise) + wiggle,
	);

	if raise > 0.5 {
		state.hand_shapes = Pose::Wave.hand_shapes();
	}

	state.head_tilt = 2.0 + (time * 3.0).sin() * 2.0 * raise;
	state.gaze = lerp(0.5, 0.0, turn);
	state.viseme = mouth(time);

	(state, STOP_X)
}

fn blink(time: f32) -> f32 {
	[1.2_f32, 3.3, 6.3]
		.into_iter()
		.map(|at| 1.0 - ((time - at).abs() / 0.07).min(1.0))
		.fold(0.0, f32::max)
}

fn frame(time: f32, width: u32, height: u32) -> String {
	let (mut state, x) = performance(time);
	state.blink = blink(time);
	let mut c = Svg::new();
	park::draw(&mut c);
	park::wormsley(&mut c, pt(2010.0, 1730.0), 1.0);
	c.begin(&format!("translate({x:.1} {GROUND:.1}) scale({SCALE})"));
	stinky::draw(&mut c, &state);
	c.end();
	park::draw_front(&mut c);
	// The 16:9 window onto the square stage, sitting low so the ground plane fills it.
	let view_h = park::SIZE * 9.0 / 16.0;
	c.finish(
		(0.0, park::SIZE - view_h - 40.0, park::SIZE, view_h),
		width,
		height,
	)
}

/// A contact strip of frames, for checking the motion without playing the video.
pub fn strip(out: &Path) -> Result<PathBuf> {
	let times = [0.6_f32, 1.4, 2.6, 3.4, 3.7, 4.4, 4.7, 5.6];
	let (w, h) = (640.0, 360.0);
	let mut doc = format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
		w * 4.0,
		h * 2.0
	);

	// Each frame is rendered on its own and placed as an image, so the frames' ids can't clash.
	for (index, time) in times.into_iter().enumerate() {
		let png = raster::png(&raster::render(&frame(time, w as u32, h as u32))?)?;
		let _ = write!(
			doc,
			r#"<image x="{}" y="{}" width="{w}" height="{h}" href="data:image/png;base64,{}"/>"#,
			(index % 4) as f32 * w,
			(index / 4) as f32 * h,
			BASE64.encode(png)
		);
	}

	doc.push_str("</svg>");
	crate::sheets::save(&doc, &out.join("film-strip.png"))
}

/// Renders the clip as an MP4 (needs ffmpeg, which the devenv shell provides).
pub fn film(out: &Path) -> Result<PathBuf> {
	let (width, height) = (1280, 720);
	let path = out.join("wave-and-walk.mp4");
	let clip = Clip {
		width,
		height,
		fps: FPS,
		frames: (LENGTH * FPS as f32) as usize,
		audio: None,
		label: "cartoon experiment",
	};
	clip.encode(&path, |index| {
		let time = index as f32 / FPS as f32;
		Ok(raster::render(&frame(time, width, height))?.take())
	})?;
	Ok(path)
}
