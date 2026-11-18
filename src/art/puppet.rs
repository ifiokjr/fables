//! Authored puppets: characters drawn from hand-made part files, on the same rig as everyone.
//!
//! Every character is drawn this way: from their own part file, or from their drawing style's
//! template until they have one. The older parametric puppet in [`figure`](super::figure) is
//! still available when a cast entry asks for it by name (`puppet = "parametric"`). An authored
//! puppet is drawn the way TV cut-out characters are: every part (head, hair, torso, sleeves,
//! hands, the mouth chart) is a drawing from the character's part file and their style's kit
//! (see [`parts`](super::parts)), hung on the bones of [`rig`](super::rig). The style pack
//! ([`pack`](super::pack)) sets the finish, the face cylinder and the bodies.
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

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;

use super::Bounds;
use super::Brush;
use super::Color;
use super::Path;
use super::Pt;
use super::Solid;
use super::Svg;
use super::dials::Fill;
use super::dials::Joints;
use super::figure::Build;
use super::figure::Extra;
use super::figure::FigureState;
use super::figure::Frame;
use super::figure::HandShape;
use super::figure::Look;
use super::figure::Mood;
use super::figure::Pattern;
use super::figure::Pose;
use super::figure::Teeth;
use super::figure::Viseme;
use super::fx;
use super::lerp;
use super::pack::Eyes;
use super::pack::Face as FaceRules;
use super::pack::Features;
use super::pack::Finish;
use super::pack::KitInks;
use super::pack::Picks;
use super::pack::StylePack;
use super::palette;
use super::parts::Feature;
use super::parts::Finding;
use super::parts::Level;
use super::parts::Parts;
use super::parts::Space;
use super::parts::spec_of;
use super::props;
use super::props::Prop;
use super::pt;
use super::rig;
use super::rig::Gait;
use super::rig::Legs;
use super::rig::Limbs;
use super::rig::Stance;
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
	/// How a slim and a stout character differ from this body, which is plain (see [`Girth`]).
	/// A cast picks one with `build`.
	#[serde(default = "Girth::slim")]
	pub slim: Girth,
	#[serde(default = "Girth::stout")]
	pub stout: Girth,
}

fn one() -> f32 {
	1.0
}

/// How heavily built a character is on their body. `plain` is the body as its style draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Physique {
	Slim,
	#[default]
	Plain,
	Stout,
}

impl Physique {
	pub const ALL: [Physique; 3] = [Physique::Slim, Physique::Plain, Physique::Stout];

	/// The name a cast file's `build` gives it.
	pub fn name(self) -> &'static str {
		match self {
			Physique::Slim => "slim",
			Physique::Plain => "plain",
			Physique::Stout => "stout",
		}
	}

	/// A physique by the name a cast file's `build` gives it.
	pub fn named(name: &str) -> Result<Physique> {
		Self::ALL
			.into_iter()
			.find(|physique| physique.name() == name)
			.with_context(|| format!("unknown build `{name}`; use slim, plain or stout"))
	}
}

/// How a slim or stout body differs from the plain one, as shares of its widths.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Girth {
	/// The torso's width down to the shoulders, and how far apart the shoulders sit.
	pub chest: f32,
	/// The torso's width from its hem down, the legwear's, and how far apart the hips sit.
	pub waist: f32,
	/// The arms' and legs' widths, and their sleeves' and trouser legs'.
	pub limbs: f32,
}

impl Girth {
	const PLAIN: Girth = Girth {
		chest: 1.0,
		waist: 1.0,
		limbs: 1.0,
	};

	fn slim() -> Girth {
		Girth {
			chest: 0.86,
			waist: 0.78,
			limbs: 0.8,
		}
	}

	fn stout() -> Girth {
		Girth {
			chest: 1.16,
			waist: 1.42,
			limbs: 1.26,
		}
	}

	/// How much of its plain width a body part is drawn at `y` on `body`: the chest's down to
	/// the shoulders, the waist's from the hem down, and an eased blend between the two.
	fn across(&self, body: &Proportions, y: f32) -> f32 {
		let along = ((y - body.shoulder_y) / (body.hem_y - body.shoulder_y)).clamp(0.0, 1.0);
		lerp(self.chest, self.waist, along * along * (3.0 - 2.0 * along))
	}
}

impl Proportions {
	/// How a physique differs from this body.
	pub fn girth(&self, physique: Physique) -> Girth {
		match physique {
			Physique::Slim => self.slim,
			Physique::Plain => Girth::PLAIN,
			Physique::Stout => self.stout,
		}
	}

	/// This body built slim, plain or stout: the shoulders and hips spread with the chest and
	/// waist, and every limb widened or narrowed.
	#[must_use]
	pub fn built(&self, physique: Physique) -> Proportions {
		let Girth {
			chest,
			waist,
			limbs,
		} = self.girth(physique);
		let pair = |(a, b): (f32, f32), by: f32| (a * by, b * by);
		let profile = |widths: [f32; 7]| widths.map(|width| width * limbs);

		Proportions {
			limbs: Limbs {
				hip_spread: self.limbs.hip_spread * waist,
				..self.limbs
			},
			shoulder_half: self.shoulder_half * chest,
			shoulders_turned: pair(self.shoulders_turned, chest),
			hips_turned: pair(self.hips_turned, waist),
			body_radius: self.body_radius * f32::midpoint(chest, waist),
			leg_width: pair(self.leg_width, limbs),
			forearm_width: pair(self.forearm_width, limbs),
			leg_profile: self.leg_profile.map(profile),
			arm_profile: self.arm_profile.map(profile),
			..*self
		}
	}

	/// A figure's parts fitted to a physique on this body: what is drawn on the body (the
	/// torso, legwear and the kit's extras worn over them) widened row by row as
	/// [`Girth::across`] says, and what is drawn along a bone (sleeves, trouser legs) by the
	/// limbs' share. The neck, head, hands and feet keep their size.
	pub fn fit(&self, parts: &Parts, physique: Physique) -> Parts {
		let girth = self.girth(physique);

		parts.redrawn(|name, shape| {
			match spec_of(name).map(|spec| (spec.name, spec.space)) {
				Some((_, Space::Body)) => shape.map(|p| pt(p.x * girth.across(self, p.y), p.y)),
				Some((bone, Space::Bone)) if bone != "neck" => {
					shape.map(|p| pt(p.x * girth.limbs, p.y))
				}
				_ => shape.clone(),
			}
		})
	}

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

/// How big a held prop is drawn, before its own held-scale rule. Props are drawn for the
/// cartoon kid, and the stage stands every kid as tall as a cartoon kid, so a prop is drawn the
/// same size on the page in every style's hands — a guest from another world holds the same
/// ball — and a grown-up's bigger hand holds it bigger, as the style draws that hand.
fn prop_scale(puppet: &Authored, build: Build, prop: &Prop) -> f32 {
	let kid = |style: &StylePack| Frame::authored(style, Build::Kid).height();
	let scale = props::held_scale(&puppet.style, build)
		* (kid(&puppet.style) / kid(&StylePack::default_pack()));
	// A held thing shows round the fist that holds it, however big the style draws its
	// hands: a white glove must not swallow a pebble.
	let (fist_lo, fist_hi) = puppet.required("hand-fist", 0.0).bounds();
	let fist = (fist_hi.x - fist_lo.x).max(fist_hi.y - fist_lo.y) * puppet.body(build).hand_scale;
	let (lo, hi) = prop.bounds();
	let longest = (hi.x - lo.x).max(hi.y - lo.y).max(1.0) * prop.held_size() * scale;

	scale * (FIST_PRESENCE * fist / longest).max(1.0)
}

/// How many times the fist holding it a held thing's longest side is drawn at least (see
/// [`prop_scale`]).
const FIST_PRESENCE: f32 = 1.7;

/// How far a fist's centre sits from the wrist, in figure units (see [`grip`]).
fn fist_reach(puppet: &Authored, build: Build) -> f32 {
	grip(&puppet.required("hand-fist", 0.0)) * props::held_scale(&puppet.style, build)
}

/// The gap a carried prop keeps from the eyes and mouth, in the cartoon fist's units: a third
/// of a fist.
const FACE_CLEARANCE: f32 = 12.0;

/// How far a mood or a nod may tilt the head either way, in degrees: the face a carried prop
/// keeps clear of is wherever the head tilts within this, so a prop stays put while a figure's
/// mood changes.
const HEAD_TILT_ROOM: f32 = 10.0;

/// The box round a figure's eyes and mouth, in its own units with its head turned by `t` (0
/// front-on, 1 three-quarters towards +x) and tilted on its neck by up to [`HEAD_TILT_ROOM`]:
/// where a carried prop must never go. Brows, ears and hair may be covered; the eyes and mouth,
/// which carry the acting, never are.
fn face_zone(puppet: &Authored, build: Build, t: f32) -> (Pt, Pt) {
	let style = &puppet.style;
	let body = style.body(build);
	let (features, eyes, face) = (style.features, style.eyes, style.face);
	let (mouth_lo, mouth_hi) = puppet
		.part("mouth-smile", 0.0)
		.map_or((pt(-30.0, -12.0), pt(30.0, 18.0)), |mouth| mouth.bounds());
	// A laugh or a shout opens the mouth wider than its smile.
	let mouth = (
		features.mouth + mouth_lo * features.mouth_scale * 1.35,
		features.mouth + mouth_hi * features.mouth_scale * 1.5,
	);
	let eye = (
		features.eye - eyes.size * 1.2,
		features.eye + eyes.size * 1.2,
	);
	let (mut left, mut right) = (f32::MAX, f32::MIN);

	for x in [eye.0.x, eye.1.x, -eye.0.x, -eye.1.x, mouth.0.x, mouth.1.x] {
		let turned = wrap(pt(x, 0.0), t, face.radius, face.turn).x;
		(left, right) = (left.min(turned), right.max(turned));
	}

	let (top, bottom) = (eye.0.y.min(mouth.0.y), eye.1.y.max(mouth.1.y));
	let (mut lo, mut hi) = (pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN));

	// The head hinges at the neck (see `head_transform`).
	for tilt in [-HEAD_TILT_ROOM, 0.0, HEAD_TILT_ROOM] {
		for corner in [
			pt(left, top),
			pt(right, top),
			pt(left, bottom),
			pt(right, bottom),
		] {
			let at = body.neck + pt(corner.x, corner.y - body.head_above_neck).rotate(tilt);
			(lo, hi) = (
				pt(lo.x.min(at.x), lo.y.min(at.y)),
				pt(hi.x.max(at.x), hi.y.max(at.y)),
			);
		}
	}

	(lo, hi)
}

/// How a figure turned `facing` is drawn: how far round it turns (0 to 1), and whether it is
/// mirrored to face left (see [`Posed::of`]).
fn turned(facing: f32) -> (f32, bool) {
	let turn = (facing / STAGED_FACING).clamp(-1.0, 1.0);
	(turn.abs(), turn < 0.0)
}

/// Where the fist on `side` (-1 near, 1 far) lands, in the figure's own units, when a figure
/// standing at ease turned `facing` sends that hand to `target` (in the same units).
fn fist_for(
	puppet: &Authored,
	build: Build,
	facing: f32,
	carry: props::Carry,
	side: f32,
	target: Pt,
) -> Pt {
	let (_, mirrored) = turned(facing);
	let body = &puppet.body(build);
	let mut state = FigureState::new(&Frame::authored(&puppet.style, build));
	state.facing = facing;
	state.carry = carry;
	let page = if mirrored {
		pt(-target.x, target.y)
	} else {
		target
	};

	if (side > 0.0) == mirrored {
		state.hands.0 = page;
	} else {
		state.hands.1 = page;
	}

	let posed = Posed::of(&state, puppet, build);
	let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
	let (_, elbow, wrist) = solved.arm(&posed, side);

	wrist + (wrist - elbow).norm() * fist_reach(puppet, build)
}

/// The target, in the figure's own units, that puts the fist on `side` at `fist`, as near as
/// the arm reaches.
fn aim(
	puppet: &Authored,
	build: Build,
	facing: f32,
	carry: props::Carry,
	side: f32,
	fist: Pt,
) -> Pt {
	let mut target = fist + pt(0.0, fist_reach(puppet, build));

	for _ in 0..5 {
		let landed = fist_for(puppet, build, facing, carry, side, target);
		target = target + (fist - landed);
	}

	target
}

/// How a prop held up beside the head is placed, in the figure's own units for a figure turned
/// `facing`: the near hand holds it, on the side the face turns away from, the way an animator
/// stages it so the arm never crosses the face. Returns where its fist goes and how far the
/// prop leans out from the head, in degrees.
fn up_place(puppet: &Authored, build: Build, prop: &Prop, facing: f32) -> (Pt, f32) {
	let (t, _) = turned(facing);
	let body = &puppet.body(build);
	let zone = face_zone(puppet, build, t);
	let scale = prop_scale(puppet, build, prop);
	let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
	let arm = body.limbs.upper_arm + body.limbs.forearm;
	let shoulder = pt(
		lerp(-body.shoulder_half, body.shoulders_turned.0, t),
		body.shoulder_y,
	);
	// How far the near fist falls short of a place, with the arm solved as it will be drawn.
	let short = |fist: Pt| {
		let target = aim(puppet, build, facing, props::Carry::Up, -1.0, fist);
		(fist_for(puppet, build, facing, props::Carry::Up, -1.0, target) - fist).len() - arm * 0.03
	};
	let beside = |(lift, lean): (f32, f32)| {
		let (_, hi) = prop.held_box(pt(0.0, 0.0), scale, lean);
		let fist = pt(
			(shoulder.x - arm * 0.45).min(zone.0.x - room - hi.x),
			shoulder.y - arm * lift,
		);
		(fist, lean)
	};
	// Over the head, all of it above the eyes.
	let (_, over_hi) = prop.held_box(pt(0.0, 0.0), scale, -8.0);
	let overhead = (
		pt(shoulder.x - arm * 0.15, zone.0.y - room - over_hi.y),
		-8.0,
	);
	// Raised up and out beside the head with the elbow below the fist; lower or leaning away
	// from the head if a short arm only nearly reaches past the face; over the head if a long
	// one can; and if none is in reach, the nearest, so the arm stretches as far as it goes.
	let places = [0.5, 0.35, 0.2, 0.0]
		.into_iter()
		.flat_map(|lift| [0.0, -12.0, -24.0, -36.0].map(|lean| beside((lift, lean))))
		.chain([overhead]);

	places
		.clone()
		.find(|(fist, _)| short(*fist) < 0.0)
		.or_else(|| places.min_by(|a, b| short(a.0).total_cmp(&short(b.0))))
		.unwrap_or(overhead)
}

/// How a figure turned `facing` carries a prop standing at ease: where its hands go, as
/// [`FigureState::hands`] and [`FigureState::hand_shapes`] have them (the page-right hand
/// holds), given where the pose put them. `None` leaves them as the pose has them.
pub fn carry_hands(
	look: &Look,
	puppet: &Authored,
	prop: &Prop,
	carry: props::Carry,
	facing: f32,
	(hands, shapes): ((Pt, Pt), (HandShape, HandShape)),
) -> Option<((Pt, Pt), (HandShape, HandShape))> {
	use props::Carry;

	let build = look.build;
	let (t, mirrored) = turned(facing);
	let page = |p: Pt| if mirrored { pt(-p.x, p.y) } else { p };

	match carry {
		Carry::Hand | Carry::Head => None,
		Carry::Up => {
			// The near hand: the page-left one, or the page-right one mirrored.
			let (fist, _) = up_place(puppet, build, prop, facing);
			let target = page(aim(puppet, build, facing, carry, -1.0, fist));

			if mirrored {
				Some(((hands.0, target), (shapes.0, HandShape::Fist)))
			} else {
				Some(((target, hands.1), (HandShape::Fist, shapes.1)))
			}
		}
		Carry::Front => {
			// Centred on the body with the hands low enough that the elbows hang and bend
			// outwards (a drawn arm cannot point its forearm at the reader), its top kept
			// under the mouth, held at its grip by the far hand and at the mirrored grip by
			// the near one.
			let body = &puppet.body(build);
			let arm = body.limbs.upper_arm + body.limbs.forearm;
			let zone = face_zone(puppet, build, t);
			let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
			let (lo, hi) = prop.held_box(pt(0.0, 0.0), prop_scale(puppet, build, prop), 0.0);
			let (near, far) = body.shoulders_turned;
			let centre = lerp(0.0, f32::midpoint(near, far), t);
			// The arms nearly straight, so the elbows only just bow out.
			let x = centre - f32::midpoint(lo.x, hi.x);
			let across = (x - body.shoulder_half).abs();
			let drop = ((arm * 0.85).powi(2) - across * across)
				.max((arm * 0.5).powi(2))
				.sqrt();
			// No lower than the hem, so a short-armed kid holds things at the tummy, unless the
			// arms are long enough to keep their elbows in lower down.
			let floor = body.hem_y.max(body.shoulder_y + arm * 0.7);
			let mut fist = pt(x, (body.shoulder_y + drop).min(floor));
			fist.y = fist.y.max(zone.1.y + room - lo.y);
			let far_hand = aim(puppet, build, facing, carry, 1.0, fist);
			let other = pt(centre * 2.0 - fist.x, fist.y);
			let near_hand = aim(puppet, build, facing, carry, -1.0, other);
			let (left, right) = if mirrored {
				(page(far_hand), page(near_hand))
			} else {
				(near_hand, far_hand)
			};
			Some(((left, right), (HandShape::Fist, HandShape::Fist)))
		}
		Carry::String => {
			// The hand hangs at the side, out of the way of the face, with the string's end in
			// its fist.
			let zone = face_zone(puppet, build, t);
			let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
			let edge = if mirrored { -zone.0.x } else { zone.1.x };
			let out = pt(hands.1.x.max(edge + room * 2.0), hands.1.y);
			Some(((hands.0, out), (shapes.0, HandShape::Fist)))
		}
	}
}

/// Where a prop carried on a string floats, from the fist that holds the string's end (in the
/// figure's own units, on `side`, for a figure turned `facing`): above the eyes and out beside
/// the face, bobbing in the air as `time` goes by. Returns its grip and its lean in degrees.
fn float_place(
	puppet: &Authored,
	build: Build,
	prop: &Prop,
	(fist, side): (Pt, f32),
	facing: f32,
	time: f32,
) -> (Pt, f32) {
	let (t, _) = turned(facing);
	let body = &puppet.body(build);
	let zone = face_zone(puppet, build, t);
	let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
	let (lo, hi) = prop.held_box(pt(0.0, 0.0), prop_scale(puppet, build, prop), 0.0);
	let length = (body.limbs.upper_arm + body.limbs.forearm) * 0.9;
	let across = if side > 0.0 {
		(fist.x + length * 0.3).max(zone.1.x + room - lo.x)
	} else {
		(fist.x - length * 0.3).min(zone.0.x - room - hi.x)
	};
	let up = (fist.y - length).min(zone.0.y - room - hi.y);
	let bob = pt(
		(time * 1.1 + side).sin() * length * 0.04,
		(time * 1.7).sin() * length * 0.025,
	);

	(pt(across, up) + bob, (time * 1.1 + side).sin() * 5.0)
}

/// A character's authored puppet: their part file over their style's kit, read and checked once.
#[derive(Clone, Debug)]
pub struct Authored {
	pub file: PathBuf,
	pub parts: Arc<Parts>,
	/// The style the parts are drawn in: its finish, face rules and bodies.
	pub style: Arc<StylePack>,
	/// The style whose `[line]` and `[shade]` dials ink this figure when it is a guest drawn
	/// in its host's finish; `None` inks it with its own style.
	pub ink_style: Option<Arc<StylePack>>,
	/// Whether the parts are still the style's template, drawn without a file of their own.
	/// A character is drawn this way until they are given (or write) a part file.
	pub template: bool,
	/// How heavily built the figure is. Its parts are already fitted to it, and
	/// [`Authored::body`] is its body built to match.
	pub physique: Physique,
	/// The ways of drawing its features a template-drawn figure was drawn with (none for a
	/// figure drawn from its own part file).
	pub picks: Picks,
}

impl Authored {
	/// Reads a part file over its style's kit, refusing parts that can't draw a figure.
	pub fn load(file: &FilePath, style: &Arc<StylePack>) -> Result<Authored> {
		Self::from_parts(file, Parts::load_puppet(file, &style.kit)?, style)
	}

	/// A character drawn straight from their style's template for their body over its kit,
	/// before they have a part file of their own: a grown-up gets the grown-up template, cut for
	/// the taller body, with the hair, brows and nose they `picks`. Their cast colours still
	/// paint it, exactly as if the template had been written to a file and left untouched.
	pub fn from_template(style: &Arc<StylePack>, build: Build, picks: &Picks) -> Result<Authored> {
		let file = style.template_file(build);
		Ok(Authored {
			template: true,
			picks: Picks {
				hair: Some(
					picks
						.hair
						.clone()
						.unwrap_or_else(|| style.own(build, Feature::Hair).to_owned()),
				),
				..picks.clone()
			},
			..Self::from_parts(&file, style.figure(build, picks)?, style)?
		})
	}

	/// This figure built slim, plain or stout on `build`'s body, its parts fitted to the new
	/// widths. A figure is built once, from plain.
	#[must_use]
	pub fn built(mut self, build: Build, physique: Physique) -> Authored {
		debug_assert_eq!(self.physique, Physique::Plain, "a figure is built once");

		if physique != Physique::Plain {
			self.parts = Arc::new(self.style.body(build).fit(&self.parts, physique));
			self.physique = physique;
		}

		self
	}

	/// The body this figure stands on: its style's body for `build`, built to its physique.
	pub fn body(&self, build: Build) -> Proportions {
		self.style.body(build).built(self.physique)
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
			ink_style: None,
			template: false,
			physique: Physique::Plain,
			picks: Picks::default(),
		})
	}

	/// The style this figure is inked with: its host's when it is a guest drawn in the host's
	/// finish, otherwise its own.
	pub fn inker(&self) -> &StylePack {
		self.ink_style.as_deref().unwrap_or(&self.style)
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
	brow: Color,
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
		Inks {
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
			brow: look.hair_color,
			top: look.top,
			stripe: look.trim,
			bottom: look.bottom,
			shoes: look.shoes,
			pupil: look.eyes.mix(Color(0x1E_12_18), style.eyes.iris_dark),
			teeth: match teeth {
				Teeth::Clean | Teeth::Sparkly => style.inks.clean_teeth,
				Teeth::Grotty => palette::GROT,
			},
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
			brow: snap(self.brow),
			top: snap(self.top),
			stripe: snap(self.stripe),
			bottom: snap(self.bottom),
			shoes: snap(self.shoes),
			pupil: snap(self.pupil),
			..self
		}
	}

	/// The inks of a shadow puppet, which is one ink all over (see [`Solid`]): what is cut
	/// out of it is painted `cut`, the colour of the light behind the screen. The whites of
	/// the eyes, the open mouth and the teeth are holes, and every line on the face (brows,
	/// shut eyes, a closed mouth) is a slit.
	fn cut_out(self, cut: Color) -> Inks {
		Inks {
			line: cut,
			brow: cut,
			teeth: cut,
			kit: KitInks {
				eye: cut,
				maw: cut,
				..self.kit
			},
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
	prop: Option<(Arc<Prop>, bool)>,
	/// How the prop is carried (see [`carry_hands`]).
	carry: props::Carry,
	teeth: Teeth,
	/// Turned three-quarters, the far arm is drawn behind the body and head: hanging down, or
	/// raised (a cheer) so the hand shows beside the head instead of across the face.
	far_arm_behind: bool,
}

impl Posed {
	fn of(state: &FigureState, puppet: &Authored, build: Build) -> Posed {
		let style = &puppet.style;
		let body = &puppet.body(build);
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
		// A prop held up beside the head is placed clear of the face already (see
		// [`carry_hands`]), in the hand the plan chose.
		let carrying_up = state.carry == props::Carry::Up;
		let swapped = !carrying_up
			&& t > 0.5
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
		if !carrying_up {
			(near, far) = (
				clear_of_head(near, -1.0, face_x, shoulder_y, hand_size),
				clear_of_head(far, 1.0, face_x, shoulder_y, hand_size),
			);
		}
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
			// Fists carrying something in front hold it, not the hips.
			wrists: if state.carry == props::Carry::Front {
				(0.0, 0.0)
			} else {
				(
					wrist(near_shape, near, -1.0, shoulder_y),
					wrist(far_shape, far, 1.0, shoulder_y),
				)
			},
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
			prop: state
				.prop
				.as_ref()
				.map(|prop| (Arc::clone(prop), !carrying_up && mirrored == swapped)),
			carry: state.carry,
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

/// The centre line of a rubber-hose leg. It bows like an arm while the foot is near its full
/// reach, but a leg folded deep, in a crouch or a sit, bends round the `knee` the rig puts there
/// instead: one smooth curve through it, so a squat reads as a squat, not as bowed legs.
fn hose_leg(hip: Pt, knee: Pt, ankle: Pt, length: f32) -> [Pt; 9] {
	/// How short of its length the hip-to-ankle line gets before the leg starts to fold, and
	/// how much shorter again before it has folded all the way round its knee.
	const FOLD_FROM: f32 = 0.15;
	const FOLD_OVER: f32 = 0.25;
	let bow = hose(hip, knee, ankle, length);
	let fold = ((1.0 - (ankle - hip).len() / length - FOLD_FROM) / FOLD_OVER).clamp(0.0, 1.0);

	if fold <= 0.0 {
		return bow;
	}

	// A quadratic through the knee halfway along.
	let control = knee * 2.0 - hip.lerp(ankle, 0.5);

	std::array::from_fn(|i| {
		let s = i as f32 / 8.0;
		let round = hip.lerp(control, s).lerp(control.lerp(ankle, s), s);
		bow[i].lerp(round, fold)
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
	draw_figure(svg, look, puppet, state).0
}

/// Where a figure's eyes and mouth are drawn, as measured from the drawing.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaceMarks {
	pub eyes: Option<Bounds>,
	pub mouth: Option<Bounds>,
}

/// Measures where an authored figure's eyes and mouth are drawn, and what it holds, in the
/// figure's own units with the soles at the origin: what a carried prop must keep clear of.
pub fn face_and_held(
	look: &Look,
	puppet: &Authored,
	state: &FigureState,
) -> (FaceMarks, Option<Bounds>) {
	let mut svg = Svg::new();
	svg.measuring(true);
	let (_, face) = draw_figure(&mut svg, look, puppet, state);
	let mut svg = Svg::new();
	svg.measuring(true);
	svg.begin_measure();
	draw_held(&mut svg, look, puppet, state);
	let held = svg.end_measure();

	(face, held)
}

/// [`draw`], also returning where the eyes and mouth were drawn when measuring.
fn draw_figure(
	svg: &mut Svg,
	look: &Look,
	puppet: &Authored,
	state: &FigureState,
) -> (Option<Bounds>, FaceMarks) {
	let frame = look.frame();
	let body = &puppet.body(look.build);
	let posed = Posed::of(state, puppet, look.build);
	let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
	// How this figure is inked: its own style, or its host's when it is a guest drawn in the
	// host's finish. Only the line and shade dials and the fill's palette snapping follow the
	// inker; the construction is always its own.
	let inker = puppet.inker();
	let mut inks = Inks::of(look, &puppet.style, state.teeth);

	if inker.shade.fill == Fill::Palette {
		inks = inks.snapped(|color| inker.shade.palette.nearest(color));
	}

	// Shadow theatre: the whole figure is the line's ink, with the face and the pins cut out.
	let shadow =
		(inker.shade.fill == Fill::Silhouette).then(|| Solid::figure(inks.line, svg.backlight()));

	if let Some(solid) = shadow {
		inks = inks.cut_out(solid.cut);
	}

	let t = solved.t;
	// The style's line and shade dials, for this figure only.
	let saved = svg.use_brush(Brush::new(inker.line, &inker.shade, inks.line));
	let lit = svg.set_solid(shadow.or(svg.solid()));

	svg.begin(&figure_transform(look, &posed, state, frame.head_r));

	// A far hand holding something keeps it in front, where it can be seen, and so do both
	// hands carrying something in front of the chest.
	let far_holds = posed.prop.as_ref().is_some_and(|(_, in_far)| *in_far);
	let far_arm_behind = posed.far_arm_behind && !far_holds && posed.carry != props::Carry::Front;

	// What each hand holds, and where the mouth is (a toothbrush near it is brushing). A prop
	// carried in front or worn is drawn on its own, not in one hand.
	let mouth = mouth_on_body(&solved, &posed, inks.face, inks.features.mouth);
	let holding = |prop: &Arc<Prop>| {
		Held {
			prop: Arc::clone(prop),
			mouth,
			time: state.time,
			scale: props::held_scale(&puppet.style, look.build),
			size: prop_scale(puppet, look.build, prop),
			carry: posed.carry,
			build: look.build,
			facing: state.facing,
			at_ease: state.at_ease,
		}
	};
	let held = |side: f32| {
		posed
			.prop
			.as_ref()
			.filter(|(_, in_far)| *in_far == (side > 0.0))
			.filter(|_| !matches!(posed.carry, props::Carry::Front | props::Carry::Head))
			.map(|(prop, _)| holding(prop))
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
	svg.begin(&head_transform(body, &posed));
	let face = draw_head(svg, puppet, look, &inks, &posed, state.time);

	if posed.carry == props::Carry::Head
		&& let Some((prop, _)) = &posed.prop
	{
		draw_hat(svg, puppet, &inks, prop, posed.t);
	}

	svg.end();
	let head = svg.end_measure();
	svg.end();

	// A prop carried in front of the chest goes under both hands.
	if posed.carry == props::Carry::Front
		&& let Some((prop, _)) = &posed.prop
	{
		// The prop's grip is its own right-hand side, which the far hand holds.
		let side = 1.0;
		let (_, elbow, wrist) = solved.arm(&posed, side);
		let along = (wrist - elbow).norm();
		let fist = wrist + along * fist_reach(puppet, look.build);
		draw_prop(svg, puppet, inks.line, &holding(prop), fist, along, side);
	}

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

	// A shadow puppet is pinned together, and the light shows at every pin.
	if let Some(solid) = shadow {
		let pin = inks.finish.detail * 1.5;
		let mut pins = vec![solved.torso(body.neck)];

		for side in [-1.0, 1.0] {
			let (hip, knee, _) = solved.leg(side);
			let (shoulder, elbow, _) = solved.arm(&posed, side);
			pins.extend([hip, knee, shoulder, elbow]);
		}

		for at in pins {
			svg.fill(&Path::circle(at, pin), solid.cut);
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
	svg.set_solid(lit);

	(head, face)
}

/// Draws only what a puppet holds, exactly where its hand draws it, for measuring.
pub fn draw_held(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState) {
	let frame = look.frame();
	let body = &puppet.body(look.build);
	let posed = Posed::of(state, puppet, look.build);
	let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
	let inker = puppet.inker();
	let inks = Inks::of(look, &puppet.style, state.teeth);
	let saved = svg.use_brush(Brush::new(inker.line, &inker.shade, inks.line));

	svg.begin(&figure_transform(look, &posed, state, frame.head_r));

	if let Some((prop, in_far)) = &posed.prop {
		if posed.carry == props::Carry::Head {
			svg.begin(&solved.torso_transform());
			svg.begin(&head_transform(body, &posed));
			draw_hat(svg, puppet, &inks, prop, posed.t);
			svg.end();
			svg.end();
		} else {
			let side = if *in_far || posed.carry == props::Carry::Front {
				1.0
			} else {
				-1.0
			};
			let (_, elbow, wrist) = solved.arm(&posed, side);
			let along = (wrist - elbow).norm();
			let fist = wrist + along * fist_reach(puppet, look.build);
			let held = Held {
				prop: Arc::clone(prop),
				mouth: mouth_on_body(&solved, &posed, inks.face, inks.features.mouth),
				time: state.time,
				scale: props::held_scale(&puppet.style, look.build),
				size: prop_scale(puppet, look.build, prop),
				carry: posed.carry,
				build: look.build,
				facing: state.facing,
				at_ease: state.at_ease,
			};
			draw_prop(svg, puppet, inks.line, &held, fist, along, side);
		}
	}

	svg.end();
	svg.brush = saved;
}

/// The head's transform on the torso: hinged at the neck, tilted, with its own origin above.
fn head_transform(body: &Proportions, posed: &Posed) -> String {
	format!(
		"translate({:.1} {:.1}) rotate({:.2}) translate(0 {:.1})",
		body.neck.x, body.neck.y, posed.head_tilt, -body.head_above_neck
	)
}

/// A prop worn on the head (a hat), in the head's own units: sized to the head, its grip on
/// the crown, and its lowest point kept above the eyes.
fn draw_hat(svg: &mut Svg, puppet: &Authored, inks: &Inks, prop: &Prop, t: f32) {
	let (head_lo, head_hi) = puppet.required("head", t).bounds();
	// Wide enough to sit over the hair, which is often wider than the head.
	let (hair_lo, hair_hi) = puppet.required("hair", t).bounds();
	let width = (head_hi.x.max(hair_hi.x) - head_lo.x.min(hair_lo.x)).max(1.0);
	let (lo, hi) = prop.bounds();
	let size = width * 1.05 / (hi.x - lo.x).max(1.0);
	let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
	let eyes = inks.features.eye.y - inks.eyes.size.y * 1.3 - room;
	let crown = lerp(head_lo.y, eyes, 0.5);
	let low = crown + (hi.y - prop.grip.y) * size;
	let at = pt(
		f32::midpoint(head_lo.x, head_hi.x),
		crown - (low - eyes).max(0.0),
	);

	prop.draw_worn(
		svg,
		at,
		size,
		props::Ink::Brushed {
			finish: &inks.finish,
			ink: inks.line,
		},
	);
}

/// A soft contact shadow under the feet, so the figure stands on the ground.
pub fn shadow(svg: &mut Svg, look: &Look, puppet: &Authored, state: &FigureState, lift: f32) {
	let fade = (1.0 - lift / 120.0).clamp(0.35, 1.0);
	let size =
		puppet.body(look.build).limbs.hip_spread / puppet.style.body(Build::Kid).limbs.hip_spread;
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
	let body = &puppet.body(look.build);
	let posed = Posed::of(state, puppet, look.build);
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
/// one tube that bows as the foot comes closer, and folds round where the knee would be when
/// it bends deep. Otherwise it is one shape bent through the knee,
/// so there's no seam at the joint: drawn with the body's anatomy if it has a leg profile, or
/// tapering evenly from hip to ankle.
fn leg_shape(puppet: &Authored, solved: &Solved, (hip, knee, ankle): (Pt, Pt, Pt)) -> Shape {
	let (start, end) = solved.body.leg_width;

	if puppet.style.motion.limbs == Joints::Hose {
		Shape::taper(
			&hose_leg(hip, knee, ankle, solved.limbs.thigh + solved.limbs.shin),
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

	// Hems and cuffs are drawn in a darker shade of the legwear; creases are fine ink lines.
	let hem_ink = far(inks.bottom, is_far, inks.finish.far).darken(0.25);

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
			svg.brush_line(&cuff.path(), hem_ink, inks.finish.detail);
		}

		draw_creases(
			svg,
			puppet,
			"legwear-shin-crease",
			side,
			ink,
			inks.finish.detail,
		);

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
			svg.brush_line(&hem.path(), hem_ink, inks.finish.detail);
		}

		draw_creases(
			svg,
			puppet,
			"legwear-leg-crease",
			side,
			ink,
			inks.finish.detail,
		);

		svg.end();
	}
}

/// Draws a trouser leg's numbered creases (`name-1`, `name-2`, …) in its bone's frame: as they
/// are drawn, on the page-right leg (`side` 1), and mirrored on the page-left one, so a seam on
/// the inside of the leg stays there.
fn draw_creases(
	svg: &mut Svg,
	puppet: &Authored,
	name: &str,
	side: f32,
	colour: Color,
	width: f32,
) {
	for crease in puppet.copies(name, 0.0) {
		svg.brush_line(&crease.map(|p| pt(p.x * side, p.y)).path(), colour, width);
	}
}

fn draw_torso(svg: &mut Svg, puppet: &Authored, look: &Look, inks: &Inks, solved: &Solved) {
	let t = solved.t;
	let ink = inks.line;
	let proportions = &puppet.body(look.build);
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
#[derive(Clone, Debug)]
struct Held {
	prop: Arc<Prop>,
	/// Where the mouth is: a toothbrush near it is brushing.
	mouth: Pt,
	time: f32,
	/// A grown-up's bigger hand holds things further from the wrist.
	scale: f32,
	/// How big the prop is drawn: it grows with the fist that holds it (see [`prop_scale`]).
	size: f32,
	carry: props::Carry,
	build: Build,
	/// Which way the figure is turned, for where a carry puts the prop.
	facing: f32,
	/// Standing at ease, so a prop in the hand leans out of the face (see [`hand_lean`]).
	at_ease: bool,
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
			draw_prop(svg, puppet, ink, &held, fist, along, side);
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

/// A held prop, drawn at the fist from its own part file (see [`props`]).
fn draw_prop(
	svg: &mut Svg,
	puppet: &Authored,
	ink: Color,
	held: &Held,
	fist: Pt,
	along: Pt,
	side: f32,
) {
	let finish = puppet.style.finish;
	let fist_size = fist_size(&puppet.required("hand-fist", 0.0));
	// Held up, a prop leans away from the head; on a string, it floats above and beside it.
	let (at, angle) = match held.carry {
		props::Carry::Up => {
			(
				fist,
				Some(up_place(puppet, held.build, &held.prop, held.facing).1),
			)
		}
		props::Carry::String => {
			let (at, lean) = float_place(
				puppet,
				held.build,
				&held.prop,
				(fist, side),
				held.facing,
				held.time,
			);
			draw_string(svg, fist, at, ink, finish.detail, held.time);
			(at, Some(lean))
		}
		props::Carry::Hand => {
			// Its own lean tips it out from the hand that holds it, away from the body.
			let own = held.prop.angle * side;
			let lean = if held.at_ease {
				hand_lean(puppet, held.build, &held.prop, (fist, own), held.facing)
			} else {
				own
			};
			(fist, Some(lean))
		}
		props::Carry::Front | props::Carry::Head => (fist, None),
	};

	held.prop.draw_held(
		svg,
		&props::Held {
			at,
			scale: held.size,
			along,
			side,
			mouth: held.mouth,
			time: held.time,
			fist_size,
			angle,
			ink: props::Ink::Brushed {
				finish: &finish,
				ink,
			},
		},
	);
}

/// How far a prop in a hand at ease leans, in degrees, so none of it covers the eyes or mouth
/// (in the figure's own units, held at `fist` by a figure turned `facing`): its `own` lean when
/// that is clear; otherwise tipped further away from the face, a little at a time, until it
/// clears, and level at most.
fn hand_lean(
	puppet: &Authored,
	build: Build,
	prop: &Prop,
	(fist, own): (Pt, f32),
	facing: f32,
) -> f32 {
	let (t, _) = turned(facing);
	let zone = face_zone(puppet, build, t);
	let room = FACE_CLEARANCE * fist_size(&puppet.required("hand-fist", 0.0));
	let scale = prop_scale(puppet, build, prop);
	let clear = |lean: f32| {
		let (lo, hi) = prop.held_box(fist, scale, lean);
		hi.x < zone.0.x - room
			|| lo.x > zone.1.x + room
			|| hi.y < zone.0.y - room
			|| lo.y > zone.1.y + room
	};

	if clear(own) {
		return own;
	}

	let away = if fist.x < f32::midpoint(zone.0.x, zone.1.x) {
		-1.0
	} else {
		1.0
	};
	let level = away * 90.0;

	(1..=9)
		.map(|step| (own + away * 10.0 * step as f32).clamp(-90.0, 90.0))
		.find(|lean| clear(*lean))
		.unwrap_or(level)
}

/// A prop's string, from the fist to where the prop is tied: slack, so it curves, and drifting
/// a little as the prop bobs. It is drawn in the style's line.
fn draw_string(svg: &mut Svg, from: Pt, to: Pt, ink: Color, weight: f32, time: f32) {
	let length = (to - from).len();
	let slack = (to - from).perp().norm() * length * (0.07 + (time * 1.3).sin() * 0.025);
	let path = Path::new()
		.move_to(from)
		.quad_to(from.lerp(to, 0.5) + slack, to);
	svg.brush_line(&path, ink, weight * 0.6);
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

/// Draws the head, and returns where its eyes and mouth were drawn when measuring.
fn draw_head(
	svg: &mut Svg,
	puppet: &Authored,
	look: &Look,
	inks: &Inks,
	posed: &Posed,
	time: f32,
) -> FaceMarks {
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

	svg.begin_measure();

	for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
		draw_eye(svg, inks, posed, face.eyes[index], side, time);
	}

	let eyes = svg.end_measure();
	draw_nose(svg, puppet, inks, t, face.wrinkle);

	if look.has(Extra::Bogey)
		&& let Some(bogey) = puppet.part("bogey", t)
	{
		let drip = bogey.map(warp).shift(pt(9.0 * t, 0.0));
		svg.brushed(&drip.path(), inks.kit.bogey, ink, inks.finish.detail);
	}

	svg.begin_measure();
	draw_mouth(svg, puppet, inks, posed, time);
	let mouth = svg.end_measure();

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
		svg.brushed(&shape.path(), inks.brow, ink, inks.features.brow_weight);
	}

	FaceMarks { eyes, mouth }
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

	// A character drawn with no nose has none to wrinkle either.
	let Some(nose) = puppet.part("nose", t) else {
		return;
	};
	let nose = nose.map(|p| p * scale).shift(at).map(warp).shift(poke);
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

/// The poses a figure's joins are measured in: standing, both arms raised and seated, each
/// front-on and three-quarters turned.
const JOIN_POSES: [(&str, Pose, Stance); 3] = [
	("standing", Pose::Idle, Stance::Stand),
	("cheering", Pose::Cheer, Stance::Stand),
	("sitting", Pose::Idle, Stance::Sit),
];

/// How wide a gap between two parts that should join may be, as a fraction of the figure's
/// height from the soles to the middle of the head: about a line's width.
const JOIN_TOLERANCE: f32 = 0.015;

/// One measured join: the part it is reported against, what should touch, and the widest gap
/// found with the pose it was found in.
struct Gap {
	part: &'static str,
	what: String,
	width: f32,
	pose: String,
}

/// Checks that every built-in prop, carried the way it is by a kid and a grown-up drawn from
/// this style's templates, standing at ease front-on and turned three-quarters either way, keeps
/// clear of their eyes and mouth: measured from the drawing, prop against eyes and mouth.
pub fn props_clear_of_the_face(style: &Arc<StylePack>) -> Vec<Finding> {
	let library = props::Library::built_in();
	let mut findings = Vec::new();

	for build in [Build::Kid, Build::Grownup] {
		let look = match crate::cast::template_look(style, build) {
			Ok(look) => look,
			Err(error) => {
				findings.push(Finding {
					level: Level::Error,
					part: format!("{} template", crate::art::pack::body_name(build)),
					message: format!("cannot be drawn to hold props: {error:#}"),
				});
				continue;
			}
		};
		let Some(puppet) = look.puppet.clone() else {
			continue;
		};
		let frame = look.frame();

		for prop in library.props() {
			for facing in [0.0, STAGED_FACING, -STAGED_FACING] {
				let mut state = FigureState::new(&frame);
				state.facing = facing;
				state.hands = frame.hands(Pose::Idle, facing);
				state.hand_shapes = Pose::Idle.hand_shapes();
				state.prop = Some(Arc::clone(prop));
				super::figure::carry(&look, Pose::Idle, &mut state);
				let (face, held) = face_and_held(&look, &puppet, &state);
				let Some(held) = held else {
					findings.push(Finding {
						level: Level::Error,
						part: format!("props.{}", prop.name()),
						message: "draws nothing in the hand".into(),
					});
					continue;
				};

				for (feature, marks) in [("eyes", face.eyes), ("mouth", face.mouth)] {
					if marks.is_some_and(|marks| overlaps(marks, held)) {
						findings.push(Finding {
							level: Level::Error,
							part: format!("props.{}", prop.name()),
							message: format!(
								"carried by the {} template turned {facing:+.2}, it covers the \
								 {feature}",
								crate::art::pack::body_name(build)
							),
						});
					}
				}
			}
		}
	}

	findings
}

/// Whether two boxes share any area.
fn overlaps(a: Bounds, b: Bounds) -> bool {
	a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Measures whether a figure's drawn parts join up on `build`'s body, in each of
/// [`JOIN_POSES`]: the head (or its neck) must touch the top and the top the legwear; each arm
/// must start inside the top (its sleeve, or a bare or rubber-hose arm at the shoulder); each leg
/// must start inside the top or the legwear (its trouser leg, or the bare leg at the hip); and a
/// sleeve or trouser leg whose lower half carries on down the forearm or shin must reach the
/// elbow or knee. Each join that opens wider than [`JOIN_TOLERANCE`] allows is an error naming
/// the parts, the pose and the gap.
pub fn joins(puppet: &Authored, build: Build) -> Vec<Finding> {
	let style = &puppet.style;
	let body = &puppet.body(build);
	let frame = Frame::of_body(style, body);
	let tolerance = JOIN_TOLERANCE * (body.head_above_neck - body.neck.y);
	let hose = style.motion.limbs == Joints::Hose;
	let mut gaps: Vec<Gap> = Vec::new();
	let mut measure = |part: &'static str, what: String, width: f32, pose: &str| {
		match gaps
			.iter_mut()
			.find(|gap| gap.part == part && gap.what == what)
		{
			Some(gap) if width > gap.width => {
				gap.width = width;
				pose.clone_into(&mut gap.pose);
			}
			Some(_) => {}
			None => {
				gaps.push(Gap {
					part,
					what,
					width,
					pose: pose.to_owned(),
				});
			}
		}
	};

	for (label, pose, stance) in JOIN_POSES {
		for (view, facing) in [("front-on", 0.0), ("three-quarters", STAGED_FACING)] {
			let pose_name = format!("{label} {view}");
			let mut state = FigureState::new(&frame);
			state.hands = frame.hands(pose, facing);
			state.hand_shapes = pose.hand_shapes();
			state.legs = frame.limbs().stance(stance, facing);
			state.facing = facing;
			let posed = Posed::of(&state, puppet, build);
			let solved = Solved::new(&posed, body, hang(puppet, body, posed.t));
			let t = posed.t;
			let on_body = |shape: Shape| shape.map(|p| solved.torso(p));
			let on_bone = |shape: Shape, from: Pt, to: Pt| {
				let d = to - from;
				let angle = (-d.x).atan2(d.y).to_degrees();
				shape.map(|p| from + p.rotate(angle))
			};

			let torso = on_body(puppet.required("torso", t));
			let legwear = puppet.required("legwear", t);
			let legwear = on_body(if body.floor && puppet.part("legwear-leg", t).is_none() {
				legwear.map(|p| solved.skirt(p))
			} else {
				legwear
			});
			let neck = on_body(match puppet.part("neck", t) {
				Some(neck) => neck.shift(body.neck),
				None => capsule(30.0, 30.0, 30.0).shift(pt(lerp(0.0, 6.0, t), body.neck.y - 24.0)),
			});
			let head =
				on_body(puppet.required("head", t).map(|p| {
					body.neck + (p + pt(0.0, -body.head_above_neck)).rotate(posed.head_tilt)
				}));
			let necked = head.gap(&neck).max(neck.gap(&torso));
			measure(
				"head",
				"the head and its neck are clear of the torso".to_owned(),
				head.gap(&torso).min(necked),
				&pose_name,
			);
			measure(
				"legwear",
				"the legwear is clear of the torso".to_owned(),
				torso.gap(&legwear),
				&pose_name,
			);

			for (side, name) in [(-1.0, "near"), (1.0, "far")] {
				let (shoulder, elbow, _) = solved.arm(&posed, side);
				let root = if hose {
					let radius = body.forearm_width.0 / 2.0;
					Shape::ellipse(shoulder, radius, radius)
				} else {
					on_bone(puppet.required("sleeve", t), shoulder, elbow)
				};
				measure(
					if hose { "arm" } else { "sleeve" },
					format!("the {name} arm starts clear of the torso"),
					root.gap(&torso),
					&pose_name,
				);

				if !hose && puppet.part("forearm-sleeve", t).is_some() {
					measure(
						"sleeve",
						format!(
							"the {name} sleeve stops short of the elbow, where the forearm sleeve \
							 starts"
						),
						root.distance(elbow),
						&pose_name,
					);
				}

				let (hip, knee, _) = solved.leg(side);
				let thigh = puppet.part("legwear-leg", t);
				let root = if let Some(cover) = &thigh {
					on_bone(cover.clone(), hip, knee)
				} else {
					let width = body
						.leg_profile
						.filter(|_| !hose)
						.map_or(body.leg_width.0, |widths| widths[0]);
					Shape::ellipse(hip, width / 2.0, width / 2.0)
				};
				measure(
					if thigh.is_some() {
						"legwear-leg"
					} else {
						"leg"
					},
					format!("the {name} leg starts clear of the torso and the legwear"),
					root.gap(&torso).min(root.gap(&legwear)),
					&pose_name,
				);

				if thigh.is_some() && puppet.part("legwear-shin", t).is_some() {
					measure(
						"legwear-leg",
						format!(
							"the {name} trouser leg stops short of the knee, where the trouser's \
							 shin starts"
						),
						root.distance(knee),
						&pose_name,
					);
				}
			}
		}
	}

	let body_name = super::pack::body_name(build);

	gaps.into_iter()
		.filter(|gap| gap.width > tolerance)
		.map(|gap| {
			Finding {
				level: Level::Error,
				part: gap.part.to_owned(),
				message: format!(
					"on the {body_name} body, {} by {:.0} units when {} (a join may open by {:.0} \
					 at most)",
					gap.what, gap.width, gap.pose, tolerance
				),
			}
		})
		.collect()
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

	/// The cartoon template kid, for posing.
	fn template() -> Authored {
		Authored::from_template(&cartoon(), Build::Kid, &Picks::default()).unwrap()
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
	fn no_carried_prop_covers_the_eyes_or_mouth_in_any_style() {
		let mut problems = Vec::new();

		for name in StylePack::names() {
			let style = StylePack::named(name).unwrap();
			problems.extend(
				props_clear_of_the_face(&style)
					.into_iter()
					.map(|finding| format!("{name}: {finding}")),
			);
		}

		assert!(problems.is_empty(), "{problems:#?}");
	}

	#[test]
	fn the_face_check_catches_a_prop_held_over_the_face() {
		// Held in the fist up at the mouth (brushing teeth), a prop covers it: the check
		// must see that, or its silence above means nothing.
		let style = cartoon();
		let look = crate::cast::template_look(&style, Build::Kid).unwrap();
		let puppet = look.puppet.clone().unwrap();
		let mut state = FigureState::new(&look.frame());
		state.hands = look.frame().hands(Pose::Brush, 0.0);
		state.hand_shapes = Pose::Brush.hand_shapes();
		state.prop = Some(props::Library::built_in().find("apple").unwrap());
		let (face, held) = face_and_held(&look, &puppet, &state);
		let (eyes, mouth, held) = (face.eyes.unwrap(), face.mouth.unwrap(), held.unwrap());
		assert!(
			overlaps(eyes, held) || overlaps(mouth, held),
			"eyes {eyes:?} mouth {mouth:?} apple {held:?}"
		);
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
			let posed = Posed::of(&state, &template(), Build::Kid);
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
		let posed = Posed::of(&state, &template(), Build::Kid);
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
			let nose = Posed::of(&state, &template(), Build::Kid).hands.1;
			let face_x = facing / STAGED_FACING * cartoon().face.slide * STAGED_FACING;
			assert!((nose.x - face_x).abs() < 30.0, "facing {facing}: {nose:?}");

			state.hands = frame.hands(Pose::Wave, facing);
			let posed = Posed::of(&state, &template(), Build::Kid);
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
		state.prop = Some(props::Library::built_in().find("jam").unwrap());
		let posed = Posed::of(&state, &template(), Build::Kid);
		// The jam is in the page-right hand, which is the far hand when facing right.
		assert_eq!(
			posed.prop.as_ref().map(|(prop, _)| prop.name.as_str()),
			Some("jam")
		);
		assert!(posed.prop.as_ref().is_some_and(|(_, in_far)| *in_far));
		assert!(drawn(&look, &state).contains(&format!("fill=\"{}\"", cartoon().inks.jam)));
	}

	#[test]
	fn brushing_keeps_the_toothbrush_hand_at_the_mouth_when_turned() {
		let frame = Frame::authored(&cartoon(), Build::Kid);
		let mut state = FigureState::new(&frame);
		state.facing = STAGED_FACING;
		state.hands = frame.hands(Pose::Brush, STAGED_FACING);
		state.prop = Some(props::Library::built_in().find("toothbrush").unwrap());
		let posed = Posed::of(&state, &template(), Build::Kid);
		assert_eq!(
			posed.prop.as_ref().map(|(prop, _)| prop.name.as_str()),
			Some("toothbrush"),
			"no wave swap"
		);
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
	fn a_kid_drawn_on_a_grown_up_body_comes_apart_and_the_grown_up_template_holds() {
		// Captain Crumpet: a rubber-hose grown-up started from the style's kid template before it
		// had a grown-up one. On the grown-up body the head floats over a kid's torso with no neck
		// between, and the hose arms start in mid-air above it.
		let style = StylePack::named("rubber-hose").unwrap();
		let parts = Parts::parse(include_str!("../../tests/fixtures/crumpet-grown-up.svg"))
			.unwrap()
			.over(style.kit.clone());
		let crumpet = Authored::from_parts(FilePath::new("crumpet.svg"), parts, &style).unwrap();
		let errors: Vec<String> = joins(&crumpet, Build::Grownup)
			.iter()
			.map(ToString::to_string)
			.collect();
		let gap = |part: &str, what: &str| {
			errors.iter().any(|error| {
				error.starts_with(&format!("error: {part}: on the grown-up body, {what} by "))
					&& error.contains(" units when ")
			})
		};
		assert!(
			gap("head", "the head and its neck are clear of the torso"),
			"{errors:#?}"
		);
		assert!(
			gap("arm", "the near arm starts clear of the torso")
				&& gap("arm", "the far arm starts clear of the torso"),
			"{errors:#?}"
		);

		// The same drawing joins up on the kid's body it was drawn for, and the style's grown-up
		// template joins up on the grown-up's.
		assert_eq!(joins(&crumpet, Build::Kid), Vec::<Finding>::new());
		let template = Authored::from_parts(
			FilePath::new("template"),
			style.template_figure(Build::Grownup),
			&style,
		)
		.unwrap();
		let errors = joins(&template, Build::Grownup);
		assert!(errors.is_empty(), "{errors:#?}");
	}

	#[test]
	fn every_puppet_in_the_repository_joins_up_on_its_own_body() {
		let worlds = FilePath::new(env!("CARGO_MANIFEST_DIR")).join("worlds");

		for entry in std::fs::read_dir(worlds).unwrap() {
			let world = crate::world::World::load(&entry.unwrap().path()).unwrap();

			for member in world.cast().unwrap().members.values() {
				if let crate::cast::Kind::Figure(look) = &member.kind
					&& let Some(puppet) = &look.puppet
				{
					let errors = joins(puppet, look.build);
					assert!(errors.is_empty(), "{}: {errors:#?}", member.id);
				}
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
	fn a_hose_leg_bows_near_its_reach_and_folds_round_its_knee_in_a_squat() {
		let hip = pt(0.0, 0.0);

		// A foot near full reach bows the leg exactly as an arm would.
		let standing = (pt(10.0, 30.0), pt(0.0, 92.0));
		assert_eq!(
			hose_leg(hip, standing.0, standing.1, 100.0),
			hose(hip, standing.0, standing.1, 100.0)
		);

		// Squatting, the foot is close under the hip and the knee juts out high: the leg goes
		// round the knee rather than bowing evenly between hip and foot.
		let (knee, ankle) = (pt(45.0, 20.0), pt(5.0, 50.0));
		let folded = hose_leg(hip, knee, ankle, 100.0);
		assert!((folded[4] - knee).len() < 1.0, "{folded:?}");
		let bowed = hose(hip, knee, ankle, 100.0);
		assert!(
			(bowed[4] - knee).len() > 10.0,
			"a plain bow misses the knee: {bowed:?}"
		);
	}

	#[test]
	fn turned_far_arms_cheer_behind_the_head_but_reach_the_face_in_front() {
		let frame = Frame::authored(&cartoon(), Build::Kid);
		let behind = |pose: Pose| {
			let mut state = FigureState::new(&frame);
			state.facing = STAGED_FACING;
			state.hands = frame.hands(pose, STAGED_FACING);
			Posed::of(&state, &template(), Build::Kid).far_arm_behind
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
				let posed = Posed::of(&state, &template(), Build::Kid);
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
		let posed = Posed::of(&state, &template(), Build::Kid);
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
			let posed = Posed::of(&state, &template(), Build::Kid);
			assert_eq!(
				posed.hand_shapes,
				(Hand::Relaxed, Hand::Relaxed),
				"{mood:?}"
			);
		}
	}

	/// Every `fill="…"` and `stroke="…"` colour a drawing uses.
	fn colours(svg: &str) -> std::collections::BTreeSet<String> {
		svg.split("fill=\"")
			.chain(svg.split("stroke=\""))
			.skip(1)
			.filter_map(|rest| rest.split('"').next())
			.filter(|colour| colour.starts_with('#'))
			.map(str::to_owned)
			.collect()
	}

	#[test]
	fn a_shadow_puppet_is_one_ink_with_the_light_cut_out() {
		let style = StylePack::named("silhouette").unwrap();
		let look = crate::cast::template_look(&style, Build::Kid).unwrap();
		let mut state = FigureState::new(&look.frame());
		state.mood = Mood::Laugh;
		state.facing = STAGED_FACING;
		let drawn = drawn(&look, &state);
		let ink = Color(0x2B_1D_2E).to_string();
		let light = Svg::new().backlight().to_string();

		assert_eq!(
			colours(&drawn),
			[ink, light.clone()].into_iter().collect(),
			"whatever the cast's colours, a shadow puppet is ink and light"
		);

		// A pin at the neck, and at both shoulders, elbows, hips and knees: nine holes, each a
		// circle of four curves, beside the face's own.
		let pin = format!("fill=\"{light}\"");
		assert!(drawn.matches(&pin).count() >= 9, "{drawn}");

		// Under a scene light the holes take the light behind the screen, not the paper's.
		let dusk = crate::art::light::Light::of(crate::art::light::Preset::Dusk);
		let mut svg = Svg::new();
		svg.set_light(dusk);
		figure::draw(&mut svg, &look, &state);
		let lit = svg.backlight().to_string();
		assert_ne!(lit, light);
		assert_eq!(
			colours(&svg.finish((-300.0, -600.0, 600.0, 700.0), 256, 256)),
			[Color(0x2B_1D_2E).to_string(), lit].into_iter().collect()
		);
	}

	#[test]
	fn paper_theatre_parts_are_card_with_a_white_edge_and_a_shadow() {
		let style = StylePack::named("paper-theatre").unwrap();
		let look = crate::cast::template_look(&style, Build::Kid).unwrap();
		let card = drawn(&look, &FigureState::new(&look.frame()));
		assert!(
			card.contains(&format!("stroke=\"{}\"", palette::WHITE)),
			"a white cut edge: {card}"
		);
		assert!(card.contains("opacity=\"0.3\""), "a hard shadow: {card}");
		assert!(
			!card.contains(&format!("stroke=\"{}\"", Color(0x2B_1D_2E))),
			"and no ink line anywhere"
		);

		// The line-free styles draw no outline at all: nothing is stroked in ink.
		let soft = StylePack::named("soft-flat").unwrap();
		let look = crate::cast::template_look(&soft, Build::Kid).unwrap();
		let soft = drawn(&look, &FigureState::new(&look.frame()));
		assert!(!soft.contains(&format!("stroke=\"{}\"", Color(0x2B_1D_2E))));
		assert!(soft.contains("mix-blend-mode:multiply"), "one soft shade");
	}
}
