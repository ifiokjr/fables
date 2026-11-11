//! The performance layer: ten rules animators use, applied between keyframes.
//!
//! [`frames`](super::frames) tweens the stage between scene keys; this module decides how each
//! part travels. Every rule is a pure function of the time since the key and the two keyframes
//! it sits between, so any frame can be rendered on its own (frames are drawn in parallel).
//! Each rule has a dial in a style's `[motion]` table; all default on, and a script's
//! `[video] acting = "plain"` switches the whole layer off and reproduces the plain tween
//! exactly.
//!
//! 1. **Eyes lead.** A gaze change lands within two frames; the head turn starts about 0.08 s
//!    later, the torso and arms about 0.08 s after that, and a blink covers the turn.
//! 2. **Overlapping action.** Parts do not arrive together: the hand furthest from its target
//!    arrives last, the head settles after the body, loose parts last.
//! 3. **Arcs.** Hands swing on an arc about the shoulder; a head turn nods through.
//! 4. **Anticipation by distance.** The wind-up, the overshoot and one damped settle beat all
//!    scale with how big the change is.
//! 5. **The take.** A mood change to shock plays squash down, stretch up, settle; laughter
//!    bounces; sadness sinks slowly.
//! 6. **Breathing and weight.** Standing figures breathe and shift from foot to foot.
//! 7. **Speech performance.** Speakers beat time with head and hand, brows answer the line,
//!    and a shouted word leans the whole body in.
//! 8. **Listening.** Listeners nod through long lines, glance at what is pointed at, and
//!    flinch a beat after a shout.
//! 9. **Walking is travelling.** An `x` change walks there on the figure's own feet, planted
//!    feet holding still while the body passes them.
//! 10. **Hold lengths.** A figure held too long without direction does one small piece of
//!     idle business.

use std::f32::consts::FRAC_2_PI;
use std::f32::consts::FRAC_PI_2;
use std::f32::consts::PI;
use std::f32::consts::TAU;

use crate::art::Pt;
use crate::art::STAGE;
use crate::art::dials::Motion;
use crate::art::dials::Travel;
use crate::art::figure::FigureState;
use crate::art::figure::HandShape;
use crate::art::figure::Mood;
use crate::art::lerp;
use crate::art::pt;
use crate::art::rig::Gait;
use crate::art::seed_of;
use crate::cast::Kind;
use crate::compose::Body;
use crate::compose::Puppet;
use crate::stage::Side;
use crate::video::lipsync::Track;
use crate::video::voice::ENVELOPE_RATE;

/// How long stage-direction changes take to play out. A travelling walk may take longer; it
/// sets its own span in [`Plan`].
pub(crate) const TWEEN: f32 = 0.6;

/// A gaze change smaller than this is not a "big gaze shift" worth a covering blink.
const BLINK_GAZE: f32 = 0.35;
/// A facing change bigger than this is a real turn (units where 1 is a full profile).
const BLINK_TURN: f32 = 0.12;
/// Eyes reach their target within two frames of film.
const EYES: f32 = 2.0 / 24.0;
/// The head turn starts this long after the eyes; the torso and arms the same again later.
const HEAD_DELAY: f32 = 0.08;
/// Two poses whose hands are closer than this travel in a straight line, not on an arc.
const ARC_MIN: f32 = 30.0;
/// Stage distances shorter than this stay a slide; walking one step would take longer than
/// the change itself.
const MIN_STEP: f32 = 130.0;
/// Seconds a walk takes to bring its feet together again after arriving.
const FOOT_SETTLE: f32 = 0.22;
/// How far offstage an entering or leaving figure waits, in stage units.
const OFFSTAGE: f32 = 400.0;
/// A natural walk's pace, in stage units a second for a figure at scale 1.
const WALK_SPEED: f32 = 340.0;
/// A run's pace, likewise.
const RUN_SPEED: f32 = 560.0;

/// Eases `t` (0 to 1) in and out: slow start, steady middle, slow end.
pub(crate) fn smoothstep(t: f32) -> f32 {
	let t = t.clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// Back easing: overshoots past the target before settling, for pops and title cards.
pub(crate) fn ease_out_back(t: f32) -> f32 {
	let c1 = 1.4;
	let c3 = c1 + 1.0;
	1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// A bounce landing, for falls.
pub(crate) fn ease_out_bounce(t: f32) -> f32 {
	let n1 = 7.5625;
	let d1 = 2.75;

	if t < 1.0 / d1 {
		n1 * t * t
	} else if t < 2.0 / d1 {
		let t = t - 1.5 / d1;
		n1 * t * t + 0.75
	} else if t < 2.5 / d1 {
		let t = t - 2.25 / d1;
		n1 * t * t + 0.9375
	} else {
		let t = t - 2.625 / d1;
		n1 * t * t + 0.984_375
	}
}

/// A bell around `centre`: 1 at the middle, 0 at `width` away or further.
fn bell(t: f32, centre: f32, width: f32) -> f32 {
	let x = (t - centre).abs() / width.max(1e-3);

	if x < 1.0 {
		(PI * x).cos() * 0.5 + 0.5
	} else {
		0.0
	}
}

/// A triangle wave in −1..1, rising and falling at a constant rate — the foot excursion of a
/// travelling walk, whose constant slope is what lets a planted foot hold still.
fn tri(psi: f32) -> f32 {
	FRAC_2_PI * psi.sin().asin()
}

/// Everything the rules need to know about the moment they are animating.
pub struct Cx<'a> {
	/// The whole layer is on (`[video] acting` is not `"plain"`).
	pub on: bool,
	pub motion: Motion,
	/// The shot's lines, resolved for the rules.
	pub lines: Vec<Spoken<'a>>,
	/// When the current scene key lands: the last direction anyone gave.
	pub key_time: f32,
}

impl Cx<'_> {
	/// A context with the layer off: every rule stands down and the plain tween plays.
	pub fn plain(motion: Motion) -> Self {
		Self {
			on: false,
			motion,
			lines: Vec::new(),
			key_time: 0.0,
		}
	}
}

/// One spoken line, resolved to what the acting rules read.
pub struct Spoken<'a> {
	pub start: f32,
	/// Seconds the line lasts.
	pub duration: f32,
	/// Actors whose mouths move with the line.
	pub speakers: &'a [String],
	/// The words with markup, so `**shouted**` lines can be spotted.
	pub text: &'a str,
	/// Loudness over the line, at [`ENVELOPE_RATE`].
	pub envelope: &'a [f32],
	/// Mouth shapes over the line.
	pub track: &'a Track,
	/// Seconds into the line where it is loudest: the strongest stress.
	pub peak: f32,
	/// The line asks a question, so brows lift.
	pub question: bool,
}

impl Spoken<'_> {
	/// Seconds into the line, while it is playing.
	fn at(&self, time: f32) -> Option<f32> {
		let t = time - self.start;
		(t >= 0.0 && t < self.duration).then_some(t)
	}

	fn shouted(&self) -> bool {
		self.text.contains("**")
	}

	fn spoken_by(&self, id: &str) -> bool {
		self.speakers.iter().any(|speaker| speaker == id)
	}

	/// How open a speaking mouth is `t` seconds into the line.
	fn mouth(&self, t: f32) -> f32 {
		if t < 0.0 {
			return 0.0;
		}

		self.envelope
			.get((t * ENVELOPE_RATE) as usize)
			.copied()
			.unwrap_or(0.0)
	}
}

/// The loudest moment of a line, in seconds from its start.
pub fn peak_of(envelope: &[f32]) -> f32 {
	let mut best = -f32::INFINITY;
	let mut best_at = 0;

	for (index, sample) in envelope.iter().enumerate() {
		let loud = *sample + envelope.get(index + 1).copied().unwrap_or(0.0);

		if loud > best {
			best = loud;
			best_at = index;
		}
	}

	best_at as f32 / ENVELOPE_RATE
}

/// Whether a line's words ask a question.
pub fn is_question(text: &str) -> bool {
	text.trim_end_matches(['*', '"', '\u{201c}', '\u{201d}', ' ', ','])
		.ends_with('?')
}

/// Whoever is talking at a moment, for the listening rules.
#[derive(Clone)]
pub struct Speaker {
	pub id: String,
	/// Their stage x.
	pub x: f32,
	/// Which way they face: -1 left, 1 right.
	pub heading: f32,
	/// They are pointing or reaching out, so listeners glance where they point.
	pub pointing: bool,
}

/// Whether a figure is pointing, or reaching out with both hands.
pub fn pointing_at(puppet: &Puppet) -> bool {
	let Body::Figure(state) = &puppet.body else {
		return false;
	};

	if state.hand_shapes.1 == HandShape::Point {
		return true;
	}

	let Kind::Figure(look) = &puppet.kind else {
		return false;
	};
	let reach = look.frame().head_r * 1.1;
	state.hands.0.x < -reach && state.hands.1.x > reach
}

/// The puppets at `time` between two scene keys, acted by the rules.
///
/// `progress` is `since / TWEEN` (the plain slide's clock, kept for smears and falls); `since`
/// is the time since the key landed. When the layer is off this is exactly yesterday's tween:
/// one easing curve, then follow-through and a smear.
pub fn tween(
	old: &Puppet,
	new: &Puppet,
	since: f32,
	progress: f32,
	knocked_down: bool,
	cx: &Cx,
) -> Puppet {
	if !cx.on {
		let eased = if knocked_down {
			ease_out_bounce(progress)
		} else {
			cx.motion.ease(progress)
		};
		let mut moved = crate::compose::blend(old, new, eased);
		follow_through(&mut moved, old, since, cx.motion.follow_through);
		moved.smear = smear(old, new, progress, cx.motion.smears);

		return moved;
	}

	let plan = plan_between(old, new, cx);
	let span = plan.as_ref().map_or(TWEEN, |plan| plan.duration);
	let size = change_size(old, new);
	let mut moved = blend_ruled(old, new, since, span, size, cx);
	follow_through(&mut moved, old, since, cx.motion.follow_through);
	moved.smear = smear(old, new, progress, cx.motion.smears);

	if let Some(plan) = &plan {
		walk(&mut moved, new, plan, since);
		moved.exerted = ((since - plan.duration) / 2.5).clamp(0.0, 1.0);
	}

	take(&mut moved, old, new, since, cx);
	settle(&mut moved, since, span, size, cx);
	moved
}

/// How big the change between two keyframes is, 0 (a blink) to 1 (a jump or a point): how far
/// the hands, body and face travel, in units where the cartoon's big moves read 1.
fn change_size(old: &Puppet, new: &Puppet) -> f32 {
	let (Body::Figure(o), Body::Figure(n)) = (&old.body, &new.body) else {
		return 0.5;
	};

	let hands = (n.hands.0 - o.hands.0).len() + (n.hands.1 - o.hands.1).len();
	let travel = (new.at.x - old.at.x).abs();
	let face = (n.facing - o.facing).abs();
	let crouch = (n.legs.crouch - o.legs.crouch).abs();

	(0.5 * hands / 420.0 + 0.5 * travel / 800.0 + 0.6 * face + 1.2 * crouch).clamp(0.0, 1.0)
}

/// When each part moves, as a (start, end) pair of seconds since the key. A part eases through
/// its window; windows their dials switch off collapse to the whole span, which is the single
/// easing curve the plain tween uses.
struct Windows {
	head: (f32, f32),
	body: (f32, f32),
	/// Per hand, left then right.
	hands: [(f32, f32); 2],
	sway: (f32, f32),
}

impl Windows {
	fn build(old: &Puppet, new: &Puppet, span: f32, cx: &Cx) -> Windows {
		let whole = (0.0, span);
		let (Body::Figure(o), Body::Figure(n)) = (&old.body, &new.body) else {
			return Windows {
				head: whole,
				body: whole,
				hands: [whole; 2],
				sway: whole,
			};
		};

		let m = cx.motion;

		// Start offsets come from lead: the eyes go first, the head follows, the body follows.
		let head_start = if m.lead > 0.0 {
			HEAD_DELAY.min(span * 0.25)
		} else {
			0.0
		};
		let body_start = if m.lead > 0.0 {
			(HEAD_DELAY * 2.0).min(span * 0.4)
		} else {
			0.0
		};

		// Settle offsets come from overlap: body first, then the near hand, then the head,
		// then the far hand; loose parts ride to the end.
		let ov = m.overlap.clamp(0.0, 2.0);
		let end = |lag: f32| span * (1.0 - lag * ov).max(0.35);
		let body_end = end(0.14);
		let near_end = end(0.09);
		let head_end = end(0.06);

		// The hand further from its target starts later and arrives last; the offsets scale
		// with how far each hand travels.
		let travels = [(n.hands.0 - o.hands.0).len(), (n.hands.1 - o.hands.1).len()];
		let farthest = travels[0].max(travels[1]).max(1e-3);
		let hands = [0, 1].map(|hand| {
			let share = travels[hand] / farthest;
			let start = body_start + span * 0.06 * ov * share;
			let finish = if travels[hand] >= farthest - 1e-3 {
				span
			} else {
				near_end
			};

			window(start, finish, span)
		});

		Windows {
			head: window(head_start, head_end, span),
			body: window(body_start, body_end, span),
			hands,
			sway: window(body_start, span, span),
		}
	}

	/// The eased progress of a window at `since`.
	fn t((start, end): (f32, f32), since: f32, m: &Motion, size: f32) -> f32 {
		let t = if end - start < 1e-3 {
			(since / end.max(1e-3)).clamp(0.0, 1.0)
		} else {
			((since - start) / (end - start)).clamp(0.0, 1.0)
		};

		m.ease_by_distance(t, size)
	}
}

/// A (start, end) pair that cannot invert; a degenerate one spans the whole move.
fn window(start: f32, end: f32, span: f32) -> (f32, f32) {
	if end - start < 1e-3 || start < 0.0 || end > span + 1e-3 {
		(0.0, span)
	} else {
		(start, end)
	}
}

/// Blends two figure puppets with the rules: lead, overlap and arcs decide when each part
/// moves and on what path; everything else rides the body's window.
fn blend_ruled(old: &Puppet, new: &Puppet, since: f32, span: f32, size: f32, cx: &Cx) -> Puppet {
	let m = cx.motion;

	let (Body::Figure(o), Body::Figure(n)) = (&old.body, &new.body) else {
		// Critters and mixed pairs tween as one piece, with the distance-scaled ease.
		let t = m.ease_by_distance((since / span).clamp(0.0, 1.0), size);

		return crate::compose::blend(old, new, t);
	};

	let windows = Windows::build(old, new, span, cx);
	let body_t = Windows::t(windows.body, since, &m, size);
	let mut moved = crate::compose::blend(old, new, body_t);
	let Body::Figure(state) = &mut moved.body else {
		return moved;
	};

	// The eyes go first: a saccade, there within two frames and no wind-up.
	state.gaze = if m.lead > 0.0 {
		lerp(
			o.gaze,
			n.gaze,
			smoothstep((since / EYES.min(span * 0.4)).clamp(0.0, 1.0)),
		)
	} else {
		lerp(o.gaze, n.gaze, Windows::t(windows.body, since, &m, size))
	};

	// The head starts after the eyes and settles after the body.
	let head_t = Windows::t(windows.head, since, &m, size);
	state.facing = lerp(o.facing, n.facing, head_t);
	state.head_tilt = lerp(o.head_tilt, n.head_tilt, head_t);
	state.brow_lift = lerp(o.brow_lift, n.brow_lift, head_t);

	// A turn nods through an arc rather than sliding level.
	let turn = n.facing - o.facing;

	if m.arcs > 0.0 && turn.abs() > BLINK_TURN {
		state.head_tilt -= (PI * head_t).sin() * 2.6 * m.arcs * turn.signum();
	}

	// Each hand swings on its own window, on an arc about the shoulder where the move is wide
	// enough to be worth one.
	let Kind::Figure(look) = &new.kind else {
		return moved;
	};
	let frame = look.frame();

	state.hands = (
		hand_between(
			o.hands.0,
			n.hands.0,
			Windows::t(windows.hands[0], since, &m, size),
			frame.shoulder(-1.0),
			m.arcs,
		),
		hand_between(
			o.hands.1,
			n.hands.1,
			Windows::t(windows.hands[1], since, &m, size),
			frame.shoulder(1.0),
			m.arcs,
		),
	);

	// Loose parts ride to the end of the move; the follow-through spring carries them past it.
	state.sway = lerp(o.sway, n.sway, Windows::t(windows.sway, since, &m, size));

	// A blink covers any head turn of more than a few degrees and any big gaze shift.
	let gaze_shift = (n.gaze - o.gaze).abs();

	if m.lead > 0.0 && (turn.abs() > BLINK_TURN || gaze_shift > BLINK_GAZE) {
		state.blink = state.blink.max(m.lead * bell(since, 0.09, 0.07));
	}

	moved
}

/// One hand from `from` to `to`: straight, unless the move is wide enough to swing on an arc
/// about `shoulder` — interpolating the angle and reach from it rather than the hand's x and y.
fn hand_between(from: Pt, to: Pt, t: f32, shoulder: Pt, arcs: f32) -> Pt {
	if arcs <= 0.0 || (to - from).len() < ARC_MIN {
		return from.lerp(to, t);
	}

	let r0 = from - shoulder;
	let r1 = to - shoulder;

	if r0.len() < 1e-3 || r1.len() < 1e-3 {
		return from.lerp(to, t);
	}

	let a0 = r0.y.atan2(r0.x);
	let a1 = r1.y.atan2(r1.x);
	let mut sweep = a1 - a0;

	if sweep > PI {
		sweep -= TAU;
	}

	if sweep < -PI {
		sweep += TAU;
	}

	let reach = lerp(r0.len(), r1.len(), t);
	let angle = a0 + sweep * t;

	pt(
		shoulder.x + angle.cos() * reach,
		shoulder.y + angle.sin() * reach,
	)
}

/// One stage-crossing on foot: everything the walk pose needs. The stride is solved so a
/// planted foot holds still while the body passes it, and the step count is whole, so the
/// walk starts and ends with both feet planted.
pub struct Plan {
	pub duration: f32,
	pub x0: f32,
	pub x1: f32,
	/// Which way it goes: -1 left, 1 right.
	pub dir: f32,
	pub gait: Gait,
	/// Stride as a fraction of leg length.
	pub stride: f32,
	pub lift: f32,
	pub bob: f32,
	/// Spine lean into the walk, in degrees.
	pub lean: f32,
	/// How far the hands counter-swing, as a fraction of leg length.
	pub swing: f32,
	/// Step cadence, in radians a second.
	pub omega: f32,
	/// The fraction of the trip spent easing in and out (about half a step).
	pub delta: f32,
	/// The gait phase at the start: both feet planted at the front and back extremes.
	pub psi0: f32,
	pub leg: f32,
	pub scale: f32,
}

impl Plan {
	/// The eased distance covered, 0 to 1, at a fraction `u` of the trip.
	fn profile(&self, u: f32) -> f32 {
		u - self.delta * (TAU * u).sin() / TAU
	}
}

/// The walk (or run) between two scene keys, when the dials say figures travel on their own
/// feet and the distance is worth the legs. `None` leaves the slide.
pub fn plan_between(old: &Puppet, new: &Puppet, cx: &Cx) -> Option<Plan> {
	if !cx.on || cx.motion.travel != Travel::Walk {
		return None;
	}

	let (Body::Figure(n), Body::Figure(_)) = (&new.body, &old.body) else {
		return None;
	};
	let Kind::Figure(look) = &new.kind else {
		return None;
	};

	let distance = new.at.x - old.at.x;

	if distance.abs() < MIN_STEP {
		return None;
	}

	// Runs when the script says run, or the distance is long for the time available.
	let running = matches!(n.gait, Some((Gait::Run, _)));
	let (gait, speed, stride0, lift, bob, lean, swing) =
		if running || distance.abs() > WALK_SPEED * 1.6 * new.scale {
			(Gait::Run, RUN_SPEED, 0.7, 0.55, 0.13, 12.0, 0.4)
		} else {
			(Gait::Walk, WALK_SPEED, 0.45, 0.3, 0.07, 4.0, 0.22)
		};

	let scale = new.scale.max(0.01);
	let duration = (distance.abs() / (speed * scale)).clamp(TWEEN, if running { 1.3 } else { 1.8 });
	let leg = look.frame().limbs().leg_length();

	// A whole number of steps, each covering one stride of the distance, so the walk starts
	// and ends planted and a planted foot can hold still while the body passes it.
	let steps = (distance.abs() / (2.0 * stride0 * leg * scale))
		.round()
		.max(1.0);
	let omega = steps * PI / duration;
	// The stride that keeps a planted foot still: the foot recedes through its stance at
	// exactly the speed the body advances.
	let stride = (PI * distance.abs() / (2.0 * leg * scale * omega * duration)).clamp(0.15, 0.85);
	let delta = (1.6 * stride * leg * scale / distance.abs()).clamp(0.1, 0.85);

	Some(Plan {
		duration,
		x0: old.at.x,
		x1: new.at.x,
		dir: distance.signum(),
		gait,
		stride,
		lift,
		bob,
		lean,
		swing,
		omega,
		delta,
		psi0: FRAC_PI_2,
		leg,
		scale,
	})
}

/// The travel pose at `since`: the body crossing on the eased profile, the feet cycling under
/// it, the arms counter-swinging, and the face turned towards where it is going until the last
/// half-step turns it back to its staged facing. After the walk, one short settle brings the
/// feet together.
fn walk(moved: &mut Puppet, target: &Puppet, plan: &Plan, since: f32) {
	let Body::Figure(state) = &mut moved.body else {
		return;
	};
	let Body::Figure(n) = &target.body else {
		return;
	};

	let feet = |psi: f32, together: f32| {
		let base = [n.legs.feet.0, n.legs.feet.1];
		let mut placed = [pt(0.0, 0.0); 2];

		for (index, side) in [(0usize, -1.0_f32), (1, 1.0)] {
			let psi = psi + if side > 0.0 { PI } else { 0.0 };
			let stride_x = base[index].x + tri(psi) * plan.stride * plan.leg * plan.dir;
			let lift_y = base[index].y - psi.cos().max(0.0) * plan.lift * plan.leg;
			placed[index] = pt(
				lerp(stride_x, base[index].x, together),
				lerp(lift_y, base[index].y, together),
			);
		}

		placed
	};

	if since < plan.duration {
		let u = (since / plan.duration).clamp(0.0, 1.0);
		let s = plan.profile(u);
		moved.at.x = plan.x0 + (plan.x1 - plan.x0) * s;
		let psi = plan.psi0 + plan.omega * plan.duration * s;
		let env = smoothstep((u / plan.delta).clamp(0.0, 1.0))
			* smoothstep(((1.0 - u) / plan.delta).clamp(0.0, 1.0));

		let placed = feet(psi, 0.0);
		state.legs.feet = (placed[0], placed[1]);
		state.legs.crouch = n.legs.crouch - plan.bob * (2.0 * psi).cos() * 0.5 * env;
		state.legs.bend = n.legs.bend + plan.lean * plan.dir * env;
		state.gait = None;

		let swings = [-1.0_f32, 1.0].map(|side| {
			let psi = psi + if side > 0.0 { PI } else { 0.0 };
			pt(
				-psi.sin() * plan.swing * plan.leg * plan.dir,
				-psi.sin().abs() * plan.swing * plan.leg * 0.3,
			) * env
		});
		state.hands = (state.hands.0 + swings[0], state.hands.1 + swings[1]);

		// It turns to face the way it is going, and back to its staged facing near the end.
		let staged = n.facing;
		let going = plan.dir * 0.7;
		let turn_in = smoothstep((u / 0.15).clamp(0.0, 1.0));
		let turn_out = smoothstep(((u - 0.85) / 0.15).clamp(0.0, 1.0));
		state.facing = lerp(lerp(staged, going, turn_in), staged, turn_out);
	} else {
		// Feet together again: the walk ended on a planted stride, so this only draws the
		// feet in over one short settle while the body stands still.
		let settle = ((since - plan.duration) / FOOT_SETTLE).clamp(0.0, 1.0);
		let psi = plan.psi0 + plan.omega * plan.duration;

		if settle >= 1.0 {
			state.legs.feet = n.legs.feet;
		} else {
			let placed = feet(psi, smoothstep(settle));
			state.legs.feet = (placed[0], placed[1]);
		}

		state.gait = None;
	}
}

/// How long a leaving puppet keeps being drawn: a walk off takes as long as it takes.
pub fn exit_span(puppet: &Puppet, exit: Option<Side>, cx: &Cx) -> f32 {
	let Some(side) = exit else {
		return TWEEN;
	};

	let mut offstage = puppet.clone();
	offstage.at.x = match side {
		Side::Left => -OFFSTAGE,
		Side::Right => STAGE + OFFSTAGE,
	};

	plan_between(puppet, &offstage, cx).map_or(TWEEN, |plan| plan.duration + FOOT_SETTLE)
}

/// What kind of take a mood change plays.
enum TakeKind {
	/// Squash down and in, stretch up past the pose with eyes wide, then settle.
	Take,
	/// A couple of quick bounces.
	Bounce,
	/// A slow sink, with no overshoot.
	Sink,
	None,
}

fn take_kind(
	o: &FigureState,
	n: &FigureState,
	frame: Option<&crate::art::figure::Frame>,
) -> TakeKind {
	match n.mood {
		Mood::Shocked | Mood::Disgusted | Mood::Fainted => TakeKind::Take,
		Mood::Laugh => TakeKind::Bounce,
		Mood::Sad | Mood::Sleepy => TakeKind::Sink,
		_ => {
			let arms_up = |state: &FigureState| {
				let Some(frame) = frame else {
					return false;
				};
				state.hand_shapes == (HandShape::Fist, HandShape::Fist)
					&& state.hands.0.y < frame.shoulder_y - frame.head_r
					&& state.hands.1.y < frame.shoulder_y - frame.head_r
			};
			let sunk = |state: &FigureState| {
				let Some(frame) = frame else {
					return false;
				};
				(state.hands.0 - state.hands.1).len() < 70.0
					&& state.hands.0.y > frame.shoulder_y + frame.head_r
			};

			if arms_up(n) && !arms_up(o) {
				TakeKind::Bounce
			} else if sunk(n) && !sunk(o) {
				TakeKind::Sink
			} else {
				TakeKind::None
			}
		}
	}
}

/// The take: a mood change plays as more than a blend.
fn take(moved: &mut Puppet, old: &Puppet, new: &Puppet, since: f32, cx: &Cx) {
	let amount = cx.motion.takes;

	if amount <= 0.0 {
		return;
	}

	let (Body::Figure(o), Body::Figure(n)) = (&old.body, &new.body) else {
		return;
	};

	if o.mood == n.mood {
		return;
	}

	let frame = match &new.kind {
		Kind::Figure(look) => Some(look.frame()),
		_ => None,
	};
	let amount = amount * (0.55 + 0.45 * cx.motion.squash);
	let Body::Figure(state) = &mut moved.body else {
		return;
	};

	match take_kind(o, n, frame.as_ref()) {
		TakeKind::Take => {
			let s = if since < 0.12 {
				1.0 - 0.24 * amount * (since / 0.12)
			} else if since < 0.42 {
				1.0 + 0.18 * amount * (PI * (since - 0.12) / 0.3).sin()
			} else {
				let t = since - 0.42;
				1.0 + (-6.0 * t).exp() * (16.0 * t).sin() * 0.12 * amount
			};
			state.squash *= s;
			// Eyes wide through the stretch.
			state.brow_lift += 0.3 * amount * bell(since, 0.3, 0.2);
		}
		TakeKind::Bounce => {
			state.squash *= 1.0 + 0.11 * amount * (-5.0 * since).exp() * (13.0 * since).sin();
		}
		TakeKind::Sink => {
			let dip = smoothstep((since / 0.6).clamp(0.0, 1.0))
				* (1.0 - smoothstep(((since - 0.9) / 0.5).clamp(0.0, 1.0)));
			state.squash *= 1.0 - 0.05 * amount * dip;
			state.head_tilt -= 1.5 * amount * dip;
		}
		TakeKind::None => {}
	}
}

/// One small damped after-beat as a held pose settles, scaled by how big the move was.
fn settle(moved: &mut Puppet, since: f32, span: f32, size: f32, cx: &Cx) {
	if since <= span || cx.motion.overshoot <= 0.0 {
		return;
	}

	let t = since - span;
	let beat = (-6.0 * t).exp() * (18.0 * t).sin() * 0.02 * cx.motion.overshoot * size;
	let Body::Figure(state) = &mut moved.body else {
		return;
	};

	state.squash *= 1.0 + beat;
	state.head_tilt += beat * 22.0;
}

/// An actor arriving: walking in from the side they enter from, or popping into place.
pub fn arrive(
	puppet: &Puppet,
	enter: Option<Side>,
	progress: f32,
	since: f32,
	time: f32,
	cx: &Cx,
) -> Puppet {
	let mut arriving = puppet.clone();

	let Some(side) = enter else {
		let pop = ease_out_back(progress);
		arriving.scale *= pop.max(0.0);
		arriving.opacity = (progress * 3.0).min(1.0);

		return arriving;
	};

	let start = match side {
		Side::Left => -OFFSTAGE,
		Side::Right => STAGE + OFFSTAGE,
	};

	if cx.on && cx.motion.travel == Travel::Walk {
		let mut offstage = puppet.clone();
		offstage.at.x = start;

		if let Body::Figure(_) = arriving.body
			&& let Some(plan) = plan_between(&offstage, puppet, cx)
		{
			walk(&mut arriving, puppet, &plan, since);

			return arriving;
		}
	}

	let eased = smoothstep(progress * 1.3);
	arriving.at.x = lerp(start, puppet.at.x, eased);
	treadmill(&mut arriving, time, 1.0 - eased);
	arriving
}

/// An actor leaving: walking off their exit side, or shrinking away.
pub fn leave(
	puppet: &Puppet,
	exit: Option<Side>,
	progress: f32,
	since: f32,
	time: f32,
	cx: &Cx,
) -> Puppet {
	let mut leaving = puppet.clone();

	let Some(side) = exit else {
		leaving.scale *= 1.0 - smoothstep(progress);
		leaving.opacity = 1.0 - progress;

		return leaving;
	};

	let end = match side {
		Side::Left => -OFFSTAGE,
		Side::Right => STAGE + OFFSTAGE,
	};

	if cx.on && cx.motion.travel == Travel::Walk {
		let mut offstage = puppet.clone();
		offstage.at.x = end;

		if let Body::Figure(_) = leaving.body
			&& let Some(plan) = plan_between(puppet, &offstage, cx)
		{
			walk(&mut leaving, &offstage, &plan, since);

			return leaving;
		}
	}

	leaving.at.x = lerp(puppet.at.x, end, smoothstep(progress));
	treadmill(&mut leaving, time, 1.0);
	leaving
}

/// A walk cycle on the spot: people use the rig's walk cycle; animals bob along. Kept for the
/// treadmill gaits a script can ask for and the slide's last flourish.
fn treadmill(puppet: &mut Puppet, time: f32, amount: f32) {
	if amount <= 0.01 {
		return;
	}

	let phase = time * Gait::Walk.rate();

	if let Body::Figure(state) = &mut puppet.body {
		// The rig plants and lifts the feet, bobs the pelvis and swings the arms.
		state.gait = Some((Gait::Walk, phase));
		state.facing = puppet.heading * 0.7;
		state.sway = (phase * 2.0).sin() * 0.3 * amount;
	} else {
		puppet.lift += (phase * 2.0).sin().abs() * 24.0 * puppet.scale * amount;
	}
}

/// A jump: stretch on the way up, squash on landing, then settle.
pub fn hop(puppet: &mut Puppet, since: f32, squash: f32) {
	let t = since / 0.5;

	if !(0.0..1.5).contains(&t) {
		return;
	}

	if t < 1.0 {
		puppet.lift += (t * PI).sin() * 120.0 * puppet.scale;
	}

	if let Body::Figure(state) = &mut puppet.body {
		state.squash = if t < 0.9 {
			1.0 + 0.09 * squash * (t / 0.9 * PI).sin()
		} else {
			1.0 - 0.16 * squash * ((t - 0.9) / 0.6 * PI).sin()
		};
		state.sway = -((t * PI).sin()) * 0.6;
	}
}

/// After a move, loose parts keep going and swing back: a damped spring, solved in closed form
/// so any frame can be rendered on its own (frames are drawn in parallel).
pub fn follow_through(puppet: &mut Puppet, before: &Puppet, since: f32, amount: f32) {
	let Body::Figure(state) = &mut puppet.body else {
		return;
	};

	let moved = puppet.at.x - before.at.x;
	let Body::Figure(old) = &before.body else {
		return;
	};

	let hands = (state.hands.0 - old.hands.0).len() + (state.hands.1 - old.hands.1).len();
	let push = (moved.abs() / 500.0 + hands / 400.0).min(1.0);

	if push < 0.02 || since < 0.0 {
		return;
	}

	let direction = if moved.abs() > 1.0 {
		-moved.signum()
	} else {
		1.0
	};
	state.sway += direction * push * (-3.2 * since).exp() * (11.0 * since).sin() * amount;
}

/// Speed lines behind a puppet crossing the stage fast: strongest mid-move, and only for
/// styles that smear.
pub fn smear(old: &Puppet, new: &Puppet, progress: f32, amount: f32) -> f32 {
	let travel = new.at.x - old.at.x;

	if amount <= 0.0 || travel.abs() < 80.0 || !(0.0..1.0).contains(&progress) {
		return 0.0;
	}

	travel.signum() * amount * (1.0 - (2.0 * progress - 1.0).abs())
}

/// Acting on a puppet while nothing changes on stage: speech, listening, breathing, weight,
/// idle business, blinking — everything that keeps a held pose alive.
pub fn animate(puppet: &mut Puppet, cx: &Cx, time: f32, speaking: Option<&Speaker>) {
	let mut cloud: f32 = puppet.breath;
	let mut mouth: f32 = 0.0;
	let mut viseme = None;
	let mut shouting = false;
	let mut active: Option<(&Spoken, f32)> = None;
	let mut finished: Option<&Spoken> = None;
	let seed = seed_of(&puppet.id);

	for spoken in cx
		.lines
		.iter()
		.filter(|spoken| spoken.spoken_by(&puppet.id))
	{
		let end = spoken.start + spoken.duration;

		if let Some(t) = spoken.at(time) {
			// Limited animation flaps the mouth a few times a second instead of every frame.
			let t = cx.motion.mouth_time(t);
			mouth = mouth.max(spoken.mouth(t) * 0.95);
			viseme = Some(spoken.track.at(t));
			shouting |= spoken.shouted();
			cloud = cloud.max(smoothstep(t / 0.3));
			active = Some((spoken, t));
		} else if time >= end {
			// The most recently finished line owns the dying fall of the breath cloud.
			finished = match finished {
				Some(last) if last.start > spoken.start => Some(last),
				_ => Some(spoken),
			};
			cloud = cloud.max(1.0 - (time - end) / 1.6);
		}
	}

	puppet.breath = cloud.clamp(0.0, 1.0);

	let Body::Figure(state) = &mut puppet.body else {
		return;
	};

	state.mouth_open = state.mouth_open.max(mouth);

	if let Some((spoken, t)) = active {
		state.viseme = viseme;
		// Speakers nod into stressed syllables and lift their brows to shout.
		state.head_tilt += (mouth - 0.45) * 6.0 * puppet.heading;
		state.brow_lift += if shouting { 0.09 } else { mouth * 0.035 };

		if cx.on {
			speech_performance(state, spoken, t, mouth, shouting, puppet.heading, cx);
		}
	} else if cx.on
		&& let Some(spoken) = finished.filter(|spoken| {
			let end = spoken.start + spoken.duration;
			time < end + 0.5
		}) {
		// Between sentences the speaker's gaze flicks away and back.
		let w = (time - spoken.start - spoken.duration) / 0.5;
		state.gaze += -puppet.heading * 0.45 * (PI * w).sin() * cx.motion.gesture;
	} else if let Some(speaker) = speaking.filter(|speaker| speaker.id != puppet.id)
		&& state.lie < 0.5
	{
		// Everyone else turns their eyes to whoever is talking, with a small listening tilt.
		let towards = (speaker.x - puppet.at.x).signum();
		state.gaze = towards * 0.9;
		state.head_tilt += towards * 3.0;

		if cx.on {
			listening(state, cx, time, speaker, towards);
		}
	}

	if cx.on {
		idle_life(puppet.exerted, seed, state, cx, time, active.is_some());
		business(puppet.heading, seed, state, cx, time);
	}

	let motion = cx.motion;

	// An idle loop (a rubber-hose bounce) for everyone standing.
	if state.lie < 0.5 && state.gait.is_none() {
		let (lift, squash) = motion.idle_at(time, seed);
		puppet.lift += lift * puppet.scale;
		state.squash *= squash;
	}

	// A moving hold: a pose held for long drifts a little, so it never freezes dead.
	if motion.moving_hold > 0.0 {
		state.tilt += (time * 0.7 + (seed % 17) as f32).sin() * motion.moving_hold;
		state.head_tilt += (time * 0.5 + (seed % 11) as f32).sin() * motion.moving_hold;
	}

	let period = motion.blink_every + (seed % 100) as f32 / 40.0;
	let phase = (time + (seed % 13) as f32 * 0.37) % period;

	if phase < 0.14 && state.lie < 0.5 {
		state.blink = (1.0 - (phase - 0.07).abs() / 0.07).max(state.blink);
	}
}

/// While a character speaks, drive more than the mouth: a hand beat on the strongest stress of
/// the line, a head beat with every accent, brows that answer the line, and a lean into
/// shouted words.
fn speech_performance(
	state: &mut FigureState,
	spoken: &Spoken,
	t: f32,
	mouth: f32,
	shouting: bool,
	heading: f32,
	cx: &Cx,
) {
	let dial = cx.motion.gesture;

	if dial <= 0.0 {
		return;
	}

	// A hand beat on the strongest peak of the line, in the current pose.
	let beat = bell(t - spoken.peak, 0.0, 0.14);

	if beat > 0.01 {
		let offset = pt(heading * 9.0, -12.0) * beat * dial;

		if heading >= 0.0 {
			state.hands.1 = state.hands.1 + offset;
		} else {
			state.hands.0 = state.hands.0 + offset;
		}
		state.head_tilt += heading * 2.0 * beat * dial;
	}

	// Brows lift on a question and drop on a line spoken cross or grumpy.
	if spoken.question {
		state.brow_lift += 0.16 * dial * smoothstep((t / 0.3).clamp(0.0, 1.0));
	}

	if matches!(state.mood, Mood::Cross | Mood::Grumpy) {
		state.brow_lift -= 0.12 * dial;
	}

	// A shouted word leans the whole body in.
	if shouting {
		state.tilt += heading * 3.5 * dial;
	}

	// Accents already nod the head through `mouth`; a strong peak gets a beat more.
	state.head_tilt += heading * mouth * beat * 3.0 * dial;
}

/// Listening: nod once or twice through a long line, glance at whatever the speaker points at,
/// and flinch a beat late when they shout.
fn listening(state: &mut FigureState, cx: &Cx, time: f32, speaker: &Speaker, towards: f32) {
	let dial = cx.motion.listen;

	if dial <= 0.0 {
		return;
	}

	for spoken in cx
		.lines
		.iter()
		.filter(|spoken| spoken.spoken_by(&speaker.id))
	{
		let Some(t) = spoken.at(time) else {
			continue;
		};

		// A long line earns a nod or two.
		if spoken.duration > 2.5 && (0.2..0.9).contains(&(t / spoken.duration)) {
			let nods = ((spoken.duration / 1.4) as usize).clamp(1, 2) as f32;
			state.head_tilt += (t / spoken.duration * nods * PI).sin() * 2.4 * dial * towards;
		}

		// Glance at whatever the speaker points at.
		if speaker.pointing && t > 0.35 {
			state.gaze = speaker.heading * 0.9;
		}

		// A shout lands a beat late: a small flinch.
		if spoken.shouted() && t > 0.15 {
			let k = t - 0.15;
			let flinch = (-9.0 * k).exp() * (26.0 * k).sin();
			state.head_tilt -= flinch * 3.0 * dial * towards;
			state.squash *= 1.0 + 0.02 * flinch * dial;
		}
	}
}

/// Breathing and weight: a standing figure breathes about once every 3.5 s (faster when
/// alarmed or just after running) and shifts its weight from foot to foot every several
/// seconds — never mid-line while speaking. Sitting and lying figures only breathe.
fn idle_life(
	exerted: f32,
	seed: u64,
	state: &mut FigureState,
	cx: &Cx,
	time: f32,
	speaking_own: bool,
) {
	let m = cx.motion;

	if m.breath > 0.0 {
		let alarmed = matches!(state.mood, Mood::Shocked | Mood::Laugh);
		let period = 3.5 / (1.0 + 0.7 * f32::from(alarmed) + exerted);
		let phase = (seed % 100) as f32 * 0.0628;
		let breath = 0.5 - 0.5 * (TAU * time / period + phase).cos();

		state.squash *= 1.0 + 0.011 * breath * m.breath;
		state.head_tilt += 0.35 * breath * m.breath;
	}

	if m.weight_shift > 0.0
		&& !speaking_own
		&& state.lie < 0.5
		&& state.gait.is_none()
		&& state.legs.crouch < 0.7
	{
		let phase = (seed % 97) as f32 * 0.0647;
		let shift = 1.0 - (TAU * time / 7.0 + phase).cos();
		state.tilt += shift * 1.6 * m.weight_shift;
		state.legs.bend += shift * 2.0 * m.weight_shift;
		state.legs.feet.0.x -= shift * 4.0 * m.weight_shift;
		state.legs.feet.1.x += shift * 4.0 * m.weight_shift;
	}
}

/// One small piece of idle business, picked by seed and mood, after about 4 s with no
/// direction, speech or reaction — and never while a shouted line is playing.
fn business(heading: f32, seed: u64, state: &mut FigureState, cx: &Cx, time: f32) {
	let dial = cx.motion.business;

	if dial <= 0.0 {
		return;
	}

	// Never during someone's shouted line.
	if cx
		.lines
		.iter()
		.any(|spoken| spoken.at(time).is_some() && spoken.shouted())
	{
		return;
	}

	let activity = cx
		.lines
		.iter()
		.filter(|spoken| spoken.start <= time)
		.map(|spoken| spoken.start + spoken.duration)
		.fold(cx.key_time, f32::max);

	let start = activity + 4.0 + (seed % 3) as f32 * 0.7;
	let length = 1.5;

	if time < start || time >= start + length {
		return;
	}

	if state.lie > 0.5 || state.gait.is_some() {
		return;
	}

	let w = (time - start) / length;
	let bump = (PI * w).sin();
	let mood_index = Mood::ALL
		.iter()
		.position(|(_, mood)| *mood == state.mood)
		.unwrap_or(0);
	let kind = (seed / 7 + mood_index as u64) % 5;

	match kind {
		// Look around.
		0 => {
			state.gaze += (w * 3.0 * PI).sin() * 0.55 * dial;
			state.head_tilt += (w * 2.0 * PI).sin() * 2.5 * dial;
		}
		// Scratch.
		1 => {
			let scratch = pt(
				heading * 10.0 * bump + (w * 30.0).sin() * 6.0,
				-150.0 * bump,
			) * dial;
			state.hands.1 = state.hands.1 + scratch;
			state.head_tilt -= 1.5 * bump * dial;
		}
		// Shift.
		2 => {
			state.tilt -= heading * 2.4 * bump * dial;
			state.legs.feet.0.x += 4.0 * bump * dial;
			state.legs.feet.1.x -= 4.0 * bump * dial;
		}
		// Sigh.
		3 => {
			state.squash *= 1.0 - 0.02 * bump * dial;
			state.mouth_open = (state.mouth_open + 0.2 * bump * dial).min(1.0);
			state.head_tilt -= 1.8 * bump * dial;
		}
		// Adjust stance.
		_ => {
			state.legs.feet.0.x += 5.0 * bump * dial;
			state.legs.feet.1.x -= 5.0 * bump * dial;
			state.legs.crouch += 0.02 * bump * dial;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::art::figure::Build;
	use crate::art::figure::Pose;
	use crate::art::pack::StylePack;
	use crate::cast::template_look;

	/// A standing kid puppet at stage x, with a pose, a mood and a facing.
	fn kid(x: f32, pose: Pose, mood: Mood, facing: f32, gaze: f32) -> Puppet {
		let look = template_look(&StylePack::default_pack(), Build::Kid).unwrap();
		let frame = look.frame();
		let mut state = FigureState::new(&frame);
		state.mood = mood;
		state.hands = frame.hands(pose, facing);
		state.hand_shapes = pose.hand_shapes();
		state.legs = frame.limbs().stance(crate::art::rig::Stance::Stand, facing);
		state.facing = facing;
		state.gaze = gaze;

		Puppet {
			id: "kid".into(),
			kind: Kind::Figure(look),
			at: pt(x, 2060.0),
			scale: 2.25,
			z: 0,
			opacity: 1.0,
			body: Body::Figure(state),
			stink: 0.0,
			mint: 0.0,
			heading: 1.0,
			breath: 0.0,
			lift: 0.0,
			smear: 0.0,
			exerted: 0.0,
		}
	}

	/// An acting context with every dial at its default.
	fn cx() -> Cx<'static> {
		Cx {
			on: true,
			motion: Motion::default(),
			lines: Vec::new(),
			key_time: 0.0,
		}
	}

	/// A context with every acting dial at zero: the layer on, but doing nothing. With
	/// `anticipation` and `overshoot` off too, the distance-scaled ease is the plain ease.
	fn zeroed() -> Cx<'static> {
		Cx {
			on: true,
			motion: Motion {
				anticipation: 0.0,
				overshoot: 0.0,
				lead: 0.0,
				overlap: 0.0,
				arcs: 0.0,
				takes: 0.0,
				breath: 0.0,
				weight_shift: 0.0,
				gesture: 0.0,
				listen: 0.0,
				business: 0.0,
				travel: Travel::Slide,
				..Motion::default()
			},
			lines: Vec::new(),
			key_time: 0.0,
		}
	}

	/// A spoken line with a simple pulse envelope, for the speech rules.
	fn spoken<'a>(
		start: f32,
		duration: f32,
		speakers: &'a [String],
		text: &'a str,
		envelope: &'a [f32],
		track: &'a Track,
	) -> Spoken<'a> {
		Spoken {
			start,
			duration,
			speakers,
			text,
			envelope,
			track,
			peak: peak_of(envelope),
			question: is_question(text),
		}
	}

	#[test]
	fn the_eyes_reach_their_target_before_the_head_starts() {
		let cx = cx();
		let old = kid(1200.0, Pose::Idle, Mood::Happy, -0.5, -0.8);
		let new = kid(1200.0, Pose::Idle, Mood::Happy, 0.5, 0.8);

		// Two frames in, the eyes have arrived while the head has not started.
		let mid = tween(&old, &new, 0.09, 0.15, false, &cx);
		let Body::Figure(state) = &mid.body else {
			panic!("kid is a figure");
		};

		assert!(
			(state.gaze - 0.8).abs() < 1e-4,
			"the eyes arrive within two frames: {}",
			state.gaze
		);
		assert!(
			(state.facing - (-0.5)).abs() < 0.01,
			"the head has barely started: {}",
			state.facing
		);

		// A turn this big plays under a covering blink.
		assert!(
			state.blink > 0.2,
			"a blink covers the turn: {}",
			state.blink
		);

		// Before the eyes are done, the head has not moved at all.
		let early = tween(&old, &new, 0.075, 0.125, false, &cx);
		let Body::Figure(early) = &early.body else {
			panic!("kid is a figure");
		};

		assert!(
			(early.facing - (-0.5)).abs() < 1e-5,
			"the head waits for the eyes: {}",
			early.facing
		);
		assert!(
			early.gaze > 0.65,
			"the eyes are nearly there first: {}",
			early.gaze
		);
	}

	#[test]
	fn the_far_hand_arrives_after_the_near_one() {
		let cx = cx();
		let old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let new = kid(1200.0, Pose::Point, Mood::Cross, 0.35, 0.4);

		// Pointing sends the right hand far and leaves the left near its target.
		let d0 = (new_body(&new).hands.0 - new_body(&old).hands.0).len();
		let d1 = (new_body(&new).hands.1 - new_body(&old).hands.1).len();
		assert!(
			d1 > d0 * 2.0,
			"the right hand travels further: {d0} vs {d1}"
		);

		let mid = tween(&old, &new, 0.4, 0.66, false, &cx);
		let Body::Figure(state) = &mid.body else {
			panic!("kid is a figure");
		};
		let target = new_body(&new);

		let near_left = 1.0 - (target.hands.0 - state.hands.0).len() / d0.max(1e-3);
		let far_right = 1.0 - (target.hands.1 - state.hands.1).len() / d1.max(1e-3);

		assert!(
			far_right < near_left,
			"the far hand ({far_right}) trails the near one ({near_left})"
		);
	}

	fn new_body(puppet: &Puppet) -> &FigureState {
		let Body::Figure(state) = &puppet.body else {
			panic!("kid is a figure");
		};

		state
	}

	#[test]
	fn a_hand_between_two_poses_swing_on_an_arc() {
		let cx = cx();
		let mut old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let mut new = kid(1200.0, Pose::ReachOut, Mood::Worried, 0.35, 0.4);
		let look = match &new.kind {
			Kind::Figure(look) => look.clone(),
			_ => panic!("kid is a figure"),
		};
		let shoulder = look.frame().shoulder(1.0);

		// Put the right hand 140 units from the shoulder in both poses, 80 degrees apart, so
		// an arc holds its reach all the way while a straight slide cuts the chord.
		let reach = 140.0;
		let angle = |degrees: f32| pt(degrees.to_radians().cos(), -degrees.to_radians().sin());
		new_body_mut(&mut old).hands.1 = shoulder + angle(20.0) * reach;
		new_body_mut(&mut new).hands.1 = shoulder + angle(100.0) * reach;

		let acted = tween(&old, &new, 0.3, 0.5, false, &cx);
		let Body::Figure(state) = &acted.body else {
			panic!("kid is a figure");
		};
		let swung = (state.hands.1 - shoulder).len();
		// The straight chord between two points 80 degrees apart at reach 140 dips to
		// 140·cos(40°) ≈ 107 at its middle; the arc must keep the hand at the reach.
		let chord = 140.0 * 40.0_f32.to_radians().cos();

		assert!(
			(swung - reach).abs() < 4.0,
			"the arc keeps its reach at the midpoint: {swung} against {reach}"
		);
		assert!(
			swung > chord * 1.1,
			"the hand bows away from the shoulder instead of cutting across: {swung} against \
			 {chord}"
		);
	}

	fn new_body_mut(puppet: &mut Puppet) -> &mut FigureState {
		let Body::Figure(state) = &mut puppet.body else {
			panic!("kid is a figure");
		};

		state
	}

	#[test]
	fn a_takes_squash_precedes_its_stretch() {
		let cx = cx();
		let old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let new = kid(1200.0, Pose::Idle, Mood::Shocked, 0.35, 0.4);

		let down = tween(&old, &new, 0.06, 0.1, false, &cx);
		let up = tween(&old, &new, 0.24, 0.4, false, &cx);
		let Body::Figure(down) = &down.body else {
			panic!("kid is a figure");
		};
		let Body::Figure(up) = &up.body else {
			panic!("kid is a figure");
		};

		assert!(
			down.squash < 1.0,
			"the take squashes first: {}",
			down.squash
		);
		assert!(
			up.squash > 1.0,
			"then stretches past the pose: {}",
			up.squash
		);
	}

	#[test]
	fn a_mood_change_to_laugh_bounces_and_to_sad_sinks() {
		let cx = cx();
		let old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);

		let laugh = kid(1200.0, Pose::Idle, Mood::Laugh, 0.35, 0.4);
		let bounced = tween(&old, &laugh, 0.15, 0.25, false, &cx);
		let sad = kid(1200.0, Pose::Idle, Mood::Sad, 0.35, 0.4);
		let sunk = tween(&old, &sad, 0.5, 0.83, false, &cx);
		let late = tween(&old, &sad, 1.4, 1.0, false, &cx);
		let Body::Figure(bounced) = &bounced.body else {
			panic!("kid is a figure");
		};
		let Body::Figure(sunk) = &sunk.body else {
			panic!("kid is a figure");
		};
		let Body::Figure(late) = &late.body else {
			panic!("kid is a figure");
		};

		assert!(
			(bounced.squash - 1.0).abs() > 1e-4,
			"laughter bounces: {}",
			bounced.squash
		);
		assert!(sunk.squash < 1.0, "sadness sinks: {}", sunk.squash);
		assert!(
			late.squash > sunk.squash,
			"the sink lifts back off, with no undershoot below"
		);
	}

	#[test]
	fn breath_phase_differs_between_two_actors() {
		let cx = cx();
		let mut a = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		a.id = "kid".into();
		let mut b = kid(1600.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		b.id = "mum".into();

		let breath_of = |puppet: &mut Puppet| {
			animate(puppet, &cx, 5.0, None);
			new_body(puppet).squash
		};
		let (a, b) = (breath_of(&mut a), breath_of(&mut b));

		assert!(
			(a - b).abs() > 1e-4,
			"two seeds breathe on different phases: {a} against {b}"
		);
	}

	/// A planted foot's stage x drift over its stance, for a walk of `distance` in `duration`.
	fn planted_foot_drift(distance: f32, duration: f32) -> f32 {
		let old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let new = kid(1200.0 + distance, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let cx = cx();
		let mut plan = plan_between(&old, &new, &cx).expect("a long crossing walks");
		let look = match &new.kind {
			Kind::Figure(look) => look.clone(),
			_ => panic!("kid is a figure"),
		};

		// The plan on its intended duration, with the stride re-solved for it.
		plan.duration = duration;
		let leg = look.frame().limbs().leg_length();
		plan.stride = (PI * distance.abs() / (2.0 * leg * plan.scale * plan.omega * duration))
			.clamp(0.05, 0.95);

		let mut worst = 0.0_f32;
		let steps = 400;
		let mut planted = [Option::<f32>::None; 2];

		for step in 0..steps {
			let since = step as f32 / steps as f32 * duration;
			let mut walked = old.clone();
			walk(&mut walked, &new, &plan, since);
			let Body::Figure(state) = &walked.body else {
				panic!("kid is a figure");
			};
			let feet = [state.legs.feet.0, state.legs.feet.1];

			for (index, psi) in [
				plan.psi0 + plan.omega * duration * plan.profile(since / duration),
				plan.psi0 + PI + plan.omega * duration * plan.profile(since / duration),
			]
			.into_iter()
			.enumerate()
			{
				if psi.cos() <= 0.0 {
					let stage_x = walked.at.x + feet[index].x * plan.scale;

					match planted[index] {
						Some(first) => {
							worst = worst.max((stage_x - first).abs());
						}
						None => planted[index] = Some(stage_x),
					}
				} else {
					planted[index] = None;
				}
			}
		}

		worst
	}

	#[test]
	fn planted_feet_hold_still_across_the_stage() {
		for (distance, duration) in [(600.0, 0.9), (1200.0, 1.4), (400.0, 0.7)] {
			let drift = planted_foot_drift(distance, duration);

			assert!(
				drift < 1.0,
				"a planted foot held within {drift} stage units over {distance} in {duration}"
			);
		}
	}

	#[test]
	fn every_dial_at_zero_returns_the_input_unchanged() {
		let old = kid(900.0, Pose::Idle, Mood::Happy, -0.4, -0.7);
		let new = kid(1500.0, Pose::Point, Mood::Cross, 0.5, 0.8);
		let cx = zeroed();

		for step in 0..13 {
			let since = step as f32 / 12.0 * TWEEN;
			let acted = tween(&old, &new, since, since / TWEEN, false, &cx);
			let plain = crate::compose::blend(&old, &new, cx.motion.ease(since / TWEEN));
			let Body::Figure(a) = &acted.body else {
				panic!("kid is a figure");
			};
			let Body::Figure(b) = &plain.body else {
				panic!("kid is a figure");
			};

			assert_eq!(a.hands, b.hands, "hands at {since}");
			assert_eq!(a.legs, b.legs, "legs at {since}");
			assert_eq!(a.facing, b.facing, "facing at {since}");
			assert_eq!(a.gaze, b.gaze, "gaze at {since}");
			assert_eq!(a.squash, b.squash, "squash at {since}");
			assert_eq!(acted.at, plain.at, "position at {since}");
		}
	}

	#[test]
	fn walking_gives_way_to_sliding_when_the_dial_says_slide() {
		let old = kid(900.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let new = kid(1900.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let cx = zeroed();

		assert!(
			plan_between(&old, &new, &cx).is_none(),
			"slide does not walk"
		);

		let acted = tween(&old, &new, 0.3, 0.5, false, &cx);
		assert!(
			(acted.at.x - lerp(900.0, 1900.0, 0.5)).abs() < 1e-3,
			"a slide covers half the distance halfway"
		);
	}

	#[test]
	fn brows_answer_the_line() {
		let envelope = [0.0, 0.1, 0.9, 0.2, 0.0, 0.1, 0.4, 0.1, 0.0];
		let speakers = vec!["kid".to_owned()];
		let track = Track::default();
		let ask = spoken(1.0, 0.4, &speakers, "Where did it go?", &envelope, &track);
		let tell = spoken(1.0, 0.4, &speakers, "It went there.", &envelope, &track);
		let mut asking = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let mut telling = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);

		let mut ask_cx = cx();
		ask_cx.lines = vec![ask];
		let mut tell_cx = cx();
		tell_cx.lines = vec![tell];

		animate(&mut asking, &ask_cx, 1.1, None);
		animate(&mut telling, &tell_cx, 1.1, None);

		assert!(
			new_body(&asking).brow_lift > new_body(&telling).brow_lift,
			"a question lifts the brows more: {} against {}",
			new_body(&asking).brow_lift,
			new_body(&telling).brow_lift
		);
	}

	#[test]
	fn a_speakers_hand_beats_on_the_strongest_stress() {
		let envelope = [0.0, 0.1, 0.2, 0.1, 0.0, 0.1, 1.0, 0.1, 0.0, 0.0];
		let speakers = vec!["kid".to_owned()];
		let track = Track::default();
		let line = spoken(0.0, 0.4, &speakers, "Give it here.", &envelope, &track);
		let mut cx = cx();
		cx.lines = vec![line];
		let peak = cx.lines[0].peak;
		let mut on = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let mut off = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);

		animate(&mut on, &cx, peak + 0.02, None);
		animate(&mut off, &cx, peak + 0.3, None);

		assert_ne!(
			new_body(&on).hands,
			new_body(&off).hands,
			"the leading hand beats on the strongest stress"
		);
	}

	#[test]
	fn listeners_nod_flinch_and_glance() {
		let quiet = [0.2; 220]; // a 3.6 s line of steady talk
		let speakers = vec!["mum".to_owned()];
		let track = Track::default();
		let line = spoken(0.0, 3.6, &speakers, "A long line.", &quiet, &track);
		let shout = spoken(0.0, 3.6, &speakers, "**LISTEN**.", &quiet, &track);
		let mut cx = cx();
		cx.lines = vec![line];
		let speaker = Speaker {
			id: "mum".into(),
			x: 1600.0,
			heading: 1.0,
			pointing: true,
		};

		let mut listener = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		animate(&mut listener, &cx, 2.0, Some(&speaker));
		let nod = new_body(&listener).head_tilt;

		let mut plain = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let plain_cx = Cx::plain(Motion::default());
		animate(&mut plain, &plain_cx, 2.0, Some(&speaker));

		assert!(
			(nod - new_body(&plain).head_tilt).abs() > 1e-4,
			"a long line earns a nod: {nod}"
		);
		assert!(
			(new_body(&listener).gaze - 0.9).abs() < 1e-3,
			"a pointer's audience glances where they point: {}",
			new_body(&listener).gaze
		);

		// The flinch, with every other dial quiet so it is the only thing that can move.
		let mut still = cx.motion;
		still.breath = 0.0;
		still.weight_shift = 0.0;
		still.gesture = 0.0;
		still.business = 0.0;
		cx.motion = still;
		cx.lines = vec![shout];
		let mut jumpy = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		animate(&mut jumpy, &cx, 0.1, Some(&speaker));
		let early = new_body(&jumpy).squash;
		animate(&mut jumpy, &cx, 0.22, Some(&speaker));
		let late = new_body(&jumpy).squash;

		assert!(
			(early - 1.0).abs() < 1e-6 && (late - 1.0).abs() > 1e-4,
			"the flinch lands after a beat, not with the shout: at 0.1 s {early}, at 0.22 s {late}"
		);
	}

	#[test]
	fn no_business_during_a_line_or_soon_after_a_direction() {
		let envelope = [0.3; 30];
		let speakers = vec!["mum".to_owned()];
		let track = Track::default();
		let line = spoken(10.0, 0.5, &speakers, "Something.", &envelope, &track);
		let shout = spoken(10.0, 0.5, &speakers, "**HEY**.", &envelope, &track);

		let fresh = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let mut cx = cx();
		cx.key_time = 12.0;
		cx.lines = vec![line];

		// One second after a direction (and after the line), nothing stirs yet.
		let mut bored = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		animate(&mut bored, &cx, 13.0, None);

		assert_eq!(new_body(&bored).hands, new_body(&fresh).hands);
		assert_eq!(new_body(&bored).gaze, new_body(&fresh).gaze);

		// Somewhere in the quiet after four seconds, one small piece of business plays.
		let mut fidgeted = false;

		for tick in 1..40 {
			let mut restless = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
			animate(&mut restless, &cx, 16.0 + tick as f32 * 0.08, None);

			if new_body(&restless).hands != new_body(&fresh).hands
				|| (new_body(&restless).gaze - new_body(&fresh).gaze).abs() > 1e-4
				|| (new_body(&restless).tilt - new_body(&fresh).tilt).abs() > 1e-4
			{
				fidgeted = true;
			}
		}

		assert!(fidgeted, "held long enough, the figure fidgets");

		// While a shout is playing, nobody fidgets.
		cx.lines = vec![shout];
		cx.key_time = 0.0;
		let mut polite = kid(800.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		animate(&mut polite, &cx, 10.2, None);

		assert_eq!(new_body(&polite).hands, new_body(&fresh).hands);
		assert_eq!(new_body(&polite).gaze, new_body(&fresh).gaze);
	}

	#[test]
	fn walk_travel_turns_to_face_the_way_it_goes() {
		let old = kid(700.0, Pose::Idle, Mood::Happy, -0.35, -0.4);
		let new = kid(1700.0, Pose::Idle, Mood::Happy, -0.35, -0.4);
		let cx = cx();
		let plan = plan_between(&old, &new, &cx).expect("a crossing walks");

		let mut walked = old.clone();
		walk(&mut walked, &new, &plan, plan.duration * 0.4);
		let Body::Figure(mid) = &walked.body else {
			panic!("kid is a figure");
		};

		assert!(
			mid.facing > 0.5,
			"facing the way it is going: {}",
			mid.facing
		);
		assert!(walked.at.x > 700.0 && walked.at.x < 1700.0, "travelling");

		let mut done = old.clone();
		walk(&mut done, &new, &plan, plan.duration + FOOT_SETTLE);
		let Body::Figure(end) = &done.body else {
			panic!("kid is a figure");
		};

		assert!(
			(end.facing - (-0.35)).abs() < 1e-4,
			"turned back to its staged facing: {}",
			end.facing
		);
		assert_eq!(end.legs.feet, new_body(&new).legs.feet, "feet together");
	}

	#[test]
	fn a_held_pose_settles_with_an_after_beat() {
		let old = kid(1200.0, Pose::Idle, Mood::Happy, 0.35, 0.4);
		let new = kid(1200.0, Pose::Point, Mood::Cross, 0.35, 0.4);
		let cx = cx();

		let mid = tween(&old, &new, 0.3, 0.5, false, &cx);
		let after = tween(&old, &new, 0.9, 1.0, false, &cx);
		let Body::Figure(mid) = &mid.body else {
			panic!("kid is a figure");
		};
		let Body::Figure(after) = &after.body else {
			panic!("kid is a figure");
		};

		assert!(
			(mid.squash - 1.0).abs() < 0.02,
			"mid-move, no settle beat yet"
		);
		assert!(
			(after.squash - 1.0).abs() > 0.0,
			"a held pose settles with a small after-beat"
		);
	}
}
