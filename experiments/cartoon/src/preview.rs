//! Model sheets for characters redrawn as authored puppets but not yet wired into the books:
//! each is drawn from their part file with the colours proposed for their cast entry, so they
//! can be signed off before `world/cast.toml` changes.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;
use fables::art::Color;
use fables::art::Path as Curve;
use fables::art::Svg;
use fables::art::figure;
use fables::art::figure::FigureState;
use fables::art::figure::Look;
use fables::art::figure::Mood;
use fables::art::figure::Pose;
use fables::art::puppet::Authored;
use fables::art::rig::Stance;
use fables::cast::Cast;
use fables::cast::Kind;

use crate::sheets::PAPER;
use crate::sheets::label;
use crate::sheets::save;
use crate::stinky;
use crate::stinky::THREE_QUARTER;

/// A character's proposed look: their cast entry, their part file, and softened colours.
struct Proposal {
	id: &'static str,
	title: &'static str,
	/// `(skin, hair, top, trim, bottom, shoes, eyes)`.
	colours: [u32; 7],
	/// Moods and poses for the sheet, as the books use them.
	beats: [(Mood, Pose, &'static str); 5],
}

const PROPOSALS: [Proposal; 2] = [
	Proposal {
		id: "bogey",
		title: "Bogey McBogeyface (born Bonnie): proposed design",
		colours: [
			0xF4_C7_66, 0x7A_4A_35, 0xEC_7A_A6, 0xFF_F8_EF, 0x2E_9A_90, 0x7A_5C_B8, 0x5D_9E_5A,
		],
		beats: [
			(Mood::Happy, Pose::Wave, "happy · wave"),
			(Mood::Grin, Pose::Pick, "grin · pick"),
			(Mood::Laugh, Pose::Cheer, "laugh · cheer"),
			(Mood::Disgusted, Pose::HoldNose, "disgusted · hold nose"),
			(Mood::Shocked, Pose::CoverMouth, "shocked"),
		],
	},
	Proposal {
		id: "grubby",
		title: "Grubby McGrubhands (born Gus): proposed design",
		colours: [
			0x62_B9_AD, 0xF2_C1_4E, 0xD9_57_3F, 0xF2_C1_4E, 0x34_36_6E, 0x7A_4F_3A, 0x5A_3A_2C,
		],
		beats: [
			(Mood::Happy, Pose::Cheer, "happy · cheer"),
			(Mood::Proud, Pose::Hips, "proud · hips"),
			(Mood::Grin, Pose::ReachOut, "grin · reach out"),
			(Mood::Disgusted, Pose::HoldNose, "disgusted · hold nose"),
			(Mood::Shocked, Pose::Idle, "shocked"),
		],
	},
];

fn proposed(cast: &Cast, proposal: &Proposal) -> Result<Look> {
	let Kind::Figure(mut look) = cast.resolve(proposal.id)? else {
		bail!("{} is a person in the cast file", proposal.id);
	};
	let file = stinky::cast_file()
		.with_file_name("puppets")
		.join(format!("{}.svg", proposal.id));
	look.puppet = Some(Authored::load(&file)?);
	let [skin, hair, top, trim, bottom, shoes, eyes] = proposal.colours.map(Color);
	(look.skin, look.hair_color, look.top, look.trim) = (skin, hair, top, trim);
	(look.bottom, look.shoes, look.eyes) = (bottom, shoes, eyes);
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

/// One model sheet per proposed character, plus a lineup with Stinky to check their sizes.
pub fn cast_preview(out: &Path) -> Result<Vec<PathBuf>> {
	let cast = Cast::load(&stinky::cast_file())?;
	let mut written = Vec::new();
	let mut lineup = vec![("Stinky".to_owned(), stinky::look().clone())];

	for proposal in &PROPOSALS {
		let look = proposed(&cast, proposal)?;
		let cell = 520.0;
		let (width, height) = (cell * 5.0, 140.0 + cell * 2.0 + 40.0);
		let mut c = Svg::new();
		c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
		label(&mut c, width / 2.0, 80.0, 46.0, proposal.title);

		for (column, (mood, pose, name)) in proposal.beats.into_iter().enumerate() {
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
		written.push(save(
			&svg,
			&out.join(format!("{}-model-sheet.png", proposal.id)),
		)?);
		lineup.push((
			proposal
				.title
				.split(' ')
				.next()
				.unwrap_or(proposal.id)
				.to_owned(),
			look,
		));
	}

	// The three kids side by side, turned towards each other, to check scale and silhouettes.
	let cell = 520.0;
	let (width, height) = (cell * lineup.len() as f32, 760.0);
	let mut c = Svg::new();
	c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
	label(
		&mut c,
		width / 2.0,
		70.0,
		42.0,
		"Lineup: silhouettes and sizes",
	);

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
