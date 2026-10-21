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
use fables::art::figure::Viseme;
use fables::art::lerp;
use fables::art::pt;
use fables::art::rig::Gait;
use fables::art::rig::Legs;
use fables::art::rig::Stance;
use fables::raster;
use fables::video::Clip;
use fables::video::lipsync;

use crate::park;
use crate::stinky;
use crate::stinky::Arms;
use crate::stinky::LIMBS;
use crate::stinky::Mood;
use crate::stinky::Pose;

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
	Gait::Walk.rate() * 0.45 * LIMBS.leg_length() * 2.0 / PI
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
fn performance(time: f32) -> (Pose, f32) {
	let stand = stinky::stance(Stance::Stand, 1.0);
	let phase = time * Gait::Walk.rate();
	let mut pose = Pose::new(Mood::Happy, Arms::Relaxed, 1.0);
	pose.head_tilt = 2.0;
	pose.gaze = pt(0.5, 0.0);

	if time < STOP_AT {
		pose.gait = Some((Gait::Walk, phase));
		let x = STOP_X - (STOP_AT - time) * walk_speed() * SCALE;
		return (pose, x);
	}

	// Settle: blend the last stride into the standing pose instead of snapping.
	let (walking, swing) = LIMBS.cycle(Gait::Walk, STOP_AT * Gait::Walk.rate(), 1.0, stand);
	let settle = ease(time, STOP_AT, STOP_AT + 0.35);
	pose.legs = Legs {
		feet: (
			walking.feet.0.lerp(stand.feet.0, settle),
			walking.feet.1.lerp(stand.feet.1, settle),
		),
		crouch: lerp(walking.crouch, stand.crouch, settle) + (settle * PI).sin() * 0.05,
		bend: lerp(walking.bend, stand.bend, settle),
	};
	pose.hands = (
		pose.hands.0 + swing.0 * (1.0 - settle),
		pose.hands.1 + swing.1 * (1.0 - settle),
	);

	// Turn to the camera, then wave.
	let turn = ease(time, 3.25, 3.75);
	let raise = ease(time, 3.55, 4.05);
	let front = Pose::new(Mood::Happy, Arms::Relaxed, 0.15);
	let mut posed = pose.blend(&front, turn);
	posed.legs = pose.legs;
	posed.legs.feet.0 = pose
		.legs
		.feet
		.0
		.lerp(stinky::stance(Stance::Stand, 0.15).feet.0, turn);
	posed.legs.feet.1 = pose
		.legs
		.feet
		.1
		.lerp(stinky::stance(Stance::Stand, 0.15).feet.1, turn);
	let waving = posed.clone().with_arms(Arms::Wave);
	let mut posed = posed.blend(&waving, raise);
	posed.wrists.0 += (time * 2.6 * 2.0 * PI).sin() * 22.0 * ease(time, 4.0, 4.2);
	posed.hands.0 = posed.hands.0 + pt((time * 2.6 * 2.0 * PI).cos() * 6.0, 0.0) * raise;
	posed.head_tilt = 2.0 + (time * 3.0).sin() * 2.0 * raise;
	posed.gaze = pt(0.05, 0.0).lerp(pt(0.5, 0.0), 1.0 - turn);
	posed.viseme = mouth(time);

	(posed, STOP_X)
}

fn blink(time: f32) -> f32 {
	[1.2_f32, 3.3, 6.3]
		.into_iter()
		.map(|at| 1.0 - ((time - at).abs() / 0.07).min(1.0))
		.fold(0.0, f32::max)
}

fn frame(time: f32, width: u32, height: u32) -> String {
	let (mut pose, x) = performance(time);
	pose.blink = blink(time);
	let mut c = Svg::new();
	park::draw(&mut c);
	park::wormsley(&mut c, pt(2010.0, 1730.0), 1.0);
	c.begin(&format!("translate({x:.1} {GROUND:.1}) scale({SCALE})"));
	stinky::draw_shadow(&mut c, &pose, 0.0);
	stinky::draw(&mut c, &pose);
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
