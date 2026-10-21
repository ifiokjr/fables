//! Stinky (born Stanley), redrawn as an authored cut-out puppet.
//!
//! Every part is a hand-placed Bézier drawing, built the way TV cut-out characters are built:
//! a head with separate eyes, lids, brows, nose, ears and a mouth chart; a torso; sleeves,
//! forearms and four-finger hands; shorts, legs, socks and shoes. Each part hangs on a bone of
//! the existing fables rig ([`fables::art::rig`]), so stances, walk cycles and two-bone IK work
//! unchanged.
//!
//! Turning is done the cut-out way too. The head outline, hair, torso and shorts each have a
//! front drawing and a three-quarter drawing with the same points, blended by `turn`. Face
//! features are drawn once, front-on, and wrapped round the head with a cylinder warp, so the
//! far eye narrows and the mouth slides towards the cheek on its own. Shoes and hands, which
//! cannot blend sensibly between mirrored drawings, swap drawings at the half-way point, which
//! is what real rigs do.

use std::f32::consts::FRAC_PI_2;

use fables::art::Color;
use fables::art::Path;
use fables::art::Pt;
use fables::art::Svg;
use fables::art::figure::Viseme;
use fables::art::lerp;
use fables::art::pt;
use fables::art::rig;
use fables::art::rig::Gait;
use fables::art::rig::Legs;
use fables::art::rig::Limbs;
use fables::art::rig::Stance;
use fables::art::shape::Shape;

/// Stinky's inks: the identity colours from the cast file (tangerine, plum, teal stripes,
/// mustard), each pulled back from full saturation so they sit together.
pub mod colors {
	use fables::art::Color;

	pub const INK: Color = Color(0x2B_1D_2E);
	pub const SKIN: Color = Color(0xF4_A2_6B);
	pub const NOSE: Color = Color(0xF2_8E_5C);
	pub const FRECKLE: Color = Color(0xC4_67_41);
	pub const BLUSH: Color = Color(0xEE_6A_5A);
	pub const HAIR: Color = Color(0x5A_2D_5C);
	pub const HAIR_LIGHT: Color = Color(0x86_52_83);
	pub const JUMPER: Color = Color(0x2E_9A_90);
	pub const STRIPE: Color = Color(0xF6_EA_CF);
	pub const SHORTS: Color = Color(0xE3_AA_48);
	pub const SOCK: Color = Color(0xF8_F1_E4);
	pub const SHOE: Color = Color(0x40_33_57);
	pub const SOLE: Color = Color(0xF2_E4_C8);
	pub const EYE: Color = Color(0xFF_FC_F5);
	pub const PUPIL: Color = Color(0x2A_19_20);
	pub const MAW: Color = Color(0x6A_20_34);
	pub const TONGUE: Color = Color(0xE9_74_7C);
	pub const TEETH: Color = Color(0xFF_F5_DA);
}

use colors::BLUSH;
use colors::EYE;
use colors::FRECKLE;
use colors::HAIR;
use colors::HAIR_LIGHT;
use colors::INK;
use colors::JUMPER;
use colors::MAW;
use colors::NOSE;
use colors::PUPIL;
use colors::SHOE;
use colors::SHORTS;
use colors::SKIN;
use colors::SOCK;
use colors::SOLE;
use colors::STRIPE;
use colors::TEETH;
use colors::TONGUE;

/// Bone lengths, in figure units with the soles at y = 0. About 500 units tall with the hair.
pub const LIMBS: Limbs = Limbs {
	pelvis_y: -126.0,
	ankle_y: -24.0,
	hip_spread: 21.0,
	thigh: 52.0,
	shin: 52.0,
	upper_arm: 50.0,
	forearm: 46.0,
};

/// Where the head hinges on the torso, and where the head's own origin sits above it.
const NECK: Pt = pt(0.0, -226.0);
const HEAD_ABOVE_NECK: f32 = 96.0;
const SHOULDER_Y: f32 = -206.0;
/// Line weights in figure units. Silhouettes are heavier than interior detail, the way an
/// inker presses harder on the outside edge.
pub const LINE: f32 = 4.2;
pub const DETAIL: f32 = 2.6;
/// Light comes from the top left; shadows fall bottom right.
const LIGHT: Pt = pt(-9.0, -8.0);
/// How far a three-quarter view turns the face, and the radius of the face cylinder.
const FACE_TURN: f32 = 0.42;
const FACE_RADIUS: f32 = 114.0;
const BODY_RADIUS: f32 = 64.0;

// The head, in head units: origin between the eyes, about 196 wide and 208 tall.

const HEAD_FRONT: &str = "M0 -100 C46 -100 84 -80 92 -36 C98 -6 104 30 96 58 C86 92 46 108 0 108 \
                          C-46 108 -86 92 -96 58 C-104 30 -98 -6 -92 -36 C-84 -80 -46 -100 0 -100 \
                          Z";
const HEAD_3Q: &str = "M-4 -102 C44 -104 86 -84 96 -40 C102 -12 112 24 102 54 C94 86 58 106 18 \
                       108 C-26 110 -70 98 -86 66 C-102 36 -106 -6 -98 -42 C-88 -84 -50 -102 -4 \
                       -102 Z";

/// The hair: a bumpy crown and a fringe of locks swept towards the page right.
const HAIR_FRONT: &str =
	"M-94 -30 C-100 -54 -98 -80 -84 -94 C-78 -102 -70 -106 -60 -108 C-48 -114 -30 -112 -20 -110 \
	 C-6 -116 14 -116 26 -110 C40 -114 58 -110 70 -102 C86 -92 98 -74 96 -50 C96 -44 94 -36 90 \
	 -30 C86 -44 80 -52 72 -58 C64 -68 54 -76 46 -78 C46 -70 44 -64 40 -58 C32 -66 24 -74 18 -80 \
	 C18 -72 16 -66 12 -60 C4 -68 -4 -76 -10 -80 C-10 -72 -12 -66 -16 -62 C-24 -70 -32 -76 -38 \
	 -78 C-38 -70 -42 -64 -46 -60 C-56 -62 -66 -60 -74 -56 C-84 -50 -90 -42 -94 -30 Z";
const HAIR_3Q: &str = "M-102 4 C-112 -40 -106 -80 -90 -98 C-84 -106 -76 -110 -64 -112 C-50 -118 \
                       -30 -116 -18 -114 C-2 -118 18 -118 30 -112 C46 -114 64 -108 76 -98 C92 -86 \
                       102 -70 100 -50 C100 -42 100 -36 98 -30 C97 -42 98 -50 100 -56 C95 -68 89 \
                       -76 84 -78 C84 -70 82 -64 79 -58 C73 -66 67 -74 62 -80 C62 -72 60 -66 57 \
                       -60 C50 -68 43 -76 37 -80 C37 -72 35 -66 31 -62 C23 -70 15 -76 9 -78 C9 \
                       -70 6 -64 1 -60 C-10 -60 -24 -54 -34 -46 C-48 -34 -66 -18 -102 4 Z";
/// Strands drawn inside the hair, following its flow from the crown to the fringe.
const HAIR_STRANDS_FRONT: [&str; 3] = [
	"M-46 -100 C-38 -94 -32 -88 -28 -80",
	"M6 -106 C14 -100 18 -94 22 -86",
	"M54 -98 C60 -92 62 -86 64 -80",
];
const HAIR_STRANDS_3Q: [&str; 3] = [
	"M-32 -104 C-22 -98 -12 -92 -4 -84",
	"M26 -108 C36 -102 42 -96 46 -88",
	"M70 -96 C76 -90 80 -86 82 -80",
];

/// The signature: one big plum tuft that rises off the crown and curls forward like a wave.
const TUFT_FRONT: &str = "M-50 -104 C-64 -136 -44 -176 -2 -182 C36 -188 62 -168 58 -144 C56 -132 \
                          44 -128 40 -138 C36 -148 24 -152 14 -146 C0 -138 6 -118 20 -104 C0 -98 \
                          -30 -98 -50 -104 Z";
const TUFT_3Q: &str = "M-64 -104 C-78 -136 -58 -178 -14 -184 C26 -190 54 -170 50 -146 C48 -134 36 \
                       -130 32 -140 C28 -150 16 -154 6 -148 C-8 -140 -6 -120 8 -104 C-12 -98 -42 \
                       -98 -64 -104 Z";
const TUFT_SHINE_FRONT: &str = "M-38 -120 C-40 -150 -22 -170 6 -174";
const TUFT_SHINE_3Q: &str = "M-52 -120 C-54 -150 -36 -172 -8 -176";
/// The page-left ear, attached at its right edge.
const EAR: &str = "M0 -16 C-12 -26 -28 -16 -26 2 C-24 18 -10 22 2 14 Z";
const EAR_CURL: &str = "M-7 -8 C-16 -10 -19 2 -12 7";

// The torso, in figure units (it hangs off the pelvis at y = -126).

const JUMPER_FRONT: &str = "M-24 -232 C-12 -226 12 -226 24 -232 C36 -230 46 -222 48 -208 C52 -188 \
                            60 -152 60 -132 C60 -120 54 -114 46 -114 C20 -110 -20 -110 -46 -114 \
                            C-54 -114 -60 -120 -60 -132 C-60 -152 -52 -188 -48 -208 C-46 -222 -36 \
                            -230 -24 -232 Z";
const JUMPER_3Q: &str = "M-14 -232 C-2 -228 18 -228 28 -232 C38 -228 46 -218 47 -206 C52 -186 64 \
                         -152 62 -132 C62 -120 56 -114 48 -114 C22 -110 -16 -110 -38 -114 C-46 \
                         -115 -52 -122 -52 -134 C-52 -154 -46 -190 -42 -208 C-38 -224 -28 -232 \
                         -14 -232 Z";
const COLLAR_FRONT: &str =
	"M-27 -233 C-14 -224 14 -224 27 -233 C25 -224 16 -217 0 -217 C-16 -217 -25 -224 -27 -233 Z";
const COLLAR_3Q: &str =
	"M-17 -233 C-4 -225 20 -225 30 -233 C29 -224 21 -217 7 -217 C-8 -217 -16 -224 -17 -233 Z";
const SHORTS_FRONT: &str = "M-56 -138 C-20 -142 20 -142 56 -138 C62 -122 64 -104 62 -90 C44 -86 \
                            24 -88 7 -94 C4 -100 -4 -100 -7 -94 C-24 -88 -44 -86 -62 -90 C-64 \
                            -104 -62 -122 -56 -138 Z";
const SHORTS_3Q: &str = "M-48 -138 C-14 -142 24 -142 58 -138 C64 -122 66 -104 64 -90 C48 -86 30 \
                         -88 15 -94 C12 -100 5 -100 2 -94 C-14 -88 -34 -86 -52 -90 C-54 -104 -52 \
                         -122 -48 -138 Z";

// Limbs, in bone units: the bone runs down +y from its joint.

const SLEEVE: &str =
	"M-14 -4 C-15 14 -14 30 -13 46 C-6 51 6 51 13 46 C14 30 15 14 14 -4 C13 -19 -13 -19 -14 -4 Z";
const SHORTS_LEG: &str =
	"M-18 -12 C-19 8 -21 26 -22 44 C-8 49 8 49 22 44 C21 26 19 8 18 -12 C10 -19 -10 -19 -18 -12 Z";

/// A right shoe seen from the front, toes turned out to the page right; the ankle is at the
/// origin and the sole at y = 24.
const SHOE_FRONT: &str = "M-12 -6 C-4 -10 8 -10 14 -4 C24 0 32 8 32 16 C32 24 22 26 2 26 C-14 26 \
                          -21 22 -21 14 C-21 6 -18 0 -12 -6 Z";
/// A shoe in three-quarter view, toes forward to the page right.
const SHOE_3Q: &str = "M-12 -8 C-2 -12 8 -8 14 -2 C26 2 38 8 40 16 C42 24 34 26 18 26 C2 26 -12 \
                       26 -17 22 C-21 18 -19 6 -12 -8 Z";

// Hands, in hand units: the wrist at the origin, fingers pointing down +y, the thumb on the -x
// side (towards the body for a page-right hand hanging at rest). Four digits, cartoon-style.

const HAND_RELAXED: &str = "M-8 0 C-12 3 -15 6 -17 10 C-19 14 -18 19 -14 19 C-12 19 -11 17 -10 15 \
                            C-11 21 -10 27 -6 31 C-3 34 0 33 0 30 C1 33 4 35 7 33 C9 31 9 29 9 28 \
                            C11 29 13 28 13 25 C14 20 12 13 10 6 C9 3 7 0 5 -1 C2 -3 -4 -3 -8 0 Z";
const HAND_RELAXED_LINES: [&str; 2] = ["M0 30 C0 27 -1 24 -2 22", "M9 28 C9 25 8 23 7 21"];
const HAND_OPEN: &str = "M-8 0 C-12 5 -19 5 -24 9 C-29 13 -25 19 -17 18 C-19 24 -19 31 -15 36 \
                         C-11 40 -5 36 -6 30 C-5 36 -2 42 3 41 C8 40 8 34 6 28 C9 34 13 36 16 33 \
                         C19 30 16 22 11 15 C10 9 9 4 8 0 C3 -3 -3 -3 -8 0 Z";
const HAND_FIST: &str = "M-9 -1 C-15 4 -17 12 -16 20 C-15 28 -7 32 2 32 C10 32 16 27 16 19 C16 11 \
                         13 3 8 -1 C3 -3 -4 -3 -9 -1 Z";
const HAND_FIST_LINES: [&str; 3] = [
	"M15 15 C10 16 8 19 8 23",
	"M14 24 C10 24 7 27 6 30",
	"M-15 12 C-9 9 -3 10 0 15",
];

// Mouths, in mouth units: the centre of the mouth at the origin.

const SMILE: &str = "M-36 -6 C-18 2 18 2 36 -6 C33 20 17 34 0 34 C-17 34 -33 20 -36 -6 Z";
const GRIN: &str = "M-48 -4 C-24 4 22 0 50 -16 C48 12 26 30 0 30 C-24 30 -44 18 -48 -4 Z";
const GRIN_TEETH: &str =
	"M-52 -8 C-24 0 22 -4 54 -20 C56 -8 54 -2 52 -6 C22 10 -24 16 -50 8 C-54 4 -54 -2 -52 -8 Z";
const POUT: &str =
	"M-22 8 C-12 -4 10 -6 26 8 C27 10 25 12 23 11 C10 2 -10 2 -19 11 C-21 12 -23 10 -22 8 Z";
const POUT_LIP: &str = "M-8 13 C-4 18 6 18 10 13";
const BLEH: &str =
	"M-30 2 C-20 -8 -8 4 2 -2 C12 -8 22 -6 30 4 C24 16 12 20 0 18 C-12 16 -24 12 -30 2 Z";
const BLEH_TONGUE: &str = "M-10 12 C-12 28 -6 40 4 40 C14 40 18 28 16 12 C8 16 -2 16 -10 12 Z";
const BLEH_GROOVE: &str = "M3 18 C3 26 4 30 4 34";

const REST: &str = "M-22 0 C-10 7 10 7 22 0 C23 2 21 4 19 3 C8 9 -8 9 -19 3 C-21 4 -23 2 -22 0 Z";
const MBP: &str = "M-18 1 C-6 4 6 4 18 1 C19 3 17 6 15 5 C6 8 -6 8 -15 5 C-17 6 -19 3 -18 1 Z";
const MBP_LIP: &str = "M-8 11 C-3 14 3 14 8 11";
const TEETH_OPEN: &str = "M-24 -3 C-8 1 8 1 24 -3 C21 8 10 12 0 12 C-10 12 -21 8 -24 -3 Z";
const OPEN: &str = "M-26 -5 C-10 0 10 0 26 -5 C23 15 11 24 0 24 C-11 24 -23 15 -26 -5 Z";
const WIDE: &str = "M-32 -8 C-12 -1 12 -1 32 -8 C30 22 16 38 0 38 C-16 38 -30 22 -32 -8 Z";
const FV: &str = "M-22 -2 C-8 2 8 2 22 -2 C17 6 8 9 0 9 C-8 9 -17 6 -22 -2 Z";

/// What the face is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
	Happy,
	Grin,
	/// Arms folded, chin up, eyes half shut: "NOPE".
	Defiant,
	Disgusted,
}

impl Mood {
	pub const ALL: [Mood; 4] = [Mood::Happy, Mood::Defiant, Mood::Disgusted, Mood::Grin];

	pub fn name(self) -> &'static str {
		match self {
			Mood::Happy => "happy",
			Mood::Grin => "grin",
			Mood::Defiant => "defiant (NOPE)",
			Mood::Disgusted => "disgusted",
		}
	}

	fn face(self) -> Face {
		let brow = |raise, tilt| Brow { raise, tilt };
		let lids = |upper, lower, tilt| Lids { upper, lower, tilt };

		match self {
			Mood::Happy => {
				Face {
					brows: [brow(5.0, -8.0), brow(5.0, -8.0)],
					lids: [lids(0.04, 0.1, 0.0), lids(0.04, 0.1, 0.0)],
					mouth: MouthShape::Smile,
					curl: 1.0,
					wrinkle: false,
				}
			}
			Mood::Grin => {
				Face {
					brows: [brow(-1.0, 10.0), brow(10.0, -12.0)],
					lids: [lids(0.22, 0.36, 0.0), lids(0.14, 0.34, -4.0)],
					mouth: MouthShape::Grin,
					curl: 1.0,
					wrinkle: false,
				}
			}
			Mood::Defiant => {
				Face {
					brows: [brow(-6.0, 20.0), brow(-6.0, 20.0)],
					lids: [lids(0.52, 0.08, 14.0), lids(0.52, 0.08, 14.0)],
					mouth: MouthShape::Pout,
					curl: -0.6,
					wrinkle: false,
				}
			}
			Mood::Disgusted => {
				Face {
					brows: [brow(9.0, -14.0), brow(-8.0, 20.0)],
					lids: [lids(0.22, 0.12, -4.0), lids(0.5, 0.42, 12.0)],
					mouth: MouthShape::Bleh,
					curl: -1.0,
					wrinkle: true,
				}
			}
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
struct Lids {
	/// How far the upper lid covers the eye, 0 (wide) to 1 (shut).
	upper: f32,
	/// How far the lower lid rises (cheeks pushing up in a smile or a squint).
	lower: f32,
	/// Degrees; positive drops the inner corner of the upper lid.
	tilt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouthShape {
	Smile,
	Grin,
	Pout,
	Bleh,
}

#[derive(Clone, Copy, Debug)]
struct Face {
	/// `[page-left, page-right]` in the authored (facing right) drawing.
	brows: [Brow; 2],
	lids: [Lids; 2],
	mouth: MouthShape,
	/// How much a talking mouth curls up (+1) or down (-1) in this mood.
	curl: f32,
	/// Scrunched nose lines.
	wrinkle: bool,
}

/// How a hand is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
	Relaxed,
	Open,
	Fist,
}

/// Named arm positions. The near arm is the page-left arm when facing right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arms {
	Relaxed,
	Wave,
	Hips,
	Folded,
	/// One hand up, palm out: "ew, get it away".
	Recoil,
}

impl Arms {
	/// Hand targets `(near, far)` at rest, plus the hand shapes and extra wrist turns.
	fn targets(self, turn: f32) -> ((Pt, Pt), (Hand, Hand), (f32, f32)) {
		let t = turn.abs().min(1.0);
		let side = |front: Pt, three: Pt| front.lerp(three, t);

		match self {
			Arms::Relaxed => {
				(
					(
						side(pt(-60.0, -122.0), pt(-46.0, -122.0)),
						side(pt(60.0, -122.0), pt(48.0, -124.0)),
					),
					(Hand::Relaxed, Hand::Relaxed),
					(6.0, -6.0),
				)
			}
			Arms::Wave => {
				(
					(
						side(pt(-120.0, -250.0), pt(-112.0, -256.0)),
						side(pt(60.0, -122.0), pt(48.0, -124.0)),
					),
					(Hand::Open, Hand::Relaxed),
					(-14.0, -6.0),
				)
			}
			Arms::Hips => {
				(
					(
						side(pt(-62.0, -140.0), pt(-50.0, -138.0)),
						side(pt(62.0, -140.0), pt(54.0, -140.0)),
					),
					(Hand::Fist, Hand::Fist),
					(70.0, -70.0),
				)
			}
			Arms::Folded => {
				(
					(
						side(pt(26.0, -168.0), pt(34.0, -168.0)),
						side(pt(-24.0, -176.0), pt(-14.0, -176.0)),
					),
					(Hand::Fist, Hand::Fist),
					(-60.0, 60.0),
				)
			}
			Arms::Recoil => {
				(
					(
						side(pt(-104.0, -232.0), pt(-86.0, -238.0)),
						side(pt(34.0, -150.0), pt(30.0, -150.0)),
					),
					(Hand::Open, Hand::Fist),
					(-30.0, -40.0),
				)
			}
		}
	}
}

/// Everything that poses Stinky for one moment.
#[derive(Clone, Debug)]
pub struct Pose {
	/// 0 is front-on; 1 is three-quarters turned to the page right, -1 to the page left.
	pub turn: f32,
	pub mood: Mood,
	/// A lip-sync mouth while talking; `None` uses the mood's mouth.
	pub viseme: Option<Viseme>,
	/// Hand targets `(near, far)` in figure units at rest; the torso carries them.
	pub hands: (Pt, Pt),
	pub hand_shapes: (Hand, Hand),
	/// Extra wrist rotation in degrees, for waves and flourishes.
	pub wrists: (f32, f32),
	pub legs: Legs,
	pub gait: Option<(Gait, f32)>,
	/// Whole-body lean in degrees.
	pub tilt: f32,
	/// Head tilt in degrees on top of the body (negative lifts the chin when facing right).
	pub head_tilt: f32,
	/// Where the pupils look, in eye units (-1..1 each way).
	pub gaze: Pt,
	/// 0 open, 1 shut.
	pub blink: f32,
}

impl Pose {
	pub fn new(mood: Mood, arms: Arms, turn: f32) -> Pose {
		let (hands, hand_shapes, wrists) = arms.targets(turn);
		let mut pose = Pose {
			turn,
			mood,
			viseme: None,
			hands,
			hand_shapes,
			wrists,
			legs: stance(Stance::Stand, turn),
			gait: None,
			tilt: 0.0,
			head_tilt: 0.0,
			gaze: pt(turn.clamp(-1.0, 1.0) * 0.35, 0.0),
			blink: 0.0,
		};

		match mood {
			Mood::Happy => pose.head_tilt = 4.0,
			Mood::Grin => {
				pose.head_tilt = 6.0;
				pose.gaze = pt(0.5, 0.1);
			}
			Mood::Defiant => {
				pose.head_tilt = -9.0;
				pose.gaze = pt(-0.85, -0.2);
				pose.tilt = -2.0;
			}
			Mood::Disgusted => {
				pose.head_tilt = -6.0;
				pose.tilt = -7.0;
				pose.gaze = pt(0.7, 0.1);
			}
		}

		pose
	}

	/// Sets the arms, keeping everything else.
	pub fn with_arms(mut self, arms: Arms) -> Pose {
		let (hands, hand_shapes, wrists) = arms.targets(self.turn);
		self.hands = hands;
		self.hand_shapes = hand_shapes;
		self.wrists = wrists;
		self
	}

	/// Blends every number towards `other`; moods and hand shapes switch half-way.
	#[must_use]
	pub fn blend(&self, other: &Pose, t: f32) -> Pose {
		let mut pose = if t < 0.5 { self.clone() } else { other.clone() };
		pose.turn = lerp(self.turn, other.turn, t);
		pose.hands = (
			self.hands.0.lerp(other.hands.0, t),
			self.hands.1.lerp(other.hands.1, t),
		);
		pose.wrists = (
			lerp(self.wrists.0, other.wrists.0, t),
			lerp(self.wrists.1, other.wrists.1, t),
		);
		pose.legs = Legs {
			feet: (
				self.legs.feet.0.lerp(other.legs.feet.0, t),
				self.legs.feet.1.lerp(other.legs.feet.1, t),
			),
			crouch: lerp(self.legs.crouch, other.legs.crouch, t),
			bend: lerp(self.legs.bend, other.legs.bend, t),
		};
		pose.tilt = lerp(self.tilt, other.tilt, t);
		pose.head_tilt = lerp(self.head_tilt, other.head_tilt, t);
		pose.gaze = self.gaze.lerp(other.gaze, t);
		pose.blink = lerp(self.blink, other.blink, t);
		pose
	}
}

/// The rig's stance, with the far foot set back and up the page in a three-quarter view so
/// the feet sit on a ground plane rather than a line.
pub fn stance(stance: Stance, turn: f32) -> Legs {
	let t = turn.abs().min(1.0);
	let mut legs = LIMBS.stance(stance, turn.signum());
	legs.feet.0 = legs.feet.0 + pt(4.0 * t, 3.0 * t);
	legs.feet.1 = legs.feet.1 + pt(6.0 * t, -9.0 * t);
	legs
}

/// The rig solved for one moment.
struct Solved {
	t: f32,
	legs: Legs,
	swing: (Pt, Pt),
	drop: f32,
	pivot: Pt,
}

impl Solved {
	fn new(pose: &Pose) -> Solved {
		let t = pose.turn.abs().min(1.0);
		let (legs, swing) = match pose.gait {
			Some((gait, phase)) => LIMBS.cycle(gait, phase, 1.0, pose.legs),
			None => (pose.legs, (pt(0.0, 0.0), pt(0.0, 0.0))),
		};

		Solved {
			t,
			legs,
			swing,
			drop: legs.crouch * LIMBS.leg_length(),
			pivot: pt(0.0, LIMBS.pelvis_y),
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
		let x = lerp(side * 21.0, if side < 0.0 { -14.0 } else { 20.0 }, self.t);
		self.torso(pt(x, LIMBS.pelvis_y))
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
		let (knee, ankle) = rig::two_bone(hip, foot, LIMBS.thigh, LIMBS.shin, pole);
		(hip, knee, ankle)
	}

	fn arm(&self, pose: &Pose, side: f32) -> (Pt, Pt, Pt) {
		let shoulder = self.shoulder(side);
		let (target, swing) = if side < 0.0 {
			(pose.hands.0, self.swing.0)
		} else {
			(pose.hands.1, self.swing.1)
		};
		let (elbow, wrist) = rig::two_bone(
			shoulder,
			self.torso(target) + swing,
			LIMBS.upper_arm,
			LIMBS.forearm,
			pt(side * 0.6, 0.8),
		);
		(shoulder, elbow, wrist)
	}
}

/// Draws Stinky with his soles at the current origin.
pub fn draw(c: &mut Svg, pose: &Pose) {
	let solved = Solved::new(pose);
	let t = solved.t;

	c.begin(&format!(
		"scale({} 1) rotate({:.2})",
		if pose.turn < 0.0 { -1 } else { 1 },
		pose.tilt
	));

	// In a three-quarter view the far arm goes behind the body, unless it folds across it.
	let far_arm_behind = t > 0.5 && pose.hands.1.x > -10.0;

	if far_arm_behind {
		draw_arm(c, pose, &solved, 1.0, true);
	}

	for side in [1.0, -1.0] {
		draw_leg(c, pose, &solved, side);
	}

	c.begin(&solved.torso_transform());
	draw_torso(c, t);
	c.begin(&format!(
		"translate({:.1} {:.1}) rotate({:.2}) translate(0 {:.1})",
		NECK.x, NECK.y, pose.head_tilt, -HEAD_ABOVE_NECK
	));
	draw_head(c, pose, t);
	c.end();
	c.end();

	// Arms that fold across the body draw the lower arm last so it sits on top.
	let order: [f32; 2] = if pose.hands.0.x > 0.0 {
		[1.0, -1.0]
	} else {
		[-1.0, 1.0]
	};

	for side in order {
		if !(side > 0.0 && far_arm_behind) {
			draw_arm(c, pose, &solved, side, side > 0.0 && t > 0.5);
		}
	}

	c.end();
}

/// A soft contact shadow under the feet, so he stands on the ground instead of floating.
pub fn draw_shadow(c: &mut Svg, pose: &Pose, lift: f32) {
	let fade = (1.0 - lift / 120.0).clamp(0.35, 1.0);
	let x = pose.turn.signum() * pose.turn.abs().min(1.0) * 6.0;
	c.fill_alpha(
		&Path::ellipse(pt(x, -2.0), 70.0 * fade, 12.0 * fade),
		INK,
		0.18 * fade,
	);
}

/// Draws the rig's bones over the figure, to show the parts hang on the fables skeleton.
pub fn draw_bones(c: &mut Svg, pose: &Pose) {
	let solved = Solved::new(pose);
	let bone = Color(0xF2_54_3D);
	let joint = Color(0xFF_C9_3C);
	let pelvis = solved.torso(solved.pivot);
	let chest = solved.torso(pt(0.0, SHOULDER_Y));
	let neck = solved.torso(NECK);
	let head = solved.torso(NECK + pt(0.0, -HEAD_ABOVE_NECK).rotate(pose.head_tilt));
	let mut bones = vec![(pelvis, chest), (chest, neck), (neck, head)];
	let mut joints = vec![pelvis, chest, neck, head];

	for side in [-1.0, 1.0] {
		let (hip, knee, ankle) = solved.leg(side);
		let (shoulder, elbow, wrist) = solved.arm(pose, side);
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

	c.begin(&format!(
		"scale({} 1) rotate({:.2})",
		if pose.turn < 0.0 { -1 } else { 1 },
		pose.tilt
	));

	for (from, to) in bones {
		c.stroke(&Path::new().move_to(from).line_to(to), bone, 3.5);
	}

	for point in joints {
		c.brushed(&Path::circle(point, 5.0), joint, INK, 1.6);
	}

	c.end();
}

/// Starts a group whose +y axis runs along the bone from `from` to `to`; returns the bone's
/// length and its angle in degrees.
fn begin_bone(c: &mut Svg, from: Pt, to: Pt) -> (f32, f32) {
	let d = to - from;
	let angle = (-d.x).atan2(d.y).to_degrees();
	c.begin(&format!(
		"translate({:.2} {:.2}) rotate({angle:.2})",
		from.x, from.y
	));
	(d.len(), angle)
}

/// A tapered tube with round ends, `w0` wide at the joint and `w1` at the far end.
fn capsule(len: f32, w0: f32, w1: f32) -> Shape {
	let (a, b) = (w0 / 2.0, w1 / 2.0);
	Shape::parse(&format!(
		"M{} 0 C{} {} {} {} {} {len} C{} {} {} {} {b} {len} C{b} {} {a} {} {a} 0 C{a} {} {} {} {} \
		 0 Z",
		-a,
		-a,
		len * 0.33,
		-b,
		len * 0.66,
		-b,
		-b,
		len + b * 1.3,
		b,
		len + b * 1.3,
		len * 0.66,
		len * 0.33,
		-a * 1.3,
		-a,
		-a * 1.3,
		-a,
	))
}

/// Pushes a colour back for limbs on the far side of the body.
fn far(color: Color, is_far: bool) -> Color {
	if is_far {
		color.mix(Color(0x6E_5A_7E), 0.16)
	} else {
		color
	}
}

fn draw_leg(c: &mut Svg, pose: &Pose, solved: &Solved, side: f32) {
	let t = solved.t;
	let is_far = side > 0.0 && t > 0.3;
	let (hip, knee, ankle) = solved.leg(side);
	let skin = far(SKIN, is_far);

	// The leg is one tapered shape bent through the knee, so there's no seam at the joint.
	let leg = Shape::taper(
		&[hip, hip.lerp(knee, 0.5), knee, knee.lerp(ankle, 0.5), ankle],
		15.5,
		12.5,
	);
	c.brushed_shaded(&leg, skin, LIGHT, INK, LINE);

	let (_, angle) = begin_bone(c, knee, ankle);
	let sock = capsule(18.0, 15.5, 15.0).shift(pt(0.0, LIMBS.shin - 16.0));
	c.brushed_shaded(&sock, far(SOCK, is_far), LIGHT.rotate(-angle), INK, LINE);
	c.begin_clip(&sock.path());
	c.fill(
		&Path::rect(-12.0, LIMBS.shin - 14.0, 24.0, 4.5),
		far(JUMPER, is_far),
	);
	c.end();
	c.brush_line(&sock.path(), INK, LINE);
	c.end();

	// The shoe stays level with the ground and tips its toe as the foot lifts in a stride.
	let lift = (LIMBS.ankle_y - ankle.y).max(0.0);
	let toe = if pose.gait.is_some() {
		-lift * 0.5
	} else {
		0.0
	};
	let three_quarter = t > 0.5;
	let mirror = side < 0.0 && !three_quarter;
	let shoe = Shape::parse(if three_quarter { SHOE_3Q } else { SHOE_FRONT });
	c.begin(&format!(
		"translate({:.2} {:.2}) rotate({toe:.2}) scale({} 1)",
		ankle.x,
		ankle.y,
		if mirror { -1 } else { 1 }
	));
	c.brush_fill(&shoe.path(), far(SHOE, is_far), INK, LINE);
	c.cel_shade(&shoe, pt(-6.0, -5.0));
	c.begin_clip(&shoe.path());
	c.fill(&Path::rect(-30.0, 17.0, 80.0, 12.0), far(SOLE, is_far));
	c.stroke(
		&Path::new().move_to(pt(-30.0, 17.0)).line_to(pt(50.0, 17.0)),
		INK,
		DETAIL,
	);
	c.end();
	let lace = if three_quarter {
		"M2 -6 C6 -4 10 -2 14 0"
	} else {
		"M-6 -6 C-2 -4 4 -4 8 -6"
	};
	c.stroke(&Shape::parse(lace).path(), far(SOLE, is_far), 3.4);
	c.brush_line(&shoe.path(), INK, LINE);
	c.end();

	// The shorts' leg rides on the thigh.
	let (_, angle) = begin_bone(c, hip, knee);
	let leg = Shape::parse(SHORTS_LEG);
	c.brushed_shaded(&leg, far(SHORTS, is_far), LIGHT.rotate(-angle), INK, LINE);
	c.stroke(
		&Shape::parse("M-21 37 C-7 41 7 41 21 37").path(),
		far(SHORTS, is_far).darken(0.25),
		DETAIL,
	);
	c.end();
}

/// The cylinder warp that turns flat front-on drawings round a body of `radius`.
fn wrap(p: Pt, t: f32, radius: f32) -> Pt {
	let angle = (p.x / radius).clamp(-1.0, 1.0).asin() + t * FACE_TURN;
	pt(radius * angle.clamp(-FRAC_PI_2, FRAC_PI_2).sin(), p.y)
}

fn draw_torso(c: &mut Svg, t: f32) {
	let shorts = Shape::parse(SHORTS_FRONT).lerp(&Shape::parse(SHORTS_3Q), t);
	c.brushed_shaded(&shorts, SHORTS, LIGHT, INK, LINE);
	c.stroke(
		&Shape::parse("M-54 -128 C-20 -132 20 -132 54 -128")
			.map(|p| wrap(p, t, BODY_RADIUS))
			.path(),
		SHORTS.darken(0.25),
		DETAIL,
	);

	let jumper = Shape::parse(JUMPER_FRONT).lerp(&Shape::parse(JUMPER_3Q), t);
	let path = jumper.path();
	c.brush_fill(&path, JUMPER, INK, LINE * 1.1);
	c.begin_clip(&path);

	for y in [-200.0, -176.0, -152.0] {
		let band = Shape::parse(&format!(
			"M-80 {y} C-30 {} 30 {} 80 {y} C80 {} 80 {} 80 {} C30 {} -30 {} -80 {} C-80 {} -80 {} \
			 -80 {y} Z",
			y + 6.0,
			y + 6.0,
			y + 4.0,
			y + 8.0,
			y + 11.0,
			y + 17.0,
			y + 17.0,
			y + 11.0,
			y + 8.0,
			y + 4.0,
		))
		.map(|p| wrap(p, t, BODY_RADIUS));
		c.fill(&band.path(), STRIPE);
	}

	// A ribbed hem.
	c.fill(&Path::rect(-80.0, -125.0, 160.0, 14.0), JUMPER.darken(0.12));
	c.end();
	c.cel_shade(&jumper, LIGHT);
	c.stroke(
		&Shape::parse("M-60 -125 C-20 -121 20 -121 60 -125")
			.map(|p| wrap(p, t, BODY_RADIUS))
			.path(),
		INK,
		DETAIL,
	);
	c.brush_line(&path, INK, LINE * 1.1);

	// The neck, then the collar over it.
	let neck = capsule(30.0, 30.0, 30.0).shift(pt(lerp(0.0, 6.0, t), -250.0));
	c.brushed_shaded(&neck, SKIN, LIGHT, INK, LINE);
	let collar = Shape::parse(COLLAR_FRONT).lerp(&Shape::parse(COLLAR_3Q), t);
	c.brushed_shaded(&collar, STRIPE, LIGHT, INK, LINE);
}

fn draw_arm(c: &mut Svg, pose: &Pose, solved: &Solved, side: f32, is_far: bool) {
	let t = solved.t;
	let (shoulder, elbow, wrist) = solved.arm(pose, side);
	let (shape, turn) = if side < 0.0 {
		(pose.hand_shapes.0, pose.wrists.0)
	} else {
		(pose.hand_shapes.1, pose.wrists.1)
	};

	let (_, angle) = begin_bone(c, elbow, wrist);
	let forearm = capsule(LIMBS.forearm, 12.0, 10.5);
	c.brushed_shaded(&forearm, far(SKIN, is_far), LIGHT.rotate(-angle), INK, LINE);
	c.end();

	let (_, angle) = begin_bone(c, shoulder, elbow);
	let sleeve = Shape::parse(SLEEVE);
	let path = sleeve.path();
	c.brush_fill(&path, far(JUMPER, is_far), INK, LINE);
	c.begin_clip(&path);

	for y in [6.0, 24.0] {
		c.fill(&Path::rect(-20.0, y, 40.0, 10.0), far(STRIPE, is_far));
	}

	c.fill(
		&Path::rect(-20.0, 38.0, 40.0, 16.0),
		far(JUMPER.darken(0.12), is_far),
	);
	c.end();
	c.cel_shade(&sleeve, LIGHT.rotate(-angle));
	c.stroke(
		&Shape::parse("M-14 38 C-6 41 6 41 14 38").path(),
		INK,
		DETAIL,
	);
	c.brush_line(&path, INK, LINE);
	c.end();

	// The thumb faces the body from the front and forwards when turned.
	let mirror = side < 0.0 || t > 0.5;
	let (_, angle) = begin_bone(c, elbow, wrist);
	c.end();
	c.begin(&format!(
		"translate({:.2} {:.2}) rotate({:.2}) scale({} 1)",
		wrist.x,
		wrist.y,
		angle + turn * if mirror { -1.0 } else { 1.0 },
		if mirror { -1 } else { 1 }
	));
	let light = LIGHT.rotate(-angle);
	let light = if mirror { pt(-light.x, light.y) } else { light };
	c.begin("scale(1.08)");
	draw_hand(c, shape, far(SKIN, is_far), light);
	c.end();
	c.end();
}

fn draw_hand(c: &mut Svg, hand: Hand, skin: Color, light: Pt) {
	let (outline, lines): (&str, &[&str]) = match hand {
		Hand::Relaxed => (HAND_RELAXED, &HAND_RELAXED_LINES),
		Hand::Open => (HAND_OPEN, &[]),
		Hand::Fist => (HAND_FIST, &HAND_FIST_LINES),
	};
	let shape = Shape::parse(outline);
	let path = shape.path();
	c.brush_fill(&path, skin, INK, LINE * 0.9);
	c.cel_shade(&shape, light * 0.6);

	for line in lines {
		c.stroke(&Shape::parse(line).path(), INK, DETAIL * 0.9);
	}

	c.brush_line(&path, INK, LINE * 0.9);
}

fn draw_head(c: &mut Svg, pose: &Pose, t: f32) {
	let face = pose.mood.face();
	let head = Shape::parse(HEAD_FRONT).lerp(&Shape::parse(HEAD_3Q), t);
	let head_path = head.path();
	let hair = Shape::parse(HAIR_FRONT).lerp(&Shape::parse(HAIR_3Q), t);
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);

	// The far ear tucks away as the head turns; the near ear slides onto the side of the head.
	let ear = Shape::parse(EAR);
	let curl = Shape::parse(EAR_CURL);
	let draw_ear = |c: &mut Svg, at: Pt, mirror: bool, squash: f32| {
		c.begin(&format!(
			"translate({:.1} {:.1}) scale({:.3} 1)",
			at.x,
			at.y,
			if mirror { -squash } else { squash }
		));
		c.brushed_shaded(&ear, SKIN, pt(-5.0, -4.0), INK, LINE);
		c.stroke(&curl.path(), INK, DETAIL);
		c.end();
	};

	if t < 0.9 {
		draw_ear(c, pt(lerp(90.0, 96.0, t), 6.0), true, 1.0 - t * 0.8);
	}

	if t < 0.5 {
		draw_ear(c, pt(-90.0, 6.0), false, 1.0);
	}

	c.brush_fill(&head_path, SKIN, INK, LINE * 1.15);
	c.begin_clip(&head_path);
	// The hair casts a soft band of shadow on the forehead.
	c.begin("translate(0 9)");
	c.fill_alpha(&hair.path(), Color(0xC0_5A_3E), 0.35);
	c.end();
	c.end();
	c.cel_shade(&head, LIGHT);

	// Cheeks and freckles, wrapped round the face. Anything turned past the edge is hidden.
	for side in [-1.0_f32, 1.0] {
		let blush = Shape::ellipse(pt(side * 58.0, 34.0), 17.0, 9.0).map(warp);
		c.fill_alpha(&blush.path(), BLUSH, 0.3);

		for (x, y) in [(52.0, 24.0), (62.0, 18.0), (64.0, 29.0)] {
			let at = pt(side * x, y);
			let angle = (at.x / FACE_RADIUS).asin() + t * FACE_TURN;

			if angle < 1.25 {
				c.fill(&Path::circle(warp(at), 2.6), FRECKLE);
			}
		}
	}

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		draw_eye(c, pose, &face.lids[index], side, t);
	}

	draw_nose(c, t, face.wrinkle);
	draw_mouth(c, pose, &face, t);
	c.brush_line(&head_path, INK, LINE * 1.15);

	// Hair over everything on the head, brows on top of the hair.
	let tuft = Shape::parse(TUFT_FRONT).lerp(&Shape::parse(TUFT_3Q), t);
	c.brushed_shaded(&tuft, HAIR, LIGHT, INK, LINE);
	c.begin_clip(&tuft.path());
	c.stroke(
		&Shape::parse(TUFT_SHINE_FRONT)
			.lerp(&Shape::parse(TUFT_SHINE_3Q), t)
			.path(),
		HAIR_LIGHT,
		7.0,
	);
	c.end();
	c.brushed_shaded(&hair, HAIR, LIGHT, INK, LINE);

	for (front, three) in HAIR_STRANDS_FRONT.iter().zip(HAIR_STRANDS_3Q) {
		let strand = Shape::parse(front).lerp(&Shape::parse(three), t);
		c.stroke(&strand.path(), INK, DETAIL * 0.8);
	}

	// In three-quarter view the near ear sits on the side of the head, over the hair behind it.
	if t >= 0.5 {
		draw_ear(c, pt(lerp(-74.0, -58.0, t), 8.0), false, 0.85);
	}

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		draw_brow(c, face.brows[index], side, t);
	}
}

fn draw_eye(c: &mut Svg, pose: &Pose, lids: &Lids, side: f32, t: f32) {
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);
	let centre = pt(side * 34.0, -6.0);
	let (rx, ry) = (20.0, 27.0);
	let white = Shape::ellipse(centre, rx, ry).map(warp);
	let white_path = white.path();
	let tilt = -side * lids.tilt;

	c.fill(&white_path, EYE);
	c.begin_clip(&white_path);
	let look = centre + pt(pose.gaze.x * 8.0, pose.gaze.y * 7.0 + 2.0);
	c.fill(&Shape::ellipse(look, 11.5, 14.5).map(warp).path(), PUPIL);
	c.fill(&Path::circle(warp(look + pt(-3.5, -5.5)), 3.8), EYE);
	c.fill(&Path::circle(warp(look + pt(3.5, 5.0)), 1.6), EYE);
	c.end();
	c.brush_line(&white_path, INK, DETAIL);

	// Lids paint over the eye's own outline, so the lash line becomes the top edge.
	let lid_clip = Shape::ellipse(centre, rx + 3.0, ry + 3.0).map(warp);
	c.begin_clip(&lid_clip.path());

	// Upper lid: skin down to a curved lash line, tilted by the mood.
	let upper = lids.upper.max(pose.blink);
	let edge_y = centre.y - ry + 2.0 * ry * upper;
	// The lash line arches with the eye when open and flattens as the lid comes down.
	let sag = -9.0 * (1.0 - upper);
	let rotate = |p: Pt| (p - pt(centre.x, edge_y)).rotate(tilt) + pt(centre.x, edge_y);
	let lid = Shape::parse(&format!(
		"M{} {edge_y} C{} {} {} {} {} {edge_y} C{} {} {} {} {} {} C{} {} {} {} {} {edge_y} Z",
		centre.x - 34.0,
		centre.x - 14.0,
		edge_y + sag,
		centre.x + 14.0,
		edge_y + sag,
		centre.x + 34.0,
		centre.x + 34.0,
		edge_y - 30.0,
		centre.x + 34.0,
		edge_y - 60.0,
		centre.x + 34.0,
		edge_y - 70.0,
		centre.x - 34.0,
		edge_y - 70.0,
		centre.x - 34.0,
		edge_y - 30.0,
		centre.x - 34.0,
	))
	.map(rotate)
	.map(warp);
	c.fill(&lid.path(), SKIN);
	let lash = Shape::parse(&format!(
		"M{} {edge_y} C{} {} {} {} {} {edge_y}",
		centre.x - 34.0,
		centre.x - 14.0,
		edge_y + sag,
		centre.x + 14.0,
		edge_y + sag,
		centre.x + 34.0,
	))
	.map(rotate)
	.map(warp);
	c.stroke(&lash.path(), INK, 8.0);

	// Lower lid: the cheek pushing up, curved upwards in a smile or a squint.
	if lids.lower > 0.01 && upper < 0.95 {
		let low_y = centre.y + ry - 2.0 * ry * lids.lower;
		let arc = 10.0 * lids.lower;
		let cheek = Shape::parse(&format!(
			"M{} {low_y} C{} {} {} {} {} {low_y} C{} {} {} {} {} {} C{} {} {} {} {} {low_y} Z",
			centre.x - 34.0,
			centre.x - 14.0,
			low_y - arc,
			centre.x + 14.0,
			low_y - arc,
			centre.x + 34.0,
			centre.x + 34.0,
			low_y + 20.0,
			centre.x + 34.0,
			low_y + 40.0,
			centre.x + 34.0,
			low_y + 50.0,
			centre.x - 34.0,
			low_y + 50.0,
			centre.x - 34.0,
			low_y + 20.0,
			centre.x - 34.0,
		))
		.map(warp);
		c.fill(&cheek.path(), SKIN);
		let edge = Shape::parse(&format!(
			"M{} {low_y} C{} {} {} {} {} {low_y}",
			centre.x - 34.0,
			centre.x - 14.0,
			low_y - arc,
			centre.x + 14.0,
			low_y - arc,
			centre.x + 34.0,
		))
		.map(warp);
		c.stroke(&edge.path(), INK, 4.4);
	}

	c.end();
}

fn draw_brow(c: &mut Svg, brow: Brow, side: f32, t: f32) {
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);
	let centre = pt(side * 36.0, -48.0 - brow.raise);
	let shape = Shape::parse(
		"M-17 3 C-10 -7 8 -9 18 -1 C19 2 16 5 13 4 C4 0 -6 2 -14 7 C-18 8 -19 5 -17 3 Z",
	)
	.map(|p| (pt(p.x * side, p.y)).rotate(-side * brow.tilt) + centre)
	.map(warp);
	c.brushed(&shape.path(), HAIR, INK, 2.0);
}

fn draw_nose(c: &mut Svg, t: f32, wrinkle: bool) {
	let warp = |p: Pt| wrap(p, t, FACE_RADIUS);
	// The nose stands proud of the face, so it travels a little further than the cylinder.
	let poke = pt(9.0 * t, 0.0);
	let nose = Shape::ellipse(pt(0.0, 22.0), 12.5, 10.0)
		.map(warp)
		.shift(poke);
	c.brush_fill(&nose.path(), NOSE, INK, DETAIL * 1.1);
	c.cel_shade(&nose, pt(-4.0, -4.0));
	c.brush_line(&nose.path(), INK, DETAIL * 1.1);
	c.fill_alpha(
		&Path::circle(warp(pt(-4.0, 18.0)) + poke, 2.6),
		Color(0xFF_FF_FF),
		0.7,
	);

	if wrinkle {
		for line in ["M-12 6 C-8 3 -4 3 -1 6", "M3 2 C6 0 10 0 13 3"] {
			c.stroke(&Shape::parse(line).map(warp).path(), INK, DETAIL);
		}
	}
}

/// The mouth chart: the mood mouths, plus the nine lip-sync shapes used while talking.
fn draw_mouth(c: &mut Svg, pose: &Pose, face: &Face, t: f32) {
	let centre = pt(0.0, 58.0);
	let curl = face.curl;
	// Talking mouths keep the mood: corners curl up or down a little.
	let curled = |p: Pt| pt(p.x, p.y - curl * 6.0 * (p.x / 30.0).powi(2));
	let place = |shape: &Shape, by: Pt| shape.map(|p| wrap(p + centre + by, t, FACE_RADIUS));

	let Some(viseme) = pose.viseme else {
		match face.mouth {
			MouthShape::Smile => {
				let outer = place(&Shape::parse(SMILE), pt(0.0, 0.0));
				open_mouth(
					c,
					&outer,
					None,
					Some(&place(
						&Shape::ellipse(pt(4.0, 28.0), 17.0, 11.0),
						pt(0.0, 0.0),
					)),
				);
				dimples(c, &place, 36.0, -6.0, -6.0);
			}
			MouthShape::Grin => {
				let outer = place(&Shape::parse(GRIN), pt(0.0, 0.0));
				open_mouth(
					c,
					&outer,
					Some(&place(&Shape::parse(GRIN_TEETH), pt(0.0, 0.0))),
					Some(&place(
						&Shape::ellipse(pt(6.0, 27.0), 20.0, 9.0),
						pt(0.0, 0.0),
					)),
				);
				c.stroke(
					&place(&Shape::parse("M2 0 C2 3 2 6 2 8"), pt(0.0, 0.0)).path(),
					TEETH.darken(0.25),
					1.8,
				);
				dimples(c, &place, 48.0, -4.0, -16.0);
			}
			MouthShape::Pout => {
				let by = pt(8.0, -2.0);
				c.fill(&place(&Shape::parse(POUT), by).path(), INK);
				c.stroke(&place(&Shape::parse(POUT_LIP), by).path(), INK, DETAIL);
			}
			MouthShape::Bleh => {
				let tilt = |p: Pt| p.rotate(-8.0);
				let by = pt(-4.0, 0.0);
				let outer = place(&Shape::parse(BLEH).map(tilt), by);
				open_mouth(c, &outer, None, None);
				let tongue = place(&Shape::parse(BLEH_TONGUE).map(tilt), by);
				c.brushed(&tongue.path(), TONGUE, INK, DETAIL * 1.2);
				c.stroke(
					&place(&Shape::parse(BLEH_GROOVE).map(tilt), by).path(),
					TONGUE.darken(0.3),
					DETAIL * 0.8,
				);
			}
		}

		return;
	};

	let mouth = |data: &str| place(&Shape::parse(data).map(curled), pt(0.0, 0.0));
	let ellipse = |c: Pt, rx: f32, ry: f32| place(&Shape::ellipse(c, rx, ry), pt(0.0, 0.0));

	match viseme {
		Viseme::Rest => c.fill(&mouth(REST).path(), INK),
		Viseme::Mbp => {
			c.fill(&mouth(MBP).path(), INK);
			c.stroke(&mouth(MBP_LIP).path(), INK, DETAIL);
		}
		Viseme::Teeth => {
			open_mouth(
				c,
				&mouth(TEETH_OPEN),
				Some(&teeth_band(&mouth, -6.0, 4.0)),
				None,
			);
		}
		Viseme::Open => {
			open_mouth(
				c,
				&mouth(OPEN),
				Some(&teeth_band(&mouth, -8.0, 2.0)),
				Some(&ellipse(pt(2.0, 22.0), 15.0, 8.0)),
			);
		}
		Viseme::Wide => {
			open_mouth(
				c,
				&mouth(WIDE),
				Some(&teeth_band(&mouth, -10.0, 1.0)),
				Some(&ellipse(pt(3.0, 34.0), 19.0, 10.0)),
			);
		}
		Viseme::Round => {
			open_mouth(
				c,
				&ellipse(pt(0.0, 9.0), 15.0, 18.0),
				None,
				Some(&ellipse(pt(0.0, 24.0), 11.0, 7.0)),
			);
		}
		Viseme::Pucker => {
			open_mouth(c, &ellipse(pt(0.0, 5.0), 8.0, 9.0), None, None);
		}
		Viseme::Fv => {
			open_mouth(c, &mouth(FV), Some(&teeth_band(&mouth, -6.0, 6.0)), None);
			c.stroke(&mouth("M-14 11 C-6 15 6 15 14 11").path(), INK, DETAIL);
		}
		Viseme::Tongue => {
			open_mouth(
				c,
				&mouth(OPEN),
				Some(&teeth_band(&mouth, -8.0, 0.0)),
				Some(&ellipse(pt(0.0, 6.0), 11.0, 8.0)),
			);
		}
	}
}

/// The upper teeth as one simple band between `top` and `bottom` (mouth units).
fn teeth_band(mouth: &dyn Fn(&str) -> Shape, top: f32, bottom: f32) -> Shape {
	mouth(&format!(
		"M-40 {top} C-14 {} 14 {} 40 {top} C40 {} 40 {} 40 {bottom} C14 {} -14 {} -40 {bottom} \
		 C-40 {} -40 {} -40 {top} Z",
		top + 6.0,
		top + 6.0,
		top + 4.0,
		bottom - 2.0,
		bottom + 6.0,
		bottom + 6.0,
		bottom - 2.0,
		top + 4.0,
	))
}

fn open_mouth(c: &mut Svg, outer: &Shape, teeth: Option<&Shape>, tongue: Option<&Shape>) {
	let path = outer.path();
	c.brush_fill(&path, MAW, INK, DETAIL * 1.4);
	c.begin_clip(&path);

	if let Some(tongue) = tongue {
		c.fill(&tongue.path(), TONGUE);
	}

	if let Some(teeth) = teeth {
		c.fill(&teeth.path(), TEETH);
	}

	c.end();
	c.brush_line(&path, INK, DETAIL * 1.4);
}

/// Little tucks at the corners of a smile.
fn dimples(c: &mut Svg, place: &dyn Fn(&Shape, Pt) -> Shape, half: f32, left_y: f32, right_y: f32) {
	for (x, y, side) in [(-half, left_y, -1.0_f32), (half, right_y, 1.0)] {
		let tuck = Shape::parse(&format!(
			"M{} {} C{} {} {} {} {} {}",
			x - side * 4.0,
			y - 6.0,
			x + side * 2.0,
			y - 4.0,
			x + side * 4.0,
			y,
			x + side * 2.0,
			y + 5.0
		));
		c.stroke(&place(&tuck, pt(0.0, 0.0)).path(), INK, DETAIL);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn turnable_parts_have_matching_front_and_three_quarter_drawings() {
		for (front, three) in [
			(HEAD_FRONT, HEAD_3Q),
			(HAIR_FRONT, HAIR_3Q),
			(TUFT_FRONT, TUFT_3Q),
			(TUFT_SHINE_FRONT, TUFT_SHINE_3Q),
			(JUMPER_FRONT, JUMPER_3Q),
			(COLLAR_FRONT, COLLAR_3Q),
			(SHORTS_FRONT, SHORTS_3Q),
		] {
			// Blending panics if the drawings don't share their commands.
			let _ = Shape::parse(front).lerp(&Shape::parse(three), 0.5);
		}
	}

	#[test]
	fn every_part_parses() {
		for data in [
			EAR,
			EAR_CURL,
			SLEEVE,
			SHORTS_LEG,
			SHOE_FRONT,
			SHOE_3Q,
			HAND_RELAXED,
			HAND_OPEN,
			HAND_FIST,
			SMILE,
			GRIN,
			GRIN_TEETH,
			POUT,
			POUT_LIP,
			BLEH,
			BLEH_TONGUE,
			BLEH_GROOVE,
			REST,
			MBP,
			MBP_LIP,
			TEETH_OPEN,
			OPEN,
			WIDE,
			FV,
		] {
			let _ = Shape::parse(data);
		}
	}

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
	fn hands_reach_their_targets_through_the_rig() {
		let pose = Pose::new(Mood::Happy, Arms::Wave, 0.0);
		let solved = Solved::new(&pose);
		let (shoulder, elbow, wrist) = solved.arm(&pose, -1.0);
		assert!(((elbow - shoulder).len() - LIMBS.upper_arm).abs() < 0.01);
		assert!((wrist - solved.torso(pose.hands.0)).len() < 0.5);
	}
}
