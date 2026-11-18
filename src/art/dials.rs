//! A style's dials: how its lines are inked, how it shades, and how it moves.
//!
//! With the construction a style pack already has (bodies, features, eyes, kit), these make a
//! style four dials, each a table in `style.toml`:
//!
//! ```toml
//! [line]
//! stroke = "brush"        # brush (thick-and-thin), uniform, broken, none, or cut: a white
//!                         # cut edge and a hard drop shadow, as card lifted off the page
//! colour = "ink"          # ink, or self: each part's own fill, darkened
//! register = [0.0, 0.0]   # fills off-register from their lines
//!
//! [shade]
//! tones = 1               # 0, 1 or 2 tones of cel shadow
//! highlight = "none"      # none, or rim: a light edge on the lit side
//! fill = "flat"           # flat, palette (the colours below), screentone, or silhouette:
//!                         # every part ink, with the eyes, mouth and pin joints cut out
//! tone = "dots"           # screentone as dots or as line tone
//! palette = []            # the colours a limited palette snaps to
//!
//! [motion]
//! timing = "ones"         # ones, twos or threes
//! modulate = false        # fast moves drop to ones, holds go to threes
//! easing = "eased"        # eased (with anticipation and overshoot), or snap
//! anticipation = 1.0
//! overshoot = 1.0
//! squash = 1.0
//! moving-hold = 0.0       # a slight drift while a pose is held
//! smears = 0.0            # speed lines behind fast moves
//! limbs = "jointed"       # jointed (IK), or hose: one smooth curve, no elbows or knees
//! follow-through = 1.0
//! blink-every = 3.0       # seconds between blinks
//! mouth-rate = 0.0        # mouth shapes a second (0 follows the voice)
//! idle = "none"           # none, or bounce: a loop while standing
//! idle-amount = 1.0
//! idle-tempo = 120.0      # beats a minute
//!
//! lead = 1.0              # eyes lead, the head follows, the body follows
//! overlap = 1.0           # parts arrive one after another, not together
//! arcs = 1.0              # hands swing on arcs about the shoulder, heads nod through turns
//! takes = 1.0             # mood changes play as takes: squash, stretch, settle
//! hang = 1.0              # seconds a strong mood holds its peak before it relaxes (0 never relaxes)
//! breath = 1.0            # a standing figure breathes
//! weight-shift = 1.0      # and shifts its weight from foot to foot
//! gesture = 1.0           # speakers beat time with head and hand, brows answer the line
//! listen = 1.0            # listeners nod, glance where the speaker points, flinch at shouts
//! business = 1.0          # a figure held too long does one small piece of idle business
//! travel = "walk"         # walk, or slide: x changes carry the figure on its own feet
//! ```
//!
//! Every default is the cartoon look as it was before the dials existed, so a style only says
//! what it changes. Line weights stay in `[finish]`: `line` outside, `detail` inside. The
//! rules the acting dials drive live in [`acting`](crate::video::acting); a script's
//! `[video] acting = "plain"` turns the whole layer off.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

use super::Color;
use super::Pt;
use super::pt;

/// How a line is inked.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Stroke {
	/// Thick-and-thin: a heavier pass swells the line on the shadow side.
	#[default]
	Brush,
	/// One even weight.
	Uniform,
	/// Even, and broken into long dashes, as a quick pen leaves it.
	Broken,
	/// No lines: shapes meet edge to edge.
	None,
	/// No lines: every part is a piece of card with a white cut edge, lifted off the page so
	/// it drops a hard shadow on whatever is under it.
	Cut,
}

/// What colour lines are.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LineColour {
	#[default]
	Ink,
	/// Each part's own fill, darkened, as much modern TV animation does.
	#[serde(rename = "self")]
	SelfColoured,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Line {
	pub stroke: Stroke,
	pub colour: LineColour,
	/// How far fills sit from their lines, in figure units.
	pub register: Pt,
}

impl Default for Line {
	fn default() -> Line {
		Line {
			stroke: Stroke::Brush,
			colour: LineColour::Ink,
			register: pt(0.0, 0.0),
		}
	}
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Highlight {
	#[default]
	None,
	/// A light band along the lit edge.
	Rim,
}

/// How colour is laid down.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Fill {
	#[default]
	Flat,
	/// Every colour snaps to the nearest in the style's palette.
	Palette,
	/// Black ink on white paper, with colours as screentone.
	Screentone,
	/// Shadow theatre: every part of a figure is one ink, with the eyes and mouth cut out
	/// of it and a pin showing at every joint.
	Silhouette,
}

/// The pattern screentone is cut from.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
	#[default]
	Dots,
	/// Parallel lines, darker tones closer together.
	Lines,
}

/// A small, fixed set of colours, kept inline so styles stay cheap to copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Palette {
	colors: [Option<Color>; Palette::MAX],
}

impl Palette {
	pub const MAX: usize = 16;

	pub fn colors(&self) -> impl Iterator<Item = Color> + '_ {
		self.colors.iter().flatten().copied()
	}

	pub fn is_empty(&self) -> bool {
		self.colors[0].is_none()
	}

	/// The palette colour nearest `color`, or `color` itself if the palette is empty.
	pub fn nearest(&self, color: Color) -> Color {
		let distance = |a: Color, b: Color| {
			let ((r1, g1, b1), (r2, g2, b2)) = (a.rgb(), b.rgb());
			let d = |x: u8, y: u8| (f32::from(x) - f32::from(y)).powi(2);
			// Weighted towards green, as eyes are.
			d(r1, r2) * 0.3 + d(g1, g2) * 0.59 + d(b1, b2) * 0.11
		};

		self.colors()
			.min_by(|a, b| distance(*a, color).total_cmp(&distance(*b, color)))
			.unwrap_or(color)
	}
}

impl<'de> Deserialize<'de> for Palette {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Palette, D::Error> {
		let colors = Vec::<Color>::deserialize(deserializer)?;

		if colors.len() > Palette::MAX {
			return Err(serde::de::Error::custom(format!(
				"a palette has at most {} colours",
				Palette::MAX
			)));
		}

		let mut palette = Palette::default();

		for (slot, color) in palette.colors.iter_mut().zip(colors) {
			*slot = Some(color);
		}

		Ok(palette)
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Shade {
	/// Tones of cel shadow: 0 (flat), 1 or 2.
	pub tones: u8,
	pub highlight: Highlight,
	pub fill: Fill,
	pub tone: Tone,
	pub palette: Palette,
}

impl Default for Shade {
	fn default() -> Shade {
		Shade {
			tones: 1,
			highlight: Highlight::None,
			fill: Fill::Flat,
			tone: Tone::Dots,
			palette: Palette::default(),
		}
	}
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Timing {
	#[default]
	Ones,
	Twos,
	Threes,
}

impl Timing {
	/// New drawings a second, at 24 frames a second.
	pub fn drawings(self) -> f32 {
		match self {
			Timing::Ones => 24.0,
			Timing::Twos => 12.0,
			Timing::Threes => 8.0,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Easing {
	/// Ease in and out, winding up before a move and overshooting after it.
	#[default]
	Eased,
	/// Hold one pose, then snap to the next.
	Snap,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Joints {
	/// Upper and lower bones with elbows and knees, posed by IK.
	#[default]
	Jointed,
	/// One smooth curve from shoulder to hand and hip to foot.
	Hose,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Idle {
	#[default]
	None,
	/// Everyone standing bobs to a beat.
	Bounce,
}

/// How a figure crosses the stage when its `x` changes between stage directions.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Travel {
	/// It walks (or runs) there on its own feet.
	#[default]
	Walk,
	/// It slides, as cut-outs did before rigs could walk.
	Slide,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Motion {
	pub timing: Timing,
	/// Fast moves drop to ones and holds go to threes, whatever the timing.
	pub modulate: bool,
	pub easing: Easing,
	/// How far a move winds up before it goes, and overshoots after: 1 is the cartoon's.
	pub anticipation: f32,
	pub overshoot: f32,
	/// How much hops squash and stretch: 1 is the cartoon's.
	pub squash: f32,
	/// A slight drift while a pose is held, in degrees of lean.
	pub moving_hold: f32,
	/// Speed lines behind fast moves: 0 for none, 1 for plenty.
	pub smears: f32,
	pub limbs: Joints,
	/// How much hair and clothes carry on after a move: 1 is the cartoon's.
	pub follow_through: f32,
	/// Seconds between blinks (each character varies a little).
	pub blink_every: f32,
	/// How many times a second the mouth changes shape while talking; 0 follows the voice.
	pub mouth_rate: f32,
	pub idle: Idle,
	pub idle_amount: f32,
	/// Beats a minute for the idle loop.
	pub idle_tempo: f32,
	/// The performance layer's dials (see [`acting`](crate::video::acting)): each scales one
	/// rule animators use, 1 the cartoon's way, 0 off.
	pub lead: f32,
	pub overlap: f32,
	pub arcs: f32,
	pub takes: f32,
	/// Seconds a strong mood holds its peak before it relaxes; 0 holds the peak for good.
	pub hang: f32,
	pub breath: f32,
	pub weight_shift: f32,
	pub gesture: f32,
	pub listen: f32,
	pub business: f32,
	pub travel: Travel,
}

impl Default for Motion {
	fn default() -> Motion {
		Motion {
			timing: Timing::Ones,
			modulate: false,
			easing: Easing::Eased,
			anticipation: 1.0,
			overshoot: 1.0,
			squash: 1.0,
			moving_hold: 0.0,
			smears: 0.0,
			limbs: Joints::Jointed,
			follow_through: 1.0,
			blink_every: 3.0,
			mouth_rate: 0.0,
			idle: Idle::None,
			idle_amount: 1.0,
			idle_tempo: 120.0,
			lead: 1.0,
			overlap: 1.0,
			arcs: 1.0,
			takes: 1.0,
			hang: 1.0,
			breath: 1.0,
			weight_shift: 1.0,
			gesture: 1.0,
			listen: 1.0,
			business: 1.0,
			travel: Travel::Walk,
		}
	}
}

impl Motion {
	/// Progress through a move, `t` from 0 to 1: eased with a wind-up and an overshoot (two of
	/// the classic principles of animation), or held and then snapped.
	pub fn ease(&self, t: f32) -> f32 {
		if self.easing == Easing::Snap {
			return if t > 0.0 { 1.0 } else { 0.0 };
		}

		if t < 0.5 {
			let c = 1.1 * 1.525 * self.anticipation;
			let x = 2.0 * t;
			x * x * ((c + 1.0) * x - c) / 2.0
		} else {
			let c = 1.1 * 1.525 * self.overshoot;
			let x = 2.0 * t - 2.0;
			f32::midpoint(x * x * ((c + 1.0) * x + c), 2.0)
		}
	}

	/// The same ease with the wind-up and overshoot scaled by how big the change is: a
	/// blink-sized change (`size` 0) goes without one, a jump or a point (`size` 1) gets the
	/// most. The `anticipation` and `overshoot` dials keep their meaning and scale these.
	pub fn ease_by_distance(&self, t: f32, size: f32) -> f32 {
		if self.easing == Easing::Snap {
			return if t > 0.0 { 1.0 } else { 0.0 };
		}

		let size = size.clamp(0.0, 1.0);

		if t < 0.5 {
			let c = 1.1 * 1.525 * self.anticipation * size;
			let x = 2.0 * t;
			x * x * ((c + 1.0) * x - c) / 2.0
		} else {
			let c = 1.1 * 1.525 * self.overshoot * size;
			let x = 2.0 * t - 2.0;
			f32::midpoint(x * x * ((c + 1.0) * x + c), 2.0)
		}
	}

	/// The moment characters are drawn at. On twos or threes they hold each drawing for two or
	/// three frames (never reaching back before `start`, where the shot began); with
	/// modulation, moves are drawn on ones and holds on threes. The camera always moves on ones.
	pub fn drawing_time(&self, time: f32, start: f32, moving: bool) -> f32 {
		let timing = match (self.modulate, moving) {
			(true, true) => Timing::Ones,
			(true, false) => Timing::Threes,
			(false, _) => self.timing,
		};

		if timing == Timing::Ones {
			return time;
		}

		let drawings = timing.drawings();
		((time * drawings).floor() / drawings).max(start)
	}

	/// The moment the mouth is drawn at, `t` seconds into a line: held between flaps when the
	/// style limits them.
	pub fn mouth_time(&self, t: f32) -> f32 {
		if self.mouth_rate <= 0.0 {
			return t;
		}

		(t * self.mouth_rate).floor() / self.mouth_rate
	}

	/// The idle loop's lift (in figure units) and squash at `time`.
	pub fn idle_at(&self, time: f32, seed: u64) -> (f32, f32) {
		if self.idle == Idle::None {
			return (0.0, 1.0);
		}

		let beat =
			(time * self.idle_tempo / 60.0 + (seed % 7) as f32 * 0.05) * std::f32::consts::PI;
		let up = beat.sin().abs();
		(
			up * 18.0 * self.idle_amount,
			1.0 + (0.5 - up) * 0.08 * self.idle_amount,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_default_ease_winds_up_and_overshoots() {
		let motion = Motion::default();
		assert!(motion.ease(0.1) < 0.0, "a wind-up before the move");
		assert!(motion.ease(0.9) > 1.0, "an overshoot after it");
		assert!((motion.ease(1.0) - 1.0).abs() < 1e-6);

		let snap = Motion {
			easing: Easing::Snap,
			..Motion::default()
		};
		assert_eq!(snap.ease(0.01), 1.0);
	}

	#[test]
	fn small_changes_get_no_wind_up_and_big_ones_get_the_most() {
		let motion = Motion::default();
		let depth = |size: f32| {
			(0..100)
				.map(|step| motion.ease_by_distance(step as f32 / 99.0, size))
				.fold(f32::INFINITY, f32::min)
		};

		assert!(
			depth(0.0).abs() < 1e-5,
			"a blink-sized change never winds up"
		);
		assert!(depth(0.5) < depth(0.0), "a middling change dips");
		assert!(depth(1.0) < depth(0.5), "a big change winds up hardest");
		assert!(motion.ease_by_distance(0.0, 0.7).abs() < 1e-5);
		assert!(
			(motion.ease_by_distance(1.0, 0.7) - 1.0).abs() < 1e-5,
			"every size still lands on target"
		);
	}

	#[test]
	fn timing_holds_drawings_and_modulation_follows_the_action() {
		let frame = 1.0 / 24.0;
		let threes = Motion {
			timing: Timing::Threes,
			..Motion::default()
		};
		let held = threes.drawing_time(1.0, 0.0, false);
		assert_eq!(threes.drawing_time(1.0 + frame * 2.0, 0.0, false), held);
		assert!(threes.drawing_time(1.0 + frame * 3.1, 0.0, false) > held);

		let modulated = Motion {
			timing: Timing::Twos,
			modulate: true,
			..Motion::default()
		};
		assert_eq!(
			modulated.drawing_time(1.03, 0.0, true),
			1.03,
			"moves on ones"
		);
		assert_eq!(
			modulated.drawing_time(1.03, 0.0, false),
			1.0,
			"holds on threes"
		);
		assert_eq!(Motion::default().drawing_time(1.03, 0.0, false), 1.03);
	}

	#[test]
	fn a_palette_snaps_colours() {
		let palette: Palette = toml::from_str::<Shade>("palette = [\"#ff0000\", \"#0000ff\"]")
			.unwrap()
			.palette;
		assert_eq!(palette.nearest(Color(0xEE_22_11)), Color(0xFF_00_00));
		assert_eq!(
			Palette::default().nearest(Color(0x12_34_56)),
			Color(0x12_34_56)
		);
	}
}
