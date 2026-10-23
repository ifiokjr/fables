//! Authored puppets: characters drawn from hand-made part files, on the same rig as everyone.
//!
//! The parametric puppet in [`figure`](super::figure) builds every character from one template.
//! An authored puppet is drawn the way TV cut-out characters are: every part (head, hair,
//! torso, sleeves, hands, the mouth chart) is a drawing from the character's part file and the
//! shared kit (see [`parts`](super::parts)), hung on the bones of [`rig`](super::rig).
//!
//! It takes the same [`FigureState`] as the parametric puppet, so stories, staging, lip-sync
//! and video drive it unchanged:
//!
//! - `facing` becomes a turn between the front and three-quarter drawings. Turning to the page
//!   left mirrors the puppet.
//! - Each of the 15 moods is a face: brows, lids or closed eyes, pupils, a mouth and a tilt of
//!   the head.
//! - Hands go where the pose puts them. An open hand hanging at the side is drawn relaxed.
//! - Visemes pick from the mouth chart, curled up or down by the mood.
//!
//! Face features are drawn once, front-on, and wrapped round the head with a cylinder warp, so
//! the far eye narrows and the mouth slides towards the cheek as the head turns.

use std::f32::consts::FRAC_PI_2;
use std::path::Path as FilePath;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use anyhow::ensure;

use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::figure::Extra;
use super::figure::FigureState;
use super::figure::HandShape;
use super::figure::Look;
use super::figure::Mood;
use super::figure::Pattern;
use super::figure::Prop;
use super::figure::Teeth;
use super::figure::Viseme;
use super::fx;
use super::lerp;
use super::noise;
use super::palette;
use super::parts::Level;
use super::parts::Parts;
use super::pt;
use super::rig;
use super::rig::Gait;
use super::rig::Legs;
use super::rig::Limbs;
use super::shape::Shape;

/// Bone lengths of an authored kid, in figure units with the soles at y = 0. About 500 units
/// tall with the hair.
pub const KID_LIMBS: Limbs = Limbs {
	pelvis_y: -126.0,
	ankle_y: -24.0,
	hip_spread: 21.0,
	thigh: 52.0,
	shin: 52.0,
	upper_arm: 50.0,
	forearm: 46.0,
};
/// Where the head hinges on the torso, and where the head's own origin sits above it.
pub const NECK: Pt = pt(0.0, -226.0);
pub const HEAD_ABOVE_NECK: f32 = 96.0;
pub const SHOULDER_Y: f32 = -206.0;
/// How far a three-quarter view turns the face, and the radius of the face cylinder.
const FACE_TURN: f32 = 0.42;
const FACE_RADIUS: f32 = 114.0;
const BODY_RADIUS: f32 = 64.0;
/// The `facing` the stage gives a figure turned towards the middle of the page. An authored
/// puppet shows its full three-quarter drawing there.
const STAGED_FACING: f32 = 0.35;
/// How far the face slides sideways per unit of `facing`, so hands that reach for the nose or
/// mouth find them.
pub const FACE_SLIDE: f32 = 133.0;
/// Line weights in figure units. Silhouettes are heavier than interior detail, the way an
/// inker presses harder on the outside edge.
const LINE: f32 = 4.2;
const DETAIL: f32 = 2.6;
/// Light comes from the top left; shadows fall bottom right.
const LIGHT: Pt = pt(-9.0, -8.0);

/// A character's authored puppet: their part file over the shared kit, read and checked once.
#[derive(Clone, Debug)]
pub struct Authored {
	pub file: PathBuf,
	pub parts: Arc<Parts>,
}

impl Authored {
	/// Reads a part file and its kit, refusing parts that can't draw a figure.
	pub fn load(file: &FilePath) -> Result<Authored> {
		let parts = Parts::load_puppet(file)?;
		let errors: Vec<String> = parts
			.check()
			.iter()
			.filter(|finding| finding.level == Level::Error)
			.map(ToString::to_string)
			.collect();
		ensure!(
			errors.is_empty(),
			"{} can't draw a puppet yet:\n  {}",
			file.display(),
			errors.join("\n  ")
		);

		Ok(Authored {
			file: file.to_path_buf(),
			parts: Arc::new(parts),
		})
	}

	/// A part at turn `t` (0 front-on, 1 three-quarters). Loading checked that every part a
	/// figure needs is there; an optional part that is missing draws nothing.
	fn part(&self, name: &str, t: f32) -> Option<Shape> {
		self.parts.turn(name).map(|turn| turn.at(t))
	}

	/// A part every figure has (loading refused files without it).
	fn required(&self, name: &str, t: f32) -> Shape {
		self.part(name, t)
			.unwrap_or_else(|| unreachable!("{} has no `{name}`", self.file.display()))
	}

	/// One of a turning part's two drawings, picked rather than blended.
	fn view(&self, name: &str, three_quarter: bool) -> Option<Shape> {
		let view = if three_quarter { "3q" } else { "front" };
		self.parts.get(&format!("{name}.{view}")).cloned()
	}

	/// Every copy of a numbered part at turn `t`.
	fn copies(&self, name: &str, t: f32) -> Vec<Shape> {
		self.parts
			.numbered(name)
			.iter()
			.map(|turn| turn.at(t))
			.collect()
	}
}

/// A character's inks: their cast colours plus the kit's shared ones.
#[derive(Clone, Copy, Debug)]
struct Inks {
	line: Color,
	skin: Color,
	nose: Color,
	freckle: Color,
	blush: Color,
	hair: Color,
	hair_light: Color,
	top: Color,
	stripe: Color,
	bottom: Color,
	shoes: Color,
	pupil: Color,
	teeth: Color,
}

const BLUSH: Color = Color(0xEE_6A_5A);
const EYE: Color = Color(0xFF_FC_F5);
const MAW: Color = Color(0x6A_20_34);
const TONGUE: Color = Color(0xE9_74_7C);
const SOCK: Color = Color(0xF8_F1_E4);
const SOLE: Color = Color(0xF2_E4_C8);
const CLEAN_TEETH: Color = Color(0xFF_F5_DA);
const JAM: Color = Color(0xB8_32_3A);
const CREAM: Color = Color(0xF6_EA_CF);
const BRUSH: Color = Color(0x4F_B3_A9);
const BRISTLES: Color = Color(0xFF_FC_F5);
const PASTE: Color = Color(0x9B_E3_CF);
const KRAFT: Color = Color(0xC9_9A_6B);
const STRING: Color = Color(0xC8_4A_3D);
const LENS: Color = Color(0xE8_F4_F8);
const BOGEY: Color = Color(0x8D_BF_4A);
const CAP: Color = Color(0xC9_47_3D);
const BADGE: Color = Color(0xF2_C2_4B);
const SATCHEL: Color = Color(0xA8_5A_3C);
const APRON: Color = Color(0xF8_F1_E4);
const MUD: Color = Color(0x6E_4A_30);

/// Cheeks: a warm blush on warm skin. Pink over teal or blue skin turns grey (they are
/// opposite colours), so cool skin gets a soft plum shading instead.
fn blush(skin: Color) -> Color {
	let (r, _, b) = skin.rgb();

	if r >= b {
		BLUSH
	} else {
		skin.mix(Color(0x8E_4A_7A), 0.35)
	}
}

impl Inks {
	fn of(look: &Look, teeth: Teeth) -> Inks {
		Inks {
			line: Color(0x2B_1D_2E),
			skin: look.skin,
			nose: look.skin.mix(Color(0xE0_60_50), 0.18),
			freckle: look.skin.mix(Color(0x8A_3A_20), 0.45),
			blush: blush(look.skin),
			hair: look.hair_color,
			hair_light: look.hair_color.lighten(0.25),
			top: look.top,
			stripe: look.trim,
			bottom: look.bottom,
			shoes: look.shoes,
			pupil: look.eyes.mix(Color(0x1E_12_18), 0.8),
			teeth: match teeth {
				Teeth::Clean | Teeth::Sparkly => CLEAN_TEETH,
				Teeth::Grotty => palette::GROT,
			},
		}
	}
}

#[derive(Clone, Copy, Debug)]
struct Brow {
	/// Upward shift in head units.
	raise: f32,
	/// Degrees; positive drops the inner end (cross), negative lifts it (soft, worried).
	tilt: f32,
}

#[derive(Clone, Copy, Debug)]
enum Eye {
	Open {
		/// How far the upper lid covers the eye, 0 (wide) to 1 (shut).
		upper: f32,
		/// How far the lower lid rises (cheeks pushing up in a smile or a squint).
		lower: f32,
		/// Degrees; positive drops the inner corner of the upper lid.
		tilt: f32,
	},
	/// Shut in a happy arch (laughing, proud).
	Smiling,
	/// Knocked out: a cross.
	Crossed,
	/// Seeing stars: a spiral.
	Spiral,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MouthShape {
	Smile,
	Grin,
	Laugh,
	/// A closed line, curled by `curl` (+1 up into a smile, -1 down into a frown).
	Line,
	/// A closed pout pushed to one side.
	Pout,
	/// Teeth gritted in a straight, tense mouth.
	Clench,
	/// Open in a round "oh!".
	Round,
	/// Tongue out.
	Bleh,
	/// A wobbling line.
	Wobble,
}

#[derive(Clone, Copy, Debug)]
struct Face {
	/// `[page-left, page-right]` in the authored (facing right) drawing.
	brows: [Brow; 2],
	eyes: [Eye; 2],
	/// Pupil size; shock shrinks them.
	pupil: f32,
	/// Where the pupils look up (-) or down (+), in eye units.
	gaze_y: f32,
	mouth: MouthShape,
	/// How much the mouth curls up (+1) or down (-1), talking or not.
	curl: f32,
	/// Scrunched nose lines.
	wrinkle: bool,
	/// Head tilt in degrees on top of the state's (negative lifts the chin when facing right).
	head_tilt: f32,
}

fn face(mood: Mood) -> Face {
	let brow = |raise, tilt| Brow { raise, tilt };
	let open = |upper, lower, tilt| Eye::Open { upper, lower, tilt };
	let both = |eye: Eye| [eye, eye];
	let neutral = Face {
		brows: [brow(0.0, 0.0); 2],
		eyes: both(open(0.04, 0.1, 0.0)),
		pupil: 1.0,
		gaze_y: 0.0,
		mouth: MouthShape::Smile,
		curl: 1.0,
		wrinkle: false,
		head_tilt: 0.0,
	};

	match mood {
		Mood::Happy => {
			Face {
				brows: [brow(5.0, -8.0); 2],
				head_tilt: 4.0,
				..neutral
			}
		}
		Mood::Grin => {
			Face {
				brows: [brow(-1.0, 10.0), brow(10.0, -12.0)],
				eyes: [open(0.22, 0.36, 0.0), open(0.14, 0.34, -4.0)],
				gaze_y: 0.1,
				mouth: MouthShape::Grin,
				head_tilt: 6.0,
				..neutral
			}
		}
		Mood::Laugh => {
			Face {
				brows: [brow(8.0, -10.0); 2],
				eyes: both(Eye::Smiling),
				mouth: MouthShape::Laugh,
				head_tilt: -7.0,
				..neutral
			}
		}
		Mood::Sad => {
			Face {
				brows: [brow(3.0, -22.0); 2],
				eyes: both(open(0.38, 0.0, -12.0)),
				gaze_y: 0.6,
				mouth: MouthShape::Line,
				curl: -2.6,
				head_tilt: 7.0,
				..neutral
			}
		}
		Mood::Grumpy => {
			Face {
				brows: [brow(-4.0, 14.0); 2],
				eyes: both(open(0.46, 0.06, 8.0)),
				gaze_y: 0.2,
				mouth: MouthShape::Pout,
				curl: -0.8,
				..neutral
			}
		}
		Mood::Cross => {
			Face {
				brows: [brow(-8.0, 26.0); 2],
				eyes: both(open(0.3, 0.16, 20.0)),
				mouth: MouthShape::Clench,
				curl: -1.0,
				wrinkle: true,
				head_tilt: 3.0,
				..neutral
			}
		}
		Mood::Defiant => {
			Face {
				brows: [brow(-6.0, 20.0); 2],
				eyes: both(open(0.52, 0.08, 14.0)),
				gaze_y: -0.2,
				mouth: MouthShape::Pout,
				curl: -0.6,
				head_tilt: -9.0,
				..neutral
			}
		}
		Mood::Shocked => {
			Face {
				brows: [brow(15.0, -6.0); 2],
				eyes: both(open(0.0, 0.0, 0.0)),
				pupil: 0.6,
				mouth: MouthShape::Round,
				curl: 0.0,
				head_tilt: -3.0,
				..neutral
			}
		}
		Mood::Disgusted => {
			Face {
				brows: [brow(9.0, -14.0), brow(-8.0, 20.0)],
				eyes: [open(0.22, 0.12, -4.0), open(0.5, 0.42, 12.0)],
				gaze_y: 0.1,
				mouth: MouthShape::Bleh,
				curl: -1.0,
				wrinkle: true,
				head_tilt: -6.0,
				..neutral
			}
		}
		Mood::Fainted => {
			Face {
				brows: [brow(4.0, -6.0); 2],
				eyes: both(Eye::Crossed),
				mouth: MouthShape::Bleh,
				curl: 0.0,
				..neutral
			}
		}
		Mood::Dizzy => {
			Face {
				brows: [brow(6.0, -12.0), brow(-2.0, 8.0)],
				eyes: both(Eye::Spiral),
				mouth: MouthShape::Wobble,
				curl: 0.0,
				head_tilt: 8.0,
				..neutral
			}
		}
		Mood::Sleepy => {
			Face {
				brows: [brow(-2.0, -6.0); 2],
				eyes: both(open(0.78, 0.0, -6.0)),
				gaze_y: 0.3,
				mouth: MouthShape::Line,
				curl: 0.0,
				head_tilt: 9.0,
				..neutral
			}
		}
		Mood::Dreamy => {
			Face {
				brows: [brow(6.0, -10.0); 2],
				eyes: both(open(0.45, 0.12, -4.0)),
				gaze_y: -0.7,
				mouth: MouthShape::Line,
				curl: 1.3,
				head_tilt: -5.0,
				..neutral
			}
		}
		Mood::Worried => {
			Face {
				brows: [brow(9.0, -24.0); 2],
				eyes: both(open(0.08, 0.2, -10.0)),
				mouth: MouthShape::Clench,
				curl: -0.7,
				head_tilt: 4.0,
				..neutral
			}
		}
		Mood::Proud => {
			Face {
				brows: [brow(7.0, -6.0); 2],
				eyes: both(Eye::Smiling),
				mouth: MouthShape::Line,
				curl: 1.5,
				head_tilt: -8.0,
				..neutral
			}
		}
	}
}

/// How a hand is drawn: the pose's shape, with an open hand hanging at the side drawn relaxed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hand {
	Relaxed,
	Open,
	Fist,
	Point,
}

/// The state turned into the puppet's own terms: authored facing right, mirrored when the
/// figure faces left.
#[derive(Clone, Debug)]
struct Posed {
	/// 0 front-on to 1 three-quarters.
	t: f32,
	mirrored: bool,
	face: Face,
	viseme: Option<Viseme>,
	mouth_open: f32,
	/// Hand targets `(near, far)` at rest; the torso carries them.
	hands: (Pt, Pt),
	hand_shapes: (Hand, Hand),
	/// The near and far hands' extra wrist turn, in degrees.
	wrists: (f32, f32),
	legs: Legs,
	gait: Option<(Gait, f32)>,
	tilt: f32,
	head_tilt: f32,
	gaze: Pt,
	blink: f32,
	brow_lift: f32,
	/// Degrees loose hair swings by: follow-through after a move, a bounce in a walk, and a
	/// slow drift at rest.
	follow: f32,
	/// A prop, and whether the far hand (rather than the near one) holds it.
	prop: Option<(Prop, bool)>,
	teeth: Teeth,
	/// Turned three-quarters, the far arm is drawn behind the body and head: hanging down, or
	/// raised (a cheer) so the hand shows beside the head instead of across the face.
	far_arm_behind: bool,
}

impl Posed {
	fn of(state: &FigureState) -> Posed {
		let turn = (state.facing / STAGED_FACING).clamp(-1.0, 1.0);
		let mirrored = turn < 0.0;
		let flip = |p: Pt| if mirrored { pt(-p.x, p.y) } else { p };
		let sign = if mirrored { -1.0 } else { 1.0 };
		// Mirroring swaps which hand and foot is near.
		let (near, far) = if mirrored {
			(state.hands.1, state.hands.0)
		} else {
			state.hands
		};
		let (mut near_shape, mut far_shape) = if mirrored {
			(state.hand_shapes.1, state.hand_shapes.0)
		} else {
			state.hand_shapes
		};
		let t = turn.abs();
		let (mut near, mut far) = (flip(near), flip(far));

		// Turned three-quarters, a far hand held up and out (a wave) would cross the face, so
		// the near hand does it instead, the way an animator would stage it. A hand reaching for
		// the face (brushing, holding the nose) is meant to be there and stays.
		let face_x = t * FACE_SLIDE * STAGED_FACING;
		let swapped = t > 0.5
			&& far.y < SHOULDER_Y - 30.0
			&& far.x > 70.0
			&& (far.x - face_x).abs() >= 30.0
			&& near.y > SHOULDER_Y;

		if swapped {
			(near, far) = (pt(-far.x, far.y), pt(-near.x, near.y));
			(near_shape, far_shape) = (far_shape, near_shape);
		}

		let far_arm_behind =
			t > 0.5 && far.x > -10.0 && (far.y > SHOULDER_Y || (far.x - face_x).abs() >= 30.0);
		(near, far) = (
			clear_of_head(near, -1.0, face_x),
			clear_of_head(far, 1.0, face_x),
		);
		let (near_foot, far_foot) = if mirrored {
			(state.legs.feet.1, state.legs.feet.0)
		} else {
			state.legs.feet
		};
		let mut legs = Legs {
			feet: (flip(near_foot), flip(far_foot)),
			crouch: state.legs.crouch,
			bend: state.legs.bend * sign,
		};
		// In a three-quarter view the far foot steps back and up the page, so the feet stand on
		// a ground plane rather than a line.
		legs.feet.0 = legs.feet.0 + pt(4.0 * t, 3.0 * t);
		legs.feet.1 = legs.feet.1 + pt(6.0 * t, -9.0 * t);
		let hand = |shape: HandShape, target: Pt| {
			match shape {
				HandShape::Open if target.y > SHOULDER_Y + 50.0 => Hand::Relaxed,
				HandShape::Open => Hand::Open,
				HandShape::Fist => Hand::Fist,
				HandShape::Point => Hand::Point,
			}
		};
		let face = face(state.mood);

		Posed {
			t,
			mirrored,
			face,
			viseme: state.viseme,
			mouth_open: state.mouth_open,
			hands: (near, far),
			hand_shapes: (hand(near_shape, near), hand(far_shape, far)),
			wrists: (wrist(near_shape, near, -1.0), wrist(far_shape, far, 1.0)),
			legs,
			gait: state.gait,
			tilt: state.tilt * sign,
			head_tilt: (state.head_tilt + face.head_tilt) * sign,
			gaze: pt(
				state.gaze * sign
					+ if state.gaze.abs() < 0.05 {
						0.35 * t
					} else {
						0.0
					},
				face.gaze_y,
			),
			blink: state.blink,
			brow_lift: state.brow_lift,
			follow: {
				let bounce = state
					.gait
					.map_or(0.0, |(_, phase)| (phase * 2.0).sin() * 3.0);
				(state.sway * 10.0 + bounce + (state.time * 1.7).sin() * 0.8) * sign
			},
			// The prop is in the page-right hand: the far one unless the figure is mirrored, and
			// it moves with the hands if they swap.
			prop: state.prop.map(|prop| (prop, mirrored == swapped)),
			teeth: state.teeth,
			far_arm_behind,
		}
	}
}

/// A hand held up beside the head (a wave, a cheer) moves out past it, so it isn't mistaken for
/// a hand scratching an ear. Hands meant for the face (nose, mouth), which sit near the face's
/// middle at `face_x`, stay put.
fn clear_of_head(target: Pt, side: f32, face_x: f32) -> Pt {
	let out = target.x * side;
	let at_face = (target.x - face_x).abs() < 30.0;

	if target.y < SHOULDER_Y - 30.0 && !at_face && (45.0..115.0).contains(&out) {
		pt(side * 115.0, target.y + 40.0)
	} else {
		target
	}
}

/// A little turn of the wrist so hands sit naturally: fists on hips turn their knuckles
/// forward, and hands hanging at the sides tip in towards the legs.
fn wrist(shape: HandShape, target: Pt, side: f32) -> f32 {
	let outside = target.x * side > 50.0;
	let low = target.y > SHOULDER_Y + 40.0;

	match shape {
		HandShape::Fist if outside && low => -70.0 * side,
		HandShape::Open if low => -6.0 * side,
		_ => 0.0,
	}
}

/// The rig solved for one moment.
struct Solved {
	t: f32,
	limbs: Limbs,
	legs: Legs,
	swing: (Pt, Pt),
	drop: f32,
	pivot: Pt,
}

impl Solved {
	fn new(posed: &Posed, limbs: Limbs) -> Solved {
		let (legs, swing) = match posed.gait {
			Some((gait, phase)) => limbs.cycle(gait, phase, 1.0, posed.legs),
			None => (posed.legs, (pt(0.0, 0.0), pt(0.0, 0.0))),
		};

		Solved {
			t: posed.t,
			limbs,
			legs,
			swing,
			drop: legs.crouch * limbs.leg_length(),
			pivot: pt(0.0, limbs.pelvis_y),
		}
	}

	fn torso(&self, rest: Pt) -> Pt {
		(rest - self.pivot).rotate(self.legs.bend) + self.pivot + pt(0.0, self.drop)
	}

	fn torso_transform(&self) -> String {
		format!(
			"translate(0 {:.2}) rotate({:.2} {:.1} {:.1})",
			self.drop, self.legs.bend, self.pivot.x, self.pivot.y
		)
	}

	/// `side` is -1 for the near (page-left) limb and 1 for the far one.
	fn hip(&self, side: f32) -> Pt {
		let x = lerp(
			side * self.limbs.hip_spread,
			if side < 0.0 { -14.0 } else { 20.0 },
			self.t,
		);
		self.torso(pt(x, self.limbs.pelvis_y))
	}

	fn shoulder(&self, side: f32) -> Pt {
		let x = lerp(side * 40.0, if side < 0.0 { -36.0 } else { 27.0 }, self.t);
		self.torso(pt(x, SHOULDER_Y))
	}

	fn leg(&self, side: f32) -> (Pt, Pt, Pt) {
		let hip = self.hip(side);
		let foot = if side < 0.0 {
			self.legs.feet.0
		} else {
			self.legs.feet.1
		};
		// Knees bend outwards from the front and forwards when turned.
		let pole = pt(lerp(side, 1.2, self.t), -0.25).norm();
		let (knee, ankle) = rig::two_bone(hip, foot, self.limbs.thigh, self.limbs.shin, pole);
		(hip, knee, ankle)
	}

	fn arm(&self, posed: &Posed, side: f32) -> (Pt, Pt, Pt) {
		let shoulder = self.shoulder(side);
		let (target, swing) = if side < 0.0 {
			(posed.hands.0, self.swing.0)
		} else {
			(posed.hands.1, self.swing.1)
		};
		let (elbow, wrist) = rig::two_bone(
			shoulder,
			self.torso(target) + swing,
			self.limbs.upper_arm,
			self.limbs.forearm,
			pt(side * 0.6, 0.8),
		);
		(shoulder, elbow, wrist)
	}
}

/// The whole-figure transform: mirroring, lying down, leaning, squash and breath.
fn figure_transform(look: &Look, posed: &Posed, state: &FigureState, head_r: f32) -> String {
	let breathe =
		1.0 + (state.time * 2.4 + look.seed as f32 % 7.0).sin() * 0.012 * (1.0 - state.lie);
	let squash = state.squash * breathe;
	// Squash and stretch keep their volume: what flattens must widen.
	let stretch = 1.0 / squash.max(0.2).sqrt();

	format!(
		"scale({} 1) translate(0 {:.1}) rotate({:.2}) rotate({:.2}) scale({stretch:.4} \
		 {squash:.4})",
		if posed.mirrored { -1 } else { 1 },
		-state.lie * head_r * 0.95,
		-state.lie * 90.0,
		posed.tilt,
	)
}

/// Draws an authored figure with its soles at the current origin.
pub fn draw(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState) {
	let frame = look.frame();
	let posed = Posed::of(state);
	let solved = Solved::new(&posed, frame.limbs());
	let inks = Inks::of(look, state.teeth);
	let t = solved.t;

	svg.begin(&figure_transform(look, &posed, state, frame.head_r));

	// A far hand holding something keeps it in front, where it can be seen.
	let far_holds = posed.prop.is_some_and(|(_, in_far)| in_far);
	let far_arm_behind = posed.far_arm_behind && !far_holds;

	// What each hand holds, and where the mouth is (a toothbrush near it is brushing).
	let mouth = mouth_on_body(&solved, &posed);
	let held = |side: f32| {
		posed
			.prop
			.filter(|&(_, in_far)| in_far == (side > 0.0))
			.map(|(prop, _)| {
				Held {
					prop,
					mouth,
					time: state.time,
				}
			})
	};
	let arm = Arm {
		puppet,
		look,
		inks: &inks,
		posed: &posed,
		solved: &solved,
	};

	if far_arm_behind {
		arm.draw(svg, 1.0, true, held(1.0));
	}

	for side in [1.0, -1.0] {
		draw_leg(svg, puppet, &inks, &posed, &solved, side);
	}

	svg.begin(&solved.torso_transform());
	draw_torso(svg, puppet, look, &inks, t);
	svg.begin(&format!(
		"translate({:.1} {:.1}) rotate({:.2}) translate(0 {:.1})",
		NECK.x, NECK.y, posed.head_tilt, -HEAD_ABOVE_NECK
	));
	draw_head(svg, puppet, look, &inks, &posed, state.time);
	svg.end();
	svg.end();

	// Arms that fold across the body draw the lower arm last so it sits on top.
	let order: [f32; 2] = if posed.hands.0.x > 0.0 {
		[1.0, -1.0]
	} else {
		[-1.0, 1.0]
	};

	for side in order {
		if !(side > 0.0 && far_arm_behind) {
			arm.draw(svg, side, side > 0.0 && t > 0.5, held(side));
		}
	}

	if matches!(state.mood, Mood::Fainted | Mood::Dizzy) {
		let above = solved.torso(frame.head + pt(0.0, -frame.head_r * 1.2));
		fx::dizzy_stars(svg, above, frame.head_r, state.time);
	}

	svg.end();
}

/// A soft contact shadow under the feet, so the figure stands on the ground.
pub fn shadow(svg: &mut Svg, state: &FigureState, lift: f32) {
	let fade = (1.0 - lift / 120.0).clamp(0.35, 1.0);
	let turn = (state.facing / STAGED_FACING).clamp(-1.0, 1.0);
	let spread = 1.0 + state.lie * 1.8;
	svg.fill_alpha(
		&Path::ellipse(
			pt(turn * 6.0 - state.lie * 120.0, -2.0),
			70.0 * fade * spread,
			12.0 * fade,
		),
		palette::INK,
		0.18 * fade,
	);
}

/// Draws the rig's bones over an authored figure, for rig checks.
pub fn draw_bones(svg: &mut Svg, look: &Look, state: &FigureState) {
	let frame = look.frame();
	let posed = Posed::of(state);
	let solved = Solved::new(&posed, frame.limbs());
	let pelvis = solved.torso(solved.pivot);
	let chest = solved.torso(pt(0.0, SHOULDER_Y));
	let neck = solved.torso(NECK);
	let head = solved.torso(NECK + pt(0.0, -HEAD_ABOVE_NECK).rotate(posed.head_tilt));
	let mut bones = vec![(pelvis, chest), (chest, neck), (neck, head)];
	let mut joints = vec![pelvis, chest, neck, head];

	for side in [-1.0, 1.0] {
		let (hip, knee, ankle) = solved.leg(side);
		let (shoulder, elbow, wrist) = solved.arm(&posed, side);
		bones.extend([
			(pelvis, hip),
			(hip, knee),
			(knee, ankle),
			(chest, shoulder),
			(shoulder, elbow),
			(elbow, wrist),
		]);
		joints.extend([hip, knee, ankle, shoulder, elbow, wrist]);
	}

	svg.begin(&figure_transform(look, &posed, state, frame.head_r));

	for (from, to) in bones {
		svg.stroke(&Path::new().move_to(from).line_to(to), palette::TOMATO, 3.5);
	}

	for point in joints {
		svg.brushed(
			&Path::circle(point, 5.0),
			palette::SUNFLOWER,
			palette::INK,
			1.6,
		);
	}

	svg.end();
}

/// Starts a group whose +y axis runs along the bone from `from` to `to`; returns the bone's
/// angle in degrees.
fn begin_bone(svg: &mut Svg, from: Pt, to: Pt) -> f32 {
	let d = to - from;
	let angle = (-d.x).atan2(d.y).to_degrees();
	svg.begin(&format!(
		"translate({:.2} {:.2}) rotate({angle:.2})",
		from.x, from.y
	));
	angle
}

/// A tapered tube with round ends, `w0` wide at the joint and `w1` at the far end.
fn capsule(len: f32, w0: f32, w1: f32) -> Shape {
	let (a, b) = (w0 / 2.0, w1 / 2.0);
	let p = |x: f32, y: f32| pt(x, y);
	let points = vec![
		p(-a, 0.0),
		p(-a, len * 0.33),
		p(-b, len * 0.66),
		p(-b, len),
		p(-b, len + b * 1.3),
		p(b, len + b * 1.3),
		p(b, len),
		p(b, len * 0.66),
		p(a, len * 0.33),
		p(a, 0.0),
		p(a, -a * 1.3),
		p(-a, -a * 1.3),
		p(-a, 0.0),
	];
	Shape::from_points(points, true).unwrap_or_else(|| unreachable!("four whole curves"))
}

/// Pushes a colour back for limbs on the far side of the body.
fn far(color: Color, is_far: bool) -> Color {
	if is_far {
		color.mix(Color(0x6E_5A_7E), 0.16)
	} else {
		color
	}
}

/// The cylinder warp that turns flat front-on drawings round a body of `radius`.
fn wrap(p: Pt, t: f32, radius: f32) -> Pt {
	let angle = (p.x / radius).clamp(-1.0, 1.0).asin() + t * FACE_TURN;
	pt(radius * angle.clamp(-FRAC_PI_2, FRAC_PI_2).sin(), p.y)
}

fn draw_leg(
	svg: &mut Svg,
	puppet: &Authored,
	inks: &Inks,
	posed: &Posed,
	solved: &Solved,
	side: f32,
) {
	let t = solved.t;
	let ink = inks.line;
	let is_far = side > 0.0 && t > 0.3;
	let (hip, knee, ankle) = solved.leg(side);
	let shin = solved.limbs.shin;

	// The leg is one tapered shape bent through the knee, so there's no seam at the joint.
	let leg = Shape::taper(
		&[hip, hip.lerp(knee, 0.5), knee, knee.lerp(ankle, 0.5), ankle],
		15.5,
		12.5,
	);
	svg.brushed_shaded(&leg, far(inks.skin, is_far), LIGHT, ink, LINE);

	// Socks, unless trousers come down over them.
	let trouser = puppet.part("legwear-shin", t);

	if trouser.is_none() {
		let angle = begin_bone(svg, knee, ankle);
		let sock = capsule(18.0, 15.5, 15.0).shift(pt(0.0, shin - 16.0));
		svg.brushed_shaded(&sock, far(SOCK, is_far), LIGHT.rotate(-angle), ink, LINE);
		svg.begin_clip(&sock.path());
		svg.fill(
			&Path::rect(-12.0, shin - 14.0, 24.0, 4.5),
			far(inks.top, is_far),
		);
		svg.end();
		svg.brush_line(&sock.path(), ink, LINE);
		svg.end();
	}

	// The shoe stays level with the ground and tips its toe as the foot lifts in a stride.
	let lift = (solved.limbs.ankle_y - ankle.y).max(0.0);
	let toe = if posed.gait.is_some() {
		-lift * 0.5
	} else {
		0.0
	};
	let three_quarter = t > 0.5;
	let mirror = side < 0.0 && !three_quarter;

	if let Some(shoe) = puppet.view("shoe", three_quarter) {
		svg.begin(&format!(
			"translate({:.2} {:.2}) rotate({toe:.2}) scale({} 1)",
			ankle.x,
			ankle.y,
			if mirror { -1 } else { 1 }
		));
		svg.brush_fill(&shoe.path(), far(inks.shoes, is_far), ink, LINE);
		svg.cel_shade(&shoe, pt(-6.0, -5.0));
		svg.begin_clip(&shoe.path());
		svg.fill(&Path::rect(-30.0, 17.0, 80.0, 12.0), far(SOLE, is_far));
		svg.brush_line(
			&Path::new().move_to(pt(-30.0, 17.0)).line_to(pt(50.0, 17.0)),
			ink,
			DETAIL,
		);
		svg.end();

		if let Some(lace) = puppet.view("shoe-lace", three_quarter) {
			svg.brush_line(&lace.path(), far(SOLE, is_far), 3.4);
		}

		svg.brush_line(&shoe.path(), ink, LINE);
		svg.end();
	}

	// Trousers ride on the shin, over the top of the shoe.
	if let Some(trouser) = trouser {
		let angle = begin_bone(svg, knee, ankle);
		svg.brushed_shaded(
			&trouser,
			far(inks.bottom, is_far),
			LIGHT.rotate(-angle),
			ink,
			LINE,
		);

		if let Some(cuff) = puppet.part("legwear-shin-cuff", t) {
			svg.brush_line(&cuff.path(), far(inks.bottom, is_far).darken(0.25), DETAIL);
		}

		svg.end();
	}

	// The legwear's leg rides on the thigh (a skirt has none).
	if let Some(cover) = puppet.part("legwear-leg", t) {
		let angle = begin_bone(svg, hip, knee);
		svg.brushed_shaded(
			&cover,
			far(inks.bottom, is_far),
			LIGHT.rotate(-angle),
			ink,
			LINE,
		);

		if let Some(hem) = puppet.part("legwear-leg-hem", t) {
			svg.brush_line(&hem.path(), far(inks.bottom, is_far).darken(0.25), DETAIL);
		}

		svg.end();
	}
}

fn draw_torso(svg: &mut Svg, puppet: &Authored, look: &Look, inks: &Inks, t: f32) {
	let ink = inks.line;
	let body = |p: Pt| wrap(p, t, BODY_RADIUS);
	let legwear = puppet.required("legwear", t);
	svg.brushed_shaded(&legwear, inks.bottom, LIGHT, ink, LINE);

	if let Some(waist) = puppet.part("legwear-waist", t) {
		svg.brush_line(&waist.map(body).path(), inks.bottom.darken(0.25), DETAIL);
	}

	let torso = puppet.required("torso", t);
	let path = torso.path();
	svg.brush_fill(&path, inks.top, ink, LINE * 1.1);
	svg.begin_clip(&path);

	// Patterns wrap round the body with it: stripes are bands, dots and stars repeat the kit's
	// motifs.
	if look.pattern == Pattern::Dots {
		let dot = puppet.required("pattern-dot", t);

		for row in 0..6 {
			for column in -4..=4 {
				let offset = if row % 2 == 0 { 0.0 } else { 11.0 };
				let at = pt(column as f32 * 22.0 + offset, -222.0 + row as f32 * 20.0);
				svg.fill(&dot.shift(at).map(body).path(), inks.stripe);
			}
		}
	}

	if look.pattern == Pattern::Star {
		let star = puppet.required("pattern-star", t).shift(pt(0.0, -176.0));
		let star = star.map(body);
		svg.brushed(&star.path(), inks.stripe, ink, DETAIL);
	}

	if look.pattern == Pattern::Stripes {
		for y in [-200.0, -176.0, -152.0] {
			let band = Shape::from_points(
				vec![
					pt(-80.0, y),
					pt(-30.0, y + 6.0),
					pt(30.0, y + 6.0),
					pt(80.0, y),
					pt(80.0, y + 4.0),
					pt(80.0, y + 8.0),
					pt(80.0, y + 11.0),
					pt(30.0, y + 17.0),
					pt(-30.0, y + 17.0),
					pt(-80.0, y + 11.0),
					pt(-80.0, y + 8.0),
					pt(-80.0, y + 4.0),
					pt(-80.0, y),
				],
				true,
			)
			.unwrap_or_else(|| unreachable!("four whole curves"));
			svg.fill(&band.map(body).path(), inks.stripe);
		}
	}

	// A ribbed hem.
	svg.fill(
		&Path::rect(-80.0, -125.0, 160.0, 14.0),
		inks.top.darken(0.12),
	);

	// An apron over the top, inside its outline so the shadow falls across both.
	if look.has(Extra::Apron) {
		if let Some(apron) = puppet.part("apron", t) {
			svg.brushed(&apron.path(), APRON, ink, DETAIL);
		}

		if let Some(pocket) = puppet.part("apron-pocket", t) {
			svg.brushed(&pocket.map(body).path(), APRON.darken(0.06), ink, DETAIL);
		}
	}

	svg.end();
	svg.cel_shade(&torso, LIGHT);

	if let Some(hem) = puppet.part("torso-hem", t) {
		svg.brush_line(&hem.map(body).path(), ink, DETAIL);
	}

	svg.brush_line(&path, ink, LINE * 1.1);

	// The neck, then the collar over it.
	let neck = capsule(30.0, 30.0, 30.0).shift(pt(lerp(0.0, 6.0, t), -250.0));
	svg.brushed_shaded(&neck, inks.skin, LIGHT, ink, LINE);

	if let Some(collar) = puppet.part("collar", t) {
		svg.brushed_shaded(&collar, inks.stripe, LIGHT, ink, LINE);
	}

	// A satchel: the strap across the body, the bag at the hip.
	if look.has(Extra::Satchel) {
		if let Some(strap) = puppet.part("satchel-strap", t) {
			svg.brush_line(&strap.path(), ink, 11.0);
			svg.brush_line(&strap.path(), SATCHEL.darken(0.15), 7.0);
		}

		if let Some(bag) = puppet.part("satchel", t) {
			svg.brushed_shaded(&bag, SATCHEL, LIGHT, ink, LINE);
		}

		if let Some(flap) = puppet.part("satchel-flap", t) {
			svg.brushed_shaded(&flap, SATCHEL.darken(0.12), LIGHT, ink, LINE);
		}
	}
}

/// Something held, and what the hand needs to know to hold it.
#[derive(Clone, Copy, Debug)]
struct Held {
	prop: Prop,
	/// Where the mouth is: a toothbrush near it is brushing.
	mouth: Pt,
	time: f32,
}

/// Where the middle of the mouth is in figure units, after the head and body have moved.
fn mouth_on_body(solved: &Solved, posed: &Posed) -> Pt {
	let local = wrap(pt(0.0, 58.0), posed.t, FACE_RADIUS) + pt(0.0, -HEAD_ABOVE_NECK);
	solved.torso(NECK + local.rotate(posed.head_tilt))
}

/// Everything an arm is drawn from.
struct Arm<'a> {
	puppet: &'a Authored,
	look: &'a Look,
	inks: &'a Inks,
	posed: &'a Posed,
	solved: &'a Solved,
}

impl Arm<'_> {
	fn draw(&self, svg: &mut Svg, side: f32, is_far: bool, held: Option<Held>) {
		let (puppet, inks, posed, solved) = (self.puppet, self.inks, self.posed, self.solved);
		let t = solved.t;
		let ink = inks.line;
		let (shoulder, elbow, wrist) = solved.arm(posed, side);
		let (hand, turn) = if side < 0.0 {
			(posed.hand_shapes.0, posed.wrists.0)
		} else {
			(posed.hand_shapes.1, posed.wrists.1)
		};
		// Mud never washed off: the hands are caked and it creeps up the forearms.
		let (hand_skin, arm_skin) = if self.look.has(Extra::MuddyHands) {
			(inks.skin.mix(MUD, 0.92), inks.skin.mix(MUD, 0.4))
		} else {
			(inks.skin, inks.skin)
		};

		let angle = begin_bone(svg, elbow, wrist);
		let forearm = capsule(solved.limbs.forearm, 12.0, 10.5);
		svg.brushed_shaded(
			&forearm,
			far(arm_skin, is_far),
			LIGHT.rotate(-angle),
			ink,
			LINE,
		);

		// A long sleeve carries on down the forearm.
		if let Some(lower) = puppet.part("forearm-sleeve", t) {
			svg.brushed_shaded(
				&lower,
				far(inks.top, is_far),
				LIGHT.rotate(-angle),
				ink,
				LINE,
			);

			if let Some(cuff) = puppet.part("forearm-sleeve-cuff", t) {
				svg.brush_line(&cuff.path(), ink, DETAIL);
			}
		}

		svg.end();

		let angle = begin_bone(svg, shoulder, elbow);
		let sleeve = puppet.required("sleeve", t);
		let path = sleeve.path();
		svg.brush_fill(&path, far(inks.top, is_far), ink, LINE);
		svg.begin_clip(&path);

		match self.look.pattern {
			Pattern::Stripes => {
				for y in [6.0, 24.0] {
					svg.fill(&Path::rect(-20.0, y, 40.0, 10.0), far(inks.stripe, is_far));
				}
			}
			Pattern::Dots => {
				let dot = puppet.required("pattern-dot", t);

				for (x, y) in [(-6.0, 6.0), (7.0, 16.0), (-5.0, 27.0)] {
					svg.fill(&dot.shift(pt(x, y)).path(), far(inks.stripe, is_far));
				}
			}
			Pattern::Plain | Pattern::Star => {}
		}

		svg.fill(
			&Path::rect(-20.0, 38.0, 40.0, 16.0),
			far(inks.top.darken(0.12), is_far),
		);
		svg.end();
		svg.cel_shade(&sleeve, LIGHT.rotate(-angle));

		if let Some(cuff) = puppet.part("sleeve-cuff", t) {
			svg.brush_line(&cuff.path(), ink, DETAIL);
		}

		svg.brush_line(&path, ink, LINE);
		svg.end();

		// Held things sit in the fist, under the fingers.
		let along = (wrist - elbow).norm();

		if let Some(held) = held {
			draw_prop(svg, puppet, ink, held, wrist + along * 17.0, along, side);
		}

		// The thumb faces the body from the front and forwards when turned.
		let mirror = side < 0.0 || t > 0.5;
		let angle = (-along.x).atan2(along.y).to_degrees();
		svg.begin(&format!(
			"translate({:.2} {:.2}) rotate({:.2}) scale({} 1) scale(1.08)",
			wrist.x,
			wrist.y,
			angle + turn * if mirror { -1.0 } else { 1.0 },
			if mirror { -1 } else { 1 }
		));
		let light = LIGHT.rotate(-angle);
		let light = if mirror { pt(-light.x, light.y) } else { light };
		draw_hand(svg, puppet, hand, far(hand_skin, is_far), ink, light);
		svg.end();
	}
}

/// A prop from the kit, held in the fist at `fist`. Jars and parcels stay upright; a
/// toothbrush sticks out of the fist, or points its bristles into the mouth while brushing.
fn draw_prop(
	svg: &mut Svg,
	puppet: &Authored,
	ink: Color,
	held: Held,
	fist: Pt,
	along: Pt,
	side: f32,
) {
	let fixed = |name: &str| puppet.part(name, 0.0);
	let light = pt(-5.0, -5.0);

	match held.prop {
		Prop::Jam => {
			svg.begin(&format!("translate({:.2} {:.2})", fist.x, fist.y));
			let jar = puppet.required("prop-jam", 0.0);
			svg.brushed_shaded(&jar, JAM, light, ink, LINE);

			if let Some(label) = fixed("prop-jam-label") {
				svg.brushed(&label.path(), CREAM, ink, DETAIL);
			}

			if let Some(shine) = fixed("prop-jam-shine") {
				svg.stroke_alpha(&shine.path(), palette::WHITE, 4.0, 0.7);
			}

			if let Some(lid) = fixed("prop-jam-lid") {
				svg.brushed_shaded(&lid, CREAM, light, ink, LINE);
			}

			svg.end();
		}
		Prop::Parcel => {
			svg.begin(&format!("translate({:.2} {:.2})", fist.x, fist.y - 8.0));

			for (name, color) in [
				("prop-parcel-side", KRAFT.darken(0.15)),
				("prop-parcel-top", KRAFT.lighten(0.12)),
				("prop-parcel", KRAFT),
			] {
				if let Some(face) = fixed(name) {
					svg.brushed(&face.path(), color, ink, LINE);
				}
			}

			for string in puppet.copies("prop-parcel-string", 0.0) {
				svg.brush_line(&string.path(), STRING, 6.0);
			}

			svg.end();
		}
		Prop::Toothbrush => {
			let brushing = (fist - held.mouth).len() < 70.0;
			let (dir, scrub) = if brushing {
				let towards = if held.mouth.x < fist.x { -1.0 } else { 1.0 };
				(pt(towards, 0.0), (held.time * 22.0).sin() * 8.0)
			} else {
				// Sticking out of the fist, sideways from the forearm, away from the body.
				(along.perp() * -side, 0.0)
			};
			let angle = dir.y.atan2(dir.x).to_degrees();
			let flip = if dir.x < 0.0 { -1 } else { 1 };
			svg.begin(&format!(
				"translate({:.2} {:.2}) rotate({angle:.2}) scale(1 {flip})",
				fist.x + scrub,
				fist.y
			));
			let handle = puppet.required("prop-toothbrush", 0.0);
			svg.brushed_shaded(&handle, BRUSH, light, ink, LINE);

			if let Some(bristles) = fixed("prop-toothbrush-bristles") {
				svg.brushed(&bristles.path(), BRISTLES, ink, DETAIL);
			}

			for line in puppet.copies("prop-toothbrush-bristle-line", 0.0) {
				svg.brush_line(&line.path(), BRISTLES.darken(0.15), 1.6);
			}

			if let Some(paste) = fixed("prop-toothpaste") {
				svg.brushed(&paste.path(), PASTE, ink, DETAIL);
			}

			svg.end();

			// Foam round the mouth while brushing.
			if brushing {
				for i in 0..5 {
					let at = held.mouth
						+ pt(noise(77, i) * 34.0, 6.0 + noise(78, i) * 16.0)
						+ pt(0.0, (held.time * 6.0 + i as f32).sin() * 2.0);
					let r = 5.0 + noise(79, i).abs() * 5.0;
					svg.brushed(&Path::circle(at, r), BRISTLES, ink, 1.6);
				}
			}
		}
	}
}

fn draw_hand(svg: &mut Svg, puppet: &Authored, hand: Hand, skin: Color, ink: Color, light: Pt) {
	let name = match hand {
		Hand::Relaxed => "hand-relaxed",
		Hand::Open => "hand-open",
		Hand::Fist => "hand-fist",
		Hand::Point => "hand-point",
	};
	let shape = puppet.required(name, 0.0);
	let path = shape.path();
	svg.brush_fill(&path, skin, ink, LINE * 0.9);
	svg.cel_shade(&shape, light * 0.6);

	for line in puppet.copies(&format!("{name}-line"), 0.0) {
		svg.brush_line(&line.path(), ink, DETAIL * 0.9);
	}

	svg.brush_line(&path, ink, LINE * 0.9);
}

fn draw_head(svg: &mut Svg, puppet: &Authored, look: &Look, inks: &Inks, posed: &Posed, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let face = &posed.face;
	let head = puppet.required("head", t);
	let head_path = head.path();
	let hair = puppet.required("hair", t);
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);

	// The far ear tucks away as the head turns; the near ear slides onto the side of the head.
	let ear = puppet.required("ear", t);
	let curl = puppet.part("ear-curl", t);
	let draw_ear = |svg: &mut Svg, at: Pt, mirror: bool, squash: f32| {
		svg.begin(&format!(
			"translate({:.1} {:.1}) scale({:.3} 1)",
			at.x,
			at.y,
			if mirror { -squash } else { squash }
		));
		svg.brushed_shaded(&ear, inks.skin, pt(-5.0, -4.0), ink, LINE);

		if let Some(curl) = &curl {
			svg.brush_line(&curl.path(), ink, DETAIL);
		}

		svg.end();
	};

	// Ears sit just inside the widest point of the head, so wide heads keep their ears.
	let half_width = puppet
		.required("head", 0.0)
		.points()
		.iter()
		.step_by(3)
		.fold(0.0_f32, |widest, p| widest.max(p.x.abs()));
	let ear_at = half_width - 6.0;
	let spread = ear_at / 90.0;

	if t < 0.9 {
		draw_ear(
			svg,
			pt(lerp(ear_at, 96.0 * spread, t), 6.0),
			true,
			1.0 - t * 0.8,
		);
	}

	if t < 0.5 {
		draw_ear(svg, pt(-ear_at, 6.0), false, 1.0);
	}

	// Pigtails and bunches hang behind the head and swing from their roots.
	let ties = puppet.copies("hair-lock-tie", t);

	for (index, lock) in puppet.copies("hair-lock", t).iter().enumerate() {
		let root = lock.points()[0];
		svg.begin(&format!(
			"rotate({:.2} {:.1} {:.1})",
			posed.follow * 0.8,
			root.x,
			root.y
		));
		svg.brushed_shaded(lock, inks.hair, LIGHT, ink, LINE);

		if let Some(tie) = ties.get(index) {
			svg.brushed(&tie.path(), inks.stripe, ink, DETAIL);
		}

		svg.end();
	}

	svg.brush_fill(&head_path, inks.skin, ink, LINE * 1.15);
	svg.begin_clip(&head_path);
	// The hair casts a soft band of shadow on the forehead.
	svg.begin("translate(0 9)");
	svg.fill_alpha(&hair.path(), Color(0xC0_5A_3E), 0.35);
	svg.end();
	svg.end();
	svg.cel_shade(&head, LIGHT);

	// Cheeks and freckles, wrapped round the face. Anything turned past the edge is hidden.
	for side in [-1.0_f32, 1.0] {
		let blush = Shape::ellipse(pt(side * 58.0, 34.0), 17.0, 9.0).map(warp);
		svg.fill_alpha(&blush.path(), inks.blush, 0.3);

		if look.has(Extra::Freckles) {
			for (x, y) in [(52.0, 24.0), (62.0, 18.0), (64.0, 29.0)] {
				let at = pt(side * x, y);

				if (at.x / FACE_RADIUS).asin() + t * FACE_TURN < 1.25 {
					svg.fill(&Path::circle(warp(at), 2.6), inks.freckle);
				}
			}
		}
	}

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		draw_eye(svg, inks, posed, face.eyes[index], side, time);
	}

	draw_nose(svg, puppet, inks, t, face.wrinkle);

	if look.has(Extra::Bogey)
		&& let Some(bogey) = puppet.part("bogey", t)
	{
		let drip = bogey.map(warp).shift(pt(9.0 * t, 0.0));
		svg.brushed(&drip.path(), BOGEY, ink, DETAIL);
	}

	draw_mouth(svg, puppet, inks, posed, time);

	if look.has(Extra::Moustache)
		&& let Some(moustache) = puppet.part("moustache", t)
	{
		let moustache = moustache.map(warp);
		svg.brush_fill(&moustache.path(), inks.hair, ink, DETAIL * 1.2);
		svg.cel_shade(&moustache, pt(-3.0, -3.0));
		svg.brush_line(&moustache.path(), ink, DETAIL * 1.2);
	}

	if look.has(Extra::Lashes) {
		for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
			if matches!(face.eyes[index], Eye::Open { .. }) {
				let centre = pt(side * 34.0, -6.0);

				for lash in puppet.copies("lash", t) {
					let lash = lash.map(|p| pt(p.x * side, p.y) + centre).map(warp);
					svg.brush_line(&lash.path(), ink, 3.2);
				}
			}
		}
	}

	if look.has(Extra::Glasses) {
		if let Some(lens) = puppet.part("glasses-lens", t) {
			for side in [-1.0_f32, 1.0] {
				let ring = lens.shift(pt(side * 34.0, -6.0)).map(warp);
				svg.fill_alpha(&ring.path(), LENS, 0.3);
				svg.brush_line(&ring.path(), ink, 4.5);
			}
		}

		if let Some(bridge) = puppet.part("glasses-bridge", t) {
			svg.brush_line(&bridge.map(warp).path(), ink, 4.5);
		}
	}

	svg.brush_line(&head_path, ink, LINE * 1.15);

	// Hair over everything on the head, brows on top of the hair. Loose hair follows through
	// after a move.
	if let Some(tuft) = puppet.part("tuft", t) {
		// The tuft swings about its root.
		svg.begin(&format!(
			"rotate({:.2} {:.1} -100)",
			posed.follow,
			lerp(-12.0, -26.0, t)
		));
		svg.brushed_shaded(&tuft, inks.hair, LIGHT, ink, LINE);

		if let Some(shine) = puppet.part("tuft-shine", t) {
			svg.begin_clip(&tuft.path());
			svg.brush_line(&shine.path(), inks.hair_light, 7.0);
			svg.end();
		}

		svg.end();
	}

	svg.brushed_shaded(&hair, inks.hair, LIGHT, ink, LINE);

	let shines = puppet.copies("hair-shine", t);

	if !shines.is_empty() {
		svg.begin_clip(&hair.path());

		for shine in &shines {
			svg.brush_line(&shine.path(), inks.hair.lighten(0.3), 8.0);
		}

		svg.end();
	}

	for strand in puppet.copies("hair-strand", t) {
		svg.brush_line(&strand.path(), ink, DETAIL * 0.8);
	}

	// In three-quarter view the near ear sits on the side of the head, over the hair behind it,
	// unless the hair comes down over the ears (a bob).
	if t >= 0.5 && !covers_ears(&hair) {
		draw_ear(svg, pt(lerp(-74.0, -58.0, t) * spread, 8.0), false, 0.85);
	}

	// A bow in the hair swings a little with it.
	if look.has(Extra::Bow)
		&& let Some(bow) = puppet.part("bow", t)
	{
		let knot = pt(30.0, -100.0);
		svg.begin(&format!(
			"rotate({:.2} {:.1} {:.1})",
			posed.follow * 0.6,
			knot.x,
			knot.y
		));
		svg.brushed_shaded(&bow, inks.stripe, pt(-4.0, -4.0), ink, LINE);

		if let Some(tie) = puppet.part("bow-knot", t) {
			svg.brushed(&tie.path(), inks.stripe.darken(0.12), ink, DETAIL);
		}

		svg.end();
	}

	// A post cap sits over the hair.
	if look.has(Extra::PostCap) {
		if let Some(cap) = puppet.part("cap", t) {
			svg.brushed_shaded(&cap, CAP, LIGHT, ink, LINE);
		}

		if let Some(brim) = puppet.part("cap-brim", t) {
			svg.brushed_shaded(&brim, CAP.darken(0.18), LIGHT, ink, LINE);
		}

		if let Some(badge) = puppet.part("cap-badge", t) {
			svg.brushed(&badge.map(warp).path(), BADGE, ink, DETAIL);
		}
	}

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		let brow = face.brows[index];
		let centre = pt(side * 36.0, -48.0 - brow.raise - posed.brow_lift * 100.0);
		let shape = puppet
			.required("brow", t)
			.map(|p| pt(p.x * side, p.y).rotate(-side * brow.tilt) + centre)
			.map(warp);
		svg.brushed(&shape.path(), inks.hair, ink, 2.0);
	}
}

/// Whether hair hangs down past the ears (they sit about y = 8 in head units).
fn covers_ears(hair: &Shape) -> bool {
	hair.bounds().1.y > 28.0
}

fn draw_eye(svg: &mut Svg, inks: &Inks, posed: &Posed, eye: Eye, side: f32, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);
	let centre = pt(side * 34.0, -6.0);
	let (rx, ry) = (20.0, 27.0);
	let line = |points: &[Pt]| Shape::taper(points, 5.5, 5.5);

	let (upper, lower, tilt) = match eye {
		Eye::Open { upper, lower, tilt } => (upper, lower, tilt),
		Eye::Smiling => {
			// A shut eye curved up like a happy arch.
			let arch = [
				centre + pt(-17.0, 6.0),
				centre + pt(-8.0, -6.0),
				centre + pt(8.0, -6.0),
				centre + pt(17.0, 6.0),
			];
			svg.fill(&line(&arch).map(warp).path(), ink);
			return;
		}
		Eye::Crossed => {
			for (a, b) in [
				(pt(-13.0, -13.0), pt(13.0, 13.0)),
				(pt(13.0, -13.0), pt(-13.0, 13.0)),
			] {
				svg.fill(&line(&[centre + a, centre + b]).map(warp).path(), ink);
			}
			return;
		}
		Eye::Spiral => {
			let white = Shape::ellipse(centre, rx, ry).map(warp);
			svg.fill(&white.path(), EYE);
			let spin = time * 4.0 * side;
			let turns: Vec<Pt> = (0..28)
				.map(|i| {
					let a = i as f32 * 0.5 + spin;
					warp(centre + pt(a.cos(), a.sin()) * (2.0 + i as f32 * 0.6))
				})
				.collect();
			svg.brush_line(&Path::spline(&turns), ink, 3.0);
			svg.brush_line(&white.path(), ink, DETAIL);
			return;
		}
	};

	let white = Shape::ellipse(centre, rx, ry).map(warp);
	let white_path = white.path();
	let tilt = -side * tilt;

	svg.fill(&white_path, EYE);
	svg.begin_clip(&white_path);
	let look = centre + pt(posed.gaze.x * 8.0, posed.gaze.y * 7.0 + 2.0);
	let size = posed.face.pupil;
	svg.fill(
		&Shape::ellipse(look, 11.5 * size, 14.5 * size)
			.map(warp)
			.path(),
		inks.pupil,
	);
	svg.fill(
		&Path::circle(warp(look + pt(-3.5, -5.5) * size), 3.8 * size),
		EYE,
	);
	svg.fill(
		&Path::circle(warp(look + pt(3.5, 5.0) * size), 1.6 * size),
		EYE,
	);
	svg.end();
	svg.brush_line(&white_path, ink, DETAIL);

	// Lids paint over the eye's own outline, so the lash line becomes the top edge.
	let lid_clip = Shape::ellipse(centre, rx + 3.0, ry + 3.0).map(warp);
	svg.begin_clip(&lid_clip.path());

	// Upper lid: skin down to a curved lash line, tilted by the mood.
	let upper = upper.max(posed.blink);
	let edge_y = centre.y - ry + 2.0 * ry * upper;
	// The lash line arches with the eye when open and flattens as the lid comes down.
	let sag = -9.0 * (1.0 - upper);
	let rotate = |p: Pt| (p - pt(centre.x, edge_y)).rotate(tilt) + pt(centre.x, edge_y);
	let edge = |y: f32, bend: f32| {
		[
			pt(centre.x - 34.0, y),
			pt(centre.x - 14.0, y + bend),
			pt(centre.x + 14.0, y + bend),
			pt(centre.x + 34.0, y),
		]
	};
	let lid_edge = edge(edge_y, sag);
	let lid = closed_over(&lid_edge, -70.0).map(rotate).map(warp);
	svg.fill(&lid.path(), inks.skin);
	let lash = open_curve(&lid_edge).map(rotate).map(warp);
	svg.brush_line(&lash.path(), ink, 8.0);

	// Lower lid: the cheek pushing up, curved upwards in a smile or a squint.
	if lower > 0.01 && upper < 0.95 {
		let low_y = centre.y + ry - 2.0 * ry * lower;
		let cheek_edge = edge(low_y, -10.0 * lower);
		svg.fill(&closed_over(&cheek_edge, 50.0).map(warp).path(), inks.skin);
		svg.brush_line(&open_curve(&cheek_edge).map(warp).path(), ink, 4.4);
	}

	svg.end();
}

/// One curve through four points (start, two controls, end).
fn open_curve(points: &[Pt; 4]) -> Shape {
	Shape::from_points(points.to_vec(), false).unwrap_or_else(|| unreachable!("one whole curve"))
}

/// The region between a curve and a straight line `reach` above (negative) or below it.
fn closed_over(points: &[Pt; 4], reach: f32) -> Shape {
	let [a, _, _, d] = *points;
	let far_a = a + pt(0.0, reach);
	let far_d = d + pt(0.0, reach);
	let mut all = points.to_vec();
	all.extend([d.lerp(far_d, 1.0 / 3.0), d.lerp(far_d, 2.0 / 3.0), far_d]);
	all.extend([
		far_d.lerp(far_a, 1.0 / 3.0),
		far_d.lerp(far_a, 2.0 / 3.0),
		far_a,
	]);
	all.extend([far_a.lerp(a, 1.0 / 3.0), far_a.lerp(a, 2.0 / 3.0), a]);
	Shape::from_points(all, true).unwrap_or_else(|| unreachable!("four whole curves"))
}

fn draw_nose(svg: &mut Svg, puppet: &Authored, inks: &Inks, t: f32, wrinkle: bool) {
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);
	// The nose stands proud of the face, so it travels a little further than the cylinder.
	let poke = pt(9.0 * t, 0.0);
	let nose = puppet
		.required("nose", t)
		.shift(pt(0.0, 22.0))
		.map(warp)
		.shift(poke);
	svg.brush_fill(&nose.path(), inks.nose, inks.line, DETAIL * 1.1);
	svg.cel_shade(&nose, pt(-4.0, -4.0));
	svg.brush_line(&nose.path(), inks.line, DETAIL * 1.1);
	svg.fill_alpha(
		&Path::circle(warp(pt(-4.0, 18.0)) + poke, 2.6),
		Color(0xFF_FF_FF),
		0.7,
	);

	if wrinkle {
		for line in puppet.copies("nose-wrinkle", t) {
			svg.brush_line(&line.map(warp).path(), inks.line, DETAIL);
		}
	}
}

/// The mouth: a lip-sync shape while talking, otherwise the mood's own mouth, opened up when
/// the state asks for an open mouth.
fn draw_mouth(svg: &mut Svg, puppet: &Authored, inks: &Inks, posed: &Posed, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let face = &posed.face;
	let centre = pt(0.0, 58.0);
	let curl = face.curl;
	// Mouths keep the mood: corners curl up or down a little.
	let curled = |p: Pt| pt(p.x, p.y - curl * 6.0 * (p.x / 30.0).powi(2));
	let place = |shape: &Shape| shape.map(|p| wrap(p + centre, t, FACE_RADIUS));
	let mouth = |shape: &Shape| place(&shape.map(curled));
	let drawn = |name: &str| puppet.required(name, t);
	let ellipse = |c: Pt, rx: f32, ry: f32| place(&Shape::ellipse(c, rx, ry));
	let open = |svg: &mut Svg, outer: &Shape, teeth: Option<&Shape>, tongue: Option<&Shape>| {
		open_mouth(svg, inks, outer, teeth, tongue);
	};
	let closed = matches!(
		face.mouth,
		MouthShape::Line | MouthShape::Pout | MouthShape::Wobble
	);
	// A closed mood mouth opens when the scene asks for an open mouth (talking stills, shouts).
	let viseme = posed.viseme.or_else(|| {
		(closed && posed.mouth_open > 0.3).then_some(if posed.mouth_open > 0.65 {
			Viseme::Wide
		} else {
			Viseme::Open
		})
	});

	match (viseme, face.mouth) {
		(None, MouthShape::Smile) => {
			open(
				svg,
				&place(&drawn("mouth-smile")),
				None,
				Some(&ellipse(pt(4.0, 28.0), 17.0, 11.0)),
			);
			dimples(svg, ink, &place, 36.0, -6.0, -6.0);
		}
		(None, MouthShape::Grin) => {
			// A grin opens downwards when it talks or shouts, keeping its teeth on show.
			let drop = 1.0 + posed.mouth_open * 0.8;
			let open_up = |p: Pt| pt(p.x, if p.y > 0.0 { p.y * drop } else { p.y });
			open(
				svg,
				&place(&drawn("mouth-grin").map(open_up)),
				Some(&place(&drawn("mouth-grin-teeth"))),
				Some(&ellipse(pt(6.0, 27.0 * drop), 20.0, 9.0 * drop)),
			);

			if let Some(gap) = puppet.part("mouth-grin-gap", t) {
				svg.brush_line(&place(&gap).path(), inks.teeth.darken(0.25), 1.8);
			}

			dimples(svg, ink, &place, 48.0, -4.0, -16.0);
		}
		(None, MouthShape::Laugh) => {
			let big = |p: Pt| pt(p.x * 1.15, p.y * 1.2);
			open(
				svg,
				&place(&drawn("mouth-wide").map(big)),
				Some(&place(&teeth_band(-12.0, -1.0).map(big))),
				Some(&ellipse(pt(3.0, 40.0), 22.0, 12.0)),
			);
		}
		(None, MouthShape::Line) | (Some(Viseme::Rest), _) => {
			svg.fill(&mouth(&drawn("mouth-rest")).path(), ink);
		}
		(None, MouthShape::Pout) => {
			let by = |shape: Shape| shape.shift(pt(8.0, -2.0));
			svg.fill(&mouth(&by(drawn("mouth-pout"))).path(), ink);
			svg.brush_line(&mouth(&by(drawn("mouth-pout-lip"))).path(), ink, DETAIL);
		}
		(None, MouthShape::Clench) => {
			open(
				svg,
				&mouth(&drawn("mouth-teeth")),
				Some(&mouth(&teeth_band(-8.0, 14.0))),
				None,
			);
			let bite = Shape::from_points(
				vec![pt(-20.0, 4.0), pt(-7.0, 4.0), pt(7.0, 4.0), pt(20.0, 4.0)],
				false,
			)
			.unwrap_or_else(|| unreachable!("one whole curve"));
			svg.brush_line(&mouth(&bite).path(), inks.teeth.darken(0.3), 1.6);
		}
		(None, MouthShape::Round) => {
			open(
				svg,
				&ellipse(pt(0.0, 12.0), 15.0, 20.0),
				None,
				Some(&ellipse(pt(0.0, 28.0), 11.0, 7.0)),
			);
		}
		(None, MouthShape::Bleh) => {
			let tilt = |p: Pt| (p + pt(-4.0, 0.0)).rotate(-8.0);
			open(svg, &place(&drawn("mouth-bleh").map(tilt)), None, None);
			let tongue = place(&drawn("mouth-bleh-tongue").map(tilt));
			svg.brushed(&tongue.path(), TONGUE, ink, DETAIL * 1.2);

			if let Some(groove) = puppet.part("mouth-bleh-groove", t) {
				svg.brush_line(
					&place(&groove.map(tilt)).path(),
					TONGUE.darken(0.3),
					DETAIL * 0.8,
				);
			}
		}
		(None, MouthShape::Wobble) => {
			let wobble = |p: Pt| pt(p.x, p.y + (p.x * 0.25 + time * 6.0).sin() * 3.0);
			svg.fill(&mouth(&drawn("mouth-rest").map(wobble)).path(), ink);
		}
		(Some(Viseme::Mbp), _) => {
			svg.fill(&mouth(&drawn("mouth-mbp")).path(), ink);
			svg.brush_line(&mouth(&drawn("mouth-mbp-lip")).path(), ink, DETAIL);
		}
		(Some(Viseme::Teeth), _) => {
			open(
				svg,
				&mouth(&drawn("mouth-teeth")),
				Some(&mouth(&teeth_band(-6.0, 4.0))),
				None,
			);
		}
		(Some(Viseme::Open), _) => {
			open(
				svg,
				&mouth(&drawn("mouth-open")),
				Some(&mouth(&teeth_band(-8.0, 2.0))),
				Some(&ellipse(pt(2.0, 22.0), 15.0, 8.0)),
			);
		}
		(Some(Viseme::Wide), _) => {
			open(
				svg,
				&mouth(&drawn("mouth-wide")),
				Some(&mouth(&teeth_band(-10.0, 1.0))),
				Some(&ellipse(pt(3.0, 34.0), 19.0, 10.0)),
			);
		}
		(Some(Viseme::Round), _) => {
			open(
				svg,
				&ellipse(pt(0.0, 9.0), 15.0, 18.0),
				None,
				Some(&ellipse(pt(0.0, 24.0), 11.0, 7.0)),
			);
		}
		(Some(Viseme::Pucker), _) => open(svg, &ellipse(pt(0.0, 5.0), 8.0, 9.0), None, None),
		(Some(Viseme::Fv), _) => {
			open(
				svg,
				&mouth(&drawn("mouth-fv")),
				Some(&mouth(&teeth_band(-6.0, 6.0))),
				None,
			);
			svg.brush_line(&mouth(&drawn("mouth-fv-lip")).path(), ink, DETAIL);
		}
		(Some(Viseme::Tongue), _) => {
			open(
				svg,
				&mouth(&drawn("mouth-open")),
				Some(&mouth(&teeth_band(-8.0, 0.0))),
				Some(&ellipse(pt(0.0, 6.0), 11.0, 8.0)),
			);
		}
	}

	// Sparkly teeth glint whenever they show.
	let teeth_show = viseme.is_some_and(|v| !matches!(v, Viseme::Rest | Viseme::Mbp))
		|| matches!(
			face.mouth,
			MouthShape::Grin | MouthShape::Laugh | MouthShape::Clench
		);

	if posed.teeth == Teeth::Sparkly && teeth_show {
		let corner = wrap(centre + pt(30.0, -10.0), t, FACE_RADIUS);
		fx::sparkle(svg, corner, 16.0, palette::WHITE, time);
	}
}

/// The upper teeth as one simple band between `top` and `bottom` (mouth units).
fn teeth_band(top: f32, bottom: f32) -> Shape {
	let corners = [
		pt(-40.0, top),
		pt(40.0, top),
		pt(40.0, bottom),
		pt(-40.0, bottom),
	];
	let points = vec![
		corners[0],
		pt(-14.0, top + 6.0),
		pt(14.0, top + 6.0),
		corners[1],
		pt(40.0, top + 4.0),
		pt(40.0, bottom - 2.0),
		corners[2],
		pt(14.0, bottom + 6.0),
		pt(-14.0, bottom + 6.0),
		corners[3],
		pt(-40.0, bottom - 2.0),
		pt(-40.0, top + 4.0),
		corners[0],
	];
	Shape::from_points(points, true).unwrap_or_else(|| unreachable!("four whole curves"))
}

fn open_mouth(
	svg: &mut Svg,
	inks: &Inks,
	outer: &Shape,
	teeth: Option<&Shape>,
	tongue: Option<&Shape>,
) {
	let path = outer.path();
	svg.brush_fill(&path, MAW, inks.line, DETAIL * 1.4);
	svg.begin_clip(&path);

	if let Some(tongue) = tongue {
		svg.fill(&tongue.path(), TONGUE);
	}

	if let Some(teeth) = teeth {
		svg.fill(&teeth.path(), inks.teeth);
	}

	svg.end();
	svg.brush_line(&path, inks.line, DETAIL * 1.4);
}

/// Little tucks at the corners of a smile.
fn dimples(
	svg: &mut Svg,
	ink: Color,
	place: &dyn Fn(&Shape) -> Shape,
	half: f32,
	left_y: f32,
	right_y: f32,
) {
	for (x, y, side) in [(-half, left_y, -1.0_f32), (half, right_y, 1.0)] {
		let tuck = Shape::from_points(
			vec![
				pt(x - side * 4.0, y - 6.0),
				pt(x + side * 2.0, y - 4.0),
				pt(x + side * 4.0, y),
				pt(x + side * 2.0, y + 5.0),
			],
			false,
		)
		.unwrap_or_else(|| unreachable!("one whole curve"));
		svg.brush_line(&place(&tuck).path(), ink, DETAIL);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure;
	use crate::art::figure::Frame;
	use crate::art::figure::Pose;
	use crate::art::parts::Turn;

	#[test]
	fn the_front_view_is_unwarped_and_turning_moves_the_face_right() {
		assert!((wrap(pt(30.0, 5.0), 0.0, FACE_RADIUS) - pt(30.0, 5.0)).len() < 1e-3);
		assert!(wrap(pt(0.0, 0.0), 1.0, FACE_RADIUS).x > 40.0);
		// The far side foreshortens: equal steps on the face shrink towards the edge.
		let near =
			wrap(pt(-20.0, 0.0), 1.0, FACE_RADIUS).x - wrap(pt(-40.0, 0.0), 1.0, FACE_RADIUS).x;
		let far = wrap(pt(40.0, 0.0), 1.0, FACE_RADIUS).x - wrap(pt(20.0, 0.0), 1.0, FACE_RADIUS).x;
		assert!(far < near);
	}

	#[test]
	fn the_face_slides_as_far_as_the_warp_moves_it() {
		let warped = wrap(pt(0.0, 0.0), 1.0, FACE_RADIUS).x;
		assert!((FACE_SLIDE * STAGED_FACING - warped).abs() < 1.0);
	}

	#[test]
	fn hands_reach_their_targets_through_the_rig() {
		let frame = Frame::authored_kid();

		// Poses whose hands land on a spot (not stretched to full reach, like a cheer).
		for pose in [
			Pose::Idle,
			Pose::Wave,
			Pose::Hips,
			Pose::HoldNose,
			Pose::ArmsCrossed,
		] {
			let mut state = FigureState::new(&frame);
			state.hands = frame.hands(pose, 0.0);
			let posed = Posed::of(&state);
			let solved = Solved::new(&posed, frame.limbs());

			for side in [-1.0, 1.0] {
				let (shoulder, elbow, wrist) = solved.arm(&posed, side);
				let target = if side < 0.0 {
					posed.hands.0
				} else {
					posed.hands.1
				};
				assert!(((elbow - shoulder).len() - KID_LIMBS.upper_arm).abs() < 0.01);
				assert!(
					(wrist - solved.torso(target)).len() < 0.5,
					"{pose:?}, side {side}"
				);
			}
		}
	}

	#[test]
	fn facing_left_mirrors_the_figure_and_swaps_the_near_hand() {
		let frame = Frame::authored_kid();
		let mut state = FigureState::new(&frame);
		state.hands = frame.hands(Pose::Point, -STAGED_FACING);
		state.hand_shapes = Pose::Point.hand_shapes();
		state.facing = -STAGED_FACING;
		let posed = Posed::of(&state);
		assert!(posed.mirrored);
		assert!((posed.t - 1.0).abs() < 1e-6);
		// The pointing hand is the page-right one, which is the far hand when facing left.
		assert_eq!(posed.hand_shapes.0, Hand::Point);
		assert!(
			posed.hands.0.x < 0.0,
			"pointing away from the body, mirrored"
		);
	}

	#[test]
	fn raised_hands_clear_the_head_but_hands_on_the_face_stay() {
		let frame = Frame::authored_kid();

		for facing in [0.0, STAGED_FACING] {
			let mut state = FigureState::new(&frame);
			state.facing = facing;
			state.hands = frame.hands(Pose::HoldNose, facing);
			let nose = Posed::of(&state).hands.1;
			let face_x = facing / STAGED_FACING * FACE_SLIDE * STAGED_FACING;
			assert!((nose.x - face_x).abs() < 30.0, "facing {facing}: {nose:?}");

			state.hands = frame.hands(Pose::Wave, facing);
			let posed = Posed::of(&state);
			let waving = if posed.hands.0.y < posed.hands.1.y {
				posed.hands.0
			} else {
				posed.hands.1
			};
			assert!(waving.x.abs() >= 115.0, "facing {facing}: {waving:?}");
		}
	}

	fn stinky() -> Look {
		let cast = crate::cast::Cast::load(
			&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("world/cast.toml"),
		)
		.unwrap();
		let Ok(crate::cast::Kind::Figure(look)) = cast.resolve("stinky") else {
			panic!("stinky is a person");
		};
		look
	}

	fn drawn(look: &Look, state: &FigureState) -> String {
		let mut svg = Svg::new();
		figure::draw(&mut svg, look, state);
		svg.finish((0.0, 0.0, 1.0, 1.0), 1, 1)
	}

	#[test]
	fn extras_are_drawn_only_when_the_cast_lists_them() {
		let mut look = stinky();
		let state = FigureState::new(&look.frame());
		let lens = format!("fill=\"{LENS}\"");
		assert!(!drawn(&look, &state).contains(&lens));

		look.extras.push(Extra::Glasses);
		assert!(drawn(&look, &state).contains(&lens));
	}

	#[test]
	fn a_held_prop_is_drawn_and_stays_in_front_when_turned() {
		let look = stinky();
		let frame = look.frame();
		let mut state = FigureState::new(&frame);
		state.facing = STAGED_FACING;
		state.hands = frame.hands(Pose::Hold, STAGED_FACING);
		state.prop = Some(Prop::Jam);
		let posed = Posed::of(&state);
		// The jam is in the page-right hand, which is the far hand when facing right.
		assert_eq!(posed.prop, Some((Prop::Jam, true)));
		assert!(drawn(&look, &state).contains(&format!("fill=\"{JAM}\"")));
	}

	#[test]
	fn brushing_keeps_the_toothbrush_hand_at_the_mouth_when_turned() {
		let frame = Frame::authored_kid();
		let mut state = FigureState::new(&frame);
		state.facing = STAGED_FACING;
		state.hands = frame.hands(Pose::Brush, STAGED_FACING);
		state.prop = Some(Prop::Toothbrush);
		let posed = Posed::of(&state);
		assert_eq!(posed.prop, Some((Prop::Toothbrush, true)), "no wave swap");
		let solved = Solved::new(&posed, frame.limbs());
		let (_, _, wrist) = solved.arm(&posed, 1.0);
		assert!((wrist - mouth_on_body(&solved, &posed)).len() < 70.0);
	}

	#[test]
	fn every_puppet_file_draws_a_figure() {
		let world = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("world/puppets");

		for name in ["stinky", "bogey", "grubby", "shouty", "fibby"] {
			let puppet = Authored::load(&world.join(format!("{name}.svg")))
				.unwrap_or_else(|error| panic!("{name}: {error:#}"));

			for turning in ["head", "hair", "torso", "legwear"] {
				assert!(
					matches!(puppet.parts.turn(turning), Some(Turn::Blend { .. })),
					"{name}'s {turning} should blend between its two views"
				);
			}
		}
	}

	#[test]
	fn only_hair_that_hangs_past_the_ears_covers_them() {
		let world = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("world/puppets");
		let hair = |name: &str| {
			Authored::load(&world.join(format!("{name}.svg")))
				.unwrap()
				.required("hair", 1.0)
		};
		assert!(covers_ears(&hair("fibby")), "a bob covers the ears");

		for name in ["stinky", "bogey", "grubby", "shouty"] {
			assert!(!covers_ears(&hair(name)), "{name}'s ears show");
		}
	}

	#[test]
	fn a_grinning_speaker_opens_their_mouth() {
		let look = stinky();
		let mut state = FigureState::new(&look.frame());
		state.mood = Mood::Grin;
		let teeth_together = drawn(&look, &state);
		state.mouth_open = 0.55;
		assert_ne!(
			drawn(&look, &state),
			teeth_together,
			"talking opens the grin"
		);
	}

	#[test]
	fn cool_skin_gets_shading_instead_of_a_pink_blush() {
		assert_eq!(blush(Color(0xF4_A2_6B)), BLUSH);
		assert_ne!(blush(Color(0x62_B9_AD)), BLUSH);
	}

	#[test]
	fn turned_far_arms_cheer_behind_the_head_but_reach_the_face_in_front() {
		let frame = Frame::authored_kid();
		let behind = |pose: Pose| {
			let mut state = FigureState::new(&frame);
			state.facing = STAGED_FACING;
			state.hands = frame.hands(pose, STAGED_FACING);
			Posed::of(&state).far_arm_behind
		};
		assert!(behind(Pose::Idle));
		assert!(
			behind(Pose::Cheer),
			"a raised far fist would cross the face"
		);
		assert!(!behind(Pose::HoldNose));
		assert!(!behind(Pose::ArmsCrossed));
	}

	#[test]
	fn every_mood_has_a_face_and_hanging_open_hands_relax() {
		let frame = Frame::authored_kid();

		for (_, mood) in Mood::ALL {
			let mut state = FigureState::new(&frame);
			state.mood = mood;
			let posed = Posed::of(&state);
			assert_eq!(
				posed.hand_shapes,
				(Hand::Relaxed, Hand::Relaxed),
				"{mood:?}"
			);
		}
	}
}
