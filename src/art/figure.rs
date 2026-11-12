//! Kerfuffle folk: the puppet every person in the series is drawn with.
//!
//! This is cut-out ("puppet") animation, the technique behind most TV cartoons: each character
//! is built once from parts (head, eyes, a chart of mouth shapes, noodle arms, mitten hands) and
//! acts by swapping and moving those parts. The look (shapes, colours, hair, clothes) comes from
//! the cast file and never changes, which is what keeps characters on-model; the state (mood,
//! hands, mouth shape, head tilt) comes from the story and changes moment to moment.
//!
//! Hands are placed by target point rather than joint angle, so "hold your nose" stays correct
//! for every body, and arms bend like noodles when the hand comes close to the shoulder.

use std::f32::consts::PI;
use std::f32::consts::TAU;

use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::fx;
use super::lerp;
use super::noise;
use super::pack::StylePack;
use super::palette;
use super::pt;
use super::puppet;
use super::rig;
use super::rig::Gait;
use super::rig::Legs;
use super::rig::Limbs;
use super::rig::Stance;

/// Body proportions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
	Kid,
	Grownup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairStyle {
	Bald,
	Tuft,
	Bun,
	Curls,
	Pigtails,
	Bob,
	Spikes,
	Puff,
	Fringe,
}

/// Silhouette of the head. Distinct silhouettes are how a cast stays readable at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadShape {
	Round,
	Egg,
	Wide,
}

/// Silhouette of the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyShape {
	Bell,
	Bean,
	Pear,
	Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nose {
	Button,
	Round,
	Long,
	None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
	Plain,
	Stripes,
	Dots,
	Star,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Legwear {
	Shorts,
	Trousers,
	Skirt,
}

/// Optional extras that make a character recognisable at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
	Freckles,
	Glasses,
	Moustache,
	Apron,
	PostCap,
	Satchel,
	Bow,
	MuddyHands,
	Bogey,
	Lashes,
}

/// Everything about how a character looks that does not change during a story.
#[derive(Clone, Debug)]
pub struct Look {
	pub build: Build,
	pub head: HeadShape,
	pub body: BodyShape,
	pub nose: Nose,
	pub skin: Color,
	pub eyes: Color,
	pub hair: HairStyle,
	pub hair_color: Color,
	pub top: Color,
	pub pattern: Pattern,
	pub trim: Color,
	pub legwear: Legwear,
	pub bottom: Color,
	pub shoes: Color,
	pub extras: Vec<Extra>,
	/// Mouth width multiplier; Stinky's mouth is the star of his book.
	pub mouth_size: f32,
	pub seed: u64,
	/// The character's authored puppet (see [`puppet`](super::puppet)), once they have been
	/// redrawn; otherwise they are drawn by this module's parametric puppet.
	pub puppet: Option<puppet::Authored>,
}

impl Look {
	/// The body measurements this character is drawn and staged with: where the head, hands
	/// and mouth sit, and the rig's limb lengths. Everything outside this module asks the look
	/// rather than the build, so a character can bring its own measurements (as authored
	/// puppets will) without touching the stage, video or sheets.
	pub fn frame(&self) -> Frame {
		if let Some(authored) = &self.puppet {
			Frame::authored(&authored.style, self.build)
		} else {
			Frame::of(self.build)
		}
	}

	pub(crate) fn has(&self, extra: Extra) -> bool {
		self.extras.contains(&extra)
	}

	/// How wide the shoulders and hips are for this body shape, as multipliers.
	fn body_widths(&self) -> (f32, f32, f32) {
		match self.body {
			BodyShape::Bell => (1.0, 1.0, 0.7),
			BodyShape::Bean => (0.98, 0.92, 0.95),
			BodyShape::Pear => (0.82, 1.22, 0.8),
			BodyShape::Round => (1.08, 1.2, 1.1),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
	Happy,
	Grin,
	Laugh,
	Sad,
	Grumpy,
	Cross,
	Defiant,
	Shocked,
	Disgusted,
	Fainted,
	Dizzy,
	Sleepy,
	Dreamy,
	Worried,
	Proud,
}

impl Mood {
	pub const ALL: [(&'static str, Mood); 15] = [
		("happy", Mood::Happy),
		("grin", Mood::Grin),
		("laugh", Mood::Laugh),
		("sad", Mood::Sad),
		("grumpy", Mood::Grumpy),
		("cross", Mood::Cross),
		("defiant", Mood::Defiant),
		("shocked", Mood::Shocked),
		("disgusted", Mood::Disgusted),
		("fainted", Mood::Fainted),
		("dizzy", Mood::Dizzy),
		("sleepy", Mood::Sleepy),
		("dreamy", Mood::Dreamy),
		("worried", Mood::Worried),
		("proud", Mood::Proud),
	];

	pub fn parse(name: &str) -> Option<Mood> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, mood)| *mood)
	}

	pub fn name(self) -> &'static str {
		Self::ALL
			.iter()
			.find(|(_, mood)| *mood == self)
			.map_or("happy", |(name, _)| *name)
	}

	/// How much a talking mouth curls up (+1) or down (-1) in this mood.
	fn smile(self) -> f32 {
		match self {
			Mood::Happy | Mood::Grin | Mood::Laugh | Mood::Proud | Mood::Dreamy => 1.0,
			Mood::Sad | Mood::Grumpy | Mood::Worried | Mood::Disgusted => -1.0,
			Mood::Cross | Mood::Defiant => -0.4,
			Mood::Shocked | Mood::Fainted | Mood::Dizzy | Mood::Sleepy => 0.0,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Teeth {
	Clean,
	Grotty,
	Sparkly,
}

/// Something carried in the right hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
	Toothbrush,
	Parcel,
	Jam,
}

/// A mouth shape for lip-sync, following the Preston Blair chart that most 2D studios (and the
/// Rhubarb lip-sync tool) use: a handful of shapes is enough for speech to read clearly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viseme {
	/// Mouth at rest (Rhubarb X).
	Rest,
	/// Lips pressed: M, B, P (Rhubarb A).
	Mbp,
	/// Slightly open, teeth together: most consonants, K, S, T, EE (Rhubarb B).
	Teeth,
	/// Open: EH, AE, I (Rhubarb C).
	Open,
	/// Wide open: AA, AH (Rhubarb D).
	Wide,
	/// Rounded: AO, ER, O (Rhubarb E).
	Round,
	/// Puckered: UW, OW, W, Q (Rhubarb F).
	Pucker,
	/// Teeth on lip: F, V (Rhubarb G).
	Fv,
	/// Tongue up: L, TH (Rhubarb H).
	Tongue,
}

/// How a hand is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandShape {
	Open,
	Fist,
	Point,
}

/// Where a hand reaches, relative to a point on the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
	Shoulder,
	Chest,
	Hip,
	Nose,
	Mouth,
}

/// A hand target: an anchor plus an offset in reach units (the head radius, for the parametric
/// puppet). Positive `x` points away from the body on that hand's side, so one definition serves
/// both arms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
	pub anchor: Anchor,
	pub x: f32,
	pub y: f32,
}

const fn reach(anchor: Anchor, x: f32, y: f32) -> Reach {
	Reach { anchor, x, y }
}

/// Named arm positions authors can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
	Idle,
	Wave,
	Point,
	ArmsCrossed,
	Hips,
	Cheer,
	HoldNose,
	CoverMouth,
	Brush,
	Shrug,
	Droop,
	Hold,
	Think,
	ReachOut,
	Pick,
}

impl Pose {
	pub const ALL: [(&'static str, Pose); 15] = [
		("idle", Pose::Idle),
		("wave", Pose::Wave),
		("point", Pose::Point),
		("arms-crossed", Pose::ArmsCrossed),
		("hips", Pose::Hips),
		("cheer", Pose::Cheer),
		("hold-nose", Pose::HoldNose),
		("cover-mouth", Pose::CoverMouth),
		("brush", Pose::Brush),
		("shrug", Pose::Shrug),
		("droop", Pose::Droop),
		("hold", Pose::Hold),
		("think", Pose::Think),
		("reach-out", Pose::ReachOut),
		("pick", Pose::Pick),
	];

	pub fn parse(name: &str) -> Option<Pose> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, pose)| *pose)
	}

	/// Hand targets as `(left, right)`.
	fn reaches(self) -> (Reach, Reach) {
		let idle = reach(Anchor::Shoulder, 0.42, 1.0);

		match self {
			Pose::Idle => (idle, idle),
			Pose::Wave => (idle, reach(Anchor::Shoulder, 0.85, -1.05)),
			Pose::Point => (idle, reach(Anchor::Shoulder, 1.45, -0.25)),
			Pose::ArmsCrossed => {
				(
					reach(Anchor::Chest, -0.5, 0.32),
					reach(Anchor::Chest, -0.42, 0.12),
				)
			}
			Pose::Hips => {
				(
					reach(Anchor::Hip, 0.5, -0.15),
					reach(Anchor::Hip, 0.5, -0.15),
				)
			}
			Pose::Cheer => {
				(
					reach(Anchor::Shoulder, 0.75, -1.3),
					reach(Anchor::Shoulder, 0.75, -1.3),
				)
			}
			Pose::HoldNose => {
				(
					reach(Anchor::Shoulder, 1.05, -0.55),
					reach(Anchor::Nose, 0.12, 0.05),
				)
			}
			Pose::CoverMouth => {
				(
					reach(Anchor::Mouth, 0.08, 0.12),
					reach(Anchor::Mouth, 0.08, 0.02),
				)
			}
			Pose::Brush => {
				(
					reach(Anchor::Hip, 0.5, -0.15),
					reach(Anchor::Mouth, 0.55, 0.05),
				)
			}
			Pose::Shrug => {
				(
					reach(Anchor::Shoulder, 0.95, -0.3),
					reach(Anchor::Shoulder, 0.95, -0.3),
				)
			}
			Pose::Droop => {
				(
					reach(Anchor::Shoulder, 0.18, 1.15),
					reach(Anchor::Shoulder, 0.18, 1.15),
				)
			}
			Pose::Hold => (idle, reach(Anchor::Chest, 0.75, 0.25)),
			Pose::Think => {
				(
					reach(Anchor::Chest, -0.25, 0.35),
					reach(Anchor::Mouth, 0.1, 0.45),
				)
			}
			Pose::ReachOut => {
				(
					reach(Anchor::Shoulder, 1.25, 0.05),
					reach(Anchor::Shoulder, 1.25, 0.05),
				)
			}
			Pose::Pick => (idle, reach(Anchor::Nose, 0.0, 0.18)),
		}
	}

	/// How each hand is held in this pose, as `(left, right)`.
	pub fn hand_shapes(self) -> (HandShape, HandShape) {
		use HandShape::Fist;
		use HandShape::Open;
		use HandShape::Point;

		match self {
			Pose::Idle
			| Pose::Wave
			| Pose::CoverMouth
			| Pose::Shrug
			| Pose::Droop
			| Pose::ReachOut => (Open, Open),
			Pose::Point | Pose::Pick => (Open, Point),
			Pose::ArmsCrossed | Pose::Hips | Pose::Cheer | Pose::Brush => (Fist, Fist),
			Pose::HoldNose | Pose::Hold => (Open, Fist),
			Pose::Think => (Fist, Point),
		}
	}
}

/// Everything about a figure that can change from moment to moment.
#[derive(Clone, Debug)]
pub struct FigureState {
	pub mood: Mood,
	/// Hand positions in figure-local units, `(left, right)`.
	pub hands: (Pt, Pt),
	pub hand_shapes: (HandShape, HandShape),
	/// 0 = mouth at rest, 1 = wide open (talking or shouting).
	pub mouth_open: f32,
	/// The lip-sync mouth shape while speaking; `None` uses the mood's mouth.
	pub viseme: Option<Viseme>,
	/// Where the eyes look: -1 (left) to 1 (right).
	pub gaze: f32,
	/// Head and face turn: -1 (left) to 1 (right).
	pub facing: f32,
	/// 0..1, eyelids closing for a blink.
	pub blink: f32,
	/// Body lean in degrees.
	pub tilt: f32,
	/// Head tilt in degrees, on top of the body lean (nods, listening tilts).
	pub head_tilt: f32,
	/// Extra eyebrow lift for emphasis (head-radius units; positive raises).
	pub brow_lift: f32,
	/// Follow-through for hair and loose parts after a move: -1..1.
	pub sway: f32,
	/// Legs and spine: foot targets, pelvis drop and spine bend (see [`rig`](super::rig)).
	pub legs: Legs,
	/// A walk or run cycle and its phase in radians; `None` when not moving.
	pub gait: Option<(Gait, f32)>,
	/// 0 = standing, 1 = flat on their back.
	pub lie: f32,
	/// Vertical squash for hops and bounces (1 = normal). Width compensates to keep volume.
	pub squash: f32,
	pub teeth: Teeth,
	pub prop: Option<Prop>,
	/// Seconds since the scene started, for little idle motions.
	pub time: f32,
}

/// Fixed body measurements for a build, in figure-local units with the feet at the origin.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
	pub head: Pt,
	pub head_r: f32,
	body_top: f32,
	body_bottom: f32,
	shoulder_half: f32,
	hip_half: f32,
	pub shoulder_y: f32,
	leg_top: f32,
	leg_width: f32,
	pub arm_width: f32,
	pub arm_len: f32,
	hand_r: f32,
	limbs: Limbs,
	/// The nose and the middle of the mouth, relative to the head centre, front-on.
	nose: Pt,
	mouth: Pt,
	/// How far the face slides sideways per unit of `facing`.
	face_slide: f32,
	/// The units pose reaches are measured in (see [`Reach`]), across and up or down.
	reach: Pt,
}

impl Frame {
	/// A frame with nothing derived yet, for the constructors above to fill in.
	const UNSET: Frame = Frame {
		head: pt(0.0, 0.0),
		head_r: 0.0,
		body_top: 0.0,
		body_bottom: 0.0,
		shoulder_half: 0.0,
		hip_half: 0.0,
		shoulder_y: 0.0,
		leg_top: 0.0,
		leg_width: 0.0,
		arm_width: 0.0,
		arm_len: 0.0,
		hand_r: 0.0,
		limbs: Limbs {
			pelvis_y: 0.0,
			ankle_y: 0.0,
			hip_spread: 0.0,
			thigh: 0.0,
			shin: 0.0,
			upper_arm: 0.0,
			forearm: 0.0,
		},
		nose: pt(0.0, 0.0),
		mouth: pt(0.0, 0.0),
		face_slide: 0.0,
		reach: pt(0.0, 0.0),
	};

	pub fn of(build: Build) -> Frame {
		let frame = match build {
			Build::Kid => {
				Frame {
					head: pt(0.0, -300.0),
					head_r: 108.0,
					body_top: -212.0,
					body_bottom: -62.0,
					shoulder_half: 56.0,
					hip_half: 80.0,
					shoulder_y: -180.0,
					leg_top: -76.0,
					leg_width: 28.0,
					arm_width: 23.0,
					arm_len: 132.0,
					hand_r: 20.0,
					..Frame::UNSET
				}
			}
			Build::Grownup => {
				Frame {
					head: pt(0.0, -515.0),
					head_r: 98.0,
					body_top: -432.0,
					body_bottom: -168.0,
					shoulder_half: 70.0,
					hip_half: 98.0,
					shoulder_y: -398.0,
					leg_top: -180.0,
					leg_width: 33.0,
					arm_width: 26.0,
					arm_len: 190.0,
					hand_r: 22.0,
					..Frame::UNSET
				}
			}
		};

		Frame {
			limbs: frame.build_limbs(),
			nose: pt(0.0, frame.head_r * 0.12),
			mouth: pt(0.0, frame.head_r * 0.45),
			face_slide: frame.head_r * 0.2,
			reach: pt(frame.head_r, frame.head_r),
			..frame
		}
	}

	/// The measurements an authored puppet is drawn to (see [`puppet`](super::puppet)).
	pub fn authored(style: &StylePack, build: Build) -> Frame {
		let body = style.body(build);
		let limbs = body.limbs;

		Frame {
			head: body.neck + pt(0.0, -body.head_above_neck),
			head_r: 100.0,
			body_top: body.neck.y - 6.0,
			body_bottom: body.hem_y + 13.0,
			shoulder_half: body.shoulder_half,
			hip_half: body.shoulder_half,
			shoulder_y: body.shoulder_y,
			leg_top: limbs.pelvis_y + 32.0,
			leg_width: body.leg_width.0 - 1.5,
			arm_width: body.forearm_width.0,
			arm_len: limbs.upper_arm + limbs.forearm,
			hand_r: 13.0 * body.hand_scale,
			limbs,
			// Where a hand reaching for the nose puts its wrist: a hand's length below the nose,
			// so the fingers, not the wrist, are on it.
			nose: style.features.nose + pt(0.0, 28.0),
			mouth: style.features.mouth,
			face_slide: style.face.slide,
			reach: body.reach,
		}
	}

	/// Total height including a typical hairdo, used for framing.
	pub fn height(&self) -> f32 {
		-(self.head.y - self.head_r * 1.25)
	}

	/// Joint measurements for the rig.
	pub fn limbs(&self) -> Limbs {
		self.limbs
	}

	/// Where the middle of the mouth is when the face is turned by `facing`.
	pub fn mouth_at(&self, facing: f32) -> Pt {
		self.head + self.mouth + pt(facing * self.face_slide, 0.0)
	}

	/// The parametric puppet's limbs, worked out from its body measurements.
	fn build_limbs(&self) -> Limbs {
		let ankle_y = -24.0;
		// The hip joints sit up inside the body (hidden by it), the way animation rigs give
		// short cartoon legs enough bone to bend visibly at the knee.
		let pelvis_y = self.leg_top - self.head_r * 0.32;
		let leg = ankle_y - pelvis_y;

		Limbs {
			pelvis_y,
			ankle_y,
			hip_spread: self.hip_half * 0.42,
			// A touch longer than the gap, so standing legs are relaxed rather than locked.
			thigh: leg * 0.506,
			shin: leg * 0.506,
			upper_arm: self.arm_len * 0.51,
			forearm: self.arm_len * 0.51,
		}
	}

	/// Half the width of the feet, for contact shadows.
	pub fn stance(&self) -> f32 {
		self.hip_half * 1.25
	}

	/// Where an arm hangs from, for rules that swing a hand on an arc about the shoulder.
	pub fn shoulder(&self, side: f32) -> Pt {
		pt(side * self.shoulder_half, self.shoulder_y)
	}

	fn anchor(&self, anchor: Anchor, side: f32, facing: f32) -> Pt {
		let face = self.head + pt(facing * self.face_slide, 0.0);

		match anchor {
			Anchor::Shoulder => self.shoulder(side),
			Anchor::Chest => pt(0.0, self.shoulder_y + 40.0),
			Anchor::Hip => pt(side * self.hip_half * 0.8, self.body_bottom - 25.0),
			Anchor::Nose => face + self.nose,
			Anchor::Mouth => face + self.mouth,
		}
	}

	fn resolve(&self, reach: Reach, side: f32, facing: f32) -> Pt {
		self.anchor(reach.anchor, side, facing)
			+ pt(side * reach.x * self.reach.x, reach.y * self.reach.y)
	}

	/// Hand positions for a named pose.
	pub fn hands(&self, pose: Pose, facing: f32) -> (Pt, Pt) {
		let (left, right) = pose.reaches();

		(
			self.resolve(left, -1.0, facing),
			self.resolve(right, 1.0, facing),
		)
	}
}

impl FigureState {
	/// Blends towards `other`; `t` = 0 gives `self`, 1 gives `other`. Numbers interpolate, so
	/// hands, feet, pelvis and spine all travel; moods snap early and hand shapes change halfway.
	#[must_use]
	pub fn blend(&self, other: &FigureState, t: f32) -> FigureState {
		let mut state = other.clone();
		state.hands = blend_hands(self.hands, other.hands, t);
		state.lie = lerp(self.lie, other.lie, t);
		state.tilt = lerp(self.tilt, other.tilt, t);
		state.facing = lerp(self.facing, other.facing, t);
		state.gaze = lerp(self.gaze, other.gaze, t);
		state.mouth_open = lerp(self.mouth_open, other.mouth_open, t);
		state.head_tilt = lerp(self.head_tilt, other.head_tilt, t);
		state.brow_lift = lerp(self.brow_lift, other.brow_lift, t);
		state.sway = lerp(self.sway, other.sway, t);
		state.legs = Legs {
			feet: (
				self.legs.feet.0.lerp(other.legs.feet.0, t),
				self.legs.feet.1.lerp(other.legs.feet.1, t),
			),
			crouch: lerp(self.legs.crouch, other.legs.crouch, t),
			bend: lerp(self.legs.bend, other.legs.bend, t),
		};

		if t < 0.25 {
			state.mood = self.mood;
		}

		if t < 0.5 {
			state.hand_shapes = self.hand_shapes;
			state.gait = self.gait;
		}

		state
	}

	/// A neutral standing figure with the body measurements in `frame`.
	pub fn new(frame: &Frame) -> Self {
		Self {
			mood: Mood::Happy,
			hands: frame.hands(Pose::Idle, 0.0),
			hand_shapes: Pose::Idle.hand_shapes(),
			mouth_open: 0.0,
			viseme: None,
			gaze: 0.0,
			facing: 0.0,
			blink: 0.0,
			tilt: 0.0,
			head_tilt: 0.0,
			brow_lift: 0.0,
			sway: 0.0,
			legs: frame.limbs().stance(Stance::Stand, 0.0),
			gait: None,
			lie: 0.0,
			squash: 1.0,
			teeth: Teeth::Clean,
			prop: None,
			time: 0.0,
		}
	}
}

/// The soft contact shadow under a standing figure, drawn at the ground point. `lift` is how far
/// the figure is off the ground (hops), which shrinks and fades the shadow.
pub fn shadow(svg: &mut Svg, look: &Look, state: &FigureState, lift: f32) {
	if let Some(authored) = &look.puppet {
		puppet::shadow(svg, look, authored, state, lift);
		return;
	}

	let frame = look.frame();
	let feet = (state.legs.feet.1.x - state.legs.feet.0.x).abs() / 2.0 + frame.leg_width * 1.4;
	let spread = feet.max(frame.stance()) * (1.0 + state.lie * 1.8);
	let away = (1.0 - lift / (frame.head_r * 4.0)).clamp(0.3, 1.0);
	let shadow = Path::ellipse(
		pt(-state.lie * frame.head_r * 1.4, 0.0),
		spread * away,
		frame.stance() * 0.2 * away,
	);
	svg.fill_alpha(&shadow, palette::INK, 0.16 * away);
}

/// The rig solved for one moment: the legs (with any gait applied) and where the torso sits.
struct Posed {
	legs: Legs,
	/// Arm counter-swing from the gait, added to the hand targets.
	swing: (Pt, Pt),
	/// Pelvis drop in figure units.
	drop: f32,
	/// The pelvis at rest, which the spine bends around.
	pivot: Pt,
	limbs: Limbs,
}

impl Posed {
	fn solve(frame: &Frame, state: &FigureState) -> Posed {
		let limbs = frame.limbs();
		let (legs, swing) = match state.gait {
			Some((gait, phase)) => limbs.cycle(gait, phase, state.facing, state.legs),
			None => (state.legs, (pt(0.0, 0.0), pt(0.0, 0.0))),
		};

		Posed {
			legs,
			swing,
			drop: legs.crouch * limbs.leg_length(),
			pivot: pt(0.0, limbs.pelvis_y),
			limbs,
		}
	}

	/// Where a point on the torso (given at rest) ends up after the pelvis drops and the spine
	/// bends.
	fn torso(&self, rest: Pt) -> Pt {
		(rest - self.pivot).rotate(self.legs.bend) + self.pivot + pt(0.0, self.drop)
	}

	/// The same as an SVG transform, for drawing the torso, head and hair.
	fn transform(&self) -> String {
		format!(
			"translate(0 {:.2}) rotate({:.2} {:.1} {:.1})",
			self.drop, self.legs.bend, self.pivot.x, self.pivot.y
		)
	}
}

/// Draws a figure with its feet at the current origin.
pub fn draw(svg: &mut Svg, look: &Look, state: &FigureState) {
	if let Some(authored) = &look.puppet {
		puppet::draw(svg, look, authored, state);
		return;
	}

	let frame = look.frame();
	let posed = Posed::solve(&frame, state);
	let lie_lift = state.lie * frame.head_r * 0.95;
	let lie_turn = -state.lie * 90.0 * if state.facing < 0.0 { -1.0 } else { 1.0 };
	let breathe =
		1.0 + (state.time * 2.4 + look.seed as f32 % 7.0).sin() * 0.012 * (1.0 - state.lie);
	let squash = state.squash * breathe;
	// Squash and stretch keep their volume: what flattens must widen.
	let stretch = 1.0 / squash.max(0.2).sqrt();
	let neck = pt(0.0, frame.body_top + 14.0);
	let head_turn = format!("rotate({:.2} {:.1} {:.1})", state.head_tilt, neck.x, neck.y);
	let torso = posed.transform();

	svg.begin(&format!(
		"translate(0 {:.1}) rotate({:.2}) rotate({:.2}) scale({stretch:.4} {squash:.4})",
		-lie_lift, lie_turn, state.tilt
	));

	svg.begin(&torso);
	svg.begin(&head_turn);
	draw_back_hair(svg, look, &frame, state);
	svg.end();
	svg.end();

	draw_legs(svg, look, &frame, state, &posed);

	svg.begin(&torso);
	draw_body(svg, look, &frame);
	svg.begin(&head_turn);
	draw_head(svg, look, &frame, state);
	draw_face(svg, look, &frame, state);
	draw_front_hair(svg, look, &frame, state);
	draw_extras_on_head(svg, look, &frame, state);
	svg.end();
	svg.end();

	draw_arms(svg, look, &frame, state, &posed);

	if matches!(state.mood, Mood::Fainted | Mood::Dizzy) {
		let above = posed.torso(frame.head + pt(0.0, -frame.head_r * 1.15));
		fx::dizzy_stars(svg, above, frame.head_r, state.time);
	}

	svg.end();
}

/// Draws the rig's bones and joints over a figure, for model sheets and rig checks.
pub fn draw_skeleton(svg: &mut Svg, look: &Look, state: &FigureState) {
	if let Some(authored) = &look.puppet {
		puppet::draw_bones(svg, look, authored, state);
		return;
	}

	let frame = look.frame();
	let posed = Posed::solve(&frame, state);
	let (shoulders, hips, _) = look.body_widths();
	let bone = palette::TOMATO;
	let joint = palette::SUNFLOWER;
	let neck = posed.torso(pt(0.0, frame.body_top + 14.0));
	let head = posed.torso(frame.head);
	let chest = posed.torso(pt(0.0, frame.shoulder_y));
	let pelvis = posed.torso(posed.pivot);
	let mut bones = vec![(pelvis, chest), (chest, neck), (neck, head)];
	let mut joints = vec![pelvis, chest, neck, head];
	let hands = arm_targets(state, &posed);

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		let hip = posed.torso(pt(
			side * posed.limbs.hip_spread * hips.min(1.1),
			posed.limbs.pelvis_y,
		));
		let foot = if index == 0 {
			posed.legs.feet.0
		} else {
			posed.legs.feet.1
		};
		let (knee, ankle) = rig::two_bone(
			hip,
			foot,
			posed.limbs.thigh,
			posed.limbs.shin,
			knee_pole(side, state.facing),
		);
		let shoulder = posed.torso(pt(side * frame.shoulder_half * shoulders, frame.shoulder_y));
		let hand = if index == 0 { hands.0 } else { hands.1 };
		let (elbow, wrist) = rig::two_bone(
			shoulder,
			hand,
			posed.limbs.upper_arm,
			posed.limbs.forearm,
			pt(side * 0.6, 0.8),
		);
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

	svg.begin(&format!(
		"translate(0 {:.1}) rotate({:.2}) rotate({:.2})",
		-state.lie * frame.head_r * 0.95,
		-state.lie * 90.0 * if state.facing < 0.0 { -1.0 } else { 1.0 },
		state.tilt
	));

	for (from, to) in bones {
		svg.stroke_alpha(&Path::new().move_to(from).line_to(to), bone, 9.0, 0.9);
	}

	for point in joints {
		svg.fill(&Path::circle(point, 11.0), joint);
		svg.stroke_alpha(&Path::circle(point, 11.0), palette::INK, 4.0, 1.0);
	}

	svg.end();
}

/// Knees bend outwards when facing the reader and forwards when turned.
fn knee_pole(side: f32, facing: f32) -> Pt {
	pt(
		side * (1.0 - facing.abs()) + facing.signum() * facing.abs() * 1.2,
		-0.25,
	)
	.norm()
}

/// Hand targets after the torso has moved, plus any gait counter-swing.
fn arm_targets(state: &FigureState, posed: &Posed) -> (Pt, Pt) {
	(
		posed.torso(state.hands.0) + posed.swing.0,
		posed.torso(state.hands.1) + posed.swing.1,
	)
}

fn silhouette(svg: &Svg) -> f32 {
	svg.line * 1.3
}

fn detail(svg: &Svg) -> f32 {
	svg.line * 0.85
}

fn draw_legs(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState, posed: &Posed) {
	let leg_color = match look.legwear {
		Legwear::Trousers => look.bottom,
		Legwear::Shorts | Legwear::Skirt => look.skin,
	};
	let (_, hips, _) = look.body_widths();
	let limbs = posed.limbs;

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		let hip = posed.torso(pt(side * limbs.hip_spread * hips.min(1.1), limbs.pelvis_y));
		let foot = if index == 0 {
			posed.legs.feet.0
		} else {
			posed.legs.feet.1
		};
		let (knee, ankle) = rig::two_bone(
			hip,
			foot,
			limbs.thigh,
			limbs.shin,
			knee_pole(side, state.facing),
		);
		let leg = rig::limb_curve(hip, knee, ankle, 8);
		let line = silhouette(svg) * 0.85;
		svg.inked_weight(
			&Path::taper(&leg, frame.leg_width * 1.2, frame.leg_width * 0.85),
			leg_color,
			line,
		);

		if look.legwear == Legwear::Shorts {
			svg.inked_weight(
				&Path::taper(&leg[..=5], frame.leg_width * 1.55, frame.leg_width * 1.45),
				look.bottom,
				line,
			);
		}

		// A lifted foot tips its toe down, the way a step leaves the ground.
		let lifted = (limbs.ankle_y - ankle.y).max(0.0) / limbs.leg_length();
		let toe = if state.facing < -0.05 { -1.0 } else { 1.0 };
		draw_shoe(
			svg,
			look,
			frame,
			ankle + pt(side * frame.hip_half * 0.12, 8.0),
			side,
			lifted * 25.0 * toe,
		);
	}
}

fn draw_shoe(svg: &mut Svg, look: &Look, frame: &Frame, c: Pt, side: f32, angle: f32) {
	svg.begin(&format!("rotate({angle:.1} {:.1} {:.1})", c.x, c.y));
	let shoe = Path::wobbly_ellipse(
		c + pt(side * 4.0, 0.0),
		frame.leg_width * 1.3,
		frame.leg_width * 0.7,
		0.04,
		svg.seed(look.seed + side as u64 + 11),
	);
	let clip = svg.next_id("shoe");
	svg.define(&format!(
		r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
		shoe.as_str()
	));
	svg.fill(&shoe, look.shoes);
	svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
	svg.fill(
		&Path::rect(c.x - 80.0, c.y + frame.leg_width * 0.32, 160.0, 40.0),
		look.shoes.lighten(0.45),
	);
	svg.fill(
		&Path::circle(
			c + pt(-side * frame.leg_width * 0.3, -frame.leg_width * 0.3),
			frame.leg_width * 0.22,
		),
		look.shoes.lighten(0.25),
	);
	svg.raw("</g>");
	let line = silhouette(svg);
	svg.outline(&shoe, line);
	svg.end();
}

fn body_path(svg: &Svg, look: &Look, frame: &Frame) -> Path {
	let (shoulders, hips, round) = look.body_widths();
	let flare = if look.legwear == Legwear::Skirt {
		1.25
	} else {
		1.0
	};
	let bottom = if look.legwear == Legwear::Skirt {
		frame.body_bottom + 18.0
	} else {
		frame.body_bottom
	};
	let corners = [
		pt(-frame.shoulder_half * shoulders, frame.body_top),
		pt(frame.shoulder_half * shoulders, frame.body_top),
		pt(frame.hip_half * hips * flare, bottom),
		pt(-frame.hip_half * hips * flare, bottom),
	];

	Path::wobbly_poly(
		&corners,
		frame.shoulder_half * round,
		3.0,
		svg.seed(look.seed + 3),
	)
}

fn draw_body(svg: &mut Svg, look: &Look, frame: &Frame) {
	let body = body_path(svg, look, frame);
	svg.cast_shadow(&body);
	let clip = svg.next_id("body");
	svg.define(&format!(
		r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
		body.as_str()
	));

	let offset = svg.registration;
	svg.begin(&format!("translate({:.1} {:.1})", offset.x, offset.y));
	svg.fill(&body, look.top);
	svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));
	draw_pattern(svg, look, frame);

	if look.legwear != Legwear::Trousers || look.has(Extra::Apron) {
		// A band at the hem suggests shorts, a skirt or an apron without extra geometry.
		let hem = frame.body_bottom - 34.0;
		svg.fill(
			&Path::rect(-200.0, hem, 400.0, 80.0),
			if look.has(Extra::Apron) {
				palette::CREAM
			} else {
				look.bottom
			},
		);
	}

	// One tone of shadow down the far side gives the body volume.
	let (_, hips, _) = look.body_widths();
	let shade = Path::blob(&[
		pt(frame.hip_half * hips * 0.55, frame.body_top - 10.0),
		pt(frame.hip_half * hips * 1.6, frame.body_top),
		pt(frame.hip_half * hips * 1.6, frame.body_bottom + 30.0),
		pt(frame.hip_half * hips * 0.75, frame.body_bottom + 30.0),
		pt(
			frame.hip_half * hips * 0.62,
			f32::midpoint(frame.body_top, frame.body_bottom),
		),
	]);
	svg.fill_alpha(&shade, palette::INK, 0.13);

	// The collar peeks out under the chin.
	let collar_y = frame.head.y + frame.head_r * 0.96;
	let collar = Path::new()
		.move_to(pt(-frame.head_r * 0.62, collar_y - 8.0))
		.quad_to(
			pt(0.0, collar_y + frame.head_r * 0.42),
			pt(frame.head_r * 0.62, collar_y - 8.0),
		)
		.line_to(pt(frame.head_r * 0.62, collar_y - 40.0))
		.line_to(pt(-frame.head_r * 0.62, collar_y - 40.0))
		.close();
	svg.fill(&collar, look.trim);
	svg.raw("</g>");
	svg.end();

	svg.finish_shape(&body, look.top);
	let line = silhouette(svg);
	svg.outline(&body, line);
	let collar_line = Path::new()
		.move_to(pt(-frame.head_r * 0.62, collar_y - 8.0))
		.quad_to(
			pt(0.0, collar_y + frame.head_r * 0.42),
			pt(frame.head_r * 0.62, collar_y - 8.0),
		);
	svg.stroke(&collar_line, palette::INK, detail(svg));

	if look.has(Extra::Satchel) {
		let strap = Path::new()
			.move_to(pt(-frame.shoulder_half * 0.9, frame.body_top + 10.0))
			.line_to(pt(frame.hip_half * 0.85, frame.body_bottom - 60.0));
		svg.inked_noodle(&strap, palette::COCOA, 14.0);
		let bag = Path::wobbly_poly(
			&[
				pt(frame.hip_half * 0.45, frame.body_bottom - 80.0),
				pt(frame.hip_half * 1.35, frame.body_bottom - 80.0),
				pt(frame.hip_half * 1.3, frame.body_bottom + 5.0),
				pt(frame.hip_half * 0.5, frame.body_bottom + 5.0),
			],
			12.0,
			2.0,
			svg.seed(look.seed + 23),
		);
		svg.inked(&bag, palette::TOMATO);
		let flap = Path::new()
			.move_to(pt(frame.hip_half * 0.45, frame.body_bottom - 80.0))
			.quad_to(
				pt(frame.hip_half * 0.9, frame.body_bottom - 30.0),
				pt(frame.hip_half * 1.35, frame.body_bottom - 80.0),
			);
		svg.stroke(&flap, palette::INK, detail(svg));
	}
}

fn draw_pattern(svg: &mut Svg, look: &Look, frame: &Frame) {
	match look.pattern {
		Pattern::Plain => {}
		Pattern::Stripes => {
			let mut y = frame.body_top + 26.0;

			while y < frame.body_bottom {
				svg.fill(&Path::rect(-200.0, y, 400.0, 18.0), look.trim);
				y += 42.0;
			}
		}
		Pattern::Dots => {
			for row in 0..6 {
				for col in -3..=3 {
					let x = col as f32 * 34.0 + if row % 2 == 0 { 0.0 } else { 17.0 };
					let y = frame.body_top + 22.0 + row as f32 * 32.0;
					svg.fill(&Path::circle(pt(x, y), 7.0), look.trim);
				}
			}
		}
		Pattern::Star => {
			let c = pt(0.0, f32::midpoint(frame.body_top, frame.body_bottom) + 6.0);
			svg.fill(&star_path(c, 34.0, 15.0, 5), look.trim);
		}
	}
}

/// A star with `points` spikes.
pub fn star_path(c: Pt, outer: f32, inner: f32, points: usize) -> Path {
	let mut path = Path::new();

	for i in 0..points * 2 {
		let r = if i % 2 == 0 { outer } else { inner };
		let angle = (i as f32 / (points * 2) as f32) * TAU - std::f32::consts::FRAC_PI_2;
		let p = c + pt(angle.cos(), angle.sin()) * r;
		path = if i == 0 {
			path.move_to(p)
		} else {
			path.line_to(p)
		};
	}

	path.close()
}

fn head_path(svg: &Svg, look: &Look, frame: &Frame) -> Path {
	let r = frame.head_r;
	let (rx, ry, egg) = match look.head {
		HeadShape::Round => (1.02, 1.0, 0.0),
		HeadShape::Egg => (0.93, 1.08, 0.1),
		HeadShape::Wide => (1.15, 0.92, -0.04),
	};
	let seed = svg.seed(look.seed + 1);
	let steps = 18;
	let points: Vec<Pt> = (0..steps)
		.map(|i| {
			let angle = i as f32 / steps as f32 * TAU;
			// Egg heads are fuller below the eyes (positive sine is down the page).
			let fullness = 1.0 + egg * angle.sin();
			let jitter = 1.0 + noise(seed, i) * 0.012;
			frame.head + pt(angle.cos() * rx * fullness, angle.sin() * ry) * (r * jitter)
		})
		.collect();

	Path::blob(&points)
}

fn draw_head(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState) {
	let r = frame.head_r;
	let ear_shift = -state.facing * r * 0.12;
	let width = match look.head {
		HeadShape::Wide => 1.12,
		HeadShape::Egg => 0.92,
		HeadShape::Round => 1.0,
	};

	for side in [-1.0, 1.0] {
		let c = frame.head + pt(side * r * 0.98 * width + ear_shift, r * 0.08);
		let ear = Path::wobbly_ellipse(
			c,
			r * 0.2,
			r * 0.25,
			0.05,
			svg.seed(look.seed + 40 + side as u64),
		);
		svg.inked_weight(&ear, look.skin, silhouette(svg));
		let inner = Path::new()
			.move_to(c + pt(-side * r * 0.02, -r * 0.1))
			.quad_to(
				c + pt(side * r * 0.08, 0.0),
				c + pt(-side * r * 0.02, r * 0.1),
			);
		svg.stroke(&inner, look.skin.darken(0.3), detail(svg));
	}

	let head = head_path(svg, look, frame);
	// The crescent of light on the head follows the scene's key light when one is up, and
	// comes from the top left as always in flat daylight.
	let towards = svg.light.map_or(pt(-0.22, -0.2), |scene| {
		let key = scene.key.norm();
		pt(key.x * 0.3, key.y * 0.3)
	});
	let light = frame.head + pt(towards.x * r, towards.y * r);
	let line = silhouette(svg);
	svg.shaded(
		&head,
		look.skin,
		look.skin.darken(0.12),
		light,
		r * 1.04,
		line,
	);
}

fn draw_face(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState) {
	let r = frame.head_r;
	let face = frame.head + pt(state.facing * r * 0.2, 0.0);
	let style = FaceStyle::of(state.mood);

	for side in [-1.0, 1.0] {
		let cheek = Path::circle(face + pt(side * r * 0.6, r * 0.32), r * 0.17);
		svg.fill_alpha(&cheek, palette::BLUSH, 0.5);
	}

	if look.has(Extra::Freckles) {
		for side in [-1.0, 1.0] {
			for i in 0..3 {
				let dot = face
					+ pt(
						side * (r * 0.5 + i as f32 * r * 0.1),
						r * (0.22 + (i % 2) as f32 * 0.08),
					);
				svg.fill(&Path::circle(dot, r * 0.03), look.skin.darken(0.45));
			}
		}
	}

	for side in [-1.0, 1.0] {
		draw_eye(
			svg,
			look,
			face + pt(side * r * 0.36, -r * 0.08),
			r,
			side,
			style.eyes,
			state,
		);
		draw_brow(
			svg,
			face + pt(side * r * 0.36, -r * 0.42 - state.brow_lift * r),
			r,
			side,
			style.brows,
		);
	}

	draw_nose(svg, look, face, r, state);

	if look.has(Extra::Bogey) {
		let drip = Path::wobbly_ellipse(
			face + pt(r * 0.05, r * 0.28),
			r * 0.05,
			r * 0.08,
			0.1,
			svg.seed(look.seed + 6),
		);
		svg.inked(&drip, palette::MEADOW);
	}

	draw_mouth(svg, look, face + pt(0.0, r * 0.45), r, style.mouth, state);

	if look.has(Extra::Moustache) {
		let tache_c = face + pt(0.0, r * 0.3);
		let tache = Path::blob(&[
			tache_c + pt(-r * 0.42, r * 0.08),
			tache_c + pt(-r * 0.2, -r * 0.1),
			tache_c + pt(0.0, -r * 0.02),
			tache_c + pt(r * 0.2, -r * 0.1),
			tache_c + pt(r * 0.42, r * 0.08),
			tache_c + pt(r * 0.15, r * 0.06),
			tache_c + pt(0.0, r * 0.1),
			tache_c + pt(-r * 0.15, r * 0.06),
		]);
		svg.inked(&tache, look.hair_color);
	}

	if look.has(Extra::Glasses) {
		let line = svg.line * 0.9;

		for side in [-1.0, 1.0] {
			let lens = Path::circle(face + pt(side * r * 0.36, -r * 0.08), r * 0.29);
			svg.fill_alpha(&lens, palette::WHITE, 0.18);
			svg.stroke(&lens, palette::INK, line);
		}

		let bridge = Path::new()
			.move_to(face + pt(-r * 0.08, -r * 0.12))
			.quad_to(face + pt(0.0, -r * 0.2), face + pt(r * 0.08, -r * 0.12));
		svg.stroke(&bridge, palette::INK, line);
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Eyes {
	Open,
	Wide,
	Lidded,
	Happy,
	Shut,
	Cross,
	Swirl,
	Squint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Brows {
	None,
	Neutral,
	Raised,
	Angry,
	Worried,
	Smug,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MouthShape {
	Smile,
	Grin,
	Frown,
	Oh,
	Wavy,
	Smirk,
	Flat,
	Tongue,
	Shout,
}

struct FaceStyle {
	eyes: Eyes,
	brows: Brows,
	mouth: MouthShape,
}

impl FaceStyle {
	fn of(mood: Mood) -> FaceStyle {
		let (eyes, brows, mouth) = match mood {
			Mood::Happy => (Eyes::Open, Brows::Neutral, MouthShape::Smile),
			Mood::Grin => (Eyes::Open, Brows::Raised, MouthShape::Grin),
			Mood::Laugh => (Eyes::Happy, Brows::Raised, MouthShape::Grin),
			Mood::Sad => (Eyes::Open, Brows::Worried, MouthShape::Frown),
			Mood::Grumpy => (Eyes::Lidded, Brows::Angry, MouthShape::Frown),
			Mood::Cross => (Eyes::Open, Brows::Angry, MouthShape::Shout),
			Mood::Defiant => (Eyes::Lidded, Brows::Smug, MouthShape::Smirk),
			Mood::Shocked => (Eyes::Wide, Brows::Raised, MouthShape::Oh),
			Mood::Disgusted => (Eyes::Squint, Brows::Angry, MouthShape::Wavy),
			Mood::Fainted => (Eyes::Cross, Brows::None, MouthShape::Tongue),
			Mood::Dizzy => (Eyes::Swirl, Brows::None, MouthShape::Wavy),
			Mood::Sleepy => (Eyes::Shut, Brows::Neutral, MouthShape::Flat),
			Mood::Dreamy => (Eyes::Happy, Brows::Raised, MouthShape::Smile),
			Mood::Worried => (Eyes::Open, Brows::Worried, MouthShape::Wavy),
			Mood::Proud => (Eyes::Happy, Brows::Smug, MouthShape::Grin),
		};

		FaceStyle { eyes, brows, mouth }
	}
}

fn draw_eye(svg: &mut Svg, look: &Look, c: Pt, r: f32, side: f32, eyes: Eyes, state: &FigureState) {
	let line = detail(svg);

	match eyes {
		Eyes::Open | Eyes::Wide | Eyes::Lidded => {
			let (rx, ry) = if eyes == Eyes::Wide {
				(r * 0.21, r * 0.27)
			} else {
				(r * 0.18, r * 0.23)
			};
			let resting_lid = if eyes == Eyes::Lidded { 0.48 } else { 0.0 };
			let lid = state.blink.max(resting_lid);

			if lid > 0.92 {
				let closed = Path::new()
					.move_to(c + pt(-rx, 0.0))
					.quad_to(c + pt(0.0, ry * 0.6), c + pt(rx, 0.0));
				svg.stroke(&closed, palette::INK, line * 1.3);
				return;
			}

			let white = Path::ellipse(c, rx, ry);
			let clip = svg.next_id("eye");
			svg.define(&format!(
				r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
				white.as_str()
			));
			svg.fill(&white, palette::WHITE);
			svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));

			let look_down = if state.mood == Mood::Sad {
				r * 0.07
			} else {
				0.0
			};
			let iris_r = if eyes == Eyes::Wide {
				rx * 0.58
			} else {
				rx * 0.72
			};
			let iris = c + pt(state.gaze * rx * 0.4, look_down + ry * 0.08);
			svg.fill(&Path::circle(iris, iris_r), look.eyes);
			svg.fill(&Path::circle(iris, iris_r * 0.55), palette::INK);
			svg.fill(
				&Path::circle(iris + pt(-iris_r * 0.32, -iris_r * 0.36), iris_r * 0.3),
				palette::WHITE,
			);
			svg.fill(
				&Path::circle(iris + pt(iris_r * 0.38, iris_r * 0.3), iris_r * 0.13),
				palette::WHITE,
			);

			if lid > 0.02 {
				let edge = c.y - ry + lid * ry * 2.0;
				let lid_shape = Path::new()
					.move_to(pt(c.x - rx * 1.3, c.y - ry * 1.3))
					.line_to(pt(c.x + rx * 1.3, c.y - ry * 1.3))
					.line_to(pt(c.x + rx * 1.3, edge))
					.quad_to(pt(c.x, edge + ry * 0.18), pt(c.x - rx * 1.3, edge))
					.close();
				svg.fill(&lid_shape, look.skin.darken(0.06));
			}

			svg.raw("</g>");

			if lid > 0.02 {
				let edge = c.y - ry + lid * ry * 2.0;
				let lid_line = Path::new()
					.move_to(pt(c.x - rx, edge))
					.quad_to(pt(c.x, edge + ry * 0.18), pt(c.x + rx, edge));
				svg.stroke(&lid_line, palette::INK, line * 1.2);
			}

			svg.stroke(&white, palette::INK, line);

			if look.has(Extra::Lashes) {
				for k in 0..2 {
					let base = c + pt(
						side * rx * (0.55 + k as f32 * 0.35),
						-ry * (0.85 - k as f32 * 0.3),
					);
					let lash = Path::new()
						.move_to(base)
						.line_to(base + pt(side * r * 0.08, -r * 0.06));
					svg.stroke(&lash, palette::INK, line);
				}
			}
		}
		Eyes::Happy => {
			let (rx, ry) = (r * 0.17, r * 0.22);
			let arc = Path::new()
				.move_to(c + pt(-rx, r * 0.04))
				.quad_to(c + pt(0.0, -ry * 1.4), c + pt(rx, r * 0.04));
			svg.stroke(&arc, palette::INK, line * 1.5);
		}
		Eyes::Shut => {
			let (rx, ry) = (r * 0.17, r * 0.22);
			let arc = Path::new()
				.move_to(c + pt(-rx, 0.0))
				.quad_to(c + pt(0.0, ry * 0.8), c + pt(rx, 0.0));
			svg.stroke(&arc, palette::INK, line * 1.4);
		}
		Eyes::Cross => {
			let k = r * 0.15;
			svg.stroke(
				&Path::new().move_to(c + pt(-k, -k)).line_to(c + pt(k, k)),
				palette::INK,
				line * 1.6,
			);
			svg.stroke(
				&Path::new().move_to(c + pt(k, -k)).line_to(c + pt(-k, k)),
				palette::INK,
				line * 1.6,
			);
		}
		Eyes::Swirl => {
			let rx = r * 0.17;
			let turns: Vec<Pt> = (0..18)
				.map(|i| {
					let t = i as f32 / 17.0;
					let angle = t * 9.0 + state.time * 6.0 * side;
					c + pt(angle.cos(), angle.sin()) * (rx * t)
				})
				.collect();
			svg.stroke(&Path::spline(&turns), palette::INK, line * 1.2);
		}
		Eyes::Squint => {
			let k = r * 0.16;
			let chevron = Path::new()
				.move_to(c + pt(-k * side, -k * 0.7))
				.line_to(c + pt(k * side * 0.6, 0.0))
				.line_to(c + pt(-k * side, k * 0.7));
			svg.stroke(&chevron, palette::INK, line * 1.5);
		}
	}
}

fn draw_brow(svg: &mut Svg, c: Pt, r: f32, side: f32, brows: Brows) {
	let w = r * 0.21;
	let (inner, outer) = match brows {
		Brows::None => return,
		Brows::Neutral => (0.0, 0.0),
		Brows::Raised => (-r * 0.1, -r * 0.1),
		Brows::Angry => (r * 0.11, -r * 0.06),
		Brows::Worried => (-r * 0.11, r * 0.06),
		Brows::Smug => {
			if side > 0.0 {
				(-r * 0.08, -r * 0.15)
			} else {
				(r * 0.04, r * 0.02)
			}
		}
	};
	let start = c + pt(-side * w, inner);
	let end = c + pt(side * w, outer);
	// A slight arch and a thick-to-thin taper read as hand-inked.
	let arch = start.lerp(end, 0.45) + pt(0.0, -r * 0.05);
	let brow = Path::new().move_to(start).quad_to(arch, end);
	svg.stroke(&brow, palette::INK, r * 0.075);
	svg.fill(&Path::circle(start, r * 0.05), palette::INK);
}

fn draw_nose(svg: &mut Svg, look: &Look, face: Pt, r: f32, state: &FigureState) {
	let c = face + pt(state.facing * r * 0.06, r * 0.14);

	match look.nose {
		Nose::None => {}
		Nose::Button => {
			let nose = Path::wobbly_ellipse(c, r * 0.1, r * 0.08, 0.06, svg.seed(look.seed + 5));
			svg.fill(&nose, look.skin.darken(0.2));
		}
		Nose::Round => {
			let nose = Path::wobbly_ellipse(
				c + pt(0.0, r * 0.02),
				r * 0.16,
				r * 0.14,
				0.05,
				svg.seed(look.seed + 5),
			);
			svg.inked_weight(&nose, look.skin.darken(0.08), detail(svg));
			svg.fill(
				&Path::circle(c + pt(-r * 0.05, -r * 0.03), r * 0.04),
				look.skin.lighten(0.35),
			);
		}
		Nose::Long => {
			let dir = if state.facing < -0.05 { -1.0 } else { 1.0 };
			let nose = Path::new()
				.move_to(c + pt(-dir * r * 0.06, -r * 0.16))
				.quad_to(
					c + pt(dir * r * 0.2, -r * 0.06),
					c + pt(dir * r * 0.28, r * 0.06),
				)
				.quad_to(
					c + pt(dir * r * 0.2, r * 0.14),
					c + pt(-dir * r * 0.04, r * 0.08),
				)
				.close();
			svg.inked_weight(&nose, look.skin.darken(0.06), detail(svg));
		}
	}
}

fn draw_mouth(svg: &mut Svg, look: &Look, c: Pt, r: f32, shape: MouthShape, state: &FigureState) {
	let line = svg.line;
	let w = r * 0.32 * look.mouth_size;

	if let Some(viseme) = state.viseme
		&& viseme != Viseme::Rest
	{
		draw_viseme(svg, look, c, r, w, viseme, state);
		return;
	}

	let resting_open = match shape {
		MouthShape::Grin | MouthShape::Shout => 0.55,
		MouthShape::Oh => 0.5,
		_ => 0.0,
	};
	let open = state.mouth_open.max(resting_open);

	if shape == MouthShape::Oh && state.mouth_open < 0.6 {
		let oh = Path::wobbly_ellipse(
			c + pt(0.0, r * 0.06),
			w * 0.38,
			r * 0.2,
			0.05,
			svg.seed(look.seed + 9),
		);
		svg.inked(&oh, palette::MAW);
		let tongue = Path::ellipse(c + pt(0.0, r * 0.17), w * 0.2, r * 0.06);
		svg.fill(&tongue, palette::BLUSH);
		return;
	}

	if open > 0.08 {
		let corner = match shape {
			MouthShape::Frown | MouthShape::Shout => r * 0.06,
			_ => -r * 0.04,
		};
		let half = w * (0.85 + open * 0.15);
		let h = r * (0.12 + open * 0.36);
		open_mouth(
			svg,
			look,
			c,
			r,
			half,
			h,
			corner,
			state,
			TongueAt::Bottom,
			false,
		);
		return;
	}

	let path = match shape {
		MouthShape::Frown => {
			Path::new()
				.move_to(c + pt(-w * 0.7, r * 0.08))
				.quad_to(c + pt(0.0, -r * 0.14), c + pt(w * 0.7, r * 0.08))
		}
		MouthShape::Wavy => {
			Path::spline(&[
				c + pt(-w * 0.8, r * 0.02),
				c + pt(-w * 0.4, -r * 0.05),
				c + pt(0.0, r * 0.04),
				c + pt(w * 0.4, -r * 0.05),
				c + pt(w * 0.8, r * 0.02),
			])
		}
		MouthShape::Smirk => {
			Path::new()
				.move_to(c + pt(-w * 0.6, r * 0.04))
				.quad_to(c + pt(w * 0.2, r * 0.08), c + pt(w * 0.75, -r * 0.1))
		}
		MouthShape::Flat => {
			Path::new()
				.move_to(c + pt(-w * 0.5, 0.0))
				.line_to(c + pt(w * 0.5, 0.0))
		}
		MouthShape::Tongue => {
			// A dazed, slack mouth with the tongue lolling out of one corner: the classic
			// knocked-out face, and still readable when the figure is lying on its back.
			let slack = Path::wobbly_ellipse(
				c + pt(-w * 0.1, r * 0.04),
				w * 0.42,
				r * 0.12,
				0.08,
				svg.seed(look.seed + 8),
			);
			svg.inked(&slack, palette::MAW);
			let tongue = Path::wobbly_ellipse(
				c + pt(w * 0.32, r * 0.16),
				w * 0.2,
				r * 0.13,
				0.08,
				svg.seed(look.seed + 7),
			);
			svg.inked(&tongue, palette::BLUSH);
			return;
		}
		_ => {
			Path::new()
				.move_to(c + pt(-w * 0.75, -r * 0.04))
				.quad_to(c + pt(0.0, r * 0.2), c + pt(w * 0.75, -r * 0.04))
		}
	};

	svg.stroke(&path, palette::INK, line * 1.3);

	// Little dimples at the corners of a smile.
	if shape == MouthShape::Smile {
		for side in [-1.0, 1.0] {
			let corner = c + pt(side * w * 0.75, -r * 0.04);
			let dimple = Path::new()
				.move_to(corner + pt(-side * r * 0.02, -r * 0.04))
				.quad_to(
					corner + pt(side * r * 0.04, 0.0),
					corner + pt(-side * r * 0.01, r * 0.04),
				);
			svg.stroke(&dimple, palette::INK, line);
		}
	}
}

/// Where the tongue sits in an open mouth.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TongueAt {
	Bottom,
	Up,
	Hidden,
}

/// Draws one of the lip-sync mouth shapes, bent into a smile or frown by the mood.
fn draw_viseme(
	svg: &mut Svg,
	look: &Look,
	c: Pt,
	r: f32,
	w: f32,
	viseme: Viseme,
	state: &FigureState,
) {
	let a = state.mouth_open.clamp(0.0, 1.0);
	let smile = state.mood.smile();
	let corner = -smile * r * 0.05;
	let line = svg.line * 1.3;

	match viseme {
		Viseme::Rest => {}
		Viseme::Mbp => {
			let lips = Path::new()
				.move_to(c + pt(-w * 0.6, corner))
				.quad_to(c + pt(0.0, r * 0.05), c + pt(w * 0.6, corner));
			svg.stroke(&lips, palette::INK, line * 1.2);
			let bulge = Path::new()
				.move_to(c + pt(-w * 0.25, r * 0.06))
				.quad_to(c + pt(0.0, r * 0.11), c + pt(w * 0.25, r * 0.06));
			svg.stroke(&bulge, palette::INK, line * 0.6);
		}
		Viseme::Teeth => {
			open_mouth(
				svg,
				look,
				c,
				r,
				w * 0.75,
				r * (0.1 + a * 0.05),
				corner,
				state,
				TongueAt::Hidden,
				true,
			);
		}
		Viseme::Open => {
			open_mouth(
				svg,
				look,
				c,
				r,
				w * 0.9,
				r * (0.16 + a * 0.12),
				corner,
				state,
				TongueAt::Bottom,
				false,
			);
		}
		Viseme::Wide => {
			open_mouth(
				svg,
				look,
				c,
				r,
				w,
				r * (0.26 + a * 0.2),
				corner,
				state,
				TongueAt::Bottom,
				false,
			);
		}
		Viseme::Tongue => {
			open_mouth(
				svg,
				look,
				c,
				r,
				w * 0.85,
				r * (0.18 + a * 0.1),
				corner,
				state,
				TongueAt::Up,
				false,
			);
		}
		Viseme::Round | Viseme::Pucker => {
			let (rx, ry) = if viseme == Viseme::Round {
				(w * 0.45, r * (0.13 + a * 0.1))
			} else {
				(w * 0.26, r * 0.1)
			};
			let o =
				Path::wobbly_ellipse(c + pt(0.0, r * 0.05), rx, ry, 0.04, svg.seed(look.seed + 9));
			svg.inked(&o, palette::MAW);

			if viseme == Viseme::Pucker {
				svg.stroke(
					&Path::ellipse(c + pt(0.0, r * 0.05), rx * 1.5, ry * 1.5),
					palette::INK,
					line * 0.5,
				);
			} else {
				svg.fill(
					&Path::ellipse(c + pt(0.0, r * 0.05 + ry * 0.55), rx * 0.6, ry * 0.35),
					palette::BLUSH,
				);
			}
		}
		Viseme::Fv => {
			let lip = Path::new()
				.move_to(c + pt(-w * 0.6, corner))
				.quad_to(c + pt(0.0, r * 0.12), c + pt(w * 0.6, corner));
			let teeth = Path::wobbly_poly(
				&[
					c + pt(-w * 0.32, -r * 0.03),
					c + pt(w * 0.32, -r * 0.03),
					c + pt(w * 0.28, r * 0.07),
					c + pt(-w * 0.28, r * 0.07),
				],
				4.0,
				0.5,
				look.seed + 120,
			);
			svg.inked(&teeth, tooth_color(state.teeth));
			svg.stroke(&lip, palette::INK, line);
		}
	}
}

fn tooth_color(teeth: Teeth) -> Color {
	match teeth {
		Teeth::Clean | Teeth::Sparkly => palette::WHITE,
		Teeth::Grotty => palette::GROT,
	}
}

// The mouth needs the look (teeth seed), geometry and state; bundling them would only move the
// argument list into a one-use struct.
#[allow(clippy::too_many_arguments)]
fn open_mouth(
	svg: &mut Svg,
	look: &Look,
	c: Pt,
	r: f32,
	half: f32,
	h: f32,
	corner: f32,
	state: &FigureState,
	tongue: TongueAt,
	bottom_teeth: bool,
) {
	let mouth = Path::new()
		.move_to(c + pt(-half, corner))
		.quad_to(c + pt(0.0, -r * 0.04), c + pt(half, corner))
		.cubic_to(
			c + pt(half * 0.9, h * 1.1),
			c + pt(-half * 0.9, h * 1.1),
			c + pt(-half, corner),
		)
		.close();
	let clip = svg.next_id("mouth");
	svg.define(&format!(
		r#"<clipPath id="{clip}"><path d="{}"/></clipPath>"#,
		mouth.as_str()
	));
	svg.fill(&mouth, palette::MAW);
	svg.raw(&format!(r#"<g clip-path="url(#{clip})">"#));

	match tongue {
		TongueAt::Bottom => {
			svg.fill(
				&Path::ellipse(c + pt(half * 0.15, h * 0.95), half * 0.55, h * 0.4),
				palette::BLUSH,
			);
		}
		TongueAt::Up => {
			svg.fill(
				&Path::ellipse(c + pt(0.0, h * 0.35), half * 0.4, h * 0.35),
				palette::BLUSH,
			);
		}
		TongueAt::Hidden => {}
	}

	let tooth_h = (h * 0.32).min(r * 0.13);
	let count = 6;
	let tooth_w = half * 2.0 / count as f32;

	for row in 0..if bottom_teeth { 2 } else { 1 } {
		for i in 0..count {
			let x = c.x - half + i as f32 * tooth_w;

			if state.teeth == Teeth::Grotty && i == 2 && row == 0 {
				continue;
			}

			let (top, bottom) = if row == 0 {
				(c.y - r * 0.1, c.y + tooth_h)
			} else {
				(c.y + h * 0.95 - tooth_h, c.y + h * 1.2)
			};
			let tooth = Path::wobbly_poly(
				&[
					pt(x + 1.5, top),
					pt(x + tooth_w - 1.5, top),
					pt(x + tooth_w - 2.5, bottom),
					pt(x + 2.5, bottom),
				],
				4.0,
				if state.teeth == Teeth::Grotty {
					2.5
				} else {
					0.5
				},
				svg.seed(look.seed + 100 + i + row * 10),
			);
			svg.fill(&tooth, tooth_color(state.teeth));

			if state.teeth == Teeth::Grotty && i % 2 == 1 && row == 0 {
				let spot = pt(
					x + tooth_w * 0.5 + noise(look.seed, i) * 4.0,
					c.y + tooth_h * 0.4,
				);
				svg.fill(&Path::circle(spot, tooth_w * 0.16), palette::COCOA);
			}
		}
	}

	svg.raw("</g>");
	svg.stroke(&mouth, palette::INK, svg.line * 1.2);

	if state.teeth == Teeth::Sparkly {
		fx::sparkle(
			svg,
			c + pt(half * 0.55, -r * 0.08),
			r * 0.16,
			palette::WHITE,
			state.time,
		);
	}
}

fn draw_back_hair(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState) {
	let r = frame.head_r;
	let h = frame.head + pt(-state.facing * r * 0.06, 0.0);
	let seed = svg.seed(look.seed + 60);
	let swing = state.sway * 14.0;

	match look.hair {
		HairStyle::Bun => {
			let bun = Path::wobbly_ellipse(
				h + pt(state.sway * r * 0.06, -r * 1.12),
				r * 0.46,
				r * 0.4,
				0.06,
				seed,
			);
			svg.inked_weight(&bun, look.hair_color, silhouette(svg));
			let twist = Path::new()
				.move_to(h + pt(-r * 0.2, -r * 1.2))
				.quad_to(h + pt(0.0, -r * 1.35), h + pt(r * 0.2, -r * 1.12));
			svg.stroke(&twist, look.hair_color.darken(0.3), detail(svg));
		}
		HairStyle::Puff => {
			let puff = Path::wobbly_ellipse(h + pt(0.0, -r * 0.2), r * 1.42, r * 1.3, 0.07, seed);
			svg.inked_weight(&puff, look.hair_color, silhouette(svg));
		}
		HairStyle::Pigtails => {
			for side in [-1.0, 1.0] {
				let root = h + pt(side * r * 0.95, -r * 0.2);
				svg.begin(&format!(
					"rotate({:.2} {:.1} {:.1})",
					swing * side.signum(),
					root.x,
					root.y
				));
				let tail = Path::wobbly_ellipse(
					h + pt(side * r * 1.18, r * 0.05),
					r * 0.32,
					r * 0.42,
					0.08,
					seed + side as u64,
				);
				svg.inked_weight(&tail, look.hair_color, silhouette(svg));
				svg.end();
			}
		}
		HairStyle::Bob => {
			let bob = Path::wobbly_poly(
				&[
					h + pt(-r * 1.12, -r * 0.4),
					h + pt(0.0, -r * 1.2),
					h + pt(r * 1.12, -r * 0.4),
					h + pt(r * 1.1 + state.sway * r * 0.05, r * 0.75),
					h + pt(-r * 1.1 + state.sway * r * 0.05, r * 0.75),
				],
				r * 0.5,
				3.0,
				seed,
			);
			svg.inked_weight(&bob, look.hair_color, silhouette(svg));
		}
		_ => {}
	}
}

fn draw_front_hair(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState) {
	let r = frame.head_r;
	let h = frame.head + pt(state.facing * r * 0.1, 0.0);
	let seed = svg.seed(look.seed + 70);
	// Tips lag behind the head (follow-through) and drift a little at rest.
	let tip = (state.time * 3.0).sin() * 2.0 + state.sway * r * 0.3;
	let line = silhouette(svg);

	match look.hair {
		HairStyle::Bald => {
			let wisp = Path::spline(&[
				h + pt(-r * 0.1, -r * 0.98),
				h + pt(tip * 0.3, -r * 1.2),
				h + pt(r * 0.15 + tip * 0.5, -r * 1.12),
			]);
			svg.stroke(&wisp, palette::INK, svg.line);
			let shine = Path::new()
				.move_to(h + pt(-r * 0.55, -r * 0.62))
				.quad_to(h + pt(-r * 0.35, -r * 0.82), h + pt(-r * 0.1, -r * 0.86));
			svg.stroke_alpha(&shine, palette::WHITE, r * 0.07, 0.6);
		}
		HairStyle::Tuft => {
			let tuft = Path::blob(&[
				h + pt(-r * 0.62, -r * 0.72),
				h + pt(-r * 0.35 + tip * 0.4, -r * 1.12),
				h + pt(-r * 0.05 + tip, -r * 1.45),
				h + pt(r * 0.12, -r * 1.08),
				h + pt(r * 0.42 + tip, -r * 1.32),
				h + pt(r * 0.5, -r * 0.95),
				h + pt(r * 0.66, -r * 0.7),
				h + pt(0.0, -r * 0.84),
			]);
			svg.inked_weight(&tuft, look.hair_color, line);
			let strand = Path::new().move_to(h + pt(-r * 0.1, -r * 0.92)).quad_to(
				h + pt(-r * 0.02 + tip * 0.5, -r * 1.12),
				h + pt(-r * 0.02 + tip, -r * 1.3),
			);
			svg.stroke(&strand, look.hair_color.darken(0.35), detail(svg));
		}
		HairStyle::Bun | HairStyle::Bob | HairStyle::Fringe => {
			let cap = Path::blob(&[
				h + pt(-r * 0.98, -r * 0.15),
				h + pt(-r * 0.8, -r * 0.75),
				h + pt(0.0, -r * 1.06),
				h + pt(r * 0.8, -r * 0.75),
				h + pt(r * 0.98, -r * 0.15),
				h + pt(r * 0.5, -r * 0.48),
				h + pt(0.0, -r * 0.42),
				h + pt(-r * 0.5, -r * 0.5),
			]);
			svg.inked_weight(&cap, look.hair_color, line);

			for k in 0..3 {
				let x = -r * 0.45 + k as f32 * r * 0.4;
				let strand = Path::new().move_to(h + pt(x, -r * 0.9)).quad_to(
					h + pt(x + r * 0.15, -r * 0.7),
					h + pt(x + r * 0.05, -r * 0.5),
				);
				svg.stroke(&strand, look.hair_color.darken(0.3), detail(svg));
			}
		}
		HairStyle::Curls | HairStyle::Puff => {
			let count = 7;

			for i in 0..count {
				let angle = PI * (1.05 + i as f32 / (count - 1) as f32 * 0.9);
				let bounce = (state.time * 4.0 + i as f32).sin() * state.sway.abs() * r * 0.05;
				let p = h + pt(angle.cos() * r * 0.88, angle.sin() * r * 0.88 + bounce);
				let curl = Path::wobbly_ellipse(p, r * 0.27, r * 0.25, 0.1, seed + i);
				svg.inked_weight(&curl, look.hair_color, line);
				let swirl = Path::new()
					.move_to(p + pt(-r * 0.08, r * 0.02))
					.quad_to(p + pt(0.0, -r * 0.1), p + pt(r * 0.08, r * 0.0));
				svg.stroke(&swirl, look.hair_color.darken(0.3), detail(svg));
			}
		}
		HairStyle::Pigtails => {
			let cap = Path::blob(&[
				h + pt(-r * 1.0, -r * 0.05),
				h + pt(-r * 0.75, -r * 0.8),
				h + pt(0.0, -r * 1.06),
				h + pt(r * 0.75, -r * 0.8),
				h + pt(r * 1.0, -r * 0.05),
				h + pt(r * 0.3, -r * 0.62),
				h + pt(-r * 0.3, -r * 0.62),
			]);
			svg.inked_weight(&cap, look.hair_color, line);
			let parting = Path::new()
				.move_to(h + pt(0.0, -r * 1.04))
				.line_to(h + pt(0.0, -r * 0.66));
			svg.stroke(&parting, look.hair_color.darken(0.35), detail(svg));
		}
		HairStyle::Spikes => {
			let mut points = Vec::new();

			for i in 0..9 {
				let t = i as f32 / 8.0;
				let angle = PI * (1.08 + t * 0.84);
				let reach = if i % 2 == 0 {
					0.92
				} else {
					1.38 + noise(seed, i) * 0.08
				};
				let lag = if i % 2 == 0 { 0.0 } else { tip * 0.6 };
				points.push(h + pt(angle.cos() * r * reach + lag, angle.sin() * r * reach));
			}

			points.push(h + pt(r * 0.4, -r * 0.6));
			points.push(h + pt(-r * 0.4, -r * 0.6));
			let spikes = Path::wobbly_poly(&points, 6.0, 2.0, seed);
			svg.inked_weight(&spikes, look.hair_color, line);
		}
	}
}

fn draw_extras_on_head(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState) {
	let r = frame.head_r;
	let h = frame.head + pt(state.facing * r * 0.1, 0.0);

	if look.has(Extra::Bow) {
		let c = h + pt(r * 0.55, -r * 0.85);
		let bow = Path::new()
			.move_to(c)
			.line_to(c + pt(-r * 0.32, -r * 0.2))
			.line_to(c + pt(-r * 0.32, r * 0.2))
			.close()
			.move_to(c)
			.line_to(c + pt(r * 0.32, -r * 0.2))
			.line_to(c + pt(r * 0.32, r * 0.2))
			.close();
		svg.inked(&bow, look.trim);
		svg.inked(&Path::circle(c, r * 0.09), look.trim.darken(0.2));
	}

	if look.has(Extra::PostCap) {
		let crown = Path::wobbly_poly(
			&[
				h + pt(-r * 0.86, -r * 0.5),
				h + pt(-r * 0.7, -r * 1.12),
				h + pt(r * 0.7, -r * 1.12),
				h + pt(r * 0.86, -r * 0.5),
			],
			r * 0.25,
			2.0,
			svg.seed(look.seed + 80),
		);
		svg.inked_weight(&crown, look.trim, silhouette(svg));
		let peak = Path::wobbly_ellipse(
			h + pt(state.facing.signum() * r * 0.35, -r * 0.5),
			r * 0.75,
			r * 0.13,
			0.04,
			svg.seed(look.seed + 81),
		);
		svg.inked(&peak, palette::INK);
		svg.fill(
			&star_path(h + pt(0.0, -r * 0.82), r * 0.15, r * 0.07, 5),
			palette::SUNFLOWER,
		);
	}
}

fn draw_arms(svg: &mut Svg, look: &Look, frame: &Frame, state: &FigureState, posed: &Posed) {
	let (left, right) = arm_targets(state, posed);
	let (shoulders, ..) = look.body_widths();
	let mut wrists = Vec::with_capacity(2);

	for (side, target, shape) in [
		(-1.0, left, state.hand_shapes.0),
		(1.0, right, state.hand_shapes.1),
	] {
		let shoulder = posed.torso(pt(side * frame.shoulder_half * shoulders, frame.shoulder_y));
		// Elbows bend out and down, like a relaxed arm.
		let (elbow, hand) = rig::two_bone(
			shoulder,
			target,
			posed.limbs.upper_arm,
			posed.limbs.forearm,
			pt(side * 0.6, 0.8),
		);
		let arm = rig::limb_curve(shoulder, elbow, hand, 10);
		let line = silhouette(svg) * 0.85;

		// Arms taper from shoulder to wrist, the way an inker draws them.
		svg.inked_weight(
			&Path::taper(&arm, frame.arm_width * 1.3, frame.arm_width * 0.8),
			look.top,
			line,
		);

		// A cuff in the trim colour where the sleeve ends.
		svg.inked_weight(
			&Path::taper(&arm[8..=9], frame.arm_width * 1.08, frame.arm_width * 1.02),
			look.trim,
			line * 0.8,
		);

		let hand_color = if look.has(Extra::MuddyHands) {
			palette::COCOA
		} else {
			look.skin
		};
		let direction = (hand - arm[8]).norm();
		draw_hand(svg, look, frame, hand, direction, side, shape, hand_color);

		if side > 0.0
			&& let Some(prop) = state.prop
		{
			draw_prop(svg, prop, hand, (hand - elbow).norm(), frame, state);
		}

		wrists.push(hand);
	}

	if look.has(Extra::MuddyHands) {
		for (i, hand) in wrists.into_iter().enumerate() {
			let drip = Path::wobbly_ellipse(
				hand + pt(4.0, frame.hand_r * 1.4),
				5.0,
				8.0,
				0.1,
				look.seed + i as u64,
			);
			svg.fill(&drip, palette::COCOA);
		}
	}
}

/// A cartoon mitten with a thumb, held open, as a fist, or pointing.
#[allow(clippy::too_many_arguments)]
fn draw_hand(
	svg: &mut Svg,
	look: &Look,
	frame: &Frame,
	at: Pt,
	direction: Pt,
	side: f32,
	shape: HandShape,
	color: Color,
) {
	let r = frame.hand_r;
	let angle = direction.y.atan2(direction.x).to_degrees();
	// The thumb sits on the side facing up and in towards the body.
	let up = if (-direction.perp().y + direction.perp().x * -side) > 0.0 {
		1.0
	} else {
		-1.0
	};
	let line = silhouette(svg) * 0.85;
	let seed = svg.seed(look.seed + 30 + side as u64);

	svg.begin(&format!(
		"translate({:.1} {:.1}) rotate({angle:.1})",
		at.x, at.y
	));

	match shape {
		HandShape::Open => {
			let palm = Path::wobbly_ellipse(pt(r * 0.25, 0.0), r * 1.15, r * 0.95, 0.06, seed);
			svg.inked_weight(&palm, color, line);
			let thumb = Path::wobbly_ellipse(
				pt(-r * 0.05, up * r * 0.85),
				r * 0.5,
				r * 0.32,
				0.06,
				seed + 1,
			);
			svg.inked_weight(&thumb, color, line);
			let gap = Path::new()
				.move_to(pt(r * 0.9, -r * 0.15))
				.line_to(pt(r * 0.55, -r * 0.05));
			svg.stroke(&gap, palette::INK, line * 0.7);
		}
		HandShape::Fist => {
			let fist = Path::wobbly_ellipse(pt(r * 0.1, 0.0), r * 0.98, r * 0.92, 0.05, seed);
			svg.inked_weight(&fist, color, line);
			let thumb = Path::wobbly_ellipse(
				pt(r * 0.1, up * r * 0.35),
				r * 0.55,
				r * 0.26,
				0.06,
				seed + 1,
			);
			svg.inked_weight(&thumb, color, line * 0.8);
		}
		HandShape::Point => {
			let finger = Path::new()
				.move_to(pt(r * 0.4, -up * r * 0.1))
				.line_to(pt(r * 1.75, -up * r * 0.1));
			svg.inked_noodle(&finger, color, r * 0.5);
			let fist = Path::wobbly_ellipse(pt(r * 0.1, 0.0), r * 0.95, r * 0.9, 0.05, seed);
			svg.inked_weight(&fist, color, line);
			let thumb = Path::wobbly_ellipse(
				pt(r * 0.15, up * r * 0.4),
				r * 0.5,
				r * 0.25,
				0.06,
				seed + 1,
			);
			svg.inked_weight(&thumb, color, line * 0.8);
		}
	}

	svg.end();
}

fn draw_prop(
	svg: &mut Svg,
	prop: Prop,
	hand: Pt,
	direction: Pt,
	frame: &Frame,
	state: &FigureState,
) {
	let r = frame.head_r;

	match prop {
		Prop::Toothbrush => {
			// While brushing the bristles sit in the mouth, so the handle points back out.
			let brushing =
				(hand - (frame.head + pt(state.facing * r * 0.2, r * 0.45))).len() < r * 0.9;
			let dir = if brushing {
				pt(-1.0, 0.0)
			} else {
				direction.perp().norm() * -1.0
			};
			let scrub = if brushing {
				(state.time * 22.0).sin() * 8.0
			} else {
				0.0
			};
			let tip = hand + dir * (r * 0.75) + pt(scrub, 0.0);
			let tail = hand - dir * (r * 0.35);
			svg.inked_noodle(&Path::new().move_to(tail).line_to(tip), palette::TEAL, 14.0);
			let head = Path::wobbly_poly(
				&[
					tip + pt(-12.0, -26.0),
					tip + pt(12.0, -26.0),
					tip + pt(12.0, -4.0),
					tip + pt(-12.0, -4.0),
				],
				4.0,
				1.0,
				9,
			);
			svg.inked(&head, palette::WHITE);
			let paste = Path::wobbly_ellipse(tip + pt(0.0, -34.0), 14.0, 9.0, 0.1, 10);
			svg.inked(&paste, palette::MINT);

			if brushing {
				for i in 0..5 {
					let bubble = hand
						+ pt(
							-r * 0.6 + noise(77, i) * r * 0.3,
							-r * 0.1 + noise(78, i) * r * 0.25,
						);
					let size = r
						* (0.05 + noise(79, i).abs() * 0.05)
						* (1.0 + (state.time * 6.0 + i as f32).sin() * 0.2);
					svg.inked(&Path::circle(bubble, size), palette::WHITE);
				}
			}
		}
		Prop::Parcel => {
			let c = hand + pt(0.0, -r * 0.25);
			let parcel = Path::wobbly_poly(
				&[
					c + pt(-r * 0.45, -r * 0.35),
					c + pt(r * 0.45, -r * 0.35),
					c + pt(r * 0.45, r * 0.35),
					c + pt(-r * 0.45, r * 0.35),
				],
				6.0,
				2.0,
				12,
			);
			svg.inked(&parcel, palette::TOFFEE);
			svg.stroke(
				&Path::new()
					.move_to(c + pt(0.0, -r * 0.35))
					.line_to(c + pt(0.0, r * 0.35)),
				palette::TOMATO,
				8.0,
			);
			svg.stroke(
				&Path::new()
					.move_to(c + pt(-r * 0.45, 0.0))
					.line_to(c + pt(r * 0.45, 0.0)),
				palette::TOMATO,
				8.0,
			);
		}
		Prop::Jam => {
			let c = hand + pt(0.0, -r * 0.2);
			let jar = Path::wobbly_poly(
				&[
					c + pt(-r * 0.3, -r * 0.3),
					c + pt(r * 0.3, -r * 0.3),
					c + pt(r * 0.32, r * 0.35),
					c + pt(-r * 0.32, r * 0.35),
				],
				10.0,
				1.5,
				13,
			);
			svg.inked(&jar, palette::TOMATO);
			let lid = Path::wobbly_poly(
				&[
					c + pt(-r * 0.34, -r * 0.48),
					c + pt(r * 0.34, -r * 0.48),
					c + pt(r * 0.34, -r * 0.3),
					c + pt(-r * 0.34, -r * 0.3),
				],
				5.0,
				1.0,
				14,
			);
			svg.inked(&lid, palette::CREAM);
			let label = Path::wobbly_ellipse(c + pt(0.0, r * 0.05), r * 0.2, r * 0.13, 0.05, 15);
			svg.fill(&label, palette::CREAM);
		}
	}
}

/// Smoothly blends two hand positions (used by the stage tweener).
pub fn blend_hands(a: (Pt, Pt), b: (Pt, Pt), t: f32) -> (Pt, Pt) {
	(a.0.lerp(b.0, t), a.1.lerp(b.1, t))
}

/// Linear blend for scalar figure parameters.
pub fn blend(a: f32, b: f32, t: f32) -> f32 {
	lerp(a, b, t)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_pose_and_mood_round_trips_by_name() {
		for (name, pose) in Pose::ALL {
			assert_eq!(Pose::parse(name), Some(pose));
		}

		for (name, mood) in Mood::ALL {
			assert_eq!(Mood::parse(name), Some(mood));
			assert_eq!(mood.name(), name);
		}
	}

	#[test]
	fn hold_nose_puts_a_hand_on_the_face() {
		for build in [Build::Kid, Build::Grownup] {
			let frame = Frame::of(build);
			let (_, right) = frame.hands(Pose::HoldNose, 0.0);
			let nose = frame.head + pt(0.0, frame.head_r * 0.12);
			assert!(
				(right - nose).len() < frame.head_r * 0.3,
				"{build:?}: hand at {right:?}, nose at {nose:?}"
			);
		}
	}

	#[test]
	fn pointing_poses_point() {
		assert_eq!(Pose::Point.hand_shapes().1, HandShape::Point);
		assert_eq!(Pose::Think.hand_shapes().1, HandShape::Point);
		assert_eq!(
			Pose::Cheer.hand_shapes(),
			(HandShape::Fist, HandShape::Fist)
		);
	}
}
