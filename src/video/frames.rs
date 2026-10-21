//! Draws one video frame at a moment on the timeline.
//!
//! The frame is the rehearsal's scene at that moment, blended between keyframes with cartoon
//! easing, mouths driven by the narration, a camera that frames the action for the output
//! shape, captions, and a paper wipe between pages.

use std::f32::consts::PI;

use anyhow::Result;
use resvg::tiny_skia::Pixmap;

use crate::art::Path;
use crate::art::Pt;
use crate::art::STAGE;
use crate::art::Svg;
use crate::art::figure::Frame;
use crate::art::lerp;
use crate::art::noise;
use crate::art::palette;
use crate::art::pt;
use crate::art::rig::Gait;
use crate::art::seed_of;
use crate::art::settings;
use crate::art::settings::Layer;
use crate::cast::Kind;
use crate::compose;
use crate::compose::Body;
use crate::compose::Puppet;
use crate::raster;
use crate::stage::Scene;
use crate::stage::Side;
use crate::story::RunStyle;
use crate::story::Story;
use crate::text;
use crate::text::Anchor;
use crate::text::Area;
use crate::text::Ink;
use crate::video::audio::Sound;
use crate::video::timeline::Shot;
use crate::video::timeline::ShotKind;
use crate::video::timeline::Timeline;

/// How long stage-direction changes take to play out.
const TWEEN: f32 = 0.6;
/// How long the camera takes to reframe.
const CAMERA_EASE: f32 = 0.9;
/// How far ahead of a line or direction the camera starts moving.
const CAMERA_LEAD: f32 = 0.45;
/// Page-turn wipe length at each end of a shot.
const WIPE: f32 = 0.35;
/// Stage area drawn past the edges, which the camera may show.
const OVERDRAW: f32 = 150.0;

/// Output shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
	/// 16:9 for `YouTube`.
	Landscape,
	/// 9:16 for Shorts, `TikTok` and Reels.
	Portrait,
}

impl Format {
	pub fn size(self, preview: bool) -> (u32, u32) {
		let (w, h) = match self {
			Format::Landscape => (1920, 1080),
			Format::Portrait => (1080, 1920),
		};

		if preview { (w / 2, h / 2) } else { (w, h) }
	}

	pub fn aspect(self) -> f32 {
		match self {
			Format::Landscape => 16.0 / 9.0,
			Format::Portrait => 9.0 / 16.0,
		}
	}

	pub fn suffix(self) -> &'static str {
		match self {
			Format::Landscape => "16x9",
			Format::Portrait => "9x16",
		}
	}

	/// Where captions are laid out, as (top, height) fractions of the frame height. Text shrinks
	/// to fit, so a caption never leaves this band (its card adds [`CARD_PAD_Y`] around it).
	fn caption_band(self) -> (f32, f32) {
		match self {
			// Low, but clear of the player controls at the very bottom.
			Format::Landscape => (0.81, 0.13),
			// Phones cover the bottom fifth and the right edge with buttons; captions sit high.
			Format::Portrait => (0.12, 0.17),
		}
	}

	/// Where the camera frames the action, as fractions of the frame height: the top of the
	/// tallest head and the ground line. Derived from the caption band, so standing, sitting,
	/// crouching and kneeling bodies all stay clear of the caption card.
	fn action_band(self) -> (f32, f32) {
		let (top, height) = self.caption_band();
		let card = self.card_pad() + FOOT_ROOM;

		match self {
			// Headroom above the tallest head for buns and hats.
			Format::Landscape => (0.07, top - card),
			Format::Portrait => (top + height + card, 0.8),
		}
	}

	/// The caption card's vertical padding as a fraction of the frame height.
	fn card_pad(self) -> f32 {
		CARD_PAD_Y / 1080.0 * self.aspect().min(1.0)
	}
}

/// Vertical padding of the caption card, in 1080p units of the frame's shorter side.
const CARD_PAD_Y: f32 = 22.0;
/// Clearance between the action band and the caption card, as a fraction of the frame height:
/// shoes and shadows reach a little below the ground line, and hair a little above a head.
const FOOT_ROOM: f32 = 0.02;

/// A camera framing: centre and height in stage units (width follows from the aspect).
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
	centre: Pt,
	height: f32,
}

impl View {
	fn blend(self, other: View, t: f32) -> View {
		View {
			centre: self.centre.lerp(other.centre, t),
			height: lerp(self.height, other.height, t),
		}
	}
}

fn ease_out_back(t: f32) -> f32 {
	let c1 = 1.4;
	let c3 = c1 + 1.0;
	1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Ease in and out with a small wind-up before the move and a little overshoot after it:
/// anticipation and follow-through, two of the classic principles of animation.
fn anticipate(t: f32) -> f32 {
	let c = 1.1 * 1.525;

	if t < 0.5 {
		let x = 2.0 * t;
		x * x * ((c + 1.0) * x - c) / 2.0
	} else {
		let x = 2.0 * t - 2.0;
		f32::midpoint(x * x * ((c + 1.0) * x + c), 2.0)
	}
}

fn ease_out_bounce(t: f32) -> f32 {
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

fn smoothstep(t: f32) -> f32 {
	let t = t.clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// Everything needed to draw frames for one output shape.
pub struct Renderer<'a> {
	pub timeline: &'a Timeline,
	pub story: &'a Story,
	pub format: Format,
	pub width: u32,
	pub height: u32,
}

impl Renderer<'_> {
	/// Renders the frame at `time` seconds. `frame` drives line boil and grain.
	pub fn render(&self, time: f32, frame: u64) -> Result<Pixmap> {
		let mut pixmap = raster::render(&self.svg(time, frame))?;
		raster::finish(&mut pixmap, &self.story.style(), 0.03, (frame / 2) as u32);

		Ok(pixmap)
	}

	/// The frame as an SVG document.
	pub fn svg(&self, time: f32, frame: u64) -> String {
		let shot = self.timeline.shot_at(time);
		let local = time - shot.start;
		let (scene, puppets) = self.stage_at(shot, time);
		let view = self.camera(shot, time);
		let (w, h) = (self.width as f32, self.height as f32);
		let view_w = view.height * self.format.aspect();
		let scale = h / view.height;
		let mut svg = Svg::styled(self.story.style());
		svg.boil = frame / 4;

		// Parallax: the sky and hills travel half as far as the camera does, from where the shot
		// opened, so pans and push-ins reveal depth.
		let opening = self.target(shot, shot.start);
		let far = opening.blend(view, 0.5);
		let far_scale = h / far.height;
		let far_w = far.height * self.format.aspect();

		svg.fill(&Path::rect(0.0, 0.0, w, h), palette::PAPER);
		svg.begin(&format!(
			"scale({far_scale:.5}) translate({:.2} {:.2})",
			-(far.centre.x - far_w / 2.0),
			-(far.centre.y - far.height / 2.0)
		));
		let mut weather = scene.weather;
		weather.time = time;
		svg.begin_layer();
		settings::backdrop_layer(&mut svg, scene.setting, &weather, Layer::Far);
		svg.end();
		svg.end();

		svg.begin(&format!(
			"scale({scale:.5}) translate({:.2} {:.2})",
			-(view.centre.x - view_w / 2.0),
			-(view.centre.y - view.height / 2.0)
		));
		svg.begin_layer();
		compose::draw_near(&mut svg, &scene, &puppets, time);
		svg.end();
		svg.end();

		svg.line = 4.0 * w.min(h) / 1080.0;
		svg.registration = svg.style.registration * (0.6 * w.min(h) / 1080.0);

		match shot.kind {
			ShotKind::Title => self.title_overlay(&mut svg, local),
			ShotKind::Page(_) | ShotKind::End => self.captions(&mut svg, shot, time),
		}

		self.wipe(&mut svg, shot, local);

		svg.finish((0.0, 0.0, w, h), self.width, self.height)
	}

	/// The scene at `time`, with puppets blended between keyframes and brought to life.
	fn stage_at(&self, shot: &Shot, time: f32) -> (Scene, Vec<Puppet>) {
		let index = shot
			.keys
			.iter()
			.rposition(|key| key.time <= time)
			.unwrap_or(0);
		let current = &shot.keys[index];
		let previous = &shot.keys[index.saturating_sub(1)];
		let since = time - current.time;
		let progress = (since / TWEEN).clamp(0.0, 1.0);
		let fresh = index == 0;
		let before = compose::puppets(&previous.scene, time, false);
		let after = compose::puppets(&current.scene, time, false);
		let mut puppets = Vec::with_capacity(after.len());

		for puppet in &after {
			let old_actor = previous
				.scene
				.actors
				.iter()
				.find(|actor| actor.id == puppet.id);
			let new_actor = current
				.scene
				.actors
				.iter()
				.find(|actor| actor.id == puppet.id);
			let mut blended = match before.iter().find(|old| old.id == puppet.id) {
				Some(old) if !fresh => {
					let knocked_down = matches!((old_actor, new_actor), (Some(a), Some(b)) if b.knocked > a.knocked + 0.5);
					// Falls bounce; every other change winds up, overshoots and settles.
					let eased = if knocked_down {
						ease_out_bounce(progress)
					} else {
						anticipate(progress)
					};
					let mut moved = compose::blend(old, puppet, eased);
					follow_through(&mut moved, old, since);
					moved
				}
				_ if fresh => puppet.clone(),
				_ => {
					arrive(
						puppet,
						new_actor.and_then(|actor| actor.enter),
						progress,
						time,
					)
				}
			};

			if let (Some(old), Some(new)) = (old_actor, new_actor)
				&& new.hops > old.hops
				&& !fresh
			{
				hop(&mut blended, since);
			}

			puppets.push(blended);
		}

		if !fresh && progress < 1.0 {
			for old in before
				.iter()
				.filter(|old| !after.iter().any(|p| p.id == old.id))
			{
				let exit = current
					.scene
					.actors
					.iter()
					.find(|actor| actor.id == old.id)
					.and_then(|actor| actor.exit);
				puppets.push(leave(old, exit, progress, time));
			}
		}

		let speaking = speaker_at(shot, time)
			.and_then(|id| puppets.iter().find(|p| p.id == id).map(|p| (id, p.at.x)));

		for puppet in &mut puppets {
			self.animate(puppet, shot, time, speaking);
		}

		let mut scene = current.scene.clone();
		let weather_t = smoothstep(since / 1.2);

		if !fresh {
			scene.weather.wilt = lerp(
				previous.scene.weather.wilt,
				current.scene.weather.wilt,
				weather_t,
			);
			scene.weather.stink = lerp(
				previous.scene.weather.stink,
				current.scene.weather.stink,
				weather_t,
			);
			scene.weather.mint = lerp(
				previous.scene.weather.mint,
				current.scene.weather.mint,
				weather_t,
			);
		}

		(scene, puppets)
	}

	/// Acting between keyframes: speech, breath, listening, blinking.
	fn animate(&self, puppet: &mut Puppet, shot: &Shot, time: f32, speaking: Option<(&str, f32)>) {
		let mut breath: f32 = puppet.breath;
		let mut mouth: f32 = 0.0;
		let mut viseme = None;
		let mut shouting = false;

		for line in shot
			.lines
			.iter()
			.filter(|line| line.speakers.contains(&puppet.id))
		{
			let speech = &self.timeline.speeches[line.speech];
			let end = line.start + speech.duration();

			if time >= line.start && time < end {
				let t = time - line.start;
				mouth = mouth.max(speech.mouth(t) * 0.95);
				viseme = Some(speech.track.at(t));
				shouting |= line.text.contains("**");
				breath = breath.max(smoothstep(t / 0.3));
			} else if time >= end {
				breath = breath.max(1.0 - (time - end) / 1.6);
			}
		}

		puppet.breath = breath.clamp(0.0, 1.0);

		let Body::Figure(state) = &mut puppet.body else {
			return;
		};

		state.mouth_open = state.mouth_open.max(mouth);

		if viseme.is_some() {
			state.viseme = viseme;
			// Speakers nod into stressed syllables and lift their brows to shout.
			state.head_tilt += (mouth - 0.45) * 6.0 * puppet.heading;
			state.brow_lift += if shouting { 0.09 } else { mouth * 0.035 };
		} else if let Some((speaker, x)) = speaking
			&& speaker != puppet.id
			&& state.lie < 0.5
		{
			// Everyone else turns their eyes to whoever is talking, with a small listening tilt.
			let towards = (x - puppet.at.x).signum();
			state.gaze = towards * 0.9;
			state.head_tilt += towards * 3.0;
		}

		let seed = seed_of(&puppet.id);
		let period = 3.0 + (seed % 100) as f32 / 40.0;
		let phase = (time + (seed % 13) as f32 * 0.37) % period;

		if phase < 0.14 && state.lie < 0.5 {
			state.blink = 1.0 - (phase - 0.07).abs() / 0.07;
		}
	}

	/// The camera at `time`: framing targets at every keyframe and line, eased between.
	fn camera(&self, shot: &Shot, time: f32) -> View {
		let mut events: Vec<f32> = shot.keys.iter().map(|key| key.time).collect();
		events.extend(shot.lines.iter().map(|line| line.start));
		events.sort_by(f32::total_cmp);
		events.dedup_by(|a, b| (*a - *b).abs() < 0.05);

		// The camera sets off a beat early so it has arrived by the time someone speaks.
		let starts: Vec<f32> = events
			.iter()
			.map(|t| (t - CAMERA_LEAD).max(shot.start))
			.collect();
		let index = starts.iter().rposition(|t| *t <= time).unwrap_or(0);
		let current = self.target(shot, events[index]);
		let previous = if index == 0 {
			current
		} else {
			self.target(shot, events[index - 1])
		};
		let mut view = previous.blend(current, smoothstep((time - starts[index]) / CAMERA_EASE));

		// A slow push-in over each shot keeps still moments alive.
		let local = ((time - shot.start) / shot.duration().max(0.1)).clamp(0.0, 1.0);
		view.height *= 1.0 - 0.04 * local;

		// Big impacts bump the camera, fading out fast.
		for cue in self
			.timeline
			.cues
			.iter()
			.filter(|cue| matches!(cue.sound, Sound::Bonk | Sound::Whoosh))
		{
			let k = (time - cue.time) / 0.45;

			if (0.0..1.0).contains(&k) {
				let tick = (time * 60.0) as u64;
				let amount = (1.0 - k).powi(2) * view.height * 0.012;
				view.centre = view.centre + pt(noise(31, tick), noise(32, tick)) * amount;
			}
		}

		view
	}

	/// Where the camera wants to be once everything settles after `time`.
	fn target(&self, shot: &Shot, time: f32) -> View {
		let index = shot
			.keys
			.iter()
			.rposition(|key| key.time <= time + 1e-3)
			.unwrap_or(0);
		let scene = &shot.keys[index].scene;
		let puppets = compose::puppets(scene, time, false);
		let ground = scene.setting.ground();
		let aspect = self.format.aspect();

		if (scene.camera.zoom - 1.0).abs() > 0.01 {
			let (lx, ly) = scene.camera.look;
			let side = STAGE / scene.camera.zoom;

			return clamp(
				self.format,
				View {
					centre: pt(lx * STAGE, ly * STAGE),
					height: if aspect > 1.0 {
						side * 0.75
					} else {
						side * 1.25
					},
				},
			);
		}

		let speakers = speakers_near(shot, time);
		let star = self.story.meta.star.as_deref();
		let visible: Vec<&Puppet> = puppets.iter().filter(|p| p.id != "wormsley").collect();
		let focus: Vec<&Puppet> = if aspect < 1.0 {
			let talking: Vec<&Puppet> = visible
				.iter()
				.copied()
				.filter(|p| speakers.contains(&p.id))
				.collect();

			if talking.is_empty() {
				let starring: Vec<&Puppet> = visible
					.iter()
					.copied()
					.filter(|p| Some(p.id.split('.').next().unwrap_or("")) == star)
					.collect();

				if starring.is_empty() {
					visible.clone()
				} else {
					starring
				}
			} else {
				talking
			}
		} else {
			visible.clone()
		};

		match bounds(&focus) {
			Some(extent) => fit(self.format, extent, ground),
			None => {
				clamp(
					self.format,
					View {
						centre: pt(STAGE / 2.0, ground - STAGE * 0.3),
						height: if aspect > 1.0 { 1400.0 } else { 2300.0 },
					},
				)
			}
		}
	}

	fn title_overlay(&self, svg: &mut Svg, local: f32) {
		let (w, h) = (self.width as f32, self.height as f32);
		let unit = w.min(h) / 1080.0;
		let pop = ease_out_back(((local - 0.15) / 0.6).clamp(0.0, 1.0));

		if pop <= 0.0 {
			return;
		}

		let meta = &self.story.meta;
		let mut paragraphs = Vec::new();

		if let Some(series) = &meta.series {
			paragraphs.push(series.clone());
		}

		paragraphs.push(format!("**{}**", meta.title));
		let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
		let (area, size) = match self.format {
			Format::Landscape => {
				(
					Area {
						x: w * 0.1,
						y: h * 0.06,
						w: w * 0.8,
						h: h * 0.3,
					},
					70.0 * unit,
				)
			}
			Format::Portrait => {
				(
					Area {
						x: w * 0.07,
						y: h * 0.12,
						w: w * 0.86,
						h: h * 0.25,
					},
					74.0 * unit,
				)
			}
		};
		let block = text::layout(&refs, area, size, Anchor::Top);
		let centre = pt(area.x + area.w / 2.0, block.bounds.y + block.bounds.h / 2.0);

		svg.begin(&format!(
			"translate({:.1} {:.1}) scale({pop:.4}) translate({:.1} {:.1})",
			centre.x, centre.y, -centre.x, -centre.y
		));
		card(svg, &block.bounds, unit);
		svg.raw(&text::svg(&block, Ink::default()));
		svg.end();
	}

	fn captions(&self, svg: &mut Svg, shot: &Shot, time: f32) {
		let Some(line) = shot.lines.iter().find(|line| {
			let duration = self.timeline.speeches[line.speech].duration();
			time >= line.start && time < line.start + duration + 0.3
		}) else {
			return;
		};

		if line.text.is_empty() {
			return;
		}

		let duration = self.timeline.speeches[line.speech].duration();
		let limit = match self.format {
			Format::Landscape => 52,
			Format::Portrait => 30,
		};
		let chunks = chunk(&line.text, limit);
		let total: usize = chunks
			.iter()
			.map(|chunk| chunk.chars().count())
			.sum::<usize>()
			.max(1);
		let progress = ((time - line.start) / duration.max(0.1)).clamp(0.0, 0.999);
		let mut seen = 0;

		// Each chunk stays up for its share of the line, measured in characters.
		let Some(current) = chunks
			.iter()
			.find(|piece| {
				seen += piece.chars().count();
				seen as f32 / total as f32 > progress
			})
			.or(chunks.last())
			.cloned()
		else {
			return;
		};

		let (w, h) = (self.width as f32, self.height as f32);
		let unit = w.min(h) / 1080.0;
		let (top, height) = self.format.caption_band();
		let (x, size, anchor) = match self.format {
			Format::Landscape => (0.12, 50.0, Anchor::Bottom),
			// Clear of the buttons down the right edge of phone apps.
			Format::Portrait => (0.08, 66.0, Anchor::Top),
		};
		let area = Area {
			x: w * x,
			y: h * top,
			w: w * 0.76,
			h: h * height,
		};
		let block = text::layout_styled(
			&[current.as_str()],
			area,
			size * unit,
			anchor,
			RunStyle::Bold,
		);
		card(svg, &block.bounds, unit);
		svg.raw(&text::svg(&block, Ink::default()));
	}

	/// A paper sheet that sweeps across at the end of a page and away at the start of the next.
	fn wipe(&self, svg: &mut Svg, shot: &Shot, local: f32) {
		let (w, h) = (self.width as f32, self.height as f32);
		let first = shot.kind == ShotKind::Title;
		let last = std::ptr::eq(shot, self.timeline.shots.last().unwrap_or(shot));
		let remaining = shot.duration() - local;
		let cover = if !first && local < WIPE {
			Some((0.0, w * (1.0 - smoothstep(local / WIPE))))
		} else if !last && remaining < WIPE {
			Some((w * smoothstep(remaining / WIPE), w))
		} else {
			None
		};

		if let Some((x0, x1)) = cover {
			if x1 - x0 < 1.0 {
				return;
			}

			// The sheet runs from its wobbly leading edge to the far side of the screen.
			let (edge, far) = if x0 > 0.0 { (x0, x1) } else { (x1, x0) };
			let points: Vec<Pt> = (0..=12)
				.map(|i| {
					let y = i as f32 / 12.0 * (h + 20.0) - 10.0;
					pt(edge + (y / h * 9.0).sin() * 14.0, y)
				})
				.collect();
			let edge_line = Path::spline(&points);
			let sheet = edge_line
				.clone()
				.line_to(pt(far, h + 10.0))
				.line_to(pt(far, -10.0))
				.close();
			svg.fill(&sheet, palette::PAPER);
			svg.stroke(&edge_line, palette::INK, svg.line * 1.5);
		}

		if last && remaining < 0.6 {
			let fade = 1.0 - remaining / 0.6;
			svg.fill_alpha(&Path::rect(0.0, 0.0, w, h), palette::PAPER, fade);
		}
	}
}

/// Who is talking at `time` (lingering briefly after the line so listeners don't snap away).
fn speaker_at(shot: &Shot, time: f32) -> Option<&str> {
	shot.lines
		.iter()
		.rfind(|line| line.start <= time && time < line.start + line.duration + 0.6)
		.and_then(|line| line.speakers.first())
		.map(String::as_str)
}

/// Speakers of the line playing at (or most recently before) `time`.
fn speakers_near(shot: &Shot, time: f32) -> Vec<String> {
	shot.lines
		.iter()
		.rfind(|line| line.start <= time + 0.05)
		.map(|line| line.speakers.clone())
		.unwrap_or_default()
}

/// Frames puppets spanning `extent` (left, top, right, bottom) who stand on `ground`: wide enough
/// for everyone, with the ground line and the tallest head inside the format's action band.
fn fit(format: Format, extent: (f32, f32, f32, f32), ground: f32) -> View {
	let (left, top, right, _) = extent;
	let aspect = format.aspect();
	let (top_at, ground_at) = format.action_band();
	// Wide shots are limited by the drawn stage, so place the ground with that height; `clamp`
	// would otherwise shrink the view around its centre and slide the ground into the captions.
	// A cast too tall for the band loses the top of the tallest hairdo rather than its feet.
	let (min_h, max_h) = if aspect > 1.0 {
		(1300.0, tallest_view(format))
	} else {
		(1900.0, 2600.0)
	};
	let wide = (right - left) * 1.15 / aspect;
	let tall = (ground - top) / (ground_at - top_at);
	let mut height = wide.max(tall).clamp(min_h, max_h);

	if aspect < 1.0 && wide > max_h {
		// Too spread out for a phone screen: favour the nearest speaker or the star.
		height = tall.clamp(min_h, max_h);
	}

	let view_bottom = ground + height * (1.0 - ground_at);
	let centre = pt(f32::midpoint(left, right), view_bottom - height / 2.0);

	clamp(format, View { centre, height })
}

/// The tallest view that stays on drawn stage in both directions.
fn tallest_view(format: Format) -> f32 {
	let drawn = STAGE + OVERDRAW * 2.0;
	drawn.min(drawn / format.aspect())
}

/// Keeps the view on drawn stage.
fn clamp(format: Format, view: View) -> View {
	let aspect = format.aspect();
	let height = view.height.min(tallest_view(format));
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

/// Rough stage-space bounds of puppets: (left, top, right, bottom).
fn bounds(puppets: &[&Puppet]) -> Option<(f32, f32, f32, f32)> {
	puppets
		.iter()
		.map(|p| extent(p))
		.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

fn extent(puppet: &Puppet) -> (f32, f32, f32, f32) {
	let s = puppet.scale;
	let (x, y) = (puppet.at.x, puppet.at.y);

	match &puppet.kind {
		Kind::Figure(look) => {
			let frame = Frame::of(look.build);
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

/// An actor arriving: walking in from the side they enter from, or popping into place.
fn arrive(puppet: &Puppet, enter: Option<Side>, progress: f32, time: f32) -> Puppet {
	let mut arriving = puppet.clone();

	if let Some(side) = enter {
		let start = match side {
			Side::Left => -400.0,
			Side::Right => STAGE + 400.0,
		};
		let eased = smoothstep(progress * 1.3);
		arriving.at.x = lerp(start, puppet.at.x, eased);
		walk(&mut arriving, time, 1.0 - eased);
	} else {
		let pop = ease_out_back(progress);
		arriving.scale *= pop.max(0.0);
		arriving.opacity = (progress * 3.0).min(1.0);
	}

	arriving
}

/// An actor leaving: walking off their exit side, or shrinking away.
fn leave(puppet: &Puppet, exit: Option<Side>, progress: f32, time: f32) -> Puppet {
	let mut leaving = puppet.clone();

	if let Some(side) = exit {
		let end = match side {
			Side::Left => -400.0,
			Side::Right => STAGE + 400.0,
		};
		leaving.at.x = lerp(puppet.at.x, end, smoothstep(progress));
		walk(&mut leaving, time, 1.0);
	} else {
		leaving.scale *= 1.0 - smoothstep(progress);
		leaving.opacity = 1.0 - progress;
	}

	leaving
}

/// A walk: people use the rig's walk cycle; animals bob along.
fn walk(puppet: &mut Puppet, time: f32, amount: f32) {
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
fn hop(puppet: &mut Puppet, since: f32) {
	let t = since / 0.5;

	if !(0.0..1.5).contains(&t) {
		return;
	}

	if t < 1.0 {
		puppet.lift += (t * PI).sin() * 120.0 * puppet.scale;
	}

	if let Body::Figure(state) = &mut puppet.body {
		state.squash = if t < 0.9 {
			1.0 + 0.09 * (t / 0.9 * PI).sin()
		} else {
			1.0 - 0.16 * ((t - 0.9) / 0.6 * PI).sin()
		};
		state.sway = -((t * PI).sin()) * 0.6;
	}
}

/// After a move, loose parts keep going and swing back: a damped spring, solved in closed form
/// so any frame can be rendered on its own (frames are drawn in parallel).
fn follow_through(puppet: &mut Puppet, before: &Puppet, since: f32) {
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
	state.sway += direction * push * (-3.2 * since).exp() * (11.0 * since).sin();
}

/// Splits a paragraph into caption-sized pieces, preferring breaks after punctuation.
fn chunk(text: &str, limit: usize) -> Vec<String> {
	let mut chunks: Vec<String> = Vec::new();
	let mut current = String::new();

	for word in text.split_whitespace() {
		if !current.is_empty() && visible_len(&current) + 1 + visible_len(word) > limit {
			chunks.push(std::mem::take(&mut current));
		}

		if !current.is_empty() {
			current.push(' ');
		}

		current.push_str(word);

		let length = visible_len(&current);
		let ends_sentence = word.trim_end_matches(['"', '*']).ends_with(['.', '!', '?']);
		let ends_clause = word.trim_end_matches(['"', '*']).ends_with([',', ';', ':']);

		if (ends_sentence && length > limit / 3) || (ends_clause && length > limit / 2) {
			chunks.push(std::mem::take(&mut current));
		}
	}

	if !current.is_empty() {
		chunks.push(current);
	}

	// A lone word on screen reads as a glitch; fold short tails back into the previous chunk.
	if chunks.len() > 1 {
		let tail = chunks[chunks.len() - 1].clone();
		let before = chunks.len() - 2;

		if visible_len(&tail) < limit / 3
			&& visible_len(&chunks[before]) + 1 + visible_len(&tail) <= limit * 13 / 10
		{
			chunks.pop();
			chunks[before].push(' ');
			chunks[before].push_str(&tail);
		}
	}

	balance_markers(chunks)
}

/// Characters a reader sees (markup asterisks excluded).
fn visible_len(text: &str) -> usize {
	text.chars().filter(|ch| *ch != '*').count()
}

/// Re-opens `**` or `*` markup that a chunk boundary split in two.
fn balance_markers(chunks: Vec<String>) -> Vec<String> {
	let mut out = Vec::with_capacity(chunks.len());
	let mut carry = "";

	for chunk in chunks {
		let mut fixed = format!("{carry}{chunk}");
		let shouts = fixed.matches("**").count();
		let singles = fixed.matches('*').count() - shouts * 2;
		carry = "";

		if shouts % 2 == 1 {
			fixed.push_str("**");
			carry = "**";
		} else if singles % 2 == 1 {
			fixed.push('*');
			carry = "*";
		}

		out.push(fixed);
	}

	out
}

/// A wobbly paper card behind overlay text.
fn card(svg: &mut Svg, bounds: &Area, unit: f32) {
	let pad_x = 34.0 * unit;
	let pad_y = CARD_PAD_Y * unit;
	let card = Path::wobbly_poly(
		&[
			pt(bounds.x - pad_x, bounds.y - pad_y),
			pt(bounds.x + bounds.w + pad_x, bounds.y - pad_y),
			pt(bounds.x + bounds.w + pad_x, bounds.y + bounds.h + pad_y),
			pt(bounds.x - pad_x, bounds.y + bounds.h + pad_y),
		],
		22.0 * unit,
		2.0 * unit,
		svg.seed(17),
	);
	svg.inked(&card, palette::PAPER);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn captions_split_on_phrases_and_keep_markup_balanced() {
		let chunks = chunk(
			"\"**YOUR BREATH SMELLS LIKE A SNOWY MOUNTAIN!**\" shouted Shouty McShoutface.",
			26,
		);
		assert!(chunks.len() >= 3, "{chunks:?}");

		for piece in &chunks {
			assert_eq!(piece.matches("**").count() % 2, 0, "{piece}");
			assert!(
				crate::story::plain_text(piece).chars().count() <= 30,
				"{piece}"
			);
		}
	}

	#[test]
	fn short_lines_stay_whole() {
		assert_eq!(chunk("Hello, birds!", 30), vec!["Hello, birds!".to_owned()]);
	}

	/// Where a stage height lands in the frame, as a fraction from the top.
	fn on_screen(view: View, y: f32) -> f32 {
		(y - (view.centre.y - view.height / 2.0)) / view.height
	}

	#[test]
	fn framed_bodies_stay_clear_of_the_caption_card() {
		use crate::art::figure::Build;

		let ground = 2060.0;
		let kid = Frame::of(Build::Kid).height() * 2.25;
		let grownup = Frame::of(Build::Grownup).height() * 1.85;
		// Tallest standing body and the cast's left and right edges.
		let casts = [
			(kid, 1000.0, 1100.0),
			(grownup, 1000.0, 1100.0),
			(kid, 300.0, 2100.0),
		];

		for format in [Format::Landscape, Format::Portrait] {
			let (top, height) = format.caption_band();
			let card = (top - format.card_pad(), top + height + format.card_pad());

			for (tallest, left, right) in casts {
				// Sitting, crouching and kneeling only lower the head, so standing is the worst case.
				let crown = ground - tallest - 20.0;
				let view = fit(format, (left, crown, right, ground + 20.0), ground);
				let (head, feet) = (on_screen(view, crown), on_screen(view, ground + 20.0));

				// A 16:9 view can't be wider than the drawn stage, so a grown-up's hairdo may just
				// touch the top edge; their face never leaves the frame.
				assert!(head >= -0.03 && feet <= 1.0, "{format:?}: {head}..{feet}");
				assert!(
					feet < card.0 || head > card.1,
					"{format:?}: body {head}..{feet} overlaps captions {card:?}"
				);
			}
		}
	}

	#[test]
	fn moves_wind_up_then_overshoot() {
		assert!(anticipate(0.0).abs() < 1e-5);
		assert!((anticipate(1.0) - 1.0).abs() < 1e-5);
		assert!(
			anticipate(0.12) < 0.0,
			"a move starts by winding up the other way"
		);
		assert!(anticipate(0.88) > 1.0, "and overshoots before it settles");
	}

	#[test]
	fn easing_lands_on_target() {
		assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
		assert!((ease_out_bounce(1.0) - 1.0).abs() < 1e-5);
		assert!(ease_out_back(0.0).abs() < 1e-5);
		assert!(ease_out_back(0.6) > 1.0, "back easing overshoots");
	}
}
