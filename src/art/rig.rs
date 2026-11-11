//! The animation rig: a skeleton under every Kerfuffle puppet.
//!
//! ```text
//!              head
//!               |
//!              neck
//!               |
//!  wrist-elbow-shoulder==chest==shoulder-elbow-wrist
//!                       |
//!                     spine (bends)
//!                       |
//!              hip===pelvis===hip
//!               |               |
//!              knee            knee
//!               |               |
//!             ankle           ankle
//! ```
//!
//! Animators pose a rig by saying where hands and feet go; elbows and knees are solved with
//! two-bone inverse kinematics (IK), the same technique Spine, Moho and Harmony rigs use. The
//! pelvis can drop (crouch, sit, kneel), the spine can bend (bow, run), and gait cycles plant
//! and lift the feet for walks and runs. Everything is computed from numbers that can be
//! blended, so any two poses animate smoothly into each other.
//!
//! The stances place the feet in hip-widths, which suits short cartoon legs. A body that keeps
//! to the floor fits the floor stances (crouch, kneel, sit) to its own legs instead (see
//! [`Limbs::fit`]).

use std::f32::consts::PI;

use super::Pt;
use super::lerp;
use super::pt;

/// How far the pelvis drops in a crouch, as a fraction of leg length.
pub const SQUAT: f32 = 0.5;
/// How far the pelvis drops in a kneel.
pub const KNEELING: f32 = 0.7;
/// How far the pelvis drops in a sit.
pub const SEATED: f32 = 0.92;
/// A pose that drops the pelvis no further than this is standing: a bow, a dip in a stride.
pub const STANDING: f32 = 0.15;

/// A body stance: what the legs and spine are doing (arms are separate, see `Pose`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
	Stand,
	Crouch,
	Sit,
	Kneel,
	/// Knees tucked up mid-jump.
	Jump,
	Bow,
	Tiptoe,
}

impl Stance {
	pub const ALL: [(&'static str, Stance); 7] = [
		("stand", Stance::Stand),
		("crouch", Stance::Crouch),
		("sit", Stance::Sit),
		("kneel", Stance::Kneel),
		("jump", Stance::Jump),
		("bow", Stance::Bow),
		("tiptoe", Stance::Tiptoe),
	];

	pub fn parse(name: &str) -> Option<Stance> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, stance)| *stance)
	}
}

/// A repeating locomotion cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
	Walk,
	Run,
}

impl Gait {
	/// Cycle speed in radians per second.
	pub fn rate(self) -> f32 {
		match self {
			Gait::Walk => 9.0,
			Gait::Run => 15.0,
		}
	}
}

/// The numbers that pose the legs and spine. All are blendable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legs {
	/// Foot (ankle) targets, `(left, right)`, in figure-local units.
	pub feet: (Pt, Pt),
	/// How far the pelvis drops, as a fraction of leg length (negative rises onto tiptoe).
	pub crouch: f32,
	/// Spine bend at the pelvis in degrees (positive leans towards the right of the page).
	pub bend: f32,
}

/// Limb measurements for a build.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Limbs {
	/// Pelvis height at rest (negative is up, feet are at 0).
	pub pelvis_y: f32,
	pub ankle_y: f32,
	/// Half the distance between the hip joints.
	pub hip_spread: f32,
	pub thigh: f32,
	pub shin: f32,
	pub upper_arm: f32,
	pub forearm: f32,
}

impl Limbs {
	pub fn leg_length(&self) -> f32 {
		self.ankle_y - self.pelvis_y
	}

	/// The legs for a stance; `facing` (-1..1) decides which way a bow goes.
	pub fn stance(&self, stance: Stance, facing: f32) -> Legs {
		let leg = self.leg_length();
		let ahead = if facing < -0.05 { -1.0 } else { 1.0 };
		let foot = |spread: f32, lift: f32| {
			(
				pt(-self.hip_spread * spread, self.ankle_y - lift * leg),
				pt(self.hip_spread * spread, self.ankle_y - lift * leg),
			)
		};

		match stance {
			Stance::Stand => {
				Legs {
					feet: foot(1.15, 0.0),
					crouch: 0.0,
					bend: 0.0,
				}
			}
			Stance::Crouch => {
				Legs {
					feet: foot(1.9, 0.0),
					crouch: SQUAT,
					bend: 10.0 * ahead,
				}
			}
			Stance::Sit => {
				Legs {
					feet: foot(2.6, 0.15),
					crouch: SEATED,
					bend: -4.0 * ahead,
				}
			}
			Stance::Kneel => {
				Legs {
					feet: foot(2.2, 0.05),
					crouch: KNEELING,
					bend: 4.0 * ahead,
				}
			}
			Stance::Jump => {
				Legs {
					feet: foot(1.3, 0.55),
					crouch: -0.05,
					bend: 6.0 * ahead,
				}
			}
			Stance::Bow => {
				Legs {
					feet: foot(1.15, 0.0),
					crouch: 0.05,
					bend: 32.0 * ahead,
				}
			}
			Stance::Tiptoe => {
				Legs {
					feet: foot(0.95, 0.12),
					crouch: -0.14,
					bend: 0.0,
				}
			}
		}
	}

	/// A point in a walk or run cycle. Returns the legs plus how far each hand swings
	/// (`(left, right)` offsets added to the arm pose) so arms counter-swing the legs.
	pub fn cycle(&self, gait: Gait, phase: f32, facing: f32, base: Legs) -> (Legs, (Pt, Pt)) {
		let leg = self.leg_length();
		let ahead = if facing < -0.05 { -1.0 } else { 1.0 };
		let (stride, lift, bob, lean, swing) = match gait {
			Gait::Walk => (0.45, 0.3, 0.07, 4.0, 0.22),
			Gait::Run => (0.7, 0.55, 0.13, 12.0, 0.4),
		};
		let mut feet = [base.feet.0, base.feet.1];
		let mut hands = [pt(0.0, 0.0), pt(0.0, 0.0)];

		for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
			let psi = phase + if side > 0.0 { PI } else { 0.0 };
			// Forward and back along the walk, lifting only on the forward (swing) half.
			feet[index].x += psi.sin() * stride * leg * ahead;
			feet[index].y -= psi.cos().max(0.0) * lift * leg;
			hands[index] = pt(
				-psi.sin() * swing * leg * ahead,
				-psi.sin().abs() * swing * leg * 0.3,
			);
		}

		let legs = Legs {
			feet: (feet[0], feet[1]),
			// The body is lowest as each foot lands and highest as the legs pass.
			crouch: base.crouch + bob * (1.0 - (phase * 2.0).cos()) * 0.5 - bob * 0.5,
			bend: base.bend + lean * ahead,
		};

		(legs, (hands[0], hands[1]))
	}

	/// Where a hip joint is, from its `x` at rest, with the pelvis dropped by `drop` and the spine
	/// bent by `bend` degrees round the pelvis.
	pub fn hip(&self, x: f32, drop: f32, bend: f32) -> Pt {
		pt(x, 0.0).rotate(bend) + pt(0.0, self.pelvis_y) + pt(0.0, drop)
	}

	/// Fits a floor stance to these legs, in the figure's own frame (see [`Floor`]).
	///
	/// The stances place the feet a number of hip-widths apart, which suits short cartoon legs;
	/// longer legs fold up behind the body or reach through the floor. So a body that keeps to
	/// the floor reads how far a pose drops the pelvis to tell what it is: standing (left as it
	/// is), squatting, kneeling or seated, or on its way between two of them, and places the hips
	/// and feet by its own bones. Between two floor stances the pose passes through the one whose
	/// drop lies between them, the way a person gets down to the floor: a sit goes down through a
	/// squat and a kneel.
	pub fn fit(&self, legs: Legs, floor: &Floor) -> Legs {
		let depth = legs.crouch;

		if depth <= STANDING {
			return legs;
		}

		let bend = legs.bend;
		// Each floor pose sets its own spine bend, from the stance's.
		let (from, to, along) = if depth < SQUAT {
			(
				legs,
				self.squat(bend, floor),
				(depth - STANDING) / (SQUAT - STANDING),
			)
		} else if depth < KNEELING {
			(
				self.squat(bend, floor),
				self.kneel(bend, floor),
				(depth - SQUAT) / (KNEELING - SQUAT),
			)
		} else {
			(
				self.kneel(bend, floor),
				self.sit(bend, floor),
				((depth - KNEELING) / (SEATED - KNEELING)).min(1.0),
			)
		};

		Legs {
			feet: (
				from.feet.0.lerp(to.feet.0, along),
				from.feet.1.lerp(to.feet.1, along),
			),
			crouch: lerp(from.crouch, to.crouch, along),
			bend: lerp(from.bend, to.bend, along),
		}
	}

	/// Legs with the hip joints `height` above the floor and each foot where `foot` puts it,
	/// given its side (-1 near, 1 far) and its hip.
	fn placed(&self, bend: f32, floor: &Floor, height: f32, foot: impl Fn(f32, Pt) -> Pt) -> Legs {
		let drop = -height - self.pelvis_y;
		let near = self.hip(floor.hips.0, drop, bend);
		let far = self.hip(floor.hips.1, drop, bend);

		Legs {
			feet: (foot(-1.0, near), foot(1.0, far)),
			crouch: drop / self.leg_length(),
			bend,
		}
	}

	/// How long the legs are for the body, from 0 (no longer than the torso: they stretch out
	/// when seated and squat deep) to 1 (well over half as long again: knees raised, thighs level).
	fn long(&self, floor: &Floor) -> f32 {
		(((self.thigh + self.shin) / floor.torso - 0.95) / 0.45).clamp(0.0, 1.0)
	}

	/// A squat with the heels down: the shins lean forward over the feet and the thighs reach
	/// forward from the hips to the knees, so the hips stay above the heels. Long legs drop their
	/// thighs towards the knees under a forward lean; short legs, under a body that may hang
	/// over their thighs, keep the thighs level and the back upright so the knees come out in
	/// front of it. Turned, the knees come forward, the far foot a step ahead of the near one;
	/// front-on the feet stand apart (wider on short legs) and the knees open over them.
	fn squat(&self, bend: f32, floor: &Floor) -> Legs {
		let long = self.long(floor);
		let (thigh, shin) = (
			lerp(86.0, 72.0, long).to_radians(),
			lerp(36.0, 32.0, long).to_radians(),
		);
		let height = self.thigh * thigh.cos() + self.shin * shin.cos() - self.ankle_y;
		let forward = self.thigh * thigh.sin() - self.shin * shin.sin();
		let apart = self.thigh * lerp(0.8, 0.3, long);
		let bend = bend * lerp(0.3, 1.0, long);

		self.placed(bend, floor, height, |side, hip| {
			let turned = forward + side * self.thigh * 0.12;
			pt(
				hip.x + lerp(side * apart, turned, floor.turn),
				self.ankle_y + floor.ground(side),
			)
		})
	}

	/// On one knee: the near thigh drops almost straight down to a knee resting on the floor,
	/// its shin lying back along the floor behind it, and the far foot is planted in front with
	/// its knee up. Front-on the far foot stands out to the side.
	fn kneel(&self, bend: f32, floor: &Floor) -> Legs {
		let lean = 10.0_f32.to_radians();
		let height = floor.knee + self.thigh * lean.cos();

		self.placed(bend, floor, height, |side, hip| {
			let ground = self.ankle_y + floor.ground(side);

			if side < 0.0 {
				// From the hip as the bent spine leaves it, so the thigh just reaches the floor.
				let drop = (-floor.knee - hip.y).min(self.thigh);
				let knee = pt(
					hip.x + (self.thigh * self.thigh - drop * drop).sqrt(),
					-floor.knee,
				);
				let rise = ground - knee.y;
				let back = (self.shin * self.shin - rise * rise).max(0.0).sqrt();
				pt(lerp(hip.x, knee.x - back, floor.turn), ground)
			} else {
				pt(
					hip.x + lerp(self.thigh * 0.3, self.thigh * 0.95, floor.turn),
					ground,
				)
			}
		})
	}

	/// Seated on the floor: the hips rest on it and both legs reach forward, the near knee more
	/// bent than the far one so the two never sit one on top of the other. Legs longer than the
	/// torso sit with their knees raised and their feet flat; shorter ones stretch out in front.
	/// Front-on, raised knees come towards the viewer and stretched legs open into a V.
	fn sit(&self, bend: f32, floor: &Floor) -> Legs {
		let reach = self.thigh + self.shin;
		let raised = self.long(floor);

		self.placed(bend, floor, floor.seat, |side, hip| {
			let ground = self.ankle_y + floor.ground(side);
			let stretch = reach
				* if side < 0.0 {
					lerp(0.9, 0.48, raised)
				} else {
					lerp(0.97, 0.64, raised)
				};
			let rise = ground - hip.y;
			let forward = (stretch * stretch - rise * rise).max(0.0).sqrt();
			let open = side * lerp(0.8 * reach, 0.35 * self.thigh, raised);
			pt(hip.x + lerp(open, forward, floor.turn), ground)
		})
	}
}

/// What fitting the floor stances to a body needs besides its bones (see [`Limbs::fit`]). It is
/// in the figure's own frame: the near leg is on the left of the page, and a turned body faces
/// right.
#[derive(Clone, Copy, Debug)]
pub struct Floor {
	/// How far the body is turned: 0 front-on, 1 three-quarters.
	pub turn: f32,
	/// The near and far hip joints' x, at this turn.
	pub hips: (f32, f32),
	/// How far below the soles' line each foot's ground is, near and far: turned, the far foot
	/// stands further up the page.
	pub ground: (f32, f32),
	/// How high the hip joints sit off the floor when seated.
	pub seat: f32,
	/// How far a knee resting on the floor sits off it: half the knee's width.
	pub knee: f32,
	/// The torso's length, shoulders to hips. Legs longer than it sit with their knees raised;
	/// shorter ones stretch out in front.
	pub torso: f32,
}

impl Floor {
	fn ground(&self, side: f32) -> f32 {
		if side < 0.0 {
			self.ground.0
		} else {
			self.ground.1
		}
	}
}

/// Which way a knee bends: out to the side front-on, forwards and a little up when turned.
/// `side` is -1 for the near leg and 1 for the far one.
pub fn knee_pole(side: f32, turn: f32) -> Pt {
	pt(lerp(side, 1.2, turn), -0.25).norm()
}

/// Which way a knee bends on a leg reaching `reach` from its hip, the way a real knee flexes:
/// out to the side front-on and, turned, so the shin folds back under the thigh whichever way
/// the leg points. A standing knee comes forward, a kneeling one goes down to the floor, and
/// one on a leg stretched out along the floor rises (where [`knee_pole`], aimed forwards and a
/// little up, can't tell up from down).
pub fn flex_pole(side: f32, turn: f32, reach: Pt) -> Pt {
	pt(side, -0.25)
		.norm()
		.lerp(reach.norm().perp() * -1.0, turn)
}

/// Solves a two-bone chain (upper `a`, lower `b`) from `root` towards `target`. Returns the
/// middle joint and the end point. The joint bends towards `pole`; an out-of-reach target
/// straightens the chain and points it at the target.
pub fn two_bone(root: Pt, target: Pt, a: f32, b: f32, pole: Pt) -> (Pt, Pt) {
	let offset = target - root;
	let reach = offset.len().clamp((a - b).abs() + 0.01, a + b - 0.01);
	let direction = if offset.len() < 1e-4 {
		pt(0.0, 1.0)
	} else {
		offset.norm()
	};
	let end = root + direction * reach;
	let cos_root = ((a * a + reach * reach - b * b) / (2.0 * a * reach)).clamp(-1.0, 1.0);
	let sin_root = (1.0 - cos_root * cos_root).max(0.0).sqrt();
	let mut side = direction.perp();

	if side.x * pole.x + side.y * pole.y < 0.0 {
		side = side * -1.0;
	}

	let joint = root + direction * (a * cos_root) + side * (a * sin_root);

	(joint, end)
}

/// Smooth points through a joint chain, for drawing a limb that bends at its joint like a
/// cartoon rubber-hose limb rather than snapping at a hard corner.
pub fn limb_curve(start: Pt, joint: Pt, end: Pt, samples: usize) -> Vec<Pt> {
	// A quadratic Bézier through `joint` at t = 0.5 has its control point at 2·joint − mid.
	let control = joint * 2.0 - start.lerp(end, 0.5);

	(0..=samples)
		.map(|i| {
			let t = i as f32 / samples as f32;
			start * ((1.0 - t) * (1.0 - t)) + control * (2.0 * (1.0 - t) * t) + end * (t * t)
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn two_bone_ik_keeps_bone_lengths_and_reaches() {
		let root = pt(0.0, 0.0);
		let target = pt(60.0, 40.0);
		let (joint, end) = two_bone(root, target, 50.0, 50.0, pt(0.0, 1.0));
		assert!(((joint - root).len() - 50.0).abs() < 0.01);
		assert!(((end - joint).len() - 50.0).abs() < 0.01);
		assert!((end - target).len() < 0.1, "a reachable target is reached");
	}

	#[test]
	fn joints_bend_towards_the_pole() {
		let (down, _) = two_bone(pt(0.0, 0.0), pt(60.0, 0.0), 50.0, 50.0, pt(0.0, 1.0));
		let (up, _) = two_bone(pt(0.0, 0.0), pt(60.0, 0.0), 50.0, 50.0, pt(0.0, -1.0));
		assert!(down.y > 0.0 && up.y < 0.0);
	}

	#[test]
	fn unreachable_targets_straighten_the_limb() {
		let (joint, end) = two_bone(pt(0.0, 0.0), pt(500.0, 0.0), 50.0, 50.0, pt(0.0, 1.0));
		assert!((end.x - 100.0).abs() < 0.1 && end.y.abs() < 0.1);
		assert!(joint.y.abs() < 1.0, "a fully extended limb is straight");
	}

	#[test]
	fn limb_curves_pass_through_the_joint() {
		let points = limb_curve(pt(0.0, 0.0), pt(10.0, 30.0), pt(0.0, 60.0), 10);
		assert!((points[5] - pt(10.0, 30.0)).len() < 0.01);
	}

	#[test]
	fn stances_round_trip_and_crouching_lowers_the_pelvis() {
		for (name, stance) in Stance::ALL {
			assert_eq!(Stance::parse(name), Some(stance));
		}

		let limbs = Limbs {
			pelvis_y: -76.0,
			ankle_y: -24.0,
			hip_spread: 34.0,
			thigh: 27.0,
			shin: 27.0,
			upper_arm: 66.0,
			forearm: 66.0,
		};
		assert!(limbs.stance(Stance::Crouch, 0.0).crouch > limbs.stance(Stance::Stand, 0.0).crouch);
		assert!(limbs.stance(Stance::Tiptoe, 0.0).crouch < 0.0);
	}

	#[test]
	fn walking_alternates_the_feet() {
		let limbs = Limbs {
			pelvis_y: -76.0,
			ankle_y: -24.0,
			hip_spread: 34.0,
			thigh: 27.0,
			shin: 27.0,
			upper_arm: 66.0,
			forearm: 66.0,
		};
		let base = limbs.stance(Stance::Stand, 1.0);
		let (legs, _) = limbs.cycle(Gait::Walk, 0.0, 1.0, base);
		let (left, right) = legs.feet;
		assert!(
			left.y < right.y,
			"at phase 0 the left foot swings while the right plants"
		);
	}

	/// The cartoon kid's bones (`styles/cartoon/style.toml`).
	const CARTOON_KID: Limbs = Limbs {
		pelvis_y: -126.0,
		ankle_y: -24.0,
		hip_spread: 21.0,
		thigh: 52.0,
		shin: 52.0,
		upper_arm: 50.0,
		forearm: 46.0,
	};

	#[test]
	fn the_cartoon_stances_are_exactly_what_they_were() {
		let legs = |near: Pt, crouch: f32, bend: f32| {
			Legs {
				feet: (near, pt(-near.x, near.y)),
				crouch,
				bend,
			}
		};
		let expected = [
			(Stance::Stand, legs(pt(-24.15, -24.0), 0.0, 0.0)),
			(Stance::Crouch, legs(pt(-39.899_998, -24.0), 0.5, 10.0)),
			(Stance::Sit, legs(pt(-54.6, -39.3), 0.92, -4.0)),
			(Stance::Kneel, legs(pt(-46.2, -29.1), 0.7, 4.0)),
			(Stance::Jump, legs(pt(-27.3, -80.100_006), -0.05, 6.0)),
			(Stance::Bow, legs(pt(-24.15, -24.0), 0.05, 32.0)),
			(
				Stance::Tiptoe,
				legs(pt(-19.949_999, -36.239_998), -0.14, 0.0),
			),
		];

		for (stance, legs) in expected {
			assert_eq!(CARTOON_KID.stance(stance, 0.35), legs, "{stance:?}");
			assert_eq!(
				CARTOON_KID.stance(stance, -0.35),
				Legs {
					bend: -legs.bend,
					..legs
				},
				"{stance:?}"
			);
		}
	}

	/// Three bodies that keep to the floor, with their floors turned by `turn`: short legs under
	/// a long top (chibi), medium legs in shorts that hang below the hips (mid-century) and
	/// long ones (anime).
	fn bodies(turn: f32) -> [(&'static str, Limbs, Floor); 3] {
		let floor = |hips: (f32, f32), spread: f32, seat: f32, knee: f32, torso: f32| {
			Floor {
				turn,
				hips: (lerp(-spread, hips.0, turn), lerp(spread, hips.1, turn)),
				ground: (3.0 * turn, -9.0 * turn),
				seat,
				knee,
				torso,
			}
		};

		[
			(
				"short",
				Limbs {
					pelvis_y: -150.0,
					ankle_y: -22.0,
					hip_spread: 18.0,
					thigh: 66.0,
					shin: 66.0,
					upper_arm: 58.0,
					forearm: 50.0,
				},
				floor((-14.0, 18.0), 18.0, 23.0, 15.0, 156.0),
			),
			(
				"medium",
				Limbs {
					pelvis_y: -190.0,
					ankle_y: -24.0,
					hip_spread: 20.0,
					thigh: 86.0,
					shin: 86.0,
					upper_arm: 76.0,
					forearm: 70.0,
				},
				floor((-14.0, 18.0), 20.0, 40.0, 9.0, 128.0),
			),
			(
				"long",
				Limbs {
					pelvis_y: -580.0,
					ankle_y: -24.0,
					hip_spread: 42.0,
					thigh: 280.0,
					shin: 280.0,
					upper_arm: 188.0,
					forearm: 166.0,
				},
				floor((-30.0, 40.0), 42.0, 50.0, 26.0, 304.0),
			),
		]
	}

	/// A stance as a turned figure's puppet takes it: the far foot a step up the page, then
	/// fitted to the body.
	fn staged(limbs: &Limbs, stance: Stance, floor: &Floor) -> Legs {
		let mut legs = limbs.stance(stance, 0.35);
		legs.feet.0 = legs.feet.0 + pt(4.0 * floor.turn, 3.0 * floor.turn);
		legs.feet.1 = legs.feet.1 + pt(6.0 * floor.turn, -9.0 * floor.turn);
		limbs.fit(legs, floor)
	}

	/// Each leg's hip, knee and ankle, near then far, solved the way a puppet that keeps to the
	/// floor solves them.
	fn joints(limbs: &Limbs, legs: &Legs, floor: &Floor) -> [(Pt, Pt, Pt); 2] {
		let drop = legs.crouch * limbs.leg_length();

		[
			(-1.0, floor.hips.0, legs.feet.0),
			(1.0, floor.hips.1, legs.feet.1),
		]
		.map(|(side, x, foot)| {
			let hip = limbs.hip(x, drop, legs.bend);
			let pole = flex_pole(side, floor.turn, foot - hip);
			let (knee, ankle) = two_bone(hip, foot, limbs.thigh, limbs.shin, pole);
			(hip, knee, ankle)
		})
	}

	#[test]
	fn a_seated_body_rests_its_hips_on_the_floor_with_both_knees_in_front() {
		for (name, limbs, floor) in bodies(1.0) {
			let legs = staged(&limbs, Stance::Sit, &floor);

			for (hip, knee, ankle) in joints(&limbs, &legs, &floor) {
				assert!(
					(hip.y + floor.seat).abs() < 4.0,
					"{name}: the hips rest on the floor, {hip:?}"
				);
				assert!(
					(limbs.ankle_y - 9.5..=limbs.ankle_y + 3.5).contains(&ankle.y),
					"{name}: the foot is on the floor, {ankle:?}"
				);
				assert!(knee.x > hip.x + limbs.thigh * 0.3, "{name}: {knee:?}");
				assert!(ankle.x > knee.x - 1.0, "{name}: no foot folds behind");
				assert!(knee.y < hip.y, "{name}: the knee rises from the hip");
			}
		}

		// Long legs sit with their knees raised, short ones with their legs out in front.
		let rise = |(_, limbs, floor): (&str, Limbs, Floor)| {
			let [(hip, knee, _), _] = joints(&limbs, &staged(&limbs, Stance::Sit, &floor), &floor);
			(hip.y - knee.y) / limbs.thigh
		};
		let [short, _, long] = bodies(1.0);
		assert!(rise(long) > 0.7 && rise(short) < 0.5);
	}

	#[test]
	fn a_kneeling_body_has_exactly_one_knee_on_the_floor_and_the_other_foot_in_front() {
		for (name, limbs, floor) in bodies(1.0) {
			let legs = staged(&limbs, Stance::Kneel, &floor);
			let [near, far] = joints(&limbs, &legs, &floor);
			let on_floor = |(_, knee, _): (Pt, Pt, Pt)| (knee.y + floor.knee).abs() < 1.0;
			assert!(on_floor(near) && !on_floor(far), "{name}: {near:?} {far:?}");
			// The kneeling shin lies back along the floor behind its knee.
			assert!(near.2.x < near.1.x - limbs.shin * 0.8, "{name}: {near:?}");
			// The far foot is planted in front, its knee up at about the hips' height.
			assert!(far.2.x > far.0.x + limbs.thigh * 0.5, "{name}: {far:?}");
			assert!(far.1.y < far.0.y + limbs.thigh * 0.3, "{name}: {far:?}");
		}
	}

	#[test]
	fn a_squatting_body_keeps_its_hips_above_its_heels_and_its_knees_apart() {
		for turn in [0.0, 1.0] {
			for (name, limbs, floor) in bodies(turn) {
				let legs = staged(&limbs, Stance::Crouch, &floor);
				let [near, far] = joints(&limbs, &legs, &floor);

				for (hip, knee, ankle) in [near, far] {
					assert!(hip.y < ankle.y - limbs.shin * 0.5, "{name} {turn}: {hip:?}");
					assert!((ankle - hip).x.abs() < limbs.thigh * 0.9, "{name} {turn}");
					// Turned, the knees come forward; front-on they open out to the sides.
					assert!(
						if turn > 0.5 {
							knee.x > hip.x + limbs.thigh * 0.8
						} else {
							(knee.x - hip.x) * (hip.x - f32::midpoint(near.0.x, far.0.x)) > 0.0
						},
						"{name} {turn}: {knee:?}"
					);
				}

				assert!(
					(near.1 - far.1).len() > floor.knee * 2.0,
					"{name} {turn}: the knees are apart"
				);
			}
		}
	}

	#[test]
	fn no_stance_puts_a_joint_through_the_floor() {
		for turn in [0.0, 0.5, 1.0] {
			for (name, limbs, floor) in bodies(turn) {
				for (_, stance) in Stance::ALL {
					let legs = staged(&limbs, stance, &floor);

					for (hip, knee, ankle) in joints(&limbs, &legs, &floor) {
						for joint in [hip, knee, ankle] {
							assert!(joint.y <= 0.0, "{name} {stance:?} turned {turn}: {joint:?}");
						}

						assert!(knee.y <= -floor.knee + 1.0, "{name} {stance:?} {turn}");
					}
				}
			}
		}
	}

	#[test]
	fn standing_poses_are_left_alone_and_getting_down_moves_smoothly() {
		for (_, limbs, floor) in bodies(1.0) {
			for stance in [Stance::Stand, Stance::Bow, Stance::Jump, Stance::Tiptoe] {
				let legs = limbs.stance(stance, 0.35);
				assert_eq!(limbs.fit(legs, &floor), legs, "{stance:?}");
			}

			// Sinking from standing to a sit, the feet and hips never jump.
			let (stand, sit) = (
				limbs.stance(Stance::Stand, 0.35),
				limbs.stance(Stance::Sit, 0.35),
			);
			let mut last: Option<Legs> = None;

			for step in 0..=100 {
				let t = step as f32 / 100.0;
				let legs = limbs.fit(
					Legs {
						feet: (
							stand.feet.0.lerp(sit.feet.0, t),
							stand.feet.1.lerp(sit.feet.1, t),
						),
						crouch: lerp(stand.crouch, sit.crouch, t),
						bend: lerp(stand.bend, sit.bend, t),
					},
					&floor,
				);

				if let Some(last) = last {
					let leg = limbs.leg_length();
					assert!((legs.feet.0 - last.feet.0).len() < leg * 0.08, "{step}");
					assert!((legs.feet.1 - last.feet.1).len() < leg * 0.08, "{step}");
					assert!((legs.crouch - last.crouch).abs() < 0.04, "{step}");
				}

				last = Some(legs);
			}
		}
	}

	#[test]
	fn a_knee_flexes_back_under_the_thigh_whichever_way_the_leg_points() {
		let knee = |foot: Pt| {
			let pole = flex_pole(1.0, 1.0, foot);
			two_bone(pt(0.0, 0.0), foot, 50.0, 50.0, pole).0
		};
		assert!(
			knee(pt(5.0, 80.0)).x > 10.0,
			"standing, the knee comes forward"
		);
		assert!(
			knee(pt(80.0, -6.0)).y < -20.0,
			"stretched along the floor, it rises"
		);
		assert!(
			knee(pt(-60.0, 40.0)).y > 30.0,
			"kneeling with the shin behind, it goes down"
		);
		// Front-on it bends out to the side, as a plain knee does.
		assert!(flex_pole(-1.0, 0.0, pt(0.0, 1.0)).x < -0.9);
		assert_eq!(flex_pole(1.0, 0.0, pt(0.3, 1.0)), knee_pole(1.0, 0.0));
	}
}
