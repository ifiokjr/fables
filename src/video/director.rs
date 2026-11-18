//! The director: what the camera frames in every shot, decided once when the timeline is built.
//!
//! Three sources of camera work, behind `[video] direction`:
//!
//! - `locked` reproduces the older camera exactly: one auto-framed view per scene key and line,
//!   gliding between them.
//! - A page with an `@camera` direction is shot exactly as written: each camera state along the
//!   page becomes a setup.
//! - `auto` (the default) shots pages without one by film rules: an establishing shot to open,
//!   a closer shot on whoever speaks, reaction shots on strong moods, wide shots for action,
//!   and no frantic cutting.
//!
//! A setup is data: when it starts, what it frames and how the camera travels there. The frame
//! renderer turns setups into views; nothing here renders.

use anyhow::Result;
use anyhow::bail;

use crate::art::Bounds;
use crate::art::Pt;
use crate::art::STAGE;
use crate::art::figure::Mood;
use crate::art::lerp;
use crate::art::pt;
use crate::cast::Kind;
use crate::compose::Body;
use crate::compose::Extents;
use crate::compose::Puppet;
use crate::stage::Camera;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::stage::Travel;
use crate::story::Direction;
use crate::story::Story;
use crate::story::TextPlace;
use crate::video::timeline::Framing;
use crate::video::timeline::Setup;
use crate::video::timeline::Shot;

/// How long the camera takes to reframe when it glides.
pub(crate) const CAMERA_EASE: f32 = 0.9;
/// How far ahead of a line or direction the camera starts moving.
pub(crate) const CAMERA_LEAD: f32 = 0.45;
/// Stage area drawn past the edges, which the camera may show.
pub(crate) const OVERDRAW: f32 = 150.0;

/// The shortest shot any director will make. A setup that would run shorter is merged into
/// its neighbour, except a cut the script wrote, which is honoured and reported as short.
pub(crate) const MIN_SHOT: f32 = 1.2;
/// No more than one cut per this many seconds.
const MIN_CUT_GAP: f32 = 1.2;
/// A line shorter than this plays over a two-shot, so the cutting never gets frantic.
const SHORT_LINE: f32 = 1.2;
/// The establishing shot that opens a page lasts at least this long.
const ESTABLISH: f32 = 1.5;
/// The same framing never holds longer than this without at least a slow push.
const LONG_HOLD: f32 = 8.0;
/// A cut never lands within this time after a line begins.
const LINE_GUARD: f32 = 0.2;
/// A cut the guard moves lands this far before the line instead.
const BEFORE_LINE: f32 = 0.25;
/// How far a right- or left-facing subject sits off centre, as a fraction of the view width.
const LEAD_ROOM: f32 = 0.12;
/// Headroom above the highest point of the head, hair and all: at least this fraction of the
/// frame height. An extreme close-up may crop hair above the brow line instead.
const HEADROOM: f32 = 0.06;
/// Room beside a subject, at least this fraction of the frame width on each side.
const SIDE_MARGIN: f32 = 0.05;
/// A long hold rides in on a slow push that tightens the view by up to this fraction of its
/// height — half of it off each edge. The framing leaves that much room, so the headroom
/// and side margins a shot is built with still hold once the push has run its course.
const PUSH_TIGHTENS: f32 = 0.04;
/// How much a `push` tightens and a `pull` widens, as a multiplier of the view height.
const PUSH_IN: f32 = 0.75;
const PULL_OUT: f32 = 1.35;
/// How far a pan travels, as a fraction of the view width.
const PAN: f32 = 0.3;
/// How far a tilt travels, as a fraction of the view height.
const TILT: f32 = 0.25;
/// How far past the frame's bottom edge a tight shot crops, as a fraction of the view height.
const CROP: f32 = 0.08;
/// A small margin between the page's text card and the part of the page a still may frame
/// its subject into.
const CARD_MARGIN: f32 = 90.0;

/// A camera view: centre and height in stage units (width follows from the aspect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
	pub centre: Pt,
	pub height: f32,
}

impl View {
	#[must_use]
	pub fn blend(self, other: View, t: f32) -> View {
		View {
			centre: self.centre.lerp(other.centre, t),
			height: lerp(self.height, other.height, t),
		}
	}

	/// The view's width on a format of `aspect`.
	pub fn width(self, aspect: f32) -> f32 {
		self.height * aspect
	}
}

/// The tallest view that stays on drawn stage in both directions.
pub(crate) fn tallest_view(aspect: f32) -> f32 {
	let drawn = STAGE + OVERDRAW * 2.0;
	drawn.min(drawn / aspect)
}

/// Keeps the view on drawn stage.
pub(crate) fn clamp(aspect: f32, view: View) -> View {
	let height = view.height.min(tallest_view(aspect));
	let width = height * aspect;
	let half_w = width / 2.0;
	let half_h = height / 2.0;
	let x = view
		.centre
		.x
		.clamp(-OVERDRAW + half_w, STAGE + OVERDRAW - half_w);
	let y = view
		.centre
		.y
		.clamp(-OVERDRAW + half_h, STAGE + OVERDRAW - half_h);

	View {
		centre: pt(x, y),
		height,
	}
}

/// The view a legacy `zoom=` and `look=` frames: a focus point, as fractions of the stage.
pub(crate) fn zoom_view(zoom: f32, look: (f32, f32), aspect: f32) -> View {
	let side = STAGE / zoom;

	clamp(
		aspect,
		View {
			centre: pt(look.0 * STAGE, look.1 * STAGE),
			height: if aspect > 1.0 {
				side * 0.75
			} else {
				side * 1.25
			},
		},
	)
}

/// Frames a shot size on its subjects from where the puppets actually stand: headroom above,
/// lead room for a subject facing sideways, and everything kept on drawn stage. `band` is the
/// vertical band, as fractions of the frame height, inside which the action plays (the video
/// format's action band; stills use their own).
pub fn shot_view(
	aspect: f32,
	band: (f32, f32),
	size: ShotSize,
	subjects: &[&Puppet],
	ground: f32,
) -> Option<View> {
	match size {
		// The whole setting width the format allows, centred on the cast.
		ShotSize::Wide => {
			let extent = bounds(subjects)?;
			let height = tallest_view(aspect);
			let view_bottom = ground + height * (1.0 - band.1);

			Some(clamp(
				aspect,
				View {
					centre: pt(
						f32::midpoint(extent.0, extent.2),
						view_bottom - height / 2.0,
					),
					height,
				},
			))
		}
		// The subjects head to toe with margin, standing on the ground line.
		ShotSize::Full => full_view(aspect, band, subjects, ground),
		ShotSize::Medium | ShotSize::Close | ShotSize::ExtremeClose => {
			tight_view(aspect, band, size, subjects)
		}
	}
}

/// A full shot: wide enough for everyone, the ground line and the tallest head inside the band,
/// with the head rule's headroom and side margins, push room included.
fn full_view(aspect: f32, band: (f32, f32), subjects: &[&Puppet], ground: f32) -> Option<View> {
	let extent = bounds(subjects)?;
	let top_at = band.0.max(HEADROOM + PUSH_TIGHTENS / 2.0);
	let ground_at = band.1;
	let wide = (extent.2 - extent.0) * 1.15 / aspect;
	let tall = (ground - extent.1) / (ground_at - top_at);
	let beside =
		(extent.2 - extent.0) / ((1.0 - 2.0 * (SIDE_MARGIN + PUSH_TIGHTENS / 2.0)) * aspect);
	let height = wide
		.max(tall)
		.max(beside)
		.clamp(600.0, tallest_view(aspect));
	let view_bottom = ground + height * (1.0 - ground_at);

	Some(clamp(
		aspect,
		View {
			centre: pt(
				f32::midpoint(extent.0, extent.2),
				view_bottom - height / 2.0,
			),
			height,
		},
	))
}

/// A medium, close or extreme-close shot: measured off the subjects' heads, cropped at the
/// waist, the shoulders or the chin. The whole head — hair and all, measured from what the
/// puppet draws, the whole animal for a critter — stays inside the frame with headroom and
/// side margins, and lead room shifts the frame only as far as those margins allow.
fn tight_view(aspect: f32, band: (f32, f32), size: ShotSize, subjects: &[&Puppet]) -> Option<View> {
	let measured: Vec<Extents> = subjects.iter().map(|puppet| puppet.extents()).collect();
	// The head as a disc, from its measured bounds: hair and head-wear included.
	let heads: Vec<(Pt, f32)> = measured.iter().map(|e| head_disc(e.head)).collect();
	let radius = heads.iter().map(|(_, r)| *r).fold(0.0, f32::max);
	let lowest_head = heads.iter().map(|(h, _)| h.y).fold(0.0, f32::max);
	let extent = union_all(measured.iter().map(|e| e.subject))?;

	// The crop line: the waist for a medium, the shoulders for a close, the chin for an
	// extreme close.
	let mut crop = lowest_head + radius * 1.6;

	if size == ShotSize::Medium {
		crop = subjects
			.iter()
			.zip(&measured)
			.map(|(puppet, extents)| waist(puppet, extents))
			.fold(0.0, f32::max);
	}

	// The highest point the frame must show: the top of the drawn subject — the hair, or
	// the critter's whole outline. An extreme close-up is the one exception: it may crop
	// hair above the brow line, so it measures to just above the brow instead, from the
	// rig's own head (the face, not the hairdo).
	let head_top = match size {
		ShotSize::ExtremeClose => {
			subjects
				.iter()
				.map(|puppet| {
					let (head, head_r) = puppet.head();
					head.y - head_r * 0.85
				})
				.fold(f32::INFINITY, f32::min)
		}
		_ => extent.1,
	};

	// The head sits below the frame's top edge by the format's action band and at least
	// [`HEADROOM`] of the view height, with room for the push.
	let top_at = band.0.max(HEADROOM + PUSH_TIGHTENS / 2.0);
	let span = (crop - head_top).max(radius * 2.0);
	let mut height = span / (1.0 + CROP - top_at);

	// Wide enough for the group across, with room to breathe either side, and always the
	// [`SIDE_MARGIN`]s of the frame width either side of the subjects, push included.
	let side = SIDE_MARGIN + PUSH_TIGHTENS / 2.0;
	let breathe = match size {
		ShotSize::ExtremeClose => radius * 0.5,
		ShotSize::Close => radius * 1.1,
		_ => radius * 1.4,
	};
	height = height.max((extent.2 - extent.0 + breathe * 2.0) / aspect);
	height = height.max((extent.2 - extent.0) / ((1.0 - 2.0 * side) * aspect));

	// Lead room: a subject facing or looking right sits left of centre, and the reverse —
	// but only as far as those margins allow.
	let width = height * aspect;
	let room = width * (0.5 - side);
	let mut centre_x = if subjects.len() == 1 {
		subjects[0].at.x - subjects[0].heading * LEAD_ROOM * width
	} else {
		f32::midpoint(extent.0, extent.2)
	};
	centre_x = centre_x.max(extent.2 - room).min(extent.0 + room);

	Some(clamp(
		aspect,
		View {
			centre: pt(centre_x, head_top + height * (0.5 - top_at)),
			height,
		},
	))
}

/// The subject's waist line: mid-body for a person, a little under the head for other beasts.
fn waist(puppet: &Puppet, extents: &Extents) -> f32 {
	if puppet.kind.is_figure() {
		let extent = extents.subject;
		extent.3 - (extent.3 - extent.1) * 0.45
	} else {
		let head = extents.head;
		head.3 + (head.3 - head.1) * 0.2
	}
}

/// The disc that stands in for a measured head: its centre line and half its greater side,
/// so a tall hairdo reads as the head it is.
fn head_disc(bounds: Bounds) -> (Pt, f32) {
	(
		pt(
			f32::midpoint(bounds.0, bounds.2),
			f32::midpoint(bounds.1, bounds.3),
		),
		((bounds.2 - bounds.0).max(bounds.3 - bounds.1)) / 2.0,
	)
}

/// The bounds covering every one of them.
fn union_all(bounds: impl Iterator<Item = Bounds>) -> Option<Bounds> {
	bounds.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// The part of the page a framed still may place its subject in, in page coordinates: the
/// page minus the laid-out text card (`card`, as [`crate::book`] measures it from the page's
/// own text layout), plus a small margin on the card's side, top or bottom as the page says.
/// The whole page when there is no card.
fn clear_area(text: TextPlace, card: Option<Bounds>) -> Bounds {
	match (card, text) {
		(None, _) => (0.0, 0.0, STAGE, STAGE),
		(Some(card), TextPlace::Bottom) => (0.0, 0.0, STAGE, card.1 - CARD_MARGIN),
		(Some(card), _) => (0.0, card.3 + CARD_MARGIN, STAGE, STAGE),
	}
}

/// The clear area as the vertical band of the frame [`shot_view`] reads: the head line keeps
/// the framing's headroom inside the clear area, and the ground line sits on its floor. A
/// square view maps onto the square page one to one, so a fraction of the frame is the same
/// fraction of the page.
fn clear_band(clear: Bounds) -> (f32, f32) {
	(
		(clear.1 + (clear.3 - clear.1) * HEADROOM) / STAGE,
		clear.3 / STAGE,
	)
}

/// Whether every subject's measured head sits inside the clear area: the framing's headroom
/// above it, [`SIDE_MARGIN`] of the page width either side, and no part of it off the page.
/// An extreme close-up may crop hair above the brow, as in video, so for it the brow line
/// stands in for the top of the head. A wide or full shot's head is small enough to clear
/// the card's far edge whole, and must; a tighter shot's head fills the frame, so there the
/// line above the card is the one that must hold.
fn heads_are_clear(view: View, subjects: &[&Puppet], clear: Bounds, size: ShotSize) -> bool {
	let scale = STAGE / view.height;
	let to_page = |x: f32, y: f32| {
		(
			STAGE / 2.0 + (x - view.centre.x) * scale,
			STAGE / 2.0 + (y - view.centre.y) * scale,
		)
	};
	let (_, top, _, bottom) = clear;
	let headroom = (bottom - top) * HEADROOM;
	let margin = (clear.2 - clear.0) * SIDE_MARGIN;
	let chin_clears = matches!(size, ShotSize::Wide | ShotSize::Full);

	subjects.iter().all(|puppet| {
		let (head_l, head_t, head_r, head_b) = puppet.extents().head;
		let (left_at, mut top_at) = to_page(head_l, head_t);
		let (right_at, bottom_at) = to_page(head_r, head_b);

		if size == ShotSize::ExtremeClose {
			let (centre, radius) = puppet.head();
			top_at = to_page(centre.x, centre.y - radius * 0.85).1;
		}

		top_at >= top + headroom - 0.5
			&& (!chin_clears || bottom_at <= bottom + 0.5)
			&& left_at >= margin - 0.5
			&& right_at <= clear.2 - margin + 0.5
	})
}

/// The sizes tighter than the one the page asked for, widest first: the order a still falls
/// back through when the text card leaves too little room for a framing.
fn tighter_than(size: ShotSize) -> Vec<ShotSize> {
	let all = [
		ShotSize::Wide,
		ShotSize::Full,
		ShotSize::Medium,
		ShotSize::Close,
		ShotSize::ExtremeClose,
	];
	let from = all
		.iter()
		.position(|candidate| *candidate == size)
		.unwrap_or(0);
	all[from..].to_vec()
}

/// Frames a still illustration from a shot: the square page's view of a shot size on its
/// subjects. `card` is the page's laid-out text card in page coordinates, whose band the
/// subject keeps clear of — `None` when the page has no card. `None` comes back when nobody
/// named is on stage, which draws the page as usual.
pub fn still_view(
	scene: &Scene,
	puppets: &[Puppet],
	size: ShotSize,
	on: &[String],
	card: Option<Bounds>,
) -> Option<View> {
	let subjects: Vec<&Puppet> = if on.is_empty() {
		// A close still finds whoever spoke on the page; everyone else shares the frame.
		let speaking: Vec<&str> = scene
			.actors
			.iter()
			.filter(|actor| actor.talked && !actor.hidden)
			.map(|actor| actor.id.as_str())
			.collect();

		puppets
			.iter()
			.filter(|puppet| {
				match size {
					ShotSize::Close | ShotSize::ExtremeClose if !speaking.is_empty() => {
						speaking.contains(&puppet.id.as_str()) && puppet.id != "wormsley"
					}
					_ => puppet.id != "wormsley",
				}
			})
			.collect()
	} else {
		puppets
			.iter()
			.filter(|puppet| on.contains(&puppet.id))
			.collect()
	};

	let ground = scene.setting.ground();
	let clear = clear_area(scene.text, card);
	let band = clear_band(clear);
	let mut widest = None;

	// The widest framing that keeps every head inside the part of the page the card leaves
	// clear wins; when even the tightest cannot manage it, the tightest still draws, best
	// it can — a framing never hides a head under the card.
	for candidate in tighter_than(size) {
		let view = shot_view(1.0, band, candidate, &subjects, ground)?;
		widest = Some(view);

		if heads_are_clear(view, &subjects, clear, candidate) {
			break;
		}
	}

	widest
}

/// Measured stage-space bounds of puppets: (left, top, right, bottom), taken from what
/// they actually draw — hair, raised arms, held props and all.
pub(crate) fn bounds(puppets: &[&Puppet]) -> Option<Bounds> {
	puppets
		.iter()
		.map(|p| p.extents().subject)
		.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// The old rough guess at a puppet's bounds: one hard-coded box per kind, whichever way it
/// faces. Kept only for the `locked` camera, which must keep producing exactly the frames
/// it always has; every other framing measures the puppet instead (see [`bounds`]).
pub(crate) fn rough_extent(puppet: &Puppet) -> Bounds {
	let s = puppet.scale;
	let (x, y) = (puppet.at.x, puppet.at.y);

	match &puppet.kind {
		Kind::Figure(look) => {
			let frame = look.frame();
			let lying = matches!(&puppet.body, Body::Figure(state) if state.lie > 0.5);

			if lying {
				let reach = frame.height() * s;
				(
					x - reach,
					y - frame.head_r * 2.4 * s,
					x + reach * 0.3,
					y + 20.0,
				)
			} else {
				let half = frame.head_r * 1.5 * s;
				(x - half, y - frame.height() * s - 20.0, x + half, y + 20.0)
			}
		}
		Kind::Bird(_) => (x - 90.0 * s, y - 70.0 * s, x + 90.0 * s, y + 70.0 * s),
		Kind::Dog(_) => (x - 200.0 * s, y - 180.0 * s, x + 260.0 * s, y),
		Kind::Toothbrush(_) => (x - 120.0 * s, y - 650.0 * s, x + 120.0 * s, y),
		Kind::Tree(_) => (x - 400.0 * s, y - 1050.0 * s, x + 400.0 * s, y),
		Kind::Flower(..) => (x - 110.0 * s, y - 520.0 * s, x + 110.0 * s, y),
		Kind::Worm => (x - 40.0 * s, y - 120.0 * s, x + 40.0 * s, y),
	}
}

/// Rough bounds of puppets, for the `locked` camera only (see [`rough_extent`]).
pub(crate) fn rough_bounds(puppets: &[&Puppet]) -> Option<Bounds> {
	puppets
		.iter()
		.map(|p| rough_extent(p))
		.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// Plans the camera for one shot, in both frame shapes. `explicit` marks a page the script
/// directs itself with an `@camera` direction.
pub fn direct(story: &Story, shot: &mut Shot, explicit: bool) -> Result<()> {
	match story.meta.video.direction {
		Direction::Locked => {
			shot.setups = locked(shot);
			shot.portrait_setups = shot.setups.clone();
		}
		Direction::Auto => {
			let events = events(shot);
			shot.setups = plan(shot, &events, false, explicit)?;
			shot.portrait_setups = plan(shot, &events, true, explicit)?;
		}
	}

	Ok(())
}

/// The older camera, kept byte for byte: one auto-framed view per scene key and line, gliding
/// between them, starting a beat early.
fn locked(shot: &Shot) -> Vec<Setup> {
	let mut moments: Vec<f32> = shot.keys.iter().map(|key| key.time).collect();
	moments.extend(shot.lines.iter().map(|line| line.start));
	moments.sort_by(f32::total_cmp);
	moments.dedup_by(|a, b| (*a - *b).abs() < 0.05);

	moments
		.iter()
		.map(|&anchor| {
			let camera = &shot.scene_at(anchor).camera;
			let framing = if (camera.zoom - 1.0).abs() > 0.01 {
				Framing::Zoom {
					zoom: camera.zoom,
					look: camera.look,
				}
			} else {
				Framing::Auto
			};

			Setup {
				time: (anchor - CAMERA_LEAD).max(shot.start),
				anchor,
				framing,
				travel: Travel::Glide,
				over: CAMERA_EASE,
				shake: 0.0,
			}
		})
		.collect()
}

/// What happens in a shot, in order: lines spoken, and stage changes worth reframing.
#[derive(Clone, Debug)]
struct Event {
	at: f32,
	kind: EventKind,
}

#[derive(Clone, Debug)]
enum EventKind {
	/// A spoken line: how long it lasts and who speaks it (nobody, for the narrator).
	Speech {
		duration: f32,
		speaker: Option<String>,
	},
	/// A strong mood on somebody who is not speaking: a reaction shot.
	Reaction { actor: String },
	/// An entrance, exit, walk, run, hop or faint: widen, so the action stays in frame.
	Action,
}

fn events(shot: &Shot) -> Vec<Event> {
	let mut out: Vec<Event> = Vec::new();

	// Stage changes between scene keys. Actions are gathered first, so an action at the same
	// moment as a line wins and the speech follows it.
	for pair in shot.keys.windows(2) {
		let (before, after) = (&pair[0].scene, &pair[1].scene);
		let at = pair[1].time;
		let speakers = speakers_near(shot, at);

		for actor in &after.actors {
			let previous = before.actors.iter().find(|other| other.id == actor.id);

			let enters = previous.is_none_or(|other| other.hidden) && !actor.hidden;
			let leaves = previous.is_some_and(|other| !other.hidden) && actor.hidden;
			let faints = previous.is_some_and(|other| actor.knocked > other.knocked + 0.5);
			let hops = previous.is_some_and(|other| actor.hops > other.hops);
			let walks = actor.gait.is_some() && previous.and_then(|other| other.gait) != actor.gait;

			// An actor walking in while they speak gets their own shot instead, which frames
			// them anyway; the widening is for action the camera would otherwise miss.
			let speaking = speakers.contains(&actor.id);

			if (enters || leaves || faints || hops || walks) && !(enters && speaking) {
				out.push(Event {
					at,
					kind: EventKind::Action,
				});
			} else if previous.is_some_and(|other| other.mood != actor.mood)
				&& is_strong(actor.mood)
				&& !speaking
			{
				out.push(Event {
					at,
					kind: EventKind::Reaction {
						actor: actor.id.clone(),
					},
				});
			}
		}
	}

	for line in &shot.lines {
		out.push(Event {
			at: line.start,
			kind: EventKind::Speech {
				duration: line.duration,
				speaker: line.speakers.first().cloned(),
			},
		});
	}

	out.sort_by(|a, b| a.at.total_cmp(&b.at));
	out
}

/// The moods that earn a reaction shot.
fn is_strong(mood: Mood) -> bool {
	matches!(
		mood,
		Mood::Shocked | Mood::Disgusted | Mood::Fainted | Mood::Laugh | Mood::Sad
	)
}

/// Speakers of the line playing at (or most recently before) `time`.
fn speakers_near(shot: &Shot, time: f32) -> Vec<String> {
	shot.lines
		.iter()
		.rfind(|line| line.start <= time + 0.05)
		.map(|line| line.speakers.clone())
		.unwrap_or_default()
}

fn plan(shot: &Shot, events: &[Event], portrait: bool, explicit: bool) -> Result<Vec<Setup>> {
	if explicit {
		return written(shot);
	}

	// Open on an establishing shot, unless the heading names its own framing.
	let opening = framing_of(&shot.scene_at(shot.start).camera)
		.unwrap_or_else(|| framing_on(establishing_size(portrait), Vec::new()));
	let mut setups = vec![Setup {
		time: shot.start,
		anchor: shot.start,
		framing: opening,
		travel: Travel::Cut,
		over: 0.0,
		shake: 0.0,
	}];
	// The establishing shot reads for at least 1.5 seconds, then one cut per 1.2 at most.
	let mut floor = shot.start + ESTABLISH;

	for event in events {
		let Some(want) = framing_for(event, shot, portrait) else {
			continue;
		};

		if setups.last().is_some_and(|setup| setup.framing == want) {
			continue;
		}

		let Some(at) = place_cut(event.at, shot, floor) else {
			continue;
		};

		setups.push(Setup {
			time: at,
			// The framing frames the scene as the event happens, even when the cut lands a
			// moment earlier or later than the event.
			anchor: event.at,
			framing: want,
			travel: Travel::Cut,
			over: 0.0,
			shake: 0.0,
		});
		floor = at + MIN_CUT_GAP;
	}

	slow_pushes(&mut setups, shot);
	validate(&setups, shot)
}

/// The size that opens a page: wide in landscape, head-to-toe in portrait.
fn establishing_size(portrait: bool) -> ShotSize {
	if portrait {
		ShotSize::Full
	} else {
		ShotSize::Wide
	}
}

/// The size the automatic director uses for one subject.
fn single_size(portrait: bool) -> ShotSize {
	if portrait {
		ShotSize::Close
	} else {
		ShotSize::Medium
	}
}

/// The framing an event asks for; `None` holds the current one.
fn framing_for(event: &Event, shot: &Shot, portrait: bool) -> Option<Framing> {
	match &event.kind {
		EventKind::Speech { duration, speaker } => {
			let speaker = speaker.clone()?;

			// Short lines share a two-shot, so the cutting never gets frantic.
			if *duration < SHORT_LINE {
				let mut on = vec![speaker];
				on.extend(other_of(shot, &on[0], event.at));
				Some(Framing::Shot {
					size: ShotSize::Medium,
					on,
				})
			} else {
				Some(framing_on(single_size(portrait), vec![speaker]))
			}
		}
		EventKind::Reaction { actor } => Some(framing_on(ShotSize::Close, vec![actor.clone()])),
		EventKind::Action => Some(framing_on(ShotSize::Full, Vec::new())),
	}
}

/// A framing of one shot size on named subjects.
fn framing_on(size: ShotSize, on: Vec<String>) -> Framing {
	Framing::Shot { size, on }
}

/// Who shares a two-shot with the speaker: whoever spoke to them last, or their scene-mate.
fn other_of(shot: &Shot, speaker: &str, at: f32) -> Option<String> {
	let previous = shot
		.lines
		.iter()
		.rfind(|line| line.start < at - 0.01)
		.and_then(|line| line.speakers.iter().find(|id| **id != speaker))
		.cloned();

	if previous.is_some() {
		return previous;
	}

	shot.scene_at(at)
		.actors
		.iter()
		.filter(|actor| actor.id != speaker && !actor.hidden)
		.map(|actor| actor.id.clone())
		.next()
}

/// Where a cut may land: never just after a line begins (it moves to just before the line
/// instead), never sooner than `floor`, never at the very end of a shot.
fn place_cut(at: f32, shot: &Shot, floor: f32) -> Option<f32> {
	let mut t = at.max(floor);

	if let Some(line) = shot
		.lines
		.iter()
		.find(|line| line.start <= t && t < line.start + LINE_GUARD)
	{
		let before = line.start - BEFORE_LINE;
		t = if before >= floor {
			before
		} else {
			line.start + LINE_GUARD
		};
	}

	(t < shot.end - MIN_SHOT).then_some(t)
}

/// A framing held longer than [`LONG_HOLD`] gets a slow push, so a still moment never dies.
/// The push starts early enough to run for at least [`MIN_SHOT`]; where there is no room for
/// one, the framing keeps holding to the end of the shot.
fn slow_pushes(setups: &mut Vec<Setup>, shot: &Shot) {
	let mut index = 0;

	while index < setups.len() {
		let hold_until = setups.get(index + 1).map_or(shot.end, |next| next.time);

		if hold_until - setups[index].time > LONG_HOLD && !setups[index].travel.is_move() {
			let time = (setups[index].time + LONG_HOLD).min(hold_until - MIN_SHOT);
			let framing = setups[index].framing.clone();
			setups.insert(
				index + 1,
				Setup {
					time,
					anchor: time,
					framing,
					travel: Travel::Push,
					over: 0.0,
					shake: 0.0,
				},
			);
			index += 2;
		} else {
			index += 1;
		}
	}
}

/// No setup runs shorter than [`MIN_SHOT`], on pages the script directs itself too. A setup
/// that would flash by is merged into its neighbour — the earlier one keeps running, or the
/// later one starts earlier — but a cut the script wrote is honoured: it stays where the
/// script put it, and the setup it leaves under the minimum is reported on the storyboard
/// as `· short`. A camera state that only shakes or moves on the same framing is not a cut.
fn enforce_minimum(setups: &mut Vec<Setup>, shot: &Shot) {
	let honoured = |index: usize, setups: &[Setup]| {
		setups[index].travel == Travel::Cut && setups[index].framing != setups[index - 1].framing
	};

	let mut index = 1;

	while index < setups.len() {
		// Too soon after the setup before it: merge into it, so the earlier one keeps
		// running until whatever comes next.
		if !honoured(index, setups) && setups[index].time - setups[index - 1].time < MIN_SHOT - 1e-3
		{
			setups.remove(index);

			// The setup before this one now runs to a later boundary: re-check it.
			if index > 1 {
				index -= 1;
			}
			continue;
		}

		// Not enough room before the next setup or the page's end: start this one earlier
		// when that still leaves it a full hold, otherwise merge it too.
		let end = setups.get(index + 1).map_or(shot.end, |next| next.time);

		if !honoured(index, setups) && end - setups[index].time < MIN_SHOT - 1e-3 {
			let earlier = end - MIN_SHOT;

			if earlier - setups[index - 1].time >= MIN_SHOT - 1e-3 {
				setups[index].time = earlier;
				index += 1;
			} else {
				setups.remove(index);

				// The setup before this one now runs to a later boundary: re-check it.
				if index > 1 {
					index -= 1;
				}
			}
			continue;
		}

		index += 1;
	}
}

/// A page the script directs itself: every camera state along the page becomes a setup, as
/// written.
fn written(shot: &Shot) -> Result<Vec<Setup>> {
	let mut setups: Vec<Setup> = Vec::new();

	for (index, key) in shot.keys.iter().enumerate() {
		let camera = &key.scene.camera;

		// The page opens on whatever its heading names, or the automatic framing.
		if index == 0 {
			let travel = camera.travel.unwrap_or(if camera.shot.is_some() {
				Travel::Cut
			} else {
				Travel::Glide
			});
			let (time, over) = match travel {
				Travel::Glide => {
					(
						shot.start,
						if camera.over > 0.0 {
							camera.over
						} else {
							CAMERA_EASE
						},
					)
				}
				Travel::Cut => {
					(
						place_cut(key.time, shot, shot.start).unwrap_or(key.time),
						0.0,
					)
				}
				_ => (key.time.max(shot.start), camera.over),
			};
			setups.push(Setup {
				time,
				anchor: key.time,
				framing: framing_of(camera).unwrap_or(Framing::Auto),
				travel,
				over,
				shake: camera.shake,
			});
			continue;
		}

		let framing = framing_of(camera).or_else(|| setups.last().map(|last| last.framing.clone()));
		let Some(framing) = framing else {
			continue;
		};
		let named_move = camera.travel.is_some_and(Travel::is_move);
		let changed = setups
			.last()
			.is_none_or(|setup| setup.framing != framing || named_move);

		if !changed && camera.shake <= 0.0 {
			continue;
		}

		let travel = camera.travel.unwrap_or(if camera.shot.is_some() {
			// A named shot arrives on the cut; a legacy zoom glides, as it always has.
			Travel::Cut
		} else {
			Travel::Glide
		});
		let (time, over) = match travel {
			Travel::Glide => {
				(
					(key.time - CAMERA_LEAD).max(shot.start),
					if camera.over > 0.0 {
						camera.over
					} else {
						CAMERA_EASE
					},
				)
			}
			Travel::Cut => {
				(
					place_cut(key.time, shot, shot.start).unwrap_or(key.time),
					0.0,
				)
			}
			_ => (key.time, camera.over),
		};

		setups.push(Setup {
			time,
			anchor: key.time,
			framing,
			travel,
			over,
			shake: camera.shake,
		});
	}

	if setups.is_empty() {
		setups.push(Setup {
			time: shot.start,
			anchor: shot.start,
			framing: Framing::Auto,
			travel: Travel::Glide,
			over: CAMERA_EASE,
			shake: 0.0,
		});
	}

	enforce_minimum(&mut setups, shot);
	validate(&setups, shot)
}

/// The framing a camera state names: its shot size, else its zoom, else nothing.
fn framing_of(camera: &Camera) -> Option<Framing> {
	if let Some(size) = camera.shot {
		return Some(Framing::Shot {
			size,
			on: camera.on.clone(),
		});
	}

	if (camera.zoom - 1.0).abs() > 0.01 {
		return Some(Framing::Zoom {
			zoom: camera.zoom,
			look: camera.look,
		});
	}

	None
}

/// How the camera arrives where a travel ends: relative moves widen or tighten the view they
/// start from, so `move=push` alone still means something.
pub(crate) fn arrives_from(travel: Travel, view: View, aspect: f32) -> View {
	let width = view.width(aspect);
	let centre = match travel {
		Travel::PanLeft => pt(view.centre.x - width * PAN, view.centre.y),
		Travel::PanRight => pt(view.centre.x + width * PAN, view.centre.y),
		Travel::TiltUp => pt(view.centre.x, view.centre.y - view.height * TILT),
		Travel::TiltDown => pt(view.centre.x, view.centre.y + view.height * TILT),
		_ => view.centre,
	};
	let height = match travel {
		Travel::Push => view.height * PUSH_IN,
		Travel::Pull => view.height * PULL_OUT,
		_ => view.height,
	};

	clamp(aspect, View { centre, height })
}

/// `on=` names must be on stage when the shot lands; anything else is a script error.
fn validate(setups: &[Setup], shot: &Shot) -> Result<Vec<Setup>> {
	let page = shot
		.kind
		.page()
		.map_or(String::new(), |page| format!("page {}: ", page + 1));

	for setup in setups {
		let Framing::Shot { on, .. } = &setup.framing else {
			continue;
		};

		let actors = &shot.scene_at(setup.anchor).actors;

		for id in on {
			if !actors.iter().any(|actor| &actor.id == id && !actor.hidden) {
				bail!(
					"{page}`on={id}` names nobody on stage when the shot lands; who is there: {}",
					actors
						.iter()
						.filter(|actor| !actor.hidden)
						.map(|actor| actor.id.as_str())
						.collect::<Vec<_>>()
						.join(", ")
				);
			}
		}
	}

	Ok(setups.to_vec())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cast::Cast;
	use crate::compose;
	use crate::stage::rehearse;
	use crate::story;
	use crate::story::Beat;
	use crate::video::lipsync::Track;
	use crate::video::timeline::Timeline;
	use crate::video::voice::SAMPLE_RATE;
	use crate::video::voice::Speech;
	use crate::video::voice::Utterance;

	/// Half a second a word, so tests can write lines long and short.
	fn silence(utterance: &Utterance<'_>) -> Speech {
		let seconds = utterance.text.split_whitespace().count() as f32 * 0.5;
		Speech {
			text: String::new(),
			samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
			envelope: Vec::new(),
			track: Track::default(),
		}
	}

	fn timeline_of(script: &str, front: &str) -> Timeline {
		let source = format!(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\nmoral = \"Be \
			 kind.\"\n{front}\n+++\n{script}"
		);
		let story = story::parse(&source).unwrap();
		let cast = Cast::parse(
			"[kid]\nname = \"Kid\"\n[kid.voice]\nname = \"Junior\"\n[mum]\nname = \"Mum\"\nkind = \
			 \"grown-up\"\n",
		)
		.unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		crate::video::timeline::build(&story, &cast, &rehearsals, |line| Ok(silence(line))).unwrap()
	}

	fn page_setups(script: &str) -> Vec<Setup> {
		timeline_of(script, "").shots[1].setups.clone()
	}

	fn subjects(setups: &[Setup]) -> Vec<String> {
		setups
			.iter()
			.map(|setup| {
				match &setup.framing {
					Framing::Shot { on, .. } if on.is_empty() => "everyone".to_owned(),
					Framing::Shot { on, .. } => on.join("+"),
					Framing::Zoom { .. } => "zoom".to_owned(),
					Framing::Auto => "auto".to_owned(),
				}
			})
			.collect()
	}

	const EXCHANGE: &str = "# park\n@kid x=0.3 talk\nHello there today my friend.\n@mum talk\nWhy \
	                        hello to you too.\n@kid talk\nWhat shall we play now?\n";
	#[test]
	fn an_exchange_produces_shot_and_reverse() {
		let framed = subjects(&page_setups(EXCHANGE));

		assert_eq!(framed[0], "everyone", "pages open on an establishing shot");
		assert_eq!(
			framed[1], "kid",
			"the first speaker gets a shot of their own"
		);
		assert_eq!(framed[2], "mum", "then the reverse shot on who answers");
		assert_eq!(framed[3], "kid", "and back again for the reply");
	}

	#[test]
	fn short_lines_produce_a_two_shot() {
		let setups =
			page_setups("# park\n@kid x=0.3\n@mum x=0.7\n@kid talk\nHi.\n@mum talk\nWell hello.\n");
		let framed = subjects(&setups);

		assert!(
			framed[1..].iter().all(|subject| subject.contains('+')),
			"short lines share a two-shot: {framed:?}"
		);
	}

	#[test]
	fn no_setup_is_shorter_than_the_minimum() {
		let tl = timeline_of(
			"# park\n@kid x=0.3 talk\nHello there today my friend.\n@kid faint\n@mum talk\nOh no, \
			 what happened here then?\n@kid wake\n@kid talk\nI am quite alright thanks.\n",
			"",
		);

		for portrait in [false, true] {
			let setups = tl.shots[1].setups_for(portrait);
			let end = tl.shots[1].end;

			assert!(
				setups[1].time - setups[0].time >= ESTABLISH - 1e-3,
				"the establishing shot reads for at least 1.5s"
			);

			for (index, pair) in setups.windows(2).enumerate() {
				assert!(
					pair[1].time - pair[0].time >= MIN_SHOT - 1e-3,
					"portrait {portrait} setup {index}: {:?} lands only {:.2}s after {:?}",
					pair[1].framing,
					pair[1].time - pair[0].time,
					pair[0].framing
				);
			}

			// The page's end is a setup boundary too: the last framing holds its minimum.
			assert!(
				end - setups.last().unwrap().time >= MIN_SHOT - 1e-3,
				"portrait {portrait}: the last setup holds only {:.2}s of the page",
				end - setups.last().unwrap().time
			);
		}
	}

	#[test]
	fn a_cut_never_lands_just_after_a_line_begins() {
		let tl = timeline_of(
			"# park\n@kid x=0.3 talk\nHello there today my friend.\n@kid faint\n@mum talk\nOh no, \
			 what happened here then?\n",
			"",
		);
		let (shot, setups) = (&tl.shots[1], &tl.shots[1].setups);

		for setup in setups {
			for line in &shot.lines {
				assert!(
					!(line.start <= setup.time && setup.time < line.start + LINE_GUARD),
					"setup at {:.2} lands just after the line at {:.2}",
					setup.time,
					line.start
				);
			}
		}
	}

	#[test]
	fn an_entrance_widens_the_shot() {
		let setups = page_setups(
			"# park\n@kid x=0.3 talk\nHello there today my friend.\n@mum enter=right\nNow who is \
			 this coming over the hill?\n@kid talk\nWhy hello to you too.\n",
		);
		let framed = subjects(&setups);

		assert_eq!(
			&framed,
			&[
				"everyone".to_owned(),
				"kid".to_owned(),
				"everyone".to_owned(),
				"kid".to_owned()
			],
			"the entrance widens to a full shot before the close on the speaker: {framed:?}"
		);
	}

	#[test]
	fn a_strong_mood_earns_a_reaction_shot() {
		let setups = page_setups(
			"# park\n@kid x=0.3\n@mum x=0.7\n@kid talk\nHello there today my friend.\n@mum \
			 shocked\n@kid talk\nWhy do you look so surprised?\n",
		);
		let framed = subjects(&setups);

		assert!(
			framed.contains(&"mum".to_owned()),
			"the shocked listener gets a close reaction shot: {framed:?}"
		);
	}

	#[test]
	fn explicit_pages_are_shot_exactly_as_written() {
		let setups = page_setups(
			"# park\n@kid x=0.3\n@camera shot=close on=kid\n@kid talk\nHello there today my \
			 friend.\n@camera shot=wide move=push over=2\n@kid talk\nWhy hello to you too.\n",
		);

		assert_eq!(
			subjects(&setups),
			vec!["kid".to_owned(), "everyone".to_owned()],
			"{:?}",
			subjects(&setups)
		);
		assert_eq!(setups[0].travel, Travel::Cut, "a shot change cuts");
		assert_eq!(setups[1].travel, Travel::Push);
		assert!((setups[1].over - 2.0).abs() < 1e-4, "the move takes 2s");
	}

	#[test]
	fn explicit_pages_hold_the_minimum_too() {
		// A shake late on the page would make its setup run 0.35s; it starts early enough
		// for a full hold instead.
		let tl = timeline_of(
			"# park\n@kid x=0.3\n@camera shot=close on=kid\nHello there today my friend.\nWhy \
			 hello to you too.\n@camera shake=0.4\n",
			"",
		);

		for portrait in [false, true] {
			let setups = tl.shots[1].setups_for(portrait);
			let end = tl.shots[1].end;

			for (index, pair) in setups.windows(2).enumerate() {
				assert!(
					pair[1].time - pair[0].time >= MIN_SHOT - 1e-3,
					"portrait {portrait} setup {index} holds only {:.2}s",
					pair[1].time - pair[0].time
				);
			}

			let last = setups.last().unwrap();
			assert!(
				end - last.time >= MIN_SHOT - 1e-3,
				"portrait {portrait}: the shake setup holds only {:.2}s",
				end - last.time
			);
			assert!(
				(last.time - (end - MIN_SHOT)).abs() < 1e-3,
				"portrait {portrait}: the shake setup starts as late as the minimum allows"
			);
		}
	}

	#[test]
	fn a_setup_too_close_to_its_neighbour_merges_into_it() {
		// A push 1.15s after the close it follows would flash by; it merges, and the close
		// keeps running until the wide shot (which inherits the page's push as its travel).
		let tl = timeline_of(
			"# park\n@kid x=0.3\nHi.\n@camera shot=close on=kid\nBye.\n@camera move=push \
			 over=0.5\nThere we are again.\n@camera shot=wide\nDone.\n",
			"",
		);
		let setups = &tl.shots[1].setups;
		let framed = subjects(setups);

		assert_eq!(
			framed,
			vec!["auto".to_owned(), "kid".to_owned(), "everyone".to_owned()],
			"the short same-framing push merged away: {framed:?}"
		);

		let end = tl.shots[1].end;

		for pair in setups.windows(2) {
			assert!(
				pair[1].time - pair[0].time >= MIN_SHOT - 1e-3,
				"{:?} lands {:.2}s after {:?}",
				pair[1].framing,
				pair[1].time - pair[0].time,
				pair[0].framing
			);
		}

		assert!(
			end - setups.last().unwrap().time >= MIN_SHOT - 1e-3,
			"the last setup holds only {:.2}s of the page",
			end - setups.last().unwrap().time
		);
	}

	#[test]
	fn a_cut_the_script_wrote_is_kept_even_when_short() {
		// The wide cut lands 0.9s after the close: short, but the script wrote it, so the
		// director honours it and the board reports the short hold.
		let tl = timeline_of(
			"# park\n@kid x=0.3\nHi.\n@camera shot=close on=kid\nBye.\n@camera shot=wide\n!\n",
			"",
		);
		let setups = &tl.shots[1].setups;
		let framed = subjects(setups);

		assert_eq!(
			framed,
			vec!["auto".to_owned(), "kid".to_owned(), "everyone".to_owned()],
			"the script's two framings both survive: {framed:?}"
		);

		let gap = setups[2].time - setups[1].time;
		assert!(
			gap < MIN_SHOT,
			"the written cut stays where the script put it: {gap:.2}s"
		);
		assert_eq!(setups[2].travel, Travel::Cut);
	}

	#[test]
	fn locked_keeps_the_old_camera() {
		let script = "# park\n@kid x=0.3\nOne two three.\n@kid faint\nFour five.\n";
		let tl = timeline_of(script, "");
		let shot = &tl.shots[1];
		let locked = timeline_of(script, "[video]\ndirection = \"locked\"");
		let setups = &locked.shots[1].setups;

		// The older camera framed one view per scene key and line, gliding between them; two
		// moments within 0.05s were one view.
		let mut moments: Vec<f32> = shot.keys.iter().map(|key| key.time).collect();
		moments.extend(shot.lines.iter().map(|line| line.start));
		moments.sort_by(f32::total_cmp);
		moments.dedup_by(|a, b| (*a - *b).abs() < 0.05);
		assert_eq!(
			setups.len(),
			moments.len(),
			"one setup per old camera event"
		);

		for setup in setups {
			assert_eq!(setup.travel, Travel::Glide);
			assert!((setup.over - CAMERA_EASE).abs() < 1e-4);
			assert_eq!(setup.framing, Framing::Auto);
			assert!(
				(setup.time - (setup.anchor - CAMERA_LEAD).max(shot.start)).abs() < 1e-4,
				"a glide starts a beat early"
			);
		}
	}

	#[test]
	fn a_long_hold_gets_a_slow_push() {
		let tl = timeline_of(
			"# park\n@kid x=0.3\nand then they sat there quietly for a very long while indeed \
			 until the sun went down.\n",
			"",
		);
		let shot = &tl.shots[1];

		assert!(shot.duration() > LONG_HOLD, "the page runs past 8s");
		assert_eq!(shot.setups.len(), 2, "the hold keeps its framing");
		assert_eq!(shot.setups[0].travel, Travel::Cut);
		assert_eq!(shot.setups[1].travel, Travel::Push);
		assert!((shot.setups[1].time - (shot.start + LONG_HOLD)).abs() < 1e-4);
	}

	fn puppets_of(script: &str) -> Vec<Puppet> {
		scene_of(script).1
	}

	/// The page's scene and puppets, after its stage directions. The cast asks for the
	/// parametric puppet, the figure these framing rules were written for; a character with
	/// no `puppet` line is now drawn from the world's template instead.
	fn scene_of(script: &str) -> (Scene, Vec<Puppet>) {
		let source = format!("+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n{script}");
		let story = story::parse(&source).unwrap();
		let cast = Cast::parse(
			"[kid]\nname = \"Kid\"\npuppet = \"parametric\"\n[mum]\nname = \"Mum\"\nkind = \
			 \"grown-up\"\npuppet = \"parametric\"\n",
		)
		.unwrap();
		let page = &story.pages[0];
		let mut scene = Scene::for_page(page, &story.settings).unwrap();
		let mut memory = std::collections::HashMap::new();

		for beat in &page.beats {
			if let Beat::Direct(directive) = beat {
				scene.apply(directive, &cast, &mut memory).unwrap();
			}
		}

		let puppets = compose::puppets(&scene, 0.0, false);
		(scene, puppets)
	}

	/// A text card as `book.rs` draws it: the laid-out text block plus its padding, ending
	/// at `bottom`. Top cards hang from the top of the page; a bottom card is a line or two
	/// of text hugging the foot of the page.
	fn card_at(bottom: f32) -> Bounds {
		(crate::book::SAFE, 105.0, STAGE - crate::book::SAFE, bottom)
	}

	fn bottom_card() -> Bounds {
		(crate::book::SAFE, 2095.0, STAGE - crate::book::SAFE, 2353.0)
	}

	/// Maps a stage point onto the page through `view`, as `book.rs` draws it: a square view
	/// fills the square page exactly.
	fn to_page(view: View, x: f32, y: f32) -> (f32, f32) {
		let scale = STAGE / view.height;

		(
			STAGE / 2.0 + (x - view.centre.x) * scale,
			STAGE / 2.0 + (y - view.centre.y) * scale,
		)
	}

	/// The framing rule on a book page: every measured head bound sits inside the part of
	/// the page the card leaves clear, at least [`HEADROOM`] of the clear height above it
	/// and [`SIDE_MARGIN`] of the page width each side, nothing under the card or off it.
	/// An extreme close-up may crop hair above the brow, as in video: where the whole head
	/// does not clear the card's line, the brow line must.
	fn assert_heads_in_clear(view: View, subjects: &[&Puppet], clear: Bounds, what: &str) {
		let (_, clear_t, _, clear_b) = clear;
		let headroom = (clear_b - clear_t) * HEADROOM;
		let margin = (clear.2 - clear.0) * SIDE_MARGIN;

		for puppet in subjects {
			let (hl, ht, hr, hb) = puppet.extents().head;
			let (left_at, top_at) = to_page(view, hl, ht);
			let (right_at, bottom_at) = to_page(view, hr, hb);
			let brow_at = {
				let (centre, radius) = puppet.head();
				to_page(view, centre.x, centre.y - radius * 0.85).1
			};

			assert!(
				top_at >= clear_t + headroom - 0.5 || brow_at >= clear_t - 0.5,
				"{what}: head top at {top_at:.0} (brow {brow_at:.0}) sits under the clear area's \
				 head line {:.0}",
				clear_t + headroom
			);
			assert!(
				bottom_at <= clear_b + 0.5,
				"{what}: head bottom at {bottom_at:.0} hangs below the clear area's floor \
				 {clear_b:.0}"
			);
			assert!(
				left_at >= margin - 0.5 && right_at <= STAGE - margin + 0.5,
				"{what}: head spans {left_at:.0}..{right_at:.0}, inside the page's {margin:.0} \
				 unit side margins"
			);
		}
	}

	/// Every shot size, a top card of three heights, a bottom card and no card, on a figure
	/// and on two: the heads stay in the area the card leaves clear, and the framing keeps
	/// the video rules it is built with.
	#[test]
	fn still_shots_keep_heads_in_the_area_the_card_leaves_clear() {
		let cases: [(&str, Option<Bounds>); 5] = [
			("text=top", Some(card_at(350.0))),
			("text=top", Some(card_at(700.0))),
			("text=top", Some(card_at(1010.0))),
			("text=bottom", Some(bottom_card())),
			("text=none", None),
		];

		for (text, card) in cases {
			for actors in ["@kid x=0.5", "@kid x=0.3\n@mum x=0.7"] {
				let (scene, puppets) = scene_of(&format!("# park {text}\n{actors}\n"));
				let subjects: Vec<&Puppet> = puppets.iter().collect();
				let clear = clear_area(scene.text, card);
				let band = clear_band(clear);
				let ground = scene.setting.ground();

				for size in [
					ShotSize::Wide,
					ShotSize::Full,
					ShotSize::Medium,
					ShotSize::Close,
					ShotSize::ExtremeClose,
				] {
					let what = format!("{text} · {actors:?} · {size:?}");
					let view = still_view(&scene, &puppets, size, &[], card)
						.unwrap_or_else(|| panic!("{what}: no framing"));
					assert_heads_in_clear(view, &subjects, clear, &what);

					// The framing still keeps the whole subject inside its own headroom and
					// side margins, as the video guarantees — the one exception being an
					// extreme close-up, whose crop may take the hair above the brow.
					let extent = bounds(&subjects).unwrap();
					let (left, top) = (
						view.centre.x - view.width(1.0) / 2.0,
						view.centre.y - view.height / 2.0,
					);
					let cropped = shot_view(1.0, band, ShotSize::ExtremeClose, &subjects, ground)
						.as_ref() == Some(&view);
					assert!(
						extent.1 >= top + HEADROOM * view.height - 1e-3 || cropped,
						"{what}: subject top {:.0} above the frame's headroom line {:.0}",
						extent.1,
						top + HEADROOM * view.height
					);
					assert!(
						extent.0 >= left + SIDE_MARGIN * view.width(1.0) - 1e-3
							&& extent.2 <= left + view.width(1.0) * (1.0 - SIDE_MARGIN) + 1e-3,
						"{what}: subject {:.0}..{:.0} breaks the frame's side margins",
						extent.0,
						extent.2
					);
				}
			}
		}
	}

	/// A grown-up under a page-deep card: a full shot has no room left to keep her head
	/// below the card, so the still falls back to the widest framing that does — which
	/// still shows the whole head.
	#[test]
	fn a_full_shot_falls_back_when_the_card_leaves_no_room() {
		let card = card_at(1010.0);
		let (scene, puppets) = scene_of("# park text=top\n@mum x=0.5\n");
		let subjects: Vec<&Puppet> = puppets.iter().collect();
		let clear = clear_area(scene.text, Some(card));
		let band = clear_band(clear);

		// The plain full shot cannot keep her head in the clear area.
		let full = shot_view(1.0, band, ShotSize::Full, &subjects, scene.setting.ground())
			.expect("a subject");
		assert!(
			!heads_are_clear(full, &subjects, clear, ShotSize::Full),
			"the card leaves room for a plain full shot; there would be no fallback to test"
		);

		let view =
			still_view(&scene, &puppets, ShotSize::Full, &[], Some(card)).expect("a subject");
		assert!(
			view.height < full.height,
			"the still fell back to a tighter framing: {} vs the full shot's {}",
			view.height,
			full.height
		);
		assert_heads_in_clear(view, &subjects, clear, "the fallback");
	}

	#[test]
	fn lead_room_puts_a_right_facing_subject_left_of_centre() {
		let landscape = (0.07, 0.76);

		for (facing, expectation) in [("right", true), ("left", false)] {
			let puppets = puppets_of(&format!("# park\n@kid x=0.5 face={facing}\n"));
			let view = shot_view(
				16.0 / 9.0,
				landscape,
				ShotSize::Close,
				&[&puppets[0]],
				2060.0,
			)
			.unwrap();

			assert_eq!(
				view.centre.x < puppets[0].at.x,
				expectation,
				"a {facing}-facing subject at {:.0} framed at {:.0}",
				puppets[0].at.x,
				view.centre.x
			);
		}
	}

	/// The framing rule, for every size, both formats, a figure and a critter, facing each
	/// way: the whole head — hair and all, from the puppet's drawn extent — stays inside the
	/// frame with [`HEADROOM`] above and [`SIDE_MARGIN`] either side, and lead room only
	/// shifts the frame as far as those margins allow. An extreme close-up may crop hair
	/// above the brow line, so it is checked for sides and brow instead.
	#[test]
	fn every_shot_keeps_the_whole_head_inside_the_frame() {
		use crate::video::frames::Format;

		for format in [Format::Landscape, Format::Portrait] {
			let aspect = format.aspect();
			let band = format.action_band();

			for facing in ["right", "left"] {
				for actor in ["kid", "dog"] {
					let script = format!("# park\n@{actor} x=0.5 face={facing}\n");
					let puppets = puppets_of(&script);
					assert_eq!(puppets.len(), 1, "{script}");
					let cast: Vec<&Puppet> = puppets.iter().collect();
					let extent = puppets[0].extents().subject;
					let (head, head_radius) = puppets[0].head();

					for size in [
						ShotSize::Wide,
						ShotSize::Full,
						ShotSize::Medium,
						ShotSize::Close,
						ShotSize::ExtremeClose,
					] {
						let view = shot_view(aspect, band, size, &cast, 2060.0).expect("a subject");
						let (width, height) = (view.width(aspect), view.height);
						let (left, top) =
							(view.centre.x - width / 2.0, view.centre.y - height / 2.0);
						let what = format!("{actor} {facing} {size:?} {format:?}");

						if size == ShotSize::ExtremeClose {
							// The one exception: hair above the brow may go, the brow stays.
							assert!(
								head.y - head_radius * 0.6 >= top - 1e-3,
								"{what}: the brow line {:.0} is above the frame top {:.0}",
								head.y - head_radius * 0.6,
								top
							);
						} else {
							let margin = (extent.1 - top) / height;
							assert!(
								extent.1 >= top + HEADROOM * height - 1e-3,
								"{what}: head top {:.0} is only {:.1}% of the {:.0} unit view \
								 below the frame top {:.0}",
								extent.1,
								margin * 100.0,
								height,
								top
							);
						}

						assert!(
							extent.0 >= left + SIDE_MARGIN * width - 1e-3,
							"{what}: left edge {:.0} is inside the {:.0} unit frame's left margin \
							 (frame left {:.0})",
							extent.0,
							width,
							left
						);
						assert!(
							extent.2 <= left + width * (1.0 - SIDE_MARGIN) + 1e-3,
							"{what}: right edge {:.0} crosses the frame's right margin (frame \
							 right {:.0})",
							extent.2,
							left + width
						);
					}
				}
			}
		}
	}

	#[test]
	fn framing_never_leaves_the_stage_bounds() {
		let cases = [
			"# park\n@kid x=0.02\n",
			"# park\n@kid x=0.98\n",
			"# park\n@mum x=0.5\n@kid x=0.1\n",
			"# park\n@bird.a y=0.2 x=0.5 fly\n",
		];
		let landscape = (0.07, 0.76);

		for script in cases {
			let puppets = puppets_of(script);
			let subjects: Vec<&Puppet> = puppets.iter().collect();

			for size in [
				ShotSize::Wide,
				ShotSize::Full,
				ShotSize::Medium,
				ShotSize::Close,
				ShotSize::ExtremeClose,
			] {
				for aspect in [16.0 / 9.0, 9.0 / 16.0] {
					let view = shot_view(aspect, landscape, size, &subjects, 2060.0).unwrap();
					let drawn = STAGE + OVERDRAW;

					assert!(
						view.height <= tallest_view(aspect) + 1e-3
							&& view.centre.x > -drawn
							&& view.centre.x < STAGE + drawn
							&& view.centre.y > -drawn
							&& view.centre.y < STAGE + drawn,
						"{script} {size:?} {aspect}: {view:?}"
					);
				}
			}
		}
	}

	#[test]
	fn portrait_shoots_closer_than_landscape() {
		let tl = timeline_of(EXCHANGE, "");
		let size_of = |setups: &[Setup]| {
			match &setups[1].framing {
				Framing::Shot { size, .. } => *size,
				other => panic!("{other:?}"),
			}
		};

		assert_eq!(size_of(&tl.shots[1].setups), ShotSize::Medium);
		assert_eq!(size_of(&tl.shots[1].portrait_setups), ShotSize::Close);
	}

	#[test]
	fn nobody_named_is_an_error() {
		let error = (|| {
			let story = story::parse(
				"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park\n@kid \
				 x=0.3\n@camera shot=close on=ghost\nHello.\n",
			)?;
			let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
			let rehearsals = rehearse(&story, &cast)?;
			crate::video::timeline::build(&story, &cast, &rehearsals, |line| Ok(silence(line)))
		})()
		.unwrap_err();

		let message = format!("{error:#}");
		assert!(message.contains("on=ghost"), "{message}");
		assert!(message.contains("page 1"), "{message}");
	}

	#[test]
	fn a_page_heading_shot_frames_the_still() {
		let source = "+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# park \
		              shot=close\n@kid x=0.5 talk\nHello there.\n";
		let story = story::parse(source).unwrap();
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let page = &story.pages[0];
		let mut scene = Scene::for_page(page, &story.settings).unwrap();
		let mut memory = std::collections::HashMap::new();

		for beat in &page.beats {
			if let Beat::Direct(directive) = beat {
				scene.apply(directive, &cast, &mut memory).unwrap();
			}
		}

		let puppets = compose::puppets(&scene, 0.0, true);
		let view = still_view(&scene, &puppets, ShotSize::Close, &scene.camera.on, None)
			.expect("a close shot on a speaking kid frames the still");

		// A real close-up: much tighter than the whole stage, centred on the kid, square.
		assert!(view.height < STAGE * 0.6, "{}", view.height);
		assert!(
			(view.centre.x - puppets[0].at.x).abs() < view.width(1.0),
			"the kid stays in frame: {} vs {}",
			view.centre.x,
			puppets[0].at.x
		);

		// An unknown subject leaves the page to draw as usual.
		assert!(
			still_view(
				&scene,
				&puppets,
				ShotSize::Close,
				&["ghost".to_owned()],
				None
			)
			.is_none(),
			"nobody named, no framing"
		);
	}

	#[test]
	fn relative_moves_arrive_where_their_names_say() {
		let view = View {
			centre: pt(1500.0, 1500.0),
			height: 1000.0,
		};
		let aspect = 16.0 / 9.0;

		let pushed = arrives_from(Travel::Push, view, aspect);
		assert!((pushed.height - view.height * PUSH_IN).abs() < 1e-3);

		let pulled = arrives_from(Travel::Pull, view, aspect);
		assert!((pulled.height - view.height * PULL_OUT).abs() < 1e-3);

		let panned = arrives_from(Travel::PanLeft, view, aspect);
		assert!((panned.centre.x - (view.centre.x - view.width(aspect) * PAN)).abs() < 1e-3);

		let tilted = arrives_from(Travel::TiltUp, view, aspect);
		assert!((tilted.centre.y - (view.centre.y - view.height * TILT)).abs() < 1e-3);
	}
}
