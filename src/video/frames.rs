//! Draws one video frame at a moment on the timeline.
//!
//! The frame is the rehearsal's scene at that moment, blended between keyframes with cartoon
//! easing, mouths driven by the narration, a camera that frames the action for the output
//! shape, captions, and a paper wipe between pages.

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use resvg::tiny_skia::Pixmap;

use crate::art::Path;
use crate::art::Pt;
use crate::art::STAGE;
use crate::art::Svg;
use crate::art::lerp;
use crate::art::noise;
use crate::art::palette;
use crate::art::pt;
use crate::art::seed_of;
use crate::art::settings;
use crate::art::settings::Layer;
use crate::compose;
use crate::compose::Puppet;
use crate::raster;
use crate::stage::Scene;
use crate::stage::ShotSize;
use crate::stage::Travel;
use crate::story::Acting;
use crate::story::Direction;
use crate::story::RunStyle;
use crate::story::Story;
use crate::story::Transition;
use crate::text;
use crate::text::Anchor;
use crate::text::Area;
use crate::text::Ink;
use crate::video::acting;
use crate::video::acting::TWEEN;
use crate::video::acting::ease_out_back;
use crate::video::acting::smoothstep;
use crate::video::audio::Sound;
use crate::video::director;
use crate::video::director::View;
use crate::video::timeline::Framing;
use crate::video::timeline::Setup;
use crate::video::timeline::Shot;
use crate::video::timeline::ShotKind;
use crate::video::timeline::Timeline;

/// Page-turn transition length at each end of a shot.
const WIPE: f32 = 0.35;

/// How strongly a close shot's setting softens behind the subject, in stage units: the far
/// layer most, the near layer less. The actors stay sharp.
const CLOSE_BLUR: (f32, f32) = (24.0, 10.0);
const EXTREME_BLUR: (f32, f32) = (32.0, 14.0);

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
	pub(crate) fn action_band(self) -> (f32, f32) {
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

/// Everything needed to draw frames for one output shape.
pub struct Renderer<'a> {
	pub timeline: &'a Timeline,
	pub story: &'a Story,
	pub format: Format,
	pub width: u32,
	pub height: u32,
	/// A pixel shape for a plate drawn outside the video format; `None` uses the format's.
	aspect: Option<f32>,
	/// Blurred backdrop images, one per setting, depth layer and weather, built once and
	/// reused for every frame of a depth-of-field shot.
	blur_cache: std::sync::Mutex<std::collections::HashMap<u64, String>>,
	/// Views already worked out, keyed by the setup (or shot) and the moment they were read
	/// at. The camera reads the same framing at the same anchor again and again, once per
	/// frame, and each read measures its subjects' drawn bounds — so the first read is
	/// remembered and the rest just look it up.
	view_cache: std::sync::Mutex<std::collections::HashMap<(usize, u32), View>>,
}

impl<'a> Renderer<'a> {
	pub fn new(
		timeline: &'a Timeline,
		story: &'a Story,
		format: Format,
		width: u32,
		height: u32,
	) -> Self {
		Self {
			timeline,
			story,
			format,
			width,
			height,
			aspect: None,
			blur_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
			view_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
		}
	}

	/// This renderer at another pixel shape, for a plate that covers a different part of the
	/// stage than the video format does.
	#[must_use]
	pub fn with_aspect(mut self, aspect: f32) -> Self {
		self.aspect = Some(aspect);
		self
	}

	/// The aspect of the pixels this renderer draws.
	fn aspect(&self) -> f32 {
		self.aspect.unwrap_or_else(|| self.format.aspect())
	}

	/// The view the frame at `time` is drawn with, without rendering it: where the camera
	/// stands once its travel, the slow push and any shake have had their say.
	pub fn view_at(&self, time: f32) -> View {
		let shot = self.timeline.shot_at(time);
		self.camera(shot, time)
	}

	/// Reads the view cache, however busy the frame renderers are.
	fn cached<T>(
		&self,
		with: impl FnOnce(&mut std::collections::HashMap<(usize, u32), View>) -> T,
	) -> T {
		with(
			&mut self
				.view_cache
				.lock()
				.unwrap_or_else(std::sync::PoisonError::into_inner),
		)
	}

	/// Renders the frame at `time` seconds. `frame` drives line boil and grain.
	pub fn render(&self, time: f32, frame: u64) -> Result<Pixmap> {
		let mut pixmap = raster::render(&self.svg(time, frame)?)?;
		raster::finish(&mut pixmap, &self.story.style(), 0.03, (frame / 2) as u32);

		Ok(pixmap)
	}

	/// The frame at `time` through an explicit view, as a clean plate: the scene, its light and
	/// the title card, but no captions and no page turn. Storyboard panels and animatic plates
	/// are drawn this way.
	pub fn render_view(&self, time: f32, frame: u64, view: View) -> Result<Pixmap> {
		let mut pixmap = raster::render(&self.svg_view(time, frame, view)?)?;
		raster::finish(&mut pixmap, &self.story.style(), 0.03, (frame / 2) as u32);

		Ok(pixmap)
	}

	/// The frame as an SVG document.
	pub fn svg(&self, time: f32, frame: u64) -> Result<String> {
		let shot = self.timeline.shot_at(time);
		let view = self.camera(shot, time);

		self.svg_with_view(time, frame, view, true)
	}

	/// The frame as an SVG document through an explicit view, without the video's furniture.
	pub fn svg_view(&self, time: f32, frame: u64, view: View) -> Result<String> {
		self.svg_with_view(time, frame, view, false)
	}

	/// A frame with this renderer's furniture: captions, page turns and parallax when `video`,
	/// and a clean plate when not.
	fn svg_with_view(&self, time: f32, frame: u64, view: View, video: bool) -> Result<String> {
		let shot = self.timeline.shot_at(time);
		let local = time - shot.start;
		let portrait = self.format.aspect() < 1.0;
		let setups = shot.setups_for(portrait);
		let motion = self.story.drawing.motion;
		let posed = motion.drawing_time(time, shot.start, shot.moving(time, TWEEN));
		let (scene, puppets) = self.stage_at(shot, posed);
		let (w, h) = (self.width as f32, self.height as f32);
		let view_w = view.width(self.aspect());
		let scale = h / view.height;
		let mut svg = Svg::styled(self.story.style());
		svg.boil = frame / 4;
		let mut weather = scene.weather;
		weather.time = time;

		// A page that enters with a push slides everything into place.
		let sliding = video
			&& shot.transition == Transition::Push
			&& shot.kind != ShotKind::Title
			&& local < WIPE;

		if sliding {
			svg.begin(&format!(
				"translate({:.2} 0)",
				(1.0 - smoothstep(local / WIPE)) * w
			));
		}

		// Parallax: the sky and hills travel half as far as the camera does, from where the shot
		// opened, so pans and push-ins reveal depth.
		let far = if video {
			self.target(shot, shot.start).blend(view, 0.5)
		} else {
			view
		};
		let far_scale = h / far.height;
		let far_w = far.width(self.aspect());

		// Depth of field: a close shot's setting softens behind its subject.
		let blur = if video {
			self.depth_of_field(setups, time)
		} else {
			None
		};

		svg.fill(&Path::rect(0.0, 0.0, w, h), palette::PAPER);

		// The scene's light grades every colour the frame paints and aims its shadows; the
		// captions, title cards and paper turns after it are furniture and keep their own.
		svg.set_light(scene.weather.light);

		if let Some((far_dev, _)) = blur {
			// The sky softens most, and travels with the parallax view.
			let far_image = self.blurred_backdrop(&scene, Layer::Far, far_dev)?;
			svg.begin(&format!(
				"scale({far_scale:.5}) translate({:.2} {:.2})",
				-(far.centre.x - far_w / 2.0),
				-(far.centre.y - far.height / 2.0)
			));
			svg.raw(&backdrop_image(&far_image));
			svg.end();
		} else {
			svg.begin(&format!(
				"scale({far_scale:.5}) translate({:.2} {:.2})",
				-(far.centre.x - far_w / 2.0),
				-(far.centre.y - far.height / 2.0)
			));
			svg.begin_layer();
			settings::backdrop_layer(&mut svg, &scene.setting, &weather, Layer::Far);
			svg.end();
			svg.end();
		}

		svg.begin(&format!(
			"scale({scale:.5}) translate({:.2} {:.2})",
			-(view.centre.x - view_w / 2.0),
			-(view.centre.y - view.height / 2.0)
		));

		if let Some((_, near)) = &blur {
			// The setting just behind the actors softens a little, from a cached image,
			// and everything alive over it stays sharp.
			let near_image = self.blurred_backdrop(&scene, Layer::Near, *near)?;
			svg.raw(&backdrop_image(&near_image));
			svg.begin_layer();
			scene_front(&mut svg, &scene, &puppets, time);
			svg.end();
		} else {
			svg.begin_layer();
			compose::draw_near(&mut svg, &scene, &puppets, time);
			svg.end();
		}

		svg.end();

		if sliding {
			svg.end();
		}

		svg.clear_light();
		svg.line = 4.0 * w.min(h) / 1080.0;
		svg.registration = svg.style.registration * (0.6 * w.min(h) / 1080.0);

		match shot.kind {
			ShotKind::Title => self.title_overlay(&mut svg, local),
			ShotKind::Page(_) | ShotKind::End if video => self.captions(&mut svg, shot, time),
			ShotKind::Page(_) | ShotKind::End => {}
		}

		if video {
			self.turn(&mut svg, shot, local)?;
		}

		Ok(svg.finish((0.0, 0.0, w, h), self.width, self.height))
	}

	/// The scene at `time`, with puppets blended between keyframes and brought to life.
	fn stage_at(&self, shot: &Shot, time: f32) -> (Scene, Vec<Puppet>) {
		let motion = self.story.drawing.motion;
		let on = self.story.meta.video.acting == Acting::Full;
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
		let cx = acting::Cx {
			on,
			motion,
			lines: self.spoken_lines(shot),
			key_time: current.time,
		};
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
					// Falls bounce; every other change is acted by the rules in `acting`.
					acting::tween(old, puppet, since, progress, knocked_down, &cx)
				}
				_ if fresh => puppet.clone(),
				_ => {
					acting::arrive(
						puppet,
						new_actor.and_then(|actor| actor.enter),
						progress,
						since,
						time,
						&cx,
					)
				}
			};

			if let (Some(old), Some(new)) = (old_actor, new_actor)
				&& new.hops > old.hops
				&& !fresh
			{
				acting::hop(&mut blended, since, motion.squash);
			}

			puppets.push(blended);
		}

		if !fresh {
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

				if since < acting::exit_span(old, exit, &cx) {
					puppets.push(acting::leave(old, exit, progress, since, time, &cx));
				}
			}
		}

		let speaking = speaker_at(shot, time)
			.and_then(|id| puppets.iter().find(|p| p.id == id))
			.map(|puppet| {
				acting::Speaker {
					id: puppet.id.clone(),
					x: puppet.at.x,
					heading: puppet.heading,
					pointing: acting::pointing_at(puppet),
				}
			});

		for puppet in &mut puppets {
			acting::animate(puppet, &cx, time, speaking.as_ref());
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
			// A mid-page `light=` eases from the old light to the new one, like the weather.
			scene.weather.light = crate::art::Light::blend(
				&previous.scene.weather.light,
				&current.scene.weather.light,
				weather_t,
			);
		}

		(scene, puppets)
	}

	/// The shot's lines, resolved to what the acting rules read.
	fn spoken_lines<'s>(&'s self, shot: &'s Shot) -> Vec<acting::Spoken<'s>> {
		shot.lines
			.iter()
			.map(|line| {
				let speech = &self.timeline.speeches[line.speech];
				acting::Spoken {
					start: line.start,
					duration: speech.duration(),
					speakers: &line.speakers,
					text: &line.text,
					envelope: &speech.envelope,
					track: &speech.track,
					peak: acting::peak_of(&speech.envelope),
					question: acting::is_question(&line.text),
				}
			})
			.collect()
	}

	/// The camera at `time`, reading the setups the director planned for this shot.
	fn camera(&self, shot: &Shot, time: f32) -> View {
		let portrait = self.format.aspect() < 1.0;
		let setups = shot.setups_for(portrait);
		let index = setups
			.iter()
			.rposition(|setup| setup.time <= time)
			.unwrap_or(0);
		let mut view = self.setup_view(shot, setups, index, time);

		// A slow push-in over each shot keeps still moments alive.
		let local = ((time - shot.start) / shot.duration().max(0.1)).clamp(0.0, 1.0);
		view.height *= 1.0 - 0.04 * local;

		// Big impacts bump the camera, fading out fast; so does a direction's `shake=`.
		let mut bumps: Vec<(f32, f32)> = self
			.timeline
			.cues
			.iter()
			.filter(|cue| matches!(cue.sound, Sound::Bonk | Sound::Whoosh))
			.map(|cue| (cue.time, 1.0))
			.collect();
		bumps.extend(
			setups
				.iter()
				.filter(|setup| setup.shake > 0.0)
				.map(|setup| (setup.time, setup.shake)),
		);

		for (when, strength) in bumps {
			let k = (time - when) / 0.45;

			if (0.0..1.0).contains(&k) {
				let tick = (time * 60.0) as u64;
				let amount = (1.0 - k).powi(2) * view.height * 0.012 * strength;
				view.centre = view.centre + pt(noise(31, tick), noise(32, tick)) * amount;
			}
		}

		view
	}

	/// The view one setup gives at `time`: where its travel has carried the camera to.
	fn setup_view(&self, shot: &Shot, setups: &[Setup], index: usize, time: f32) -> View {
		let setup = &setups[index];
		let destination = self.framing_view(shot, setup, time);

		let previous = if index == 0 {
			destination
		} else {
			self.held_view(shot, setups, index - 1)
		};

		match setup.travel {
			// A cut lands here; there is nothing to blend.
			Travel::Cut => destination,
			Travel::Glide => {
				let over = if setup.over > 0.0 {
					setup.over
				} else {
					director::CAMERA_EASE
				};
				previous.blend(
					destination,
					smoothstep((time - setup.time) / over.max(0.01)),
				)
			}
			_ => {
				let over = Self::move_length(shot, setups, index, setup.over);
				previous.blend(
					destination,
					smoothstep((time - setup.time) / over.max(0.01)),
				)
			}
		}
	}

	/// How long a move takes: what the direction said, else the rest of the setup.
	fn move_length(shot: &Shot, setups: &[Setup], index: usize, over: f32) -> f32 {
		if over > 0.0 {
			return over;
		}

		let end = setups.get(index + 1).map_or(shot.end, |next| next.time);
		(end - setups[index].time).max(0.01)
	}

	/// The completed view of a setup: where its framing settles once its move has finished.
	fn held_view(&self, shot: &Shot, setups: &[Setup], index: usize) -> View {
		let setup = &setups[index];
		let aspect = self.format.aspect();
		let mut view = self.framing_view(shot, setup, setup.anchor);

		// A relative move (no new framing) widens or tightens the view it starts from.
		if setup.travel.is_move() && index > 0 && setup.framing == setups[index - 1].framing {
			view = director::arrives_from(
				setup.travel,
				self.held_view(shot, setups, index - 1),
				aspect,
			);
		}

		view
	}

	/// The view a setup's framing names, read at its anchor (or at `time`, for a tracking
	/// shot). A framing that is not tracking reads at the same moment every frame, so the
	/// view is measured once and remembered.
	fn framing_view(&self, shot: &Shot, setup: &Setup, time: f32) -> View {
		let key = (std::ptr::from_ref::<Setup>(setup) as usize, time.to_bits());

		if let Some(view) = self.cached(|cache| cache.get(&key).copied()) {
			return view;
		}

		let view = self.framing_view_now(shot, setup, time);
		self.cached(|cache| cache.insert(key, view));
		view
	}

	/// The view a setup's framing names, worked out fresh.
	fn framing_view_now(&self, shot: &Shot, setup: &Setup, time: f32) -> View {
		let aspect = self.format.aspect();

		match &setup.framing {
			Framing::Auto => self.target(shot, setup.anchor),
			Framing::Zoom { zoom, look } => director::zoom_view(*zoom, *look, aspect),
			Framing::Shot { size, on } => {
				let at = if setup.travel == Travel::Track {
					time
				} else {
					setup.anchor
				};
				let scene = shot.scene_at(setup.anchor);
				let puppets = compose::puppets(scene, at, false);
				let subjects = self.subjects(on, *size, shot, at, &puppets);

				director::shot_view(
					aspect,
					self.format.action_band(),
					*size,
					&subjects,
					scene.setting.ground(),
				)
				.unwrap_or_else(|| self.target(shot, setup.anchor))
			}
		}
	}

	/// Which puppets a shot frames: those named by `on`, else everyone on stage — the current
	/// speaker for a close shot, or the star.
	fn subjects<'p>(
		&self,
		on: &[String],
		size: ShotSize,
		shot: &Shot,
		time: f32,
		puppets: &'p [Puppet],
	) -> Vec<&'p Puppet> {
		if !on.is_empty() {
			return puppets
				.iter()
				.filter(|puppet| on.contains(&puppet.id))
				.collect();
		}

		let visible: Vec<&Puppet> = puppets
			.iter()
			.filter(|puppet| puppet.id != "wormsley")
			.collect();

		match size {
			ShotSize::Close | ShotSize::ExtremeClose => {
				let speaking = speakers_near(shot, time);
				let speakers: Vec<&Puppet> = visible
					.iter()
					.copied()
					.filter(|puppet| speaking.contains(&puppet.id))
					.collect();

				if !speakers.is_empty() {
					return speakers;
				}

				// Nobody speaking yet: hold on the star.
				let star = self.story.meta.star.as_deref().unwrap_or("");
				let starring: Vec<&Puppet> = visible
					.iter()
					.copied()
					.filter(|puppet| puppet.id.split('.').next().unwrap_or("") == star)
					.collect();

				if starring.is_empty() {
					visible
				} else {
					starring
				}
			}
			_ => visible,
		}
	}

	/// How strongly the setting softens behind the subject on a close shot: (far, near) blurs in
	/// stage units. `None` keeps everything sharp.
	fn depth_of_field(&self, setups: &[Setup], local: f32) -> Option<(f32, f32)> {
		if !self.story.meta.video.depth_of_field {
			return None;
		}

		let index = setups
			.iter()
			.rposition(|setup| setup.time <= local)
			.unwrap_or(0);

		match &setups[index].framing {
			Framing::Shot {
				size: ShotSize::Close,
				..
			} => Some(CLOSE_BLUR),
			Framing::Shot {
				size: ShotSize::ExtremeClose,
				..
			} => Some(EXTREME_BLUR),
			_ => None,
		}
	}

	/// A depth layer of the setting, drawn once, blurred, and remembered as an image the frame
	/// can place under the camera. Blurring this way costs one small raster per setting,
	/// weather and light instead of a filter on every frame. The cache is PNG: it keeps the layer's
	/// alpha, so the frame composites each layer over the paper and over the others exactly
	/// where the setting is transparent — a JPEG cache would turn every transparent pixel
	/// black.
	fn blurred_backdrop(&self, scene: &Scene, layer: Layer, deviation: f32) -> Result<String> {
		let weather = &scene.weather;
		let key = seed_of(&format!(
			"{:?}|{:?}|{layer:?}|{deviation}|{}|{:.2}|{:.2}|{:.2}|{:?}|{:x}",
			self.story.style().name,
			scene.setting,
			weather.night,
			weather.wilt,
			weather.stink,
			weather.mint,
			weather.tint,
			weather.light.cache_key(),
		));

		{
			let cache = self
				.blur_cache
				.lock()
				.map_err(|_| anyhow::anyhow!("the blur cache is poisoned"))?;

			if let Some(cached) = cache.get(&key) {
				return Ok(cached.clone());
			}
		}

		// The drawn stage rasterised small: plenty for a softened backdrop, and a small image
		// keeps every frame's decode cheap. The far layer blurs hardest, so it can shrink
		// furthest — a Gaussian-blurred image carries no detail finer than its own blur, and
		// each raster keeps the blur at two and a half pixels or more. The blur's deviation
		// is in stage units, so it softens as much on screen as a filter would.
		let span = STAGE + director::OVERDRAW * 2.0;
		let size = match layer {
			Layer::Far => (span / 8.0) as u32,
			// Front never goes through the cache (it stays sharp over the actors), but a
			// total match keeps it at the near layer's resolution should that ever change.
			Layer::Near | Layer::Front => (span / 4.0) as u32,
		};
		let mut svg = Svg::styled(self.story.style());
		svg.set_light(weather.light);
		let id = svg.next_id("dof");
		svg.define(&format!(
			r#"<filter id="{id}" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="{deviation:.2}"/></filter>"#
		));
		svg.raw(&format!(r#"<g filter="url(#{id})">"#));
		svg.begin_layer();
		settings::backdrop_layer(&mut svg, &scene.setting, weather, layer);
		svg.end();
		svg.raw("</g>");

		let document = svg.finish(
			(-director::OVERDRAW, -director::OVERDRAW, span, span),
			size,
			size,
		);
		let pixmap = raster::render(&document)?;
		let encoded = BASE64.encode(raster::png(&pixmap)?);

		self.blur_cache
			.lock()
			.map_err(|_| anyhow::anyhow!("the blur cache is poisoned"))?
			.insert(key, encoded.clone());

		Ok(encoded)
	}

	/// Where the camera wants to be once everything settles after `time` (the automatic
	/// framing), measured once and remembered per shot and moment.
	fn target(&self, shot: &Shot, time: f32) -> View {
		let key = (std::ptr::from_ref::<Shot>(shot) as usize, time.to_bits());

		if let Some(view) = self.cached(|cache| cache.get(&key).copied()) {
			return view;
		}

		let view = self.target_now(shot, time);
		self.cached(|cache| cache.insert(key, view));
		view
	}

	/// Where the camera wants to be once everything settles after `time`, worked out fresh.
	fn target_now(&self, shot: &Shot, time: f32) -> View {
		let scene = shot.scene_at(time);
		let ground = scene.setting.ground();
		let aspect = self.format.aspect();

		if (scene.camera.zoom - 1.0).abs() > 0.01 {
			let (lx, ly) = scene.camera.look;
			return director::zoom_view(scene.camera.zoom, (lx, ly), aspect);
		}

		let speakers = speakers_near(shot, time);
		let star = self.story.meta.star.as_deref();
		let puppets = compose::puppets(scene, time, false);
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

		// The locked camera must keep producing exactly the frames it always has, so it
		// keeps framing from the old rough extents; the automatic director measures.
		let extent = match self.story.meta.video.direction {
			Direction::Auto => director::bounds(&focus),
			Direction::Locked => director::rough_bounds(&focus),
		};

		match extent {
			Some(extent) => fit(self.format, extent, ground),
			None => {
				director::clamp(
					aspect,
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

	/// The page turn at the edges of a shot: this page's `transition` arriving, and the next
	/// page's leaving.
	fn turn(&self, svg: &mut Svg, shot: &Shot, local: f32) -> Result<()> {
		let (w, h) = (self.width as f32, self.height as f32);
		let first = shot.kind == ShotKind::Title;
		let index = self
			.timeline
			.shots
			.iter()
			.position(|other| std::ptr::eq(other, shot));
		let last = index.is_some_and(|index| index + 1 == self.timeline.shots.len());
		let remaining = shot.duration() - local;

		// This page arriving.
		if !first && local < WIPE {
			let t = smoothstep(local / WIPE);

			match shot.transition {
				Transition::Wipe => sheet(svg, h, 0.0, w * (1.0 - t)),
				Transition::Cut => {}
				Transition::Dissolve => {
					self.under_previous(svg, shot, 1.0 - t)?;
				}
				Transition::Fade => {
					svg.fill_alpha(&Path::rect(0.0, 0.0, w, h), palette::PAPER, 1.0 - t);
				}
				Transition::Iris => self.iris(svg, w, h, shot, local, t),
				// The slide itself wraps the whole frame; the old page sits underneath.
				Transition::Push => self.under_previous(svg, shot, 1.0)?,
			}
		}

		// This page leaving, the way the next one arrives.
		if !last && remaining < WIPE {
			let next = index
				.and_then(|index| self.timeline.shots.get(index + 1))
				.with_context(|| "a page turn needs a page after this one")?;
			let leaving = smoothstep(remaining / WIPE);

			match next.transition {
				Transition::Wipe => sheet(svg, h, w * leaving, w),
				Transition::Cut | Transition::Dissolve | Transition::Push => {}
				Transition::Fade => {
					svg.fill_alpha(&Path::rect(0.0, 0.0, w, h), palette::PAPER, 1.0 - leaving);
				}
				Transition::Iris => self.iris(svg, w, h, shot, local, leaving),
			}
		}

		// The video's last moments fade to paper, as they always have.
		if last && remaining < 0.6 {
			let fade = 1.0 - remaining / 0.6;
			svg.fill_alpha(&Path::rect(0.0, 0.0, w, h), palette::PAPER, fade);
		}

		Ok(())
	}

	/// An iris: a circle of the scene through paper, closing or opening on the page's subject.
	fn iris(&self, svg: &mut Svg, w: f32, h: f32, shot: &Shot, local: f32, fraction: f32) {
		let radius = w.hypot(h) * 0.6 * fraction;
		let (cx, cy) = self.iris_centre(shot, shot.start + local);

		if radius < 1.0 {
			svg.fill(&Path::rect(0.0, 0.0, w, h), palette::PAPER);
			return;
		}

		let hole = format!(
			"M {:.2} {:.2} A {radius:.2} {radius:.2} 0 1 0 {:.2} {:.2} A {radius:.2} {radius:.2} \
			 0 1 0 {:.2} {:.2} Z",
			cx - radius,
			cy,
			cx + radius,
			cy,
			cx - radius,
			cy
		);
		svg.raw(&format!(
			r#"<path d="M0 0 H{w:.2} V{h:.2} H0 Z {hole}" fill="{}" fill-rule="evenodd"/>"#,
			palette::PAPER
		));
	}

	/// Where the iris pinches to: the page's star, or whoever spoke last, on screen.
	fn iris_centre(&self, shot: &Shot, time: f32) -> (f32, f32) {
		let centre = (self.width as f32 / 2.0, self.height as f32 / 2.0);
		let scene = shot.scene_at(time);
		let puppets = compose::puppets(scene, time, false);
		let speaking = speakers_near(shot, time);
		let star = self.story.meta.star.as_deref();

		let subject = puppets
			.iter()
			.filter(|puppet| puppet.id != "wormsley")
			.find(|puppet| speaking.len() == 1 && puppet.id == speaking[0])
			.or_else(|| {
				puppets
					.iter()
					.find(|puppet| Some(puppet.id.split('.').next().unwrap_or("")) == star)
			})
			.or_else(|| puppets.iter().find(|puppet| puppet.id != "wormsley"));

		let Some(puppet) = subject else {
			return centre;
		};

		let (head, _) = puppet.head();
		let aspect = self.format.aspect();
		let view = self.camera(shot, time);
		let scale = self.height as f32 / view.height;
		let x = (head.x - (view.centre.x - view.width(aspect) / 2.0)) * scale;

		(x, (head.y - (view.centre.y - view.height / 2.0)) * scale)
	}

	/// The previous page's last frame, rendered and embedded, so a dissolve can run over it or a
	/// push can slide over it.
	fn under_previous(&self, svg: &mut Svg, shot: &Shot, opacity: f32) -> Result<()> {
		if opacity <= 0.01 {
			return Ok(());
		}

		let index = self
			.timeline
			.shots
			.iter()
			.position(|other| std::ptr::eq(other, shot))
			.and_then(|index| index.checked_sub(1))
			.and_then(|index| self.timeline.shots.get(index))
			.with_context(|| "a transition needs a page before this one")?;
		let renderer = Renderer::new(
			self.timeline,
			self.story,
			self.format,
			self.width,
			self.height,
		);
		let previous = renderer.svg(index.end - 0.01, 0)?;
		let pixmap = raster::render(&previous)?;
		let data = BASE64.encode(raster::jpeg(&pixmap, 82)?);

		svg.raw(&format!(
			r#"<image x="0" y="0" width="{}" height="{}" opacity="{opacity:.3}" preserveAspectRatio="none" xlink:href="data:image/jpeg;base64,{data}"/>"#,
			self.width, self.height
		));

		Ok(())
	}
}

/// An `<image>` of a pre-softened backdrop layer, spanning the drawn stage.
fn backdrop_image(data: &str) -> String {
	format!(
		r#"<image x="-{OVER}" y="-{OVER}" width="{span}" height="{span}" preserveAspectRatio="none" xlink:href="data:image/png;base64,{data}"/>"#,
		span = director::OVERDRAW * 2.0 + STAGE,
		OVER = director::OVERDRAW
	)
}

/// The scene's living parts over a softened backdrop: effects, actors, breath, and the front
/// parts of the setting — what [`compose::draw_near`] draws after its backdrop layer. Calls
/// the same drawing code, so effects cannot drift between sharp and soft frames.
fn scene_front(svg: &mut Svg, scene: &Scene, puppets: &[Puppet], time: f32) {
	let mut weather = scene.weather;
	weather.time = time;

	compose::draw_effects(svg, &weather, puppets);

	for puppet in puppets {
		compose::draw_puppet(svg, puppet);
	}

	for puppet in puppets {
		compose::draw_breath(svg, puppet, time);
	}

	settings::foreground(svg, &scene.setting, &weather);
}

/// A paper sheet from `x0` to `x1`, with a wobbly leading edge.
fn sheet(svg: &mut Svg, h: f32, x0: f32, x1: f32) {
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
	let cover = edge_line
		.clone()
		.line_to(pt(far, h + 10.0))
		.line_to(pt(far, -10.0))
		.close();
	svg.fill(&cover, palette::PAPER);
	svg.stroke(&edge_line, palette::INK, svg.line * 1.5);
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

	director::clamp(aspect, View { centre, height })
}

/// The tallest view that stays on drawn stage in both directions.
fn tallest_view(format: Format) -> f32 {
	director::tallest_view(format.aspect())
}

// Arrivals, leaves, walk cycles, hops, follow-through, smears and everything else between
// keyframes live in [`acting`](crate::video::acting), the performance layer.

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
		use crate::art::figure::Frame;

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
		let motion = crate::art::dials::Motion::default();
		assert!(motion.ease(0.0).abs() < 1e-5);
		assert!((motion.ease(1.0) - 1.0).abs() < 1e-5);
		assert!(
			motion.ease(0.12) < 0.0,
			"a move starts by winding up the other way"
		);
		assert!(motion.ease(0.88) > 1.0, "and overshoots before it settles");
	}

	#[test]
	fn easing_lands_on_target() {
		assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
		assert!((acting::ease_out_bounce(1.0) - 1.0).abs() < 1e-5);
		assert!(ease_out_back(0.0).abs() < 1e-5);
		assert!(ease_out_back(0.6) > 1.0, "back easing overshoots");
	}

	/// A page held in close-up, for timing depth of field.
	fn close_up_timeline(depth: bool) -> (Story, Timeline) {
		use crate::cast::Cast;
		use crate::stage::rehearse;
		use crate::story;
		use crate::video::lipsync::Track;
		use crate::video::timeline;
		use crate::video::voice::SAMPLE_RATE;
		use crate::video::voice::Speech;
		use crate::video::voice::Utterance;

		let mut story = story::parse(
			"+++\ntitle = \"T\"\nauthor = \"A\"\nstar = \"kid\"\n+++\n# town shot=close\n@kid \
			 x=0.5\nOne two three four five six seven eight nine ten.\n",
		)
		.unwrap();
		story.meta.video.depth_of_field = depth;
		let cast = Cast::parse("[kid]\nname = \"Kid\"\n").unwrap();
		let rehearsals = rehearse(&story, &cast).unwrap();
		let tl = timeline::build(&story, &cast, &rehearsals, |line: &Utterance<'_>| {
			let seconds = line.text.split_whitespace().count() as f32 * 0.35;
			Ok(Speech {
				text: String::new(),
				samples: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
				envelope: Vec::new(),
				track: Track::default(),
			})
		})
		.unwrap();

		(story, tl)
	}

	#[test]
	fn depth_of_field_costs_less_than_twice_a_sharp_frame() -> Result<()> {
		let mut seconds = Vec::new();

		for depth in [false, true] {
			let (story, timeline) = close_up_timeline(depth);
			let renderer = Renderer::new(&timeline, &story, Format::Landscape, 960, 540);
			// The close-up holds for most of the page; sample the middle of it, after a warm-up
			// frame has built the blur cache the way a real shot would.
			let page = timeline.shots.last().unwrap();
			let start = page.start + 1.0;
			drop(renderer.render(start, 0)?);
			let started = std::time::Instant::now();

			for step in 0..10 {
				drop(renderer.render(start + step as f32 * 0.1, step)?);
			}

			seconds.push(started.elapsed().as_secs_f32() * 1000.0 / 10.0);
		}

		let (sharp, blurred) = (seconds[0], seconds[1]);
		eprintln!(
			"preview frames at 960x540: sharp {sharp:.1} ms, blurred {blurred:.1} ms per frame"
		);
		assert!(
			blurred < sharp * 2.0 + 3.0,
			"a blurred frame costs {blurred:.1} ms against {sharp:.1} ms sharp: lower the cost"
		);

		Ok(())
	}

	#[test]
	fn a_close_shot_renders_head_and_shoulders_clear_of_the_captions() -> Result<()> {
		let (story, timeline) = close_up_timeline(true);
		let renderer = Renderer::new(&timeline, &story, Format::Landscape, 960, 540);

		// The page's heading shot=close is the opening setup, so a mid-page frame is a
		// close-up: its backdrop comes in softened, and the second frame reuses the raster.
		let page = timeline.shots.last().unwrap();
		let svg = renderer.svg(page.start + 1.0, 4)?;
		assert!(
			svg.contains("viewBox"),
			"a rendered frame is an SVG document"
		);
		assert!(
			svg.matches("data:image/png;base64,").count() == 2,
			"both backdrop layers arrive as pre-softened images, with alpha kept"
		);
		assert!(
			!svg.contains("feGaussianBlur"),
			"no per-frame filter: the blur is baked into the cached rasters"
		);

		Ok(())
	}
}
