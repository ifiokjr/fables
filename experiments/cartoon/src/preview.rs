//! Model sheets for the kids redrawn after Stinky, plus a lineup to check their silhouettes and
//! sizes together. Characters already wired in are drawn from their cast entries exactly as the
//! books draw them; proposed ones are drawn from their part file with proposed colours, so they
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
use fables::art::figure::Prop;
use fables::art::figure::Viseme;
use fables::art::puppet::Authored;
use fables::art::rig::Stance;
use fables::cast::Cast;
use fables::cast::Kind;

use crate::sheets::PAPER;
use crate::sheets::label;
use crate::sheets::save;
use crate::stinky;
use crate::stinky::THREE_QUARTER;

/// A moment on a model sheet: a mood, a pose, a lip-sync mouth if they are talking, and what
/// they hold.
type Beat = (Mood, Pose, Option<Viseme>, Option<Prop>, &'static str);

/// A cast member's model sheet: who, and the moods and poses the books give them.
struct Sheet {
	id: &'static str,
	title: &'static str,
	beats: [Beat; 5],
	/// Proposed colours for a character not yet wired in, as
	/// `(skin, hair, top, trim, bottom, shoes, eyes)`.
	proposed: Option<[u32; 7]>,
}

const SHEETS: [Sheet; 7] = [
	Sheet {
		id: "bogey",
		title: "Bogey McBogeyface (born Bonnie): model sheet",
		beats: [
			(Mood::Happy, Pose::Wave, None, None, "happy · wave"),
			(Mood::Grin, Pose::Pick, None, None, "grin · pick"),
			(Mood::Laugh, Pose::Cheer, None, None, "laugh · cheer"),
			(
				Mood::Disgusted,
				Pose::HoldNose,
				None,
				None,
				"disgusted · hold nose",
			),
			(Mood::Shocked, Pose::CoverMouth, None, None, "shocked"),
		],
		proposed: None,
	},
	Sheet {
		id: "grubby",
		title: "Grubby McGrubhands (born Gus): model sheet",
		beats: [
			(Mood::Happy, Pose::Cheer, None, None, "happy · cheer"),
			(Mood::Proud, Pose::Hips, None, None, "proud · hips"),
			(Mood::Grin, Pose::ReachOut, None, None, "grin · reach out"),
			(
				Mood::Disgusted,
				Pose::HoldNose,
				None,
				None,
				"disgusted · hold nose",
			),
			(Mood::Shocked, Pose::Idle, None, None, "shocked"),
		],
		proposed: None,
	},
	Sheet {
		id: "shouty",
		title: "Shouty McShoutface (born Sasha): model sheet",
		beats: [
			(
				Mood::Happy,
				Pose::Cheer,
				Some(Viseme::Wide),
				None,
				"SHOUTING · cheer",
			),
			(Mood::Laugh, Pose::Hips, None, None, "laugh · hips"),
			(Mood::Grin, Pose::Point, None, None, "grin · point"),
			(
				Mood::Disgusted,
				Pose::HoldNose,
				None,
				None,
				"disgusted · hold nose",
			),
			(Mood::Shocked, Pose::CoverMouth, None, None, "shocked"),
		],
		proposed: None,
	},
	Sheet {
		id: "fibby",
		title: "Fibby McFibberpants (born Felix): model sheet",
		beats: [
			(Mood::Grin, Pose::Hips, None, None, "fibbing · grin"),
			(Mood::Worried, Pose::Shrug, None, None, "caught out · shrug"),
			(Mood::Happy, Pose::Wave, None, None, "happy · wave"),
			(Mood::Proud, Pose::Think, None, None, "proud · think"),
			(Mood::Shocked, Pose::CoverMouth, None, None, "shocked"),
		],
		proposed: None,
	},
	Sheet {
		id: "mum",
		title: "Mum: model sheet",
		beats: [
			(
				Mood::Cross,
				Pose::Point,
				Some(Viseme::Open),
				None,
				"cross · point",
			),
			(Mood::Happy, Pose::Cheer, None, None, "happy · cheer"),
			(Mood::Grin, Pose::Hips, None, None, "grin · hips"),
			(
				Mood::Disgusted,
				Pose::HoldNose,
				None,
				None,
				"disgusted · hold nose",
			),
			(Mood::Worried, Pose::CoverMouth, None, None, "worried"),
		],
		proposed: None,
	},
	Sheet {
		id: "dad",
		title: "Dad: model sheet",
		beats: [
			(
				Mood::Happy,
				Pose::Hold,
				None,
				Some(Prop::Toothbrush),
				"happy · toothbrush",
			),
			(Mood::Grin, Pose::Wave, None, None, "grin · wave"),
			(Mood::Proud, Pose::Hips, None, None, "proud · hips"),
			(Mood::Laugh, Pose::Cheer, None, None, "laugh · cheer"),
			(Mood::Shocked, Pose::CoverMouth, None, None, "shocked"),
		],
		proposed: None,
	},
	Sheet {
		id: "parcel",
		title: "Parcel McParcelface: model sheet",
		beats: [
			(
				Mood::Happy,
				Pose::Hold,
				None,
				Some(Prop::Parcel),
				"happy · parcel",
			),
			(Mood::Grin, Pose::Wave, None, None, "grin · wave"),
			(Mood::Proud, Pose::Point, None, None, "proud · point"),
			(
				Mood::Disgusted,
				Pose::HoldNose,
				None,
				None,
				"disgusted · hold nose",
			),
			(Mood::Fainted, Pose::Droop, None, None, "fainted"),
		],
		proposed: None,
	},
];

/// How big to draw someone, relative to a kid, so the lineup matches the book's pages.
fn stage_scale(look: &Look) -> f32 {
	fables::stage::figure_size(look) / fables::stage::figure_size(stinky::look())
}

/// A cast member's look as an authored puppet: from the cast if they are wired in, otherwise
/// from their part file with the proposed colours.
fn authored(cast: &Cast, sheet: &Sheet) -> Result<Look> {
	let id = sheet.id;
	let Kind::Figure(mut look) = cast.resolve(id)? else {
		bail!("{id} is a person in the cast file");
	};

	match (&look.puppet, sheet.proposed) {
		(Some(_), _) => {}
		(None, Some(colours)) => {
			let file = stinky::cast_file()
				.with_file_name("puppets")
				.join(format!("{id}.svg"));
			look.puppet = Some(Authored::load(&file)?);
			let [skin, hair, top, trim, bottom, shoes, eyes] = colours.map(Color);
			(look.skin, look.hair_color, look.top, look.trim) = (skin, hair, top, trim);
			(look.bottom, look.shoes, look.eyes) = (bottom, shoes, eyes);
		}
		(None, None) => bail!("{id} has no `puppet` in the cast file and no proposed colours"),
	}

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
		let look = authored(&cast, sheet)?;
		let cell = 520.0;
		let (width, height) = (cell * 5.0, 140.0 + cell * 2.0 + 40.0);
		let mut c = Svg::new();
		c.fill(&Curve::rect(0.0, 0.0, width, height), PAPER);
		label(&mut c, width / 2.0, 80.0, 46.0, sheet.title);

		// Everyone fills their own sheet: a grown-up is drawn smaller to fit.
		let fit = (cell * 0.82 / look.frame().height()).min(0.88);

		for (column, (mood, pose, viseme, prop, name)) in sheet.beats.into_iter().enumerate() {
			for (row, facing) in [0.0_f32, THREE_QUARTER].into_iter().enumerate() {
				let x = cell * (column as f32 + 0.5);
				let ground = 140.0 + cell * (row as f32 + 1.0) - 60.0;
				c.begin(&format!("translate({x:.1} {ground:.1}) scale({fit:.3})"));
				let mut state = posed(&look, mood, pose, facing);
				state.viseme = viseme;
				state.prop = prop;
				draw(&mut c, &look, &state);
				c.end();
				let view = if row == 0 { "front" } else { "three-quarter" };
				label(&mut c, x, ground + 46.0, 26.0, &format!("{name} · {view}"));
			}
		}

		let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
		written.push(save(
			&svg,
			&out.join(format!("{}-model-sheet.png", sheet.id)),
		)?);
		lineup.push((
			sheet.title.split([' ', ':']).next().unwrap_or(sheet.id),
			look,
		));
	}

	// The kids side by side, turned towards each other, to check scale and silhouettes.
	let cell = 340.0;
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
		let middle = (lineup.len() as f32 - 1.0) / 2.0;
		let facing = (middle - index as f32).signum() * THREE_QUARTER;
		c.begin(&format!(
			"translate({x:.1} {ground:.1}) scale({:.3})",
			0.72 * stage_scale(look)
		));
		draw(&mut c, look, &posed(look, Mood::Happy, Pose::Idle, facing));
		c.end();
		label(&mut c, x, ground + 60.0, 30.0, name);
	}

	let svg = c.finish((0.0, 0.0, width, height), width as u32, height as u32);
	written.push(save(&svg, &out.join("kids-lineup.png"))?);
	Ok(written)
}
