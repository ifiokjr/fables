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

use std::f32::consts::PI;

use super::Pt;
use super::pt;

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
					crouch: 0.5,
					bend: 10.0 * ahead,
				}
			}
			Stance::Sit => {
				Legs {
					feet: foot(2.6, 0.15),
					crouch: 0.92,
					bend: -4.0 * ahead,
				}
			}
			Stance::Kneel => {
				Legs {
					feet: foot(2.2, 0.05),
					crouch: 0.7,
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
}
