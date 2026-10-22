//! Model sheets for the kids redrawn after Stinky, drawn from their cast entries and part files
//! exactly as the books draw them, plus a lineup to check their silhouettes and sizes together.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use fables::art::Path as Curve;
use fables::art::Svg;
use fables::art::figure;
use fables::art::figure::FigureState;
use fables::art::figure::Look;
use fables::art::figure::Mood;
use fables::art::figure::Pose;
use fables::art::rig::Stance;
use fables::cast::Cast;
use fables::cast::Kind;

use crate::sheets::PAPER;
use crate::sheets::label;
use crate::sheets::save;
use crate::stinky;
use crate::stinky::THREE_QUARTER;

/// A cast member's model sheet: who, and the moods and poses the books give them.
struct Sheet {
	id: &'static str,
	title: &'static str,
	beats: [(Mood, Pose, &'static str); 5],
}

const SHEETS: [Sheet; 2] = [
	Sheet {
		id: "bogey",
		title: "Bogey McBogeyface (born Bonnie): model sheet",
		beats: [
			(Mood::Happy, Pose::Wave, "happy · wave"),
			(Mood::Grin, Pose::Pick, "grin · pick"),
			(Mood::Laugh, Pose::Cheer, "laugh · cheer"),
			(Mood::Disgusted, Pose::HoldNose, "disgusted · hold nose"),
			(Mood::Shocked, Pose::CoverMouth, "shocked"),
		],
	},
	Sheet {
		id: "grubby",
		title: "Grubby McGrubhands (born Gus): model sheet",
		beats: [
			(Mood::Happy, Pose::Cheer, "happy · cheer"),
			(Mood::Proud, Pose::Hips, "proud · hips"),
			(Mood::Grin, Pose::ReachOut, "grin · reach out"),
			(Mood::Disgusted, Pose::HoldNose, "disgusted · hold nose"),
			(Mood::Shocked, Pose::Idle, "shocked"),
		],
	},
];

/// A cast member's look, which must be drawn by an authored puppet.
fn authored(cast: &Cast, id: &str) -> Result<Look> {
	let Kind::Figure(look) = cast.resolve(id)? else {
		bail!("{id} is a person in the cast file");
	};
	look.puppet
		.as_ref()
		.with_context(|| format!("{id} has no `puppet` in the cast file"))?;
	Ok(look)
}

fn posed(look: &Look, mood: Mood, pose: Pose, facing: f32) -> FigureState {
	let frame = look.frame();
	let mut state = FigureState::new(&frame);
	state.mood = mood;
	state.facing = facing;
	state.hands = frame.hands(pose, facing);
	state.hand_shapes = pose.hand_shapes();
	state.legs = frame.limbs().stance(Stance::Stand, facing);
	state
}

fn draw(svg: &mut Svg, look: &Look, state: &FigureState) {
	figure::shadow(svg, look, state, 0.0);
	figure::draw(svg, look, state);
}

/// One model sheet per redrawn kid, plus a lineup with Stinky.
pub fn cast_preview(out: &Path) -> Result<Vec<PathBuf>> {
	let cast = Cast::load(&stinky::cast_file())?;
	let mut written = Vec::new();
	let mut lineup = vec![("Stinky", stinky::look().clone())];

	for sheet in &SHEETS {
		let look = authored(&cast, sheet.id)?;
		let cell = 520.0;
		let (width, height) = (cell * 5.0, 140.0 + cell * 2.0 + 40.0);
		let mut c = Svg::new();
		c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
		label(&mut c, width / 2.0, 80.0, 46.0, sheet.title);

		for (column, (mood, pose, name)) in sheet.beats.into_iter().enumerate() {
			for (row, facing) in [0.0_f32, THREE_QUARTER].into_iter().enumerate() {
				let x = cell * (column as f32 + 0.5);
				let ground = 140.0 + cell * (row as f32 + 1.0) - 60.0;
				c.begin(&format!("translate({x:.1} {ground:.1}) scale(0.88)"));
				draw(&mut c, &look, &posed(&look, mood, pose, facing));
				c.end();
				let view = if row == 0 { "front" } else { "three-quarter" };
				label(&mut c, x, ground + 46.0, 26.0, &format!("{name} · {view}"));
			}
		}

		let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
		written.push(save(&svg, &out.join(format!("{}-model-sheet.png", sheet.id)))?);
		lineup.push((sheet.title.split(' ').next().unwrap_or(sheet.id), look));
	}

	// The kids side by side, turned towards each other, to check scale and silhouettes.
	let cell = 520.0;
	let (width, height) = (cell * lineup.len() as f32, 760.0);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(&mut c, width / 2.0, 70.0, 42.0, "Lineup: silhouettes and sizes");

	for (index, (name, look)) in lineup.iter().enumerate() {
		let x = cell * (index as f32 + 0.5);
		let ground = height - 110.0;
		let facing = (1.0 - index as f32) * THREE_QUARTER;
		c.begin(&format!("translate({x:.1} {ground:.1})"));
		draw(&mut c, look, &posed(look, Mood::Happy, Pose::Idle, facing));
		c.end();
		label(&mut c, x, ground + 60.0, 30.0, name);
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	written.push(save(&svg, &out.join("kids-lineup.png"))?);
	Ok(written)
}
