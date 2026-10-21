//! Model sheets: the moods in both views, the mouth chart and the rig check.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;
use fables::art::Color;
use fables::art::Path as Curve;
use fables::art::Svg;
use fables::art::escape;
use fables::art::figure;
use fables::art::figure::FigureState;
use fables::art::figure::Mood as OldMood;
use fables::art::figure::Pose as OldPose;
use fables::art::figure::Viseme;
use fables::art::rig::Gait;
use fables::art::rig::Stance;
use fables::art::style::Style;
use fables::art::style::StyleName;
use fables::cast::Cast;
use fables::cast::Kind;
use fables::raster;

use crate::stinky;
use crate::stinky::Arms;
use crate::stinky::Mood;
use crate::stinky::Pose;

pub const PAPER: Color = Color(0xFB_F3_E4);
pub const LABEL: Color = Color(0x4A_3A_4E);

/// Writes `svg` as a PNG at `path`.
pub fn save(svg: &str, path: &Path) -> Result<PathBuf> {
	let pixmap = raster::render(svg)?;
	std::fs::write(path, raster::png(&pixmap)?)?;
	Ok(path.to_path_buf())
}

pub fn label(c: &mut Svg, x: f32, y: f32, size: f32, text: &str) {
	c.raw(&format!(
		r#"<text x="{x:.1}" y="{y:.1}" font-family="{}" font-size="{size:.1}" fill="{LABEL}" text-anchor="middle">{}</text>"#,
		raster::BODY_FAMILY,
		escape(text)
	));
}

/// The pose each mood is shown in on the model sheet.
pub fn mood_pose(mood: Mood, turn: f32) -> Pose {
	let arms = match mood {
		Mood::Happy => Arms::Wave,
		Mood::Grin => Arms::Hips,
		Mood::Defiant => Arms::Folded,
		Mood::Disgusted => Arms::Recoil,
	};
	Pose::new(mood, arms, turn)
}

/// Four moods across, front view above and three-quarter view below.
pub fn model_sheet(out: &Path) -> Result<PathBuf> {
	let cell = 560.0;
	let (width, height) = (cell * 4.0, cell * 2.0 + 120.0);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(
		&mut c,
		width / 2.0,
		70.0,
		46.0,
		"Stinky McStinkbreath: model sheet (experiment)",
	);

	for (column, mood) in Mood::ALL.into_iter().enumerate() {
		for (row, turn) in [0.0_f32, 1.0].into_iter().enumerate() {
			let x = cell * (column as f32 + 0.5);
			let ground = 120.0 + cell * (row as f32 + 1.0) - 70.0;
			let pose = mood_pose(mood, turn);
			c.begin(&format!("translate({x:.1} {ground:.1}) scale(0.92)"));
			stinky::draw_shadow(&mut c, &pose, 0.0);
			stinky::draw(&mut c, &pose);
			c.end();
			let view = if row == 0 { "front" } else { "three-quarter" };
			label(
				&mut c,
				x,
				ground + 48.0,
				28.0,
				&format!("{} · {view}", mood.name()),
			);
		}
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("model-sheet.png"))
}

/// Stinky turning from three-quarter left, through the front, to three-quarter right. The
/// in-between turns are blends of the two drawings, not extra drawings.
pub fn turnaround(out: &Path) -> Result<PathBuf> {
	let turns = [-1.0_f32, -0.5, 0.0, 0.5, 1.0];
	let cell = 640.0;
	let (width, height) = (cell * turns.len() as f32, 1260.0);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(
		&mut c,
		width / 2.0,
		80.0,
		50.0,
		"Turnaround: two drawings per part, blended",
	);

	for (index, turn) in turns.into_iter().enumerate() {
		let x = cell * (index as f32 + 0.5);
		let ground = height - 130.0;
		let pose = Pose::new(Mood::Happy, Arms::Relaxed, turn);
		c.begin(&format!("translate({x:.1} {ground:.1}) scale(2)"));
		stinky::draw_shadow(&mut c, &pose, 0.0);
		stinky::draw(&mut c, &pose);
		c.end();
		label(&mut c, x, ground + 80.0, 34.0, &format!("turn {turn:+.1}"));
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("turnaround.png"))
}

/// The lip-sync chart: the nine mouth shapes fables' lip-sync already produces, drawn for the
/// new head, plus the four mood mouths.
pub fn mouth_chart(out: &Path) -> Result<PathBuf> {
	let shapes: [(&str, Option<Viseme>, Mood); 13] = [
		("rest", Some(Viseme::Rest), Mood::Happy),
		("M B P", Some(Viseme::Mbp), Mood::Happy),
		("teeth (S T K)", Some(Viseme::Teeth), Mood::Happy),
		("open (EH)", Some(Viseme::Open), Mood::Happy),
		("wide (AH)", Some(Viseme::Wide), Mood::Happy),
		("round (O)", Some(Viseme::Round), Mood::Happy),
		("pucker (OO W)", Some(Viseme::Pucker), Mood::Happy),
		("F V", Some(Viseme::Fv), Mood::Happy),
		("L TH", Some(Viseme::Tongue), Mood::Happy),
		("mood: happy", None, Mood::Happy),
		("mood: NOPE", None, Mood::Defiant),
		("mood: disgusted", None, Mood::Disgusted),
		("mood: grin", None, Mood::Grin),
	];
	let (cell_w, cell_h, columns) = (380.0, 420.0, 5);
	let rows = shapes.len().div_ceil(columns);
	let (width, height) = (cell_w * columns as f32, 130.0 + cell_h * rows as f32);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(
		&mut c,
		width / 2.0,
		80.0,
		46.0,
		"Mouth chart: the nine lip-sync shapes, then the mood mouths",
	);

	for (index, (name, viseme, mood)) in shapes.into_iter().enumerate() {
		let x = cell_w * ((index % columns) as f32 + 0.5);
		let y = 130.0 + cell_h * (index / columns) as f32;
		let mut pose = Pose::new(mood, Arms::Relaxed, 0.7);
		pose.viseme = viseme;
		pose.head_tilt = 0.0;
		pose.tilt = 0.0;
		c.begin_clip(&Curve::rect(
			x - cell_w / 2.0 + 14.0,
			y,
			cell_w - 28.0,
			cell_h - 70.0,
		));
		c.fill(
			&Curve::rect(x - cell_w / 2.0 + 14.0, y, cell_w - 28.0, cell_h - 70.0),
			Color(0xF3_E7_D3),
		);
		c.begin(&format!(
			"translate({:.1} {:.1}) scale(1.25)",
			x - 30.0,
			y + 545.0
		));
		stinky::draw(&mut c, &pose);
		c.end();
		c.end();
		label(&mut c, x, y + cell_h - 25.0, 30.0, name);
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("mouth-chart.png"))
}

/// The new parts posed by the unchanged fables rig: stances from `Limbs::stance`, a walk from
/// `Limbs::cycle`, elbows and knees from `two_bone`, with the skeleton drawn over the top.
pub fn rig_sheet(out: &Path) -> Result<PathBuf> {
	let mut cells: Vec<(String, Pose)> = Vec::new();

	for (name, stance) in [
		("stand", Stance::Stand),
		("crouch", Stance::Crouch),
		("jump", Stance::Jump),
		("tiptoe", Stance::Tiptoe),
	] {
		let mut pose = Pose::new(Mood::Happy, Arms::Relaxed, 1.0);
		pose.legs = stinky::stance(stance, 1.0);
		cells.push((name.to_owned(), pose));
	}

	for step in 0..4 {
		let mut pose = Pose::new(Mood::Happy, Arms::Relaxed, 1.0);
		let phase = step as f32 * std::f32::consts::FRAC_PI_2;
		pose.gait = Some((Gait::Walk, phase));
		cells.push((format!("walk {}/4", step + 1), pose));
	}

	let (cell, columns) = (480.0, 4);
	let rows = cells.len().div_ceil(columns);
	let (width, height) = (cell * columns as f32, 130.0 + cell * 1.25 * rows as f32);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(
		&mut c,
		width / 2.0,
		80.0,
		46.0,
		"The new parts on the existing fables rig (stances, walk cycle, IK)",
	);

	for (index, (name, pose)) in cells.iter().enumerate() {
		let x = cell * ((index % columns) as f32 + 0.5);
		let ground = 130.0 + cell * 1.25 * ((index / columns) as f32 + 1.0) - 90.0;
		c.fill(
			&Curve::rect(x - cell / 2.0 + 20.0, ground, cell - 40.0, 4.0),
			Color(0xD9_C6_A6),
		);
		c.begin(&format!("translate({x:.1} {ground:.1}) scale(0.95)"));
		stinky::draw(&mut c, pose);
		c.begin_opacity(0.85);
		stinky::draw_bones(&mut c, pose);
		c.end();
		c.end();
		label(&mut c, x, ground + 55.0, 30.0, name);
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("rig-check.png"))
}

/// The current Stinky (the fables puppet, tv style) beside the experiment, same height.
pub fn comparison(out: &Path) -> Result<PathBuf> {
	let cast =
		Cast::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../world/cast.toml"))?;
	let Kind::Figure(look) = cast.resolve("stinky")? else {
		bail!("stinky is a person in the cast file");
	};
	let (cell_w, cell_h) = (900.0, 760.0);
	let (width, height) = (cell_w * 2.0, 150.0 + cell_h * 2.0);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(&mut c, cell_w * 0.5, 100.0, 54.0, "Now");
	label(&mut c, cell_w * 1.5, 100.0, 54.0, "Experiment");
	c.fill(
		&Curve::rect(cell_w - 2.0, 150.0, 4.0, cell_h * 2.0 - 40.0),
		Color(0xE4_D4_BC),
	);

	let rows = [
		(
			OldPose::Wave,
			OldMood::Happy,
			Mood::Happy,
			Arms::Wave,
			"hello",
		),
		(
			OldPose::ArmsCrossed,
			OldMood::Defiant,
			Mood::Defiant,
			Arms::Folded,
			"NOPE",
		),
	];

	for (row, (old_pose, old_mood, mood, arms, caption)) in rows.into_iter().enumerate() {
		let top = 150.0 + cell_h * row as f32;
		let ground = top + cell_h - 110.0;
		let figure_height = cell_h - 170.0;

		// The current puppet, drawn by fables itself into its own nested document.
		let frame = look.frame();
		let mut state = FigureState::new(&frame);
		state.mood = old_mood;
		state.hands = frame.hands(old_pose, 0.0);
		state.hand_shapes = old_pose.hand_shapes();
		let mut old = Svg::styled(Style::named(StyleName::Tv));
		figure::shadow(&mut old, &look, &state, 0.0);
		figure::draw(&mut old, &look, &state);
		let tall = frame.height() * 1.08;
		let doc = old.finish(
			(-tall * 0.6, -tall, tall * 1.2, tall * 1.04),
			(figure_height * 1.2) as u32,
			(figure_height * 1.04) as u32,
		);
		c.raw(&doc.replacen(
			"<svg ",
			&format!(
				r#"<svg x="{:.1}" y="{:.1}" "#,
				cell_w * 0.5 - figure_height * 0.6,
				ground - figure_height
			),
			1,
		));

		let pose = Pose::new(mood, arms, 1.0);
		let scale = figure_height / 500.0;
		c.begin(&format!(
			"translate({:.1} {ground:.1}) scale({scale:.3})",
			cell_w * 1.5
		));
		stinky::draw_shadow(&mut c, &pose, 0.0);
		stinky::draw(&mut c, &pose);
		c.end();

		for x in [cell_w * 0.5, cell_w * 1.5] {
			label(&mut c, x, ground + 70.0, 32.0, caption);
		}
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	save(&svg, &out.join("compare-now-vs-experiment.png"))
}
