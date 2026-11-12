//! Authored puppets: characters drawn from hand-made part files, on the same rig as everyone.
//!
//! The parametric puppet in [`figure`](super::figure) builds every character from one template.
//! An authored puppet is drawn the way TV cut-out characters are: every part (head, hair,
//! torso, sleeves, hands, the mouth chart) is a drawing from the character's part file and
//! their style's kit (see [`parts`](super::parts)), hung on the bones of [`rig`](super::rig).
//! The style pack ([`pack`](super::pack)) sets the finish, the face cylinder and the bodies.
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

use super::Bounds;
use super::Brush;
use super::Color;
use super::Path;
use super::Pt;
use super::Svg;
use super::dials::Fill;
use super::dials::Joints;
use super::figure::Build;
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
use super::pack::Eyes;
use super::pack::Face as FaceRules;
use super::pack::Features;
use super::pack::Finish;
use super::pack::KitInks;
use super::pack::StylePack;
use super::palette;
use super::parts::Level;
use super::parts::Parts;
use super::pt;
use super::rig;
use super::rig::Gait;
use super::rig::Legs;
use super::rig::Limbs;
use super::shape::Shape;

/// The body an authored puppet is drawn on: where its joints sit, how long its bones are and how
/// thick its limbs. Kids and grown-ups share the head (head units are the same size), so a
/// grown-up is a kid-sized head on a taller body with longer limbs.
///
/// A character's own torso, sleeves, legwear and shoes are drawn to their body; the kit's
/// hands are scaled to it. The kit's body extras (satchel, apron) are drawn for a kid, so a
/// grown-up who wears one redraws it in their own file.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Proportions {
	/// Bone lengths, in figure units with the soles at y = 0.
	pub limbs: Limbs,
	/// Where the head hinges on the torso, and how far above it the head's own origin sits.
	pub neck: Pt,
	pub head_above_neck: f32,
	pub shoulder_y: f32,
	/// Half the shoulders' width, front-on.
	pub shoulder_half: f32,
	/// The near and far shoulder's x, and the near and far hip's x, three-quarters turned.
	pub shoulders_turned: (f32, f32),
	pub hips_turned: (f32, f32),
	/// The top of the ribbed hem at the bottom of the top.
	pub hem_y: f32,
	/// The radius of the cylinder the torso wraps round as it turns.
	pub body_radius: f32,
	/// Leg widths at the hip and ankle, and the forearm's at the elbow and wrist.
	pub leg_width: (f32, f32),
	pub forearm_width: (f32, f32),
	/// How much bigger than the kit's drawings hands are.
	pub hand_scale: f32,
	/// The units pose reaches are measured in, across and up or down (see `figure::Reach`).
	pub reach: Pt,
	/// The legs' anatomy: their width at the hip, a third and two thirds of the way down the
	/// thigh, the knee, a third and two thirds of the way down the shin, and the ankle (see
	/// [`Shape::limb`]). Without it, a leg tapers evenly through `leg_width`.
	#[serde(default)]
	pub leg_profile: Option<[f32; 7]>,
	/// The bare arm's anatomy, shoulder to elbow to wrist, drawn under the sleeve. Without it,
	/// only the forearm shows, as a tube tapering through `forearm_width`.
	#[serde(default)]
	pub arm_profile: Option<[f32; 7]>,
	/// How much bigger than the kit's drawings shoes are. They grow from the sole.
	#[serde(default = "one")]
	pub foot_scale: f32,
	/// How much of a bent knee's sideways reach shows front-on. At 1 knees bend out across the
	/// page; less brings them towards the viewer, so a squat or a kneel foreshortens the thighs
	/// instead of splaying them. Three-quarters turned, knees always bend forwards in full.
	#[serde(default = "one")]
	pub knee_splay: f32,
	/// Whether the legs keep to the floor: a knee that would go through it rests on it, with
	/// the shin reaching back towards the foot (so a deep sit kneels back on the heels), and a
	/// foot lifted off it while standing points its toe down to it.
	#[serde(default)]
	pub floor: bool,
}

fn one() -> f32 {
	1.0
}

impl Proportions {
	/// A hip joint's x, `side` -1 near and 1 far, turned by `t`.
	fn hip_x(&self, side: f32, t: f32) -> f32 {
		let (near, far) = self.hips_turned;
		lerp(
			side * self.limbs.hip_spread,
			if side < 0.0 { near } else { far },
			t,
		)
	}

	/// What fitting the floor stances to this body needs, turned by `t` (see [`Limbs::fit`]).
	/// Seated, the hips rest on the thighs, or on clothes that `hang` further below the hip
	/// joints. The feet's grounds are where [`Posed::of`] stands them.
	fn floor(&self, t: f32, hang: f32) -> rig::Floor {
		rig::Floor {
			turn: t,
			hips: (self.hip_x(-1.0, t), self.hip_x(1.0, t)),
			ground: (3.0 * t, -9.0 * t),
			seat: (self
				.leg_profile
				.map_or(self.leg_width.0, |widths| widths[0])
				/ 2.0)
				.max(hang),
			knee: self.knee_radius(),
			torso: self.limbs.pelvis_y - self.shoulder_y,
		}
	}

	/// Half the knee's width, how far its round sits off the floor when it kneels.
	fn knee_radius(&self) -> f32 {
		self.leg_profile.map_or_else(
			|| (self.leg_width.0 + self.leg_width.1) / 4.0,
			|widths| widths[3] / 2.0,
		)
	}

	/// Checks the construction dials: every width, scale and the splay must be positive.
	pub fn problems(&self) -> Vec<String> {
		let mut problems = Vec::new();

		for (dial, profile) in [
			("leg-profile", self.leg_profile),
			("arm-profile", self.arm_profile),
		] {
			if profile.is_some_and(|widths| widths.iter().any(|&width| width <= 0.0)) {
				problems.push(format!("every width in {dial} must be more than zero"));
			}
		}

		if self.foot_scale <= 0.0 {
			problems.push("foot-scale must be more than zero".to_owned());
		}

		if !(self.knee_splay > 0.0 && self.knee_splay <= 1.0) {
			problems.push("knee-splay must be more than 0 and at most 1".to_owned());
		}

		problems
	}
}

/// The `facing` the stage gives a figure turned towards the middle of the page. An authored
/// puppet shows its full three-quarter drawing there.
const STAGED_FACING: f32 = 0.35;

/// How far down a `fist` drawn for a kid's hand, from the wrist, a held thing sits: a little
/// past halfway, so a bigger hand holds it further from the wrist. That is 17 of the cartoon
/// fist's 32 units (a fraction exact in binary), where the cartoon's props have always been.
fn grip(fist: &Shape) -> f32 {
	fist.bounds().1.y * (17.0 / 32.0)
}

/// How many of the cartoon fist's 32 units long a kit draws its `fist`: 1 for the cartoon, more
/// for a kit drawn bigger in its own units, so distances round a held thing grow with the hand.
fn fist_size(fist: &Shape) -> f32 {
	fist.bounds().1.y / 32.0
}

/// A character's authored puppet: their part file over their style's kit, read and checked once.
#[derive(Clone, Debug)]
pub struct Authored {
	pub file: PathBuf,
	pub parts: Arc<Parts>,
	/// The style the parts are drawn in: its finish, face rules and bodies.
	pub style: Arc<StylePack>,
}

impl Authored {
	/// Reads a part file over its style's kit, refusing parts that can't draw a figure.
	pub fn load(file: &FilePath, style: &Arc<StylePack>) -> Result<Authored> {
		Self::from_parts(file, Parts::load_puppet(file, &style.kit)?, style)
	}

	/// A puppet from parts already read (over the kit), refusing parts that can't draw a
	/// figure. `file` names where they came from.
	pub fn from_parts(file: &FilePath, parts: Parts, style: &Arc<StylePack>) -> Result<Authored> {
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
			style: Arc::clone(style),
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

/// A character's inks: their cast colours, plus their style's finish and the kit's shared inks.
#[derive(Clone, Copy, Debug)]
struct Inks {
	finish: Finish,
	face: FaceRules,
	features: Features,
	eyes: Eyes,
	kit: KitInks,
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

/// Cheeks: a warm blush on warm skin. Pink over teal or blue skin turns grey (they are
/// opposite colours), so cool skin gets a soft plum shading instead.
fn blush(skin: Color, warm: Color) -> Color {
	let (r, _, b) = skin.rgb();

	if r >= b {
		warm
	} else {
		skin.mix(Color(0x8E_4A_7A), 0.35)
	}
}

impl Inks {
	fn of(look: &Look, style: &StylePack, teeth: Teeth) -> Inks {
		let inks = Inks {
			finish: style.finish,
			face: style.face,
			features: style.features,
			eyes: style.eyes,
			kit: style.inks,
			line: Color(0x2B_1D_2E),
			skin: look.skin,
			nose: style
				.features
				.nose_colour
				.unwrap_or_else(|| look.skin.mix(Color(0xE0_60_50), 0.18)),
			freckle: look.skin.mix(Color(0x8A_3A_20), 0.45),
			blush: blush(look.skin, style.inks.blush),
			hair: look.hair_color,
			hair_light: look.hair_color.lighten(0.25),
			top: look.top,
			stripe: look.trim,
			bottom: look.bottom,
			shoes: look.shoes,
			pupil: look.eyes.mix(Color(0x1E_12_18), style.eyes.iris_dark),
			teeth: match teeth {
				Teeth::Clean | Teeth::Sparkly => style.inks.clean_teeth,
				Teeth::Grotty => palette::GROT,
			},
		};

		if style.shade.fill == Fill::Palette {
			inks.snapped(|color| style.shade.palette.nearest(color))
		} else {
			inks
		}
	}

	/// The character's colours snapped to a limited palette (the line stays ink).
	fn snapped(self, snap: impl Fn(Color) -> Color) -> Inks {
		Inks {
			skin: snap(self.skin),
			nose: snap(self.nose),
			freckle: snap(self.freckle),
			blush: snap(self.blush),
			hair: snap(self.hair),
			hair_light: snap(self.hair_light),
			top: snap(self.top),
			stripe: snap(self.stripe),
			bottom: snap(self.bottom),
			shoes: snap(self.shoes),
			pupil: snap(self.pupil),
			..self
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
	fn of(state: &FigureState, style: &StylePack, build: Build) -> Posed {
		let body = style.body(build);
		// Hands are drawn for a kid; a grown-up's bigger hands need more room.
		let hand_size = body.hand_scale / style.body(Build::Kid).hand_scale;
		let shoulder_y = body.shoulder_y;
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
		let face_x = t * style.face.slide * STAGED_FACING;
		let swapped = t > 0.5
			&& far.y < shoulder_y - 30.0
			&& far.x > 70.0
			&& (far.x - face_x).abs() >= 30.0
			&& near.y > shoulder_y;

		if swapped {
			(near, far) = (pt(-far.x, far.y), pt(-near.x, near.y));
			(near_shape, far_shape) = (far_shape, near_shape);
		}

		let far_arm_behind =
			t > 0.5 && far.x > -10.0 && (far.y > shoulder_y || (far.x - face_x).abs() >= 30.0);
		(near, far) = (
			clear_of_head(near, -1.0, face_x, shoulder_y, hand_size),
			clear_of_head(far, 1.0, face_x, shoulder_y, hand_size),
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
				HandShape::Open if target.y > shoulder_y + 50.0 => Hand::Relaxed,
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
			wrists: (
				wrist(near_shape, near, -1.0, shoulder_y),
				wrist(far_shape, far, 1.0, shoulder_y),
			),
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
/// middle at `face_x`, stay put. Bigger hands need more room.
fn clear_of_head(target: Pt, side: f32, face_x: f32, shoulder_y: f32, hand_size: f32) -> Pt {
	let out = target.x * side;
	let at_face = (target.x - face_x).abs() < 30.0;
	let clear = 115.0 * hand_size;

	if target.y < shoulder_y - 30.0 && !at_face && (45.0..clear).contains(&out) {
		pt(side * clear, target.y + 40.0)
	} else {
		target
	}
}

/// A little turn of the wrist so hands sit naturally: fists on hips turn their knuckles
/// forward, and hands hanging at the sides tip in towards the legs.
fn wrist(shape: HandShape, target: Pt, side: f32, shoulder_y: f32) -> f32 {
	let outside = target.x * side > 50.0;
	let low = target.y > shoulder_y + 40.0;

	match shape {
		HandShape::Fist if outside && low => -70.0 * side,
		HandShape::Open if low => -6.0 * side,
		_ => 0.0,
	}
}

/// The centre line of a rubber-hose limb: one tube of a fixed `length` from `root` to `end`.
/// Pulled taut it runs straight; when the end comes closer, the slack bows it into one smooth
/// arc, on the side the `joint` of a jointed limb would bend to. So the curve flexes with every
/// move of the hand or foot, and there's never an elbow or knee.
fn hose(root: Pt, joint: Pt, end: Pt, length: f32) -> [Pt; 9] {
	let chord = end - root;
	let reach = chord.len().max(1.0);
	let slack = (length - reach).max(0.0);
	// A shallow arc of height h over a chord d is about d + 8h²/3d long.
	let height = (3.0 * reach * slack / 8.0).sqrt().min(reach * 0.45);
	let normal = chord.norm().perp();
	let towards = joint - root;
	let side = if towards.x * normal.x + towards.y * normal.y < 0.0 {
		-1.0
	} else {
		1.0
	};

	std::array::from_fn(|i| {
		let s = i as f32 / 8.0;
		root.lerp(end, s) + normal * (side * height * 4.0 * s * (1.0 - s))
	})
}

/// A bent knee seen front-on: it comes towards the viewer, so only `reach` of its sideways
/// offset from the hip–ankle line shows, and the thigh and shin foreshorten.
fn splay(hip: Pt, knee: Pt, ankle: Pt, reach: f32) -> Pt {
	let line = (ankle - hip).norm();
	let along = (knee - hip).x * line.x + (knee - hip).y * line.y;
	let base = hip + line * along;
	base + (knee - base) * reach
}

/// The rig solved for one moment.
struct Solved {
	t: f32,
	body: Proportions,
	limbs: Limbs,
	legs: Legs,
	swing: (Pt, Pt),
	drop: f32,
	pivot: Pt,
}

impl Solved {
	/// The rig posed by `posed` on `body`. A body that keeps to the floor sits on clothes that
	/// `hang` below its hip joints (see [`hang`]).
	fn new(posed: &Posed, body: &Proportions, hang: f32) -> Solved {
		let limbs = body.limbs;
		// A body that keeps to the floor sits, kneels and crouches on its own legs.
		let legs = if body.floor {
			limbs.fit(posed.legs, &body.floor(posed.t, hang))
		} else {
			posed.legs
		};
		let (legs, swing) = match posed.gait {
			Some((gait, phase)) => limbs.cycle(gait, phase, 1.0, legs),
			None => (legs, (pt(0.0, 0.0), pt(0.0, 0.0))),
		};

		Solved {
			t: posed.t,
			body: *body,
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

	/// Where a skirt's point (in torso units) hangs: cloth below the waist falls from it under
	/// gravity, whichever way the spine bends, and drapes forwards over thighs that lift towards
	/// the lap. It never goes through the floor.
	fn skirt(&self, rest: Pt) -> Pt {
		let waist = self.limbs.pelvis_y - 40.0;
		let depth = rest.y - waist;

		if depth <= 0.0 {
			return rest;
		}

		let hips = self.hip(-1.0).lerp(self.hip(1.0), 0.5);
		let (_, near, _) = self.leg(-1.0);
		let (_, far, _) = self.leg(1.0);
		let thighs = (near.lerp(far, 0.5) - hips).norm();
		let lap = pt(0.0, 1.0).lerp(thighs, 0.75).norm();
		// Across the cloth: sideways when it hangs, thinning to a thigh's thickness on the lap.
		let across = pt(lap.y, -lap.x) * lerp(1.0, 0.45, ((1.0 - lap.y) / 0.6).clamp(0.0, 1.0));
		let centre = self.torso(pt(0.0, waist));
		let seam = self.torso(pt(rest.x, waist)) - centre;
		let below = (depth / (self.limbs.thigh * 0.3)).clamp(0.0, 1.0);
		let hanging = centre + seam.lerp(across * rest.x, below) + lap * depth;
		let world = pt(hanging.x, hanging.y.min(-4.0));
		(world - self.pivot - pt(0.0, self.drop)).rotate(-self.legs.bend) + self.pivot
	}

	/// `side` is -1 for the near (page-left) limb and 1 for the far one.
	fn hip(&self, side: f32) -> Pt {
		self.torso(pt(self.body.hip_x(side, self.t), self.limbs.pelvis_y))
	}

	fn shoulder(&self, side: f32) -> Pt {
		let (near, far) = self.body.shoulders_turned;
		let x = lerp(
			side * self.body.shoulder_half,
			if side < 0.0 { near } else { far },
			self.t,
		);
		self.torso(pt(x, self.body.shoulder_y))
	}

	fn leg(&self, side: f32) -> (Pt, Pt, Pt) {
		let hip = self.hip(side);
		let foot = if side < 0.0 {
			self.legs.feet.0
		} else {
			self.legs.feet.1
		};
		// Knees bend outwards from the front and forwards when turned; on a body that keeps to
		// the floor, the way a real knee flexes, so a leg stretched along the floor raises it.
		let pole = if self.body.floor {
			rig::flex_pole(side, self.t, foot - hip)
		} else {
			rig::knee_pole(side, self.t)
		};
		let (mut knee, mut ankle) =
			rig::two_bone(hip, foot, self.limbs.thigh, self.limbs.shin, pole);

		if self.body.floor && knee.y > -self.body.knee_radius() {
			(knee, ankle) = self.kneel(hip, knee, foot);
		}

		if self.body.knee_splay < 1.0 {
			knee = splay(hip, knee, ankle, lerp(self.body.knee_splay, 1.0, self.t));
		}

		(hip, knee, ankle)
	}

	/// Rests a knee that would go through the floor on it, swung on the thigh to the side it
	/// bent to, with the shin reaching from it towards the foot's target (and never through the
	/// floor either). Where the knee just touches the floor this meets the plain solution, so a
	/// figure sinking into a sit moves without a jump.
	fn kneel(&self, hip: Pt, knee: Pt, target: Pt) -> (Pt, Pt) {
		let floor = -self.body.knee_radius();
		let (thigh, shin) = (self.limbs.thigh, self.limbs.shin);
		let drop = (floor - hip.y).clamp(0.0, thigh);
		let ahead = if knee.x >= hip.x { 1.0 } else { -1.0 };
		let knee = pt(hip.x + ahead * (thigh * thigh - drop * drop).sqrt(), floor);
		let mut ankle = knee + (target - knee).norm() * shin;
		let ground = self.limbs.ankle_y;

		if ankle.y > ground {
			let rise = ground - knee.y;
			let back = if ankle.x >= knee.x { 1.0 } else { -1.0 };
			ankle = pt(
				knee.x + back * (shin * shin - rise * rise).max(0.0).sqrt(),
				ground,
			);
		}

		(knee, ankle)
	}

	/// Whether a leg kneels: on a body that keeps to the floor, its knee rests on the floor
	/// with the foot behind it.
	fn kneels(&self, (hip, knee, ankle): (Pt, Pt, Pt)) -> bool {
		self.body.floor
			&& knee.y >= -self.body.knee_radius() - 0.5
			&& (ankle.x - knee.x) * (knee.x - hip.x) < 0.0
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

/// How far below the hip joints a figure's clothes hang where they don't drape over the lap:
/// its top, and legwear that rides with the hips rather than hanging like a skirt. Seated, the
/// figure rests on them, so they never go through the floor.
fn hang(puppet: &Authored, body: &Proportions, t: f32) -> f32 {
	let drapes = body.floor && puppet.part("legwear-leg", t).is_none();

	["torso", "legwear"]
		.into_iter()
		.filter(|name| !(drapes && *name == "legwear"))
		.filter_map(|name| puppet.part(name, t))
		.map(|part| part.bounds().1.y - body.limbs.pelvis_y)
		.fold(0.0, f32::max)
}

/// Draws an authored figure with its soles at the current origin, and returns the bounds of
/// its head — hair and anything riding on it — in the document's units, for whoever is
/// measuring (see [`Svg::begin_measure`]).
pub fn draw(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState) -> Option<Bounds> {
	let frame = look.frame();
	let body = puppet.style.body(look.build);
	let posed = Posed::of(state, &puppet.style, look.build);
	let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
	let inks = Inks::of(look, &puppet.style, state.teeth);
	let t = solved.t;
	// The style's line and shade dials, for this figure only.
	let saved = svg.use_brush(Brush::new(
		puppet.style.line,
		&puppet.style.shade,
		inks.line,
	));

	svg.begin(&figure_transform(look, &posed, state, frame.head_r));

	// A far hand holding something keeps it in front, where it can be seen.
	let far_holds = posed.prop.is_some_and(|(_, in_far)| in_far);
	let far_arm_behind = posed.far_arm_behind && !far_holds;

	// What each hand holds, and where the mouth is (a toothbrush near it is brushing).
	let mouth = mouth_on_body(&solved, &posed, inks.face, inks.features.mouth);
	let held = |side: f32| {
		posed
			.prop
			.filter(|&(_, in_far)| in_far == (side > 0.0))
			.map(|(prop, _)| {
				Held {
					prop,
					mouth,
					time: state.time,
					scale: body.hand_scale / puppet.style.body(Build::Kid).hand_scale,
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
	draw_torso(svg, puppet, look, &inks, &solved);
	svg.begin_measure();
	svg.begin(&format!(
		"translate({:.1} {:.1}) rotate({:.2}) translate(0 {:.1})",
		body.neck.x, body.neck.y, posed.head_tilt, -body.head_above_neck
	));
	draw_head(svg, puppet, look, &inks, &posed, state.time);
	svg.end();
	let head = svg.end_measure();
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

	// Dizzy stars are an effect a mood draws over the figure, not part of it.
	let measuring = svg.pause_measure();

	if matches!(state.mood, Mood::Fainted | Mood::Dizzy) {
		let above = solved.torso(frame.head + pt(0.0, -frame.head_r * 1.2));
		fx::dizzy_stars(svg, above, frame.head_r, state.time);
	}

	svg.resume_measure(measuring);
	svg.end();
	svg.brush = saved;

	head
}

/// A soft contact shadow under the feet, so the figure stands on the ground.
pub fn shadow(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState, lift: f32) {
	let fade = (1.0 - lift / 120.0).clamp(0.35, 1.0);
	let size = puppet.style.body(look.build).limbs.hip_spread
		/ puppet.style.body(Build::Kid).limbs.hip_spread;
	let turn = (state.facing / STAGED_FACING).clamp(-1.0, 1.0);
	let spread = 1.0 + state.lie * 1.8;
	svg.fill_alpha(
		&Path::ellipse(
			pt(turn * 6.0 - state.lie * 120.0, -2.0),
			70.0 * size * fade * spread,
			12.0 * size * fade,
		),
		palette::INK,
		0.18 * fade,
	);
}

/// Draws the rig's bones over an authored figure, for rig checks.
pub fn draw_bones(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState) {
	let frame = look.frame();
	let body = puppet.style.body(look.build);
	let posed = Posed::of(state, &puppet.style, look.build);
	let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
	let pelvis = solved.torso(solved.pivot);
	let chest = solved.torso(pt(0.0, body.shoulder_y));
	let neck = solved.torso(body.neck);
	let head = solved.torso(body.neck + pt(0.0, -body.head_above_neck).rotate(posed.head_tilt));
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
fn far(color: Color, is_far: bool, tint: Color) -> Color {
	if is_far { color.mix(tint, 0.16) } else { color }
}

/// The cylinder warp that turns flat front-on drawings round a body of `radius`.
fn wrap(p: Pt, t: f32, radius: f32, turn: f32) -> Pt {
	let angle = (p.x / radius).clamp(-1.0, 1.0).asin() + t * turn;
	pt(radius * angle.clamp(-FRAC_PI_2, FRAC_PI_2).sin(), p.y)
}

/// A leg's skin outline through its hip, knee and ankle. A rubber-hose leg has no knee: it's
/// one tube that bows as the foot comes closer. Otherwise it is one shape bent through the knee,
/// so there's no seam at the joint: drawn with the body's anatomy if it has a leg profile, or
/// tapering evenly from hip to ankle.
fn leg_shape(puppet: &Authored, solved: &Solved, (hip, knee, ankle): (Pt, Pt, Pt)) -> Shape {
	let (start, end) = solved.body.leg_width;

	if puppet.style.motion.limbs == Joints::Hose {
		Shape::taper(
			&hose(hip, knee, ankle, solved.limbs.thigh + solved.limbs.shin),
			start,
			end,
		)
	} else if let Some(widths) = &solved.body.leg_profile {
		Shape::limb(hip, knee, ankle, widths)
	} else {
		Shape::taper(
			&[hip, hip.lerp(knee, 0.5), knee, knee.lerp(ankle, 0.5), ankle],
			start,
			end,
		)
	}
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

	svg.brushed_shaded(
		&leg_shape(puppet, solved, (hip, knee, ankle)),
		far(inks.skin, is_far, inks.finish.far),
		inks.finish.light,
		inks.finish.cel_shadow,
		ink,
		inks.finish.line,
	);

	// Socks, unless trousers come down over them. They fit the ankle they are drawn on.
	let trouser = puppet.part("legwear-shin", t);

	if trouser.is_none() {
		let angle = begin_bone(svg, knee, ankle);
		let fit = solved
			.body
			.leg_profile
			.map_or(1.0, |widths| widths[6] / 12.5);
		let sock = capsule(18.0 * fit, 15.5 * fit, 15.0 * fit).shift(pt(0.0, shin - 16.0 * fit));
		svg.brushed_shaded(
			&sock,
			far(inks.kit.sock, is_far, inks.finish.far),
			inks.finish.light.rotate(-angle),
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);
		svg.begin_clip(&sock.path());
		svg.fill(
			&Path::rect(-12.0 * fit, shin - 14.0 * fit, 24.0 * fit, 4.5 * fit),
			far(inks.top, is_far, inks.finish.far),
		);
		svg.end();
		svg.brush_line(&sock.path(), ink, inks.finish.line);
		svg.end();
	}

	let three_quarter = t > 0.5;
	let mirror = side < 0.0 && !three_quarter;

	if let Some(shoe) = puppet.view("shoe", three_quarter) {
		let scale = solved.body.foot_scale;
		// The shoe stays level with the ground and tips its toe as the foot lifts in a stride.
		// On a body that keeps to the floor, a foot lifted while standing points its toe down to
		// it (on tiptoe, in a jump).
		let lift = (solved.limbs.ankle_y - ankle.y).max(0.0);
		let toe = if posed.gait.is_some() {
			-lift * 0.5
		} else if three_quarter && solved.kneels((hip, knee, ankle)) {
			// Behind a knee on the floor, the foot lies along the floor carrying on the shin's
			// line: toe back, sole up.
			let shin = ankle - knee;
			shin.y.atan2(shin.x).to_degrees()
		} else if solved.body.floor {
			// Turned, the far foot's ground is further up the page (see `Posed::of`).
			let ground = solved.limbs.ankle_y + if side < 0.0 { 3.0 } else { -9.0 } * t;
			let length = shoe.bounds().1.x.max(1.0) * scale;
			let reach = ((ground - ankle.y) / length).clamp(0.0, 0.85);
			reach.asin().to_degrees() * if mirror { -1.0 } else { 1.0 }
		} else {
			0.0
		};
		// Shoes drawn bigger grow from the sole, so they still stand on the ground.
		let grow = if (scale - 1.0).abs() > f32::EPSILON {
			format!(" translate(0 24) scale({scale:.3}) translate(0 -24)")
		} else {
			String::new()
		};
		svg.begin(&format!(
			"translate({:.2} {:.2}) rotate({toe:.2}) scale({} 1){grow}",
			ankle.x,
			ankle.y,
			if mirror { -1 } else { 1 }
		));
		svg.brush_fill(
			&shoe.path(),
			far(inks.shoes, is_far, inks.finish.far),
			ink,
			inks.finish.line,
		);
		svg.cel_shade(&shoe, pt(-6.0, -5.0), inks.finish.cel_shadow);
		svg.begin_clip(&shoe.path());
		if let Some(sole) = puppet.view("shoe-sole", three_quarter) {
			// A drawn sole, trimmed to the shoe: only its top edge shows as a line.
			svg.fill(&sole.path(), far(inks.kit.sole, is_far, inks.finish.far));
			svg.brush_line(&sole.path(), ink, inks.finish.detail);
		} else {
			// The sole runs under the whole shoe, however big it is drawn.
			let (lo, hi) = shoe.bounds();
			let (heel, toe) = ((lo.x - 2.0).min(-30.0), (hi.x + 2.0).max(50.0));
			svg.fill(
				&Path::rect(heel, 17.0, toe - heel, 12.0),
				far(inks.kit.sole, is_far, inks.finish.far),
			);
			svg.brush_line(
				&Path::new().move_to(pt(heel, 17.0)).line_to(pt(toe, 17.0)),
				ink,
				inks.finish.detail,
			);
		}
		svg.end();

		if let Some(lace) = puppet.view("shoe-lace", three_quarter) {
			svg.brush_line(
				&lace.path(),
				far(inks.kit.sole, is_far, inks.finish.far),
				3.4,
			);
		}

		svg.brush_line(&shoe.path(), ink, inks.finish.line);
		svg.end();
	}

	// Trousers ride on the shin, over the top of the shoe.
	if let Some(trouser) = trouser {
		let angle = begin_bone(svg, knee, ankle);
		svg.brushed_shaded(
			&trouser,
			far(inks.bottom, is_far, inks.finish.far),
			inks.finish.light.rotate(-angle),
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(cuff) = puppet.part("legwear-shin-cuff", t) {
			svg.brush_line(
				&cuff.path(),
				far(inks.bottom, is_far, inks.finish.far).darken(0.25),
				inks.finish.detail,
			);
		}

		svg.end();
	}

	// The legwear's leg rides on the thigh (a skirt has none).
	if let Some(cover) = puppet.part("legwear-leg", t) {
		let angle = begin_bone(svg, hip, knee);
		svg.brushed_shaded(
			&cover,
			far(inks.bottom, is_far, inks.finish.far),
			inks.finish.light.rotate(-angle),
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(hem) = puppet.part("legwear-leg-hem", t) {
			svg.brush_line(
				&hem.path(),
				far(inks.bottom, is_far, inks.finish.far).darken(0.25),
				inks.finish.detail,
			);
		}

		svg.end();
	}
}

fn draw_torso(svg: &mut Svg, puppet: &Authored, look: &Look, inks: &Inks, solved: &Solved) {
	let t = solved.t;
	let ink = inks.line;
	let proportions = puppet.style.body(look.build);
	let (neck_y, hem_y) = (proportions.neck.y, proportions.hem_y);
	// Patterns are laid across a little more than the body, and clipped to it.
	let half = proportions.body_radius * 1.25;
	let body = |p: Pt| wrap(p, t, proportions.body_radius, inks.face.turn);
	// A drawn neck sits under the top, which covers its base with the neckline.
	let drawn_neck = puppet.part("neck", t);

	if let Some(neck) = &drawn_neck {
		svg.begin(&format!(
			"translate({:.1} {:.1})",
			proportions.neck.x, proportions.neck.y
		));
		svg.brushed_shaded(
			neck,
			inks.skin,
			inks.finish.light,
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);
		svg.end();
	}

	let legwear = puppet.required("legwear", t);
	let legwear = if proportions.floor && puppet.part("legwear-leg", t).is_none() {
		legwear.map(|p| solved.skirt(p))
	} else {
		legwear
	};
	svg.brushed_shaded(
		&legwear,
		inks.bottom,
		inks.finish.light,
		inks.finish.cel_shadow,
		ink,
		inks.finish.line,
	);

	if let Some(waist) = puppet.part("legwear-waist", t) {
		svg.brush_line(
			&waist.map(body).path(),
			inks.bottom.darken(0.25),
			inks.finish.detail,
		);
	}

	let torso = puppet.required("torso", t);
	let path = torso.path();
	svg.brush_fill(&path, inks.top, ink, inks.finish.line * 1.1);
	svg.begin_clip(&path);

	// Patterns wrap round the body with it: stripes are bands, dots and stars repeat the kit's
	// motifs.
	if look.pattern == Pattern::Dots {
		let dot = puppet.required("pattern-dot", t);

		let columns = (proportions.body_radius / 16.0) as i32;
		let mut row = 0;

		while neck_y + 4.0 + row as f32 * 20.0 <= hem_y + 3.0 {
			for column in -columns..=columns {
				let offset = if row % 2 == 0 { 0.0 } else { 11.0 };
				let at = pt(
					column as f32 * 22.0 + offset,
					neck_y + 4.0 + row as f32 * 20.0,
				);
				svg.fill(&dot.shift(at).map(body).path(), inks.stripe);
			}

			row += 1;
		}
	}

	if look.pattern == Pattern::Star {
		let star = puppet
			.required("pattern-star", t)
			.shift(pt(0.0, neck_y + 50.0));
		let star = star.map(body);
		svg.brushed(&star.path(), inks.stripe, ink, inks.finish.detail);
	}

	if look.pattern == Pattern::Stripes {
		let mut y = neck_y + 26.0;

		while y <= hem_y - 27.0 {
			let band = Shape::from_points(
				vec![
					pt(-half, y),
					pt(-half * 0.375, y + 6.0),
					pt(half * 0.375, y + 6.0),
					pt(half, y),
					pt(half, y + 4.0),
					pt(half, y + 8.0),
					pt(half, y + 11.0),
					pt(half * 0.375, y + 17.0),
					pt(-half * 0.375, y + 17.0),
					pt(-half, y + 11.0),
					pt(-half, y + 8.0),
					pt(-half, y + 4.0),
					pt(-half, y),
				],
				true,
			)
			.unwrap_or_else(|| unreachable!("four whole curves"));
			svg.fill(&band.map(body).path(), inks.stripe);
			y += 24.0;
		}
	}

	// A ribbed hem.
	svg.fill(
		&Path::rect(-half, hem_y, half * 2.0, 14.0),
		inks.top.darken(0.12),
	);

	// An apron over the top, inside its outline so the shadow falls across both.
	if look.has(Extra::Apron) {
		if let Some(apron) = puppet.part("apron", t) {
			svg.brushed(&apron.path(), inks.kit.apron, ink, inks.finish.detail);
		}

		if let Some(pocket) = puppet.part("apron-pocket", t) {
			svg.brushed(
				&pocket.map(body).path(),
				inks.kit.apron.darken(0.06),
				ink,
				inks.finish.detail,
			);
		}
	}

	svg.end();
	svg.cel_shade(&torso, inks.finish.light, inks.finish.cel_shadow);

	if let Some(hem) = puppet.part("torso-hem", t) {
		svg.brush_line(&hem.map(body).path(), ink, inks.finish.detail);
	}

	svg.brush_line(&path, ink, inks.finish.line * 1.1);

	// A plain tube of a neck, then the collar over it.
	if drawn_neck.is_none() {
		let neck = capsule(30.0, 30.0, 30.0).shift(pt(lerp(0.0, 6.0, t), neck_y - 24.0));
		svg.brushed_shaded(
			&neck,
			inks.skin,
			inks.finish.light,
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);
	}

	if let Some(collar) = puppet.part("collar", t) {
		svg.brushed_shaded(
			&collar,
			inks.stripe,
			inks.finish.light,
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);
	}

	// A satchel: the strap across the body, the bag at the hip.
	if look.has(Extra::Satchel) {
		if let Some(strap) = puppet.part("satchel-strap", t) {
			svg.brush_line(&strap.path(), ink, 11.0);
			svg.brush_line(&strap.path(), inks.kit.satchel.darken(0.15), 7.0);
		}

		if let Some(bag) = puppet.part("satchel", t) {
			svg.brushed_shaded(
				&bag,
				inks.kit.satchel,
				inks.finish.light,
				inks.finish.cel_shadow,
				ink,
				inks.finish.line,
			);
		}

		if let Some(flap) = puppet.part("satchel-flap", t) {
			svg.brushed_shaded(
				&flap,
				inks.kit.satchel.darken(0.12),
				inks.finish.light,
				inks.finish.cel_shadow,
				ink,
				inks.finish.line,
			);
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
	/// Props are drawn for a kid's hand; a grown-up's bigger hand holds them bigger.
	scale: f32,
}

/// Where the middle of the mouth is in figure units, after the head and body have moved.
fn mouth_on_body(solved: &Solved, posed: &Posed, face: FaceRules, mouth: Pt) -> Pt {
	let local =
		wrap(mouth, posed.t, face.radius, face.turn) + pt(0.0, -solved.body.head_above_neck);
	solved.torso(solved.body.neck + local.rotate(posed.head_tilt))
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
			(
				inks.skin.mix(inks.kit.mud, 0.92),
				inks.skin.mix(inks.kit.mud, 0.4),
			)
		} else {
			(inks.skin, inks.skin)
		};
		// Gloved hands are the gloves' colour, whatever the skin.
		let hand_skin = inks.features.gloves.unwrap_or(hand_skin);

		if puppet.style.motion.limbs == Joints::Hose {
			// Rubber hose: one tube from shoulder to wrist, with no elbow, that bows as the hand
			// comes closer.
			let width = solved.body.forearm_width.0;
			let tube = Shape::taper(
				&hose(
					shoulder,
					elbow,
					wrist,
					solved.limbs.upper_arm + solved.limbs.forearm,
				),
				width,
				width,
			);
			svg.brushed_shaded(
				&tube,
				far(arm_skin, is_far, inks.finish.far),
				inks.finish.light,
				inks.finish.cel_shadow,
				ink,
				inks.finish.line,
			);
		} else {
			// A body with arm anatomy draws the whole bare arm, shoulder to wrist, as one shape
			// bent through the elbow; the sleeves go over it. Otherwise only the forearm shows.
			if let Some(widths) = &solved.body.arm_profile {
				svg.brushed_shaded(
					&Shape::limb(shoulder, elbow, wrist, widths),
					far(arm_skin, is_far, inks.finish.far),
					inks.finish.light,
					inks.finish.cel_shadow,
					ink,
					inks.finish.line,
				);
			}

			let angle = begin_bone(svg, elbow, wrist);

			if solved.body.arm_profile.is_none() {
				let (elbow_width, wrist_width) = solved.body.forearm_width;
				let forearm = capsule(solved.limbs.forearm, elbow_width, wrist_width);
				svg.brushed_shaded(
					&forearm,
					far(arm_skin, is_far, inks.finish.far),
					inks.finish.light.rotate(-angle),
					inks.finish.cel_shadow,
					ink,
					inks.finish.line,
				);
			}

			// A long sleeve carries on down the forearm.
			if let Some(lower) = puppet.part("forearm-sleeve", t) {
				svg.brushed_shaded(
					&lower,
					far(inks.top, is_far, inks.finish.far),
					inks.finish.light.rotate(-angle),
					inks.finish.cel_shadow,
					ink,
					inks.finish.line,
				);

				if let Some(cuff) = puppet.part("forearm-sleeve-cuff", t) {
					svg.brush_line(&cuff.path(), ink, inks.finish.detail);
				}
			}

			svg.end();

			let angle = begin_bone(svg, shoulder, elbow);
			let sleeve = puppet.required("sleeve", t);
			let path = sleeve.path();
			svg.brush_fill(
				&path,
				far(inks.top, is_far, inks.finish.far),
				ink,
				inks.finish.line,
			);
			svg.begin_clip(&path);

			// The pattern runs down the sleeve to its cuff, a band at the end of the upper arm.
			let cuff_y = solved.limbs.upper_arm - 12.0;

			match self.look.pattern {
				Pattern::Stripes => {
					let mut y = 6.0;

					while y <= cuff_y - 4.0 {
						svg.fill(
							&Path::rect(-20.0, y, 40.0, 10.0),
							far(inks.stripe, is_far, inks.finish.far),
						);
						y += 18.0;
					}
				}
				Pattern::Dots => {
					let dot = puppet.required("pattern-dot", t);
					let mut step = 0.0;

					while 6.0 + step <= cuff_y - 4.0 {
						for (x, y) in [(-6.0, 6.0), (7.0, 16.0), (-5.0, 27.0)] {
							if y + step <= cuff_y {
								svg.fill(
									&dot.shift(pt(x, y + step)).path(),
									far(inks.stripe, is_far, inks.finish.far),
								);
							}
						}

						step += 30.0;
					}
				}
				Pattern::Plain | Pattern::Star => {}
			}

			svg.fill(
				&Path::rect(-20.0, cuff_y, 40.0, 16.0),
				far(inks.top.darken(0.12), is_far, inks.finish.far),
			);
			svg.end();
			svg.cel_shade(
				&sleeve,
				inks.finish.light.rotate(-angle),
				inks.finish.cel_shadow,
			);

			if let Some(cuff) = puppet.part("sleeve-cuff", t) {
				svg.brush_line(&cuff.path(), ink, inks.finish.detail);
			}

			svg.brush_line(&path, ink, inks.finish.line);
			svg.end();
		}

		// Held things sit in the fist, under the fingers: a little past halfway down it, so a
		// bigger hand holds them further from the wrist.
		let along = (wrist - elbow).norm();

		if let Some(held) = held {
			let fist = wrist + along * grip(&puppet.required("hand-fist", 0.0)) * held.scale;
			draw_prop(svg, puppet, ink, held, fist, along, side);
		}

		// The thumb faces the body from the front and forwards when turned.
		let mirror = side < 0.0 || t > 0.5;
		let angle = (-along.x).atan2(along.y).to_degrees();
		svg.begin(&format!(
			"translate({:.2} {:.2}) rotate({:.2}) scale({} 1) scale({})",
			wrist.x,
			wrist.y,
			angle + turn * if mirror { -1.0 } else { 1.0 },
			if mirror { -1 } else { 1 },
			solved.body.hand_scale
		));
		let light = inks.finish.light.rotate(-angle);
		let light = if mirror { pt(-light.x, light.y) } else { light };
		draw_hand(
			svg,
			puppet,
			hand,
			far(hand_skin, is_far, inks.finish.far),
			ink,
			light,
		);
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
	let (finish, kit) = (puppet.style.finish, puppet.style.inks);
	let fixed = |name: &str| puppet.part(name, 0.0);
	let light = pt(-5.0, -5.0);

	match held.prop {
		Prop::Jam => {
			svg.begin(&format!(
				"translate({:.2} {:.2}) scale({:.3})",
				fist.x, fist.y, held.scale
			));
			let jar = puppet.required("prop-jam", 0.0);
			svg.brushed_shaded(&jar, kit.jam, light, finish.cel_shadow, ink, finish.line);

			if let Some(label) = fixed("prop-jam-label") {
				svg.brushed(&label.path(), kit.cream, ink, finish.detail);
			}

			if let Some(shine) = fixed("prop-jam-shine") {
				svg.stroke_alpha(&shine.path(), palette::WHITE, 4.0, 0.7);
			}

			if let Some(lid) = fixed("prop-jam-lid") {
				svg.brushed_shaded(&lid, kit.cream, light, finish.cel_shadow, ink, finish.line);
			}

			svg.end();
		}
		Prop::Parcel => {
			svg.begin(&format!(
				"translate({:.2} {:.2}) scale({:.3})",
				fist.x,
				fist.y - 8.0 * held.scale,
				held.scale
			));

			for (name, color) in [
				("prop-parcel-side", kit.kraft.darken(0.15)),
				("prop-parcel-top", kit.kraft.lighten(0.12)),
				("prop-parcel", kit.kraft),
			] {
				if let Some(face) = fixed(name) {
					svg.brushed(&face.path(), color, ink, finish.line);
				}
			}

			for string in puppet.copies("prop-parcel-string", 0.0) {
				svg.brush_line(&string.path(), kit.string, 6.0);
			}

			svg.end();
		}
		Prop::Toothbrush => {
			// Near the mouth, it is brushing; how near, and how far it scrubs, grow with the
			// hand the kit draws.
			let size = fist_size(&puppet.required("hand-fist", 0.0));
			let brushing = (fist - held.mouth).len() < 70.0 * size;
			let (dir, scrub) = if brushing {
				let towards = if held.mouth.x < fist.x { -1.0 } else { 1.0 };
				(pt(towards, 0.0), (held.time * 22.0).sin() * 8.0 * size)
			} else {
				// Sticking out of the fist, sideways from the forearm, away from the body.
				(along.perp() * -side, 0.0)
			};
			let angle = dir.y.atan2(dir.x).to_degrees();
			let flip = if dir.x < 0.0 { -1.0 } else { 1.0 };
			svg.begin(&format!(
				"translate({:.2} {:.2}) rotate({angle:.2}) scale({:.3} {:.3})",
				fist.x + scrub,
				fist.y,
				held.scale,
				flip * held.scale
			));
			let handle = puppet.required("prop-toothbrush", 0.0);
			svg.brushed_shaded(
				&handle,
				kit.brush,
				light,
				finish.cel_shadow,
				ink,
				finish.line,
			);

			if let Some(bristles) = fixed("prop-toothbrush-bristles") {
				svg.brushed(&bristles.path(), kit.bristles, ink, finish.detail);
			}

			for line in puppet.copies("prop-toothbrush-bristle-line", 0.0) {
				svg.brush_line(&line.path(), kit.bristles.darken(0.15), 1.6);
			}

			if let Some(paste) = fixed("prop-toothpaste") {
				svg.brushed(&paste.path(), kit.paste, ink, finish.detail);
			}

			svg.end();

			// Foam round the mouth while brushing.
			if brushing {
				for i in 0..5 {
					let at = held.mouth
						+ pt(noise(77, i) * 34.0, 6.0 + noise(78, i) * 16.0)
						+ pt(0.0, (held.time * 6.0 + i as f32).sin() * 2.0);
					let r = 5.0 + noise(79, i).abs() * 5.0;
					svg.brushed(&Path::circle(at, r), kit.bristles, ink, 1.6);
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
	let finish = puppet.style.finish;
	let shape = puppet.required(name, 0.0);
	let path = shape.path();
	svg.brush_fill(&path, skin, ink, finish.line * 0.9);
	svg.cel_shade(&shape, light * 0.6, finish.cel_shadow);

	for line in puppet.copies(&format!("{name}-line"), 0.0) {
		svg.brush_line(&line.path(), ink, finish.detail * 0.9);
	}

	svg.brush_line(&path, ink, finish.line * 0.9);

	if let Some(cuff) = puppet.part("glove-cuff", 0.0) {
		svg.brushed(&cuff.path(), skin, ink, finish.line * 0.9);
	}
}

fn draw_head(svg: &mut Svg, puppet: &Authored, look: &Look, inks: &Inks, posed: &Posed, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let face = &posed.face;
	let head = puppet.required("head", t);
	let head_path = head.path();
	let hair = puppet.required("hair", t);
	let warp = |p: Pt| wrap(p, t, inks.face.radius, inks.face.turn);

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
		svg.brushed_shaded(
			&ear,
			inks.skin,
			pt(-5.0, -4.0),
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(curl) = &curl {
			svg.brush_line(&curl.path(), ink, inks.finish.detail);
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
		svg.brushed_shaded(
			lock,
			inks.hair,
			inks.finish.light,
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(tie) = ties.get(index) {
			svg.brushed(&tie.path(), inks.stripe, ink, inks.finish.detail);
		}

		svg.end();
	}

	svg.brush_fill(&head_path, inks.skin, ink, inks.finish.line * 1.15);
	svg.begin_clip(&head_path);
	// The hair casts a soft band of shadow on the forehead.
	svg.begin("translate(0 9)");
	svg.fill_alpha(&hair.path(), Color(0xC0_5A_3E), 0.35);
	svg.end();
	svg.end();
	svg.cel_shade(&head, inks.finish.light, inks.finish.cel_shadow);

	// A bald head shines.
	let shines = puppet.copies("head-shine", t);

	if !shines.is_empty() {
		svg.begin_clip(&head_path);

		for shine in &shines {
			svg.brush_line(&shine.path(), inks.skin.lighten(0.45), 7.0);
		}

		svg.end();
	}

	// Cheeks and freckles, wrapped round the face. Anything turned past the edge is hidden.
	for side in [-1.0_f32, 1.0] {
		let (at, size) = (inks.features.blush, inks.features.blush_size);
		let blush = Shape::ellipse(pt(side * at.x, at.y), size.x, size.y).map(warp);
		svg.fill_alpha(&blush.path(), inks.blush, 0.3);

		if look.has(Extra::Freckles) {
			for (x, y) in [(52.0, 24.0), (62.0, 18.0), (64.0, 29.0)] {
				let at = pt(side * x, y);

				if (at.x / inks.face.radius).asin() + t * inks.face.turn < 1.25 {
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
		svg.brushed(&drip.path(), inks.kit.bogey, ink, inks.finish.detail);
	}

	draw_mouth(svg, puppet, inks, posed, time);

	if look.has(Extra::Moustache)
		&& let Some(moustache) = puppet.part("moustache", t)
	{
		let moustache = moustache.map(warp);
		svg.brush_fill(&moustache.path(), inks.hair, ink, inks.finish.detail * 1.2);
		svg.cel_shade(&moustache, pt(-3.0, -3.0), inks.finish.cel_shadow);
		svg.brush_line(&moustache.path(), ink, inks.finish.detail * 1.2);
	}

	if inks.eyes.lashes || look.has(Extra::Lashes) {
		for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
			if matches!(face.eyes[index], Eye::Open { .. }) {
				let centre = pt(side * inks.features.eye.x, inks.features.eye.y);

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
				let eye = inks.features.eye;
				let ring = lens.shift(pt(side * eye.x, eye.y)).map(warp);
				svg.fill_alpha(&ring.path(), inks.kit.lens, 0.3);
				svg.brush_line(&ring.path(), ink, 4.5);
			}
		}

		if let Some(bridge) = puppet.part("glasses-bridge", t) {
			svg.brush_line(&bridge.map(warp).path(), ink, 4.5);
		}
	}

	svg.brush_line(&head_path, ink, inks.finish.line * 1.15);

	// Hair over everything on the head, brows on top of the hair. Loose hair follows through
	// after a move.
	if let Some(tuft) = puppet.part("tuft", t) {
		// The tuft swings about its root.
		svg.begin(&format!(
			"rotate({:.2} {:.1} -100)",
			posed.follow,
			lerp(-12.0, -26.0, t)
		));
		svg.brushed_shaded(
			&tuft,
			inks.hair,
			inks.finish.light,
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(shine) = puppet.part("tuft-shine", t) {
			svg.begin_clip(&tuft.path());
			svg.brush_line(&shine.path(), inks.hair_light, 7.0);
			svg.end();
		}

		svg.end();
	}

	svg.brushed_shaded(
		&hair,
		inks.hair,
		inks.finish.light,
		inks.finish.cel_shadow,
		ink,
		inks.finish.line,
	);

	let shines = puppet.copies("hair-shine", t);

	if !shines.is_empty() {
		svg.begin_clip(&hair.path());

		for shine in &shines {
			svg.brush_line(&shine.path(), inks.hair.lighten(0.3), 8.0);
		}

		svg.end();
	}

	for strand in puppet.copies("hair-strand", t) {
		svg.brush_line(&strand.path(), ink, inks.finish.detail * 0.8);
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
		svg.brushed_shaded(
			&bow,
			inks.stripe,
			pt(-4.0, -4.0),
			inks.finish.cel_shadow,
			ink,
			inks.finish.line,
		);

		if let Some(tie) = puppet.part("bow-knot", t) {
			svg.brushed(
				&tie.path(),
				inks.stripe.darken(0.12),
				ink,
				inks.finish.detail,
			);
		}

		svg.end();
	}

	// A post cap sits over the hair.
	if look.has(Extra::PostCap) {
		if let Some(cap) = puppet.part("cap", t) {
			svg.brushed_shaded(
				&cap,
				inks.kit.cap,
				inks.finish.light,
				inks.finish.cel_shadow,
				ink,
				inks.finish.line,
			);
		}

		if let Some(brim) = puppet.part("cap-brim", t) {
			svg.brushed_shaded(
				&brim,
				inks.kit.cap.darken(0.18),
				inks.finish.light,
				inks.finish.cel_shadow,
				ink,
				inks.finish.line,
			);
		}

		if let Some(badge) = puppet.part("cap-badge", t) {
			svg.brushed(
				&badge.map(warp).path(),
				inks.kit.badge,
				ink,
				inks.finish.detail,
			);
		}
	}

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		let brow = face.brows[index];
		let rest = inks.features.brow;
		let centre = pt(side * rest.x, rest.y - brow.raise - posed.brow_lift * 100.0);
		let shape = puppet
			.required("brow", t)
			.map(|p| pt(p.x * side, p.y).rotate(-side * brow.tilt) + centre)
			.map(warp);
		svg.brushed(&shape.path(), inks.hair, ink, inks.features.brow_weight);
	}
}

/// Whether hair hangs down past the ears (they sit about y = 8 in head units).
fn covers_ears(hair: &Shape) -> bool {
	hair.bounds().1.y > 28.0
}

fn draw_eye(svg: &mut Svg, inks: &Inks, posed: &Posed, eye: Eye, side: f32, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let build = inks.eyes;
	let warp = |p: Pt| wrap(p, t, inks.face.radius, inks.face.turn);
	let centre = pt(side * inks.features.eye.x, inks.features.eye.y);
	let (rx, ry) = (build.size.x, build.size.y);
	// Shut and dizzy eyes were drawn for the cartoon eye; they grow with the style's.
	let k = rx / 20.0;
	let line = |points: &[Pt]| Shape::taper(points, 5.5 * k, 5.5 * k);

	let (upper, lower, tilt) = match eye {
		Eye::Open { upper, lower, tilt } => (upper, lower, tilt),
		Eye::Smiling => {
			// A shut eye curved up like a happy arch.
			let arch = [
				centre + pt(-17.0, 6.0) * k,
				centre + pt(-8.0, -6.0) * k,
				centre + pt(8.0, -6.0) * k,
				centre + pt(17.0, 6.0) * k,
			];
			svg.fill(&line(&arch).map(warp).path(), ink);
			return;
		}
		Eye::Crossed => {
			for (a, b) in [
				(pt(-13.0, -13.0), pt(13.0, 13.0)),
				(pt(13.0, -13.0), pt(-13.0, 13.0)),
			] {
				svg.fill(
					&line(&[centre + a * k, centre + b * k]).map(warp).path(),
					ink,
				);
			}
			return;
		}
		Eye::Spiral => {
			let white = Shape::ellipse(centre, rx, ry).map(warp);
			svg.fill(&white.path(), inks.kit.eye);
			let spin = time * 4.0 * side;
			let turns: Vec<Pt> = (0..28)
				.map(|i| {
					let a = i as f32 * 0.5 + spin;
					warp(centre + pt(a.cos(), a.sin()) * ((2.0 + i as f32 * 0.6) * k))
				})
				.collect();
			svg.brush_line(&Path::spline(&turns), ink, 3.0);
			svg.brush_line(&white.path(), ink, inks.finish.detail);
			return;
		}
	};

	let white = Shape::ellipse(centre, rx, ry).map(warp);
	let white_path = white.path();
	let tilt = -side * tilt;

	svg.fill(&white_path, inks.kit.eye);
	svg.begin_clip(&white_path);
	let look = centre
		+ pt(
			posed.gaze.x * build.gaze.x,
			posed.gaze.y * build.gaze.y + 2.0,
		);
	let size = posed.face.pupil;
	let iris = Shape::ellipse(look, build.iris.x * size, build.iris.y * size).map(warp);
	svg.fill(&iris.path(), inks.pupil);

	// Layered irises: a darker band across the top, a pupil, and a rim.
	if build.iris_band > 0.0 {
		let drop = build.iris.y * size * 2.0 * (1.0 - build.iris_band);
		let band = Shape::ellipse(
			look - pt(0.0, drop),
			build.iris.x * size,
			build.iris.y * size,
		)
		.map(warp);
		svg.begin_clip(&iris.path());
		svg.fill(&band.path(), inks.pupil.darken(0.5));
		svg.end();
	}

	if build.pupil.x > 0.0 {
		let pupil = Shape::ellipse(
			look + pt(0.0, build.iris.y * size * 0.1),
			build.pupil.x * size,
			build.pupil.y * size,
		)
		.map(warp);
		svg.fill(&pupil.path(), Color(0x1E_12_18));
	}

	if build.rim > 0.0 {
		svg.brush_line(&iris.path(), inks.pupil.darken(0.6), build.rim);
	}

	// Pie-cut eyes: a wedge cut from the pupil towards the light, instead of round glints.
	if build.pie_cut {
		let pupil = if build.pupil.x > 0.0 {
			build.pupil
		} else {
			build.iris
		} * size;
		let centre = look + pt(0.0, build.iris.y * size * 0.1);
		let edge = |angle: f32| {
			let angle = angle.to_radians();
			warp(centre + pt(angle.cos() * pupil.x * 1.1, angle.sin() * pupil.y * 1.1))
		};
		let wedge = Path::new()
			.move_to(warp(centre + pt(pupil.x * 0.1, -pupil.y * 0.05)))
			.line_to(edge(-80.0))
			.line_to(edge(-55.0))
			.line_to(edge(-30.0))
			.close();
		svg.fill(&wedge, inks.kit.eye);
	}

	for [x, y, r] in build.shines {
		svg.fill(
			&Path::circle(warp(look + pt(x, y) * size), r * size),
			inks.kit.eye,
		);
	}

	svg.end();
	svg.brush_line(&white_path, ink, inks.finish.detail);

	// Lids paint over the eye's own outline, so the lash line becomes the top edge.
	let lid_clip = Shape::ellipse(centre, rx + 3.0, ry + 3.0).map(warp);
	svg.begin_clip(&lid_clip.path());

	// Upper lid: skin down to a curved lash line, tilted by the mood.
	let upper = upper.max(posed.blink);
	let edge_y = centre.y - ry + 2.0 * ry * upper;
	// The lash line arches with the eye when open and flattens as the lid comes down.
	let sag = -9.0 * k * (1.0 - upper);
	let rotate = |p: Pt| (p - pt(centre.x, edge_y)).rotate(tilt) + pt(centre.x, edge_y);
	let edge = |y: f32, bend: f32| {
		[
			pt(centre.x - 34.0 * k, y),
			pt(centre.x - 14.0 * k, y + bend),
			pt(centre.x + 14.0 * k, y + bend),
			pt(centre.x + 34.0 * k, y),
		]
	};
	let lid_edge = edge(edge_y, sag);
	let lid = closed_over(&lid_edge, -70.0 * k).map(rotate).map(warp);
	svg.fill(&lid.path(), inks.skin);
	let lash = open_curve(&lid_edge).map(rotate).map(warp);
	svg.brush_line(&lash.path(), ink, build.lash);

	// Lower lid: the cheek pushing up, curved upwards in a smile or a squint.
	if lower > 0.01 && upper < 0.95 {
		let low_y = centre.y + ry - 2.0 * ry * lower;
		let cheek_edge = edge(low_y, -10.0 * k * lower);
		svg.fill(
			&closed_over(&cheek_edge, 50.0 * k).map(warp).path(),
			inks.skin,
		);
		svg.brush_line(
			&open_curve(&cheek_edge).map(warp).path(),
			ink,
			build.lower_lid,
		);
	}

	svg.end();

	// A short lower lash under the outer half of the eye.
	if build.lower_lash > 0.0 && upper < 0.95 {
		let under: Vec<Pt> = (0..7)
			.map(|i| {
				let a = 0.12 + i as f32 * 0.07;
				warp(centre + pt(side * rx * a.cos() * 1.02, ry * a.sin() * 1.02))
			})
			.collect();
		svg.brush_line(&Path::spline(&under), ink, build.lower_lash);
	}
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
	let warp = |p: Pt| wrap(p, t, inks.face.radius, inks.face.turn);
	// The nose stands proud of the face, so it travels a little further than the cylinder.
	let poke = pt(9.0 * t, 0.0);
	let (at, scale) = (inks.features.nose, inks.features.nose_scale);
	let nose = puppet
		.required("nose", t)
		.map(|p| p * scale)
		.shift(at)
		.map(warp)
		.shift(poke);
	svg.brush_fill(&nose.path(), inks.nose, inks.line, inks.finish.detail * 1.1);
	svg.cel_shade(&nose, pt(-4.0, -4.0), inks.finish.cel_shadow);
	svg.brush_line(&nose.path(), inks.line, inks.finish.detail * 1.1);

	if inks.features.nose_shine {
		svg.fill_alpha(
			&Path::circle(warp(at + pt(-4.0, -4.0) * scale) + poke, 2.6 * scale),
			Color(0xFF_FF_FF),
			0.7,
		);
	}

	if wrinkle {
		for line in puppet.copies("nose-wrinkle", t) {
			svg.brush_line(&line.map(warp).path(), inks.line, inks.finish.detail);
		}
	}
}

/// The mouth: a lip-sync shape while talking, otherwise the mood's own mouth, opened up when
/// the state asks for an open mouth.
fn draw_mouth(svg: &mut Svg, puppet: &Authored, inks: &Inks, posed: &Posed, time: f32) {
	let t = posed.t;
	let ink = inks.line;
	let face = &posed.face;
	let (centre, scale) = (inks.features.mouth, inks.features.mouth_scale);
	let curl = face.curl;
	// Mouths keep the mood: corners curl up or down a little.
	let curled = |p: Pt| pt(p.x, p.y - curl * 6.0 * (p.x / 30.0).powi(2));
	let place = |shape: &Shape| {
		shape.map(|p| wrap(p * scale + centre, t, inks.face.radius, inks.face.turn))
	};
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
			dimples(svg, ink, inks.finish.detail, &place, 36.0, -6.0, -6.0);
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

			dimples(svg, ink, inks.finish.detail, &place, 48.0, -4.0, -16.0);
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
			svg.brush_line(
				&mouth(&by(drawn("mouth-pout-lip"))).path(),
				ink,
				inks.finish.detail,
			);
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
			svg.brushed(
				&tongue.path(),
				inks.kit.tongue,
				ink,
				inks.finish.detail * 1.2,
			);

			if let Some(groove) = puppet.part("mouth-bleh-groove", t) {
				svg.brush_line(
					&place(&groove.map(tilt)).path(),
					inks.kit.tongue.darken(0.3),
					inks.finish.detail * 0.8,
				);
			}
		}
		(None, MouthShape::Wobble) => {
			let wobble = |p: Pt| pt(p.x, p.y + (p.x * 0.25 + time * 6.0).sin() * 3.0);
			svg.fill(&mouth(&drawn("mouth-rest").map(wobble)).path(), ink);
		}
		(Some(Viseme::Mbp), _) => {
			svg.fill(&mouth(&drawn("mouth-mbp")).path(), ink);
			svg.brush_line(
				&mouth(&drawn("mouth-mbp-lip")).path(),
				ink,
				inks.finish.detail,
			);
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
			svg.brush_line(
				&mouth(&drawn("mouth-fv-lip")).path(),
				ink,
				inks.finish.detail,
			);
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
		let corner = wrap(
			centre + pt(30.0, -10.0),
			t,
			inks.face.radius,
			inks.face.turn,
		);
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
	svg.brush_fill(&path, inks.kit.maw, inks.line, inks.finish.detail * 1.4);
	svg.begin_clip(&path);

	if let Some(tongue) = tongue {
		svg.fill(&tongue.path(), inks.kit.tongue);
	}

	if let Some(teeth) = teeth {
		svg.fill(&teeth.path(), inks.teeth);
	}

	svg.end();
	svg.brush_line(&path, inks.line, inks.finish.detail * 1.4);
}

/// Little tucks at the corners of a smile.
fn dimples(
	svg: &mut Svg,
	ink: Color,
	weight: f32,
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
		svg.brush_line(&place(&tuck).path(), ink, weight);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure;
	use crate::art::figure::Frame;
	use crate::art::figure::Pose;
	use crate::art::parts::Turn;
	use crate::art::rig::Stance;

	fn cartoon() -> Arc<StylePack> {
		StylePack::default_pack()
	}

	fn kid() -> Proportions {
		*cartoon().body(Build::Kid)
	}

	/// The cartoon face cylinder's warp.
	fn warp(p: Pt, t: f32) -> Pt {
		let face = cartoon().face;
		wrap(p, t, face.radius, face.turn)
	}

	#[test]
	fn the_front_view_is_unwarped_and_turning_moves_the_face_right() {
		assert!((warp(pt(30.0, 5.0), 0.0) - pt(30.0, 5.0)).len() < 1e-3);
		assert!(warp(pt(0.0, 0.0), 1.0).x > 40.0);
		// The far side foreshortens: equal steps on the face shrink towards the edge.
		let near = warp(pt(-20.0, 0.0), 1.0).x - warp(pt(-40.0, 0.0), 1.0).x;
		let far = warp(pt(40.0, 0.0), 1.0).x - warp(pt(20.0, 0.0), 1.0).x;
		assert!(far < near);
	}

	#[test]
	fn the_face_slides_as_far_as_the_warp_moves_it() {
		let warped = warp(pt(0.0, 0.0), 1.0).x;
		assert!((cartoon().face.slide * STAGED_FACING - warped).abs() < 1.0);
	}

	#[test]
	fn hands_reach_their_targets_through_the_rig() {
		let frame = Frame::authored(&cartoon(), Build::Kid);

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
			let posed = Posed::of(&state, &cartoon(), Build::Kid);
			let solved = Solved::new(&posed, &kid(), 0.0);

			for side in [-1.0, 1.0] {
				let (shoulder, elbow, wrist) = solved.arm(&posed, side);
				let target = if side < 0.0 {
					posed.hands.0
				} else {
					posed.hands.1
				};
				assert!(((elbow - shoulder).len() - kid().limbs.upper_arm).abs() < 0.01);
				assert!(
					(wrist - solved.torso(target)).len() < 0.5,
					"{pose:?}, side {side}"
				);
			}
		}
	}

	#[test]
	fn facing_left_mirrors_the_figure_and_swaps_the_near_hand() {
		let frame = Frame::authored(&cartoon(), Build::Kid);
		let mut state = FigureState::new(&frame);
		state.hands = frame.hands(Pose::Point, -STAGED_FACING);
		state.hand_shapes = Pose::Point.hand_shapes();
		state.facing = -STAGED_FACING;
		let posed = Posed::of(&state, &cartoon(), Build::Kid);
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
		let frame = Frame::authored(&cartoon(), Build::Kid);

		for facing in [0.0, STAGED_FACING] {
			let mut state = FigureState::new(&frame);
			state.facing = facing;
			state.hands = frame.hands(Pose::HoldNose, facing);
			let nose = Posed::of(&state, &cartoon(), Build::Kid).hands.1;
			let face_x = facing / STAGED_FACING * cartoon().face.slide * STAGED_FACING;
			assert!((nose.x - face_x).abs() < 30.0, "facing {facing}: {nose:?}");

			state.hands = frame.hands(Pose::Wave, facing);
			let posed = Posed::of(&state, &cartoon(), Build::Kid);
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
			&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/kerfuffle/cast.toml"),
			&cartoon(),
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
		let lens = format!("fill=\"{}\"", cartoon().inks.lens);
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
		let posed = Posed::of(&state, &cartoon(), Build::Kid);
		// The jam is in the page-right hand, which is the far hand when facing right.
		assert_eq!(posed.prop, Some((Prop::Jam, true)));
		assert!(drawn(&look, &state).contains(&format!("fill=\"{}\"", cartoon().inks.jam)));
	}

	#[test]
	fn brushing_keeps_the_toothbrush_hand_at_the_mouth_when_turned() {
		let frame = Frame::authored(&cartoon(), Build::Kid);
		let mut state = FigureState::new(&frame);
		state.facing = STAGED_FACING;
		state.hands = frame.hands(Pose::Brush, STAGED_FACING);
		state.prop = Some(Prop::Toothbrush);
		let posed = Posed::of(&state, &cartoon(), Build::Kid);
		assert_eq!(posed.prop, Some((Prop::Toothbrush, true)), "no wave swap");
		let solved = Solved::new(&posed, &kid(), 0.0);
		let (_, _, wrist) = solved.arm(&posed, 1.0);
		assert!(
			(wrist - mouth_on_body(&solved, &posed, cartoon().face, cartoon().features.mouth))
				.len() < 70.0
		);
	}

	#[test]
	fn every_puppet_file_draws_a_figure() {
		let world =
			std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/kerfuffle/puppets");

		for name in ["stinky", "bogey", "grubby", "shouty", "fibby"] {
			let puppet = Authored::load(&world.join(format!("{name}.svg")), &cartoon())
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
		let world =
			std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/kerfuffle/puppets");
		let hair = |name: &str| {
			Authored::load(&world.join(format!("{name}.svg")), &cartoon())
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
		let warm = cartoon().inks.blush;
		assert_eq!(blush(Color(0xF4_A2_6B), warm), warm);
		assert_ne!(blush(Color(0x62_B9_AD), warm), warm);
	}

	#[test]
	fn a_hose_limb_bows_with_its_slack_and_keeps_its_length() {
		let length = |line: &[Pt]| {
			line.windows(2)
				.map(|pair| (pair[1] - pair[0]).len())
				.sum::<f32>()
		};
		let root = pt(0.0, 0.0);

		// Pulled taut, the tube is straight.
		let taut = hose(root, pt(0.0, 50.0), pt(0.0, 100.0), 100.0);
		assert!(taut.iter().all(|p| p.x.abs() < 1e-3), "{taut:?}");

		// With slack it bows towards the joint, as one arc, and stays about as long.
		for joint in [pt(30.0, 40.0), pt(-30.0, 40.0)] {
			let bowed = hose(root, joint, pt(0.0, 80.0), 100.0);
			let middle = bowed[4];
			assert!(
				middle.x.signum() == joint.x.signum() && middle.x.abs() > 15.0,
				"{bowed:?}"
			);
			assert!((length(&bowed) - 100.0).abs() < 6.0, "{}", length(&bowed));
			assert!(
				bowed.windows(2).all(|pair| pair[1].y > pair[0].y),
				"no kinks: {bowed:?}"
			);
		}
	}

	#[test]
	fn turned_far_arms_cheer_behind_the_head_but_reach_the_face_in_front() {
		let frame = Frame::authored(&cartoon(), Build::Kid);
		let behind = |pose: Pose| {
			let mut state = FigureState::new(&frame);
			state.facing = STAGED_FACING;
			state.hands = frame.hands(pose, STAGED_FACING);
			Posed::of(&state, &cartoon(), Build::Kid).far_arm_behind
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
	fn held_things_sit_in_the_fist_however_big_the_hand() {
		let fist = |style: &str| {
			StylePack::named(style)
				.unwrap_or_else(|error| unreachable!("{error}"))
				.kit
				.get("hand-fist")
				.cloned()
				.unwrap_or_else(|| unreachable!("every kit has a fist"))
		};
		let (cartoon, anime) = (fist("cartoon"), fist("anime"));
		assert_eq!(
			grip(&cartoon),
			17.0,
			"the cartoon's props stay where they were"
		);
		// The anime fist is nearly three times as long, so its grip is further down it.
		assert!(grip(&anime) > 40.0 && grip(&anime) < anime.bounds().1.y * 0.6);
		// And a toothbrush counts as near the mouth from nearly three times as far away, while
		// the cartoon's distances stay exactly what they were.
		assert_eq!(fist_size(&cartoon), 1.0);
		assert!(fist_size(&anime) > 2.5 && fist_size(&anime) < 3.0);
	}

	/// Every stance, solved on `body`, three-quarters turned.
	/// A slender body with long legs that keep to the floor, the way anime bodies are built.
	fn long_legged() -> Proportions {
		Proportions {
			limbs: Limbs {
				pelvis_y: -580.0,
				hip_spread: 42.0,
				thigh: 280.0,
				shin: 280.0,
				..kid().limbs
			},
			shoulder_y: -884.0,
			hips_turned: (-30.0, 40.0),
			leg_profile: Some([74.0, 66.0, 50.0, 42.0, 50.0, 36.0, 26.0]),
			knee_splay: 0.45,
			floor: true,
			..kid()
		}
	}

	/// Only the legs matter here, so the figure is posed in the cartoon style.
	fn stances(body: &Proportions) -> Vec<(Stance, Solved, Posed)> {
		let style = cartoon();
		let frame = Frame::authored(&style, Build::Kid);

		Stance::ALL
			.iter()
			.map(|&(_, stance)| {
				let mut state = FigureState::new(&frame);
				state.facing = STAGED_FACING;
				state.legs = body.limbs.stance(stance, STAGED_FACING);
				let posed = Posed::of(&state, &style, Build::Kid);
				(stance, Solved::new(&posed, body, 0.0), posed)
			})
			.collect()
	}

	#[test]
	fn without_construction_dials_limbs_are_drawn_as_they_always_were() {
		let body = kid();
		assert_eq!(
			(body.leg_profile, body.arm_profile, body.floor),
			(None, None, false)
		);
		assert!((body.foot_scale - 1.0).abs() < f32::EPSILON);
		assert!((body.knee_splay - 1.0).abs() < f32::EPSILON);
		let puppet = stinky().puppet.unwrap();

		for (stance, solved, _) in stances(&body) {
			for side in [-1.0, 1.0] {
				let (hip, knee, ankle) = solved.leg(side);
				let foot = if side < 0.0 {
					solved.legs.feet.0
				} else {
					solved.legs.feet.1
				};
				let pole = pt(lerp(side, 1.2, solved.t), -0.25).norm();
				let plain = rig::two_bone(hip, foot, body.limbs.thigh, body.limbs.shin, pole);
				assert_eq!((knee, ankle), plain, "{stance:?}");
				assert_eq!(
					leg_shape(&puppet, &solved, (hip, knee, ankle)),
					Shape::taper(
						&[hip, hip.lerp(knee, 0.5), knee, knee.lerp(ankle, 0.5), ankle],
						body.leg_width.0,
						body.leg_width.1
					),
					"{stance:?}"
				);
			}
		}
	}

	#[test]
	fn a_body_that_keeps_to_the_floor_kneels_on_it_instead_of_going_through() {
		let body = long_legged();
		let floor = -body.knee_radius();

		for (stance, solved, _) in stances(&body) {
			for side in [-1.0, 1.0] {
				let (hip, knee, ankle) = solved.leg(side);
				assert!(knee.y <= floor + 0.01, "{stance:?}: knee {knee:?}");
				// Turned, the near foot stands a little down the page (see `Posed::of`).
				assert!(
					ankle.y <= body.limbs.ankle_y + 3.01,
					"{stance:?}: ankle {ankle:?}"
				);
				assert!(
					((knee - hip).len() - body.limbs.thigh).abs() < 0.5,
					"{stance:?}"
				);
				assert!(
					((ankle - knee).len() - body.limbs.shin).abs() < 0.5,
					"{stance:?}"
				);
			}
		}

		// Sinking from a squat through a kneel to a sit moves the near knee smoothly, with no
		// jump where it reaches the floor or leaves it again.
		let base = Legs {
			feet: (pt(-70.0, body.limbs.ankle_y), pt(70.0, body.limbs.ankle_y)),
			crouch: 0.0,
			bend: 0.0,
		};
		let mut last: Option<(Pt, Pt)> = None;
		let mut knelt = false;

		for step in 0..=240 {
			let mut legs = base;
			legs.crouch = lerp(rig::SQUAT, 0.97, step as f32 / 240.0);
			let posed = Posed {
				legs,
				..stances(&body)[0].2.clone()
			};
			let (_, knee, ankle) = Solved::new(&posed, &body, 0.0).leg(-1.0);
			knelt |= (knee.y - floor).abs() < 0.01;

			if let Some((knee_before, ankle_before)) = last {
				assert!((knee - knee_before).len() < 12.0, "step {step}");
				assert!((ankle - ankle_before).len() < 12.0, "step {step}");
			}

			last = Some((knee, ankle));
		}

		assert!(knelt, "on the way down the knee rests on the floor");
		assert!(
			last.is_some_and(|(knee, _)| knee.y < floor - body.limbs.thigh * 0.5),
			"seated, the knee is raised"
		);
	}

	#[test]
	fn a_kneeling_leg_is_one_whose_knee_rests_on_the_floor_with_the_foot_behind() {
		let body = long_legged();
		let solved = stances(&body);
		let kneels = |stance: Stance| {
			let (_, solved, _) = solved
				.iter()
				.find(|(s, ..)| *s == stance)
				.unwrap_or_else(|| unreachable!("every stance is solved"));
			[-1.0, 1.0].map(|side| solved.kneels(solved.leg(side)))
		};

		assert_eq!(kneels(Stance::Stand), [false, false]);
		assert_eq!(kneels(Stance::Tiptoe), [false, false]);
		assert!(kneels(Stance::Kneel).contains(&true), "a kneel kneels");

		let plain = Solved::new(&stances(&kid())[4].2, &kid(), 0.0);
		assert!(
			!plain.kneels(plain.leg(-1.0)),
			"a body off the floor never kneels"
		);
	}

	#[test]
	fn a_skirt_hangs_from_the_waist_and_never_goes_through_the_floor() {
		let body = long_legged();
		let waist = body.limbs.pelvis_y - 40.0;

		for (stance, solved, _) in stances(&body) {
			// Above the waist the top of the skirt rides with the body, untouched.
			let band = pt(30.0, waist - 20.0);
			assert_eq!(solved.skirt(band), band, "{stance:?}");

			// The waist itself joins the hanging cloth without a seam.
			let at_waist = solved.torso(solved.skirt(pt(-60.0, waist + 0.01)));
			assert!(
				(at_waist - solved.torso(pt(-60.0, waist))).len() < 0.5,
				"{stance:?}: {at_waist:?}"
			);

			for x in [-120.0, 0.0, 120.0] {
				let hem = solved.torso(solved.skirt(pt(x, waist + 190.0)));
				assert!(hem.y <= -4.0 + 0.01, "{stance:?}: hem {hem:?}");
			}
		}

		// Bowing, the cloth still falls from the waist: the hem hangs below it, not along the
		// leaning spine.
		let (_, bow, _) = stances(&body)
			.into_iter()
			.find(|(s, ..)| *s == Stance::Bow)
			.unwrap_or_else(|| unreachable!("every stance is solved"));
		let waist_mid = bow.torso(pt(0.0, waist));
		let hem_mid = bow.torso(bow.skirt(pt(0.0, waist + 150.0)));
		let leaning = bow.torso(pt(0.0, waist + 150.0));
		assert!(
			(hem_mid.x - waist_mid.x).abs() < (leaning.x - waist_mid.x).abs(),
			"hangs {hem_mid:?} rather than leaning to {leaning:?}"
		);
	}

	#[test]
	fn front_on_knees_bend_towards_the_viewer() {
		let body = long_legged();
		let mut state = FigureState::new(&Frame::authored(&cartoon(), Build::Kid));
		state.legs = body.limbs.stance(Stance::Crouch, 0.0);
		let posed = Posed::of(&state, &cartoon(), Build::Kid);
		let (hip, knee, ankle) = Solved::new(&posed, &body, 0.0).leg(-1.0);
		let splayed = Solved::new(
			&posed,
			&Proportions {
				knee_splay: 1.0,
				..body
			},
			0.0,
		)
		.leg(-1.0)
		.1;
		let off = |knee: Pt| (knee - splay(hip, knee, ankle, 0.0)).len();
		assert!((off(knee) - off(splayed) * body.knee_splay).abs() < 0.5);
	}

	#[test]
	fn every_mood_has_a_face_and_hanging_open_hands_relax() {
		let frame = Frame::authored(&cartoon(), Build::Kid);

		for (_, mood) in Mood::ALL {
			let mut state = FigureState::new(&frame);
			state.mood = mood;
			let posed = Posed::of(&state, &cartoon(), Build::Kid);
			assert_eq!(
				posed.hand_shapes,
				(Hand::Relaxed, Hand::Relaxed),
				"{mood:?}"
			);
		}
	}
}
