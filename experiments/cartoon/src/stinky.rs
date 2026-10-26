//! Stinky as the books draw him: his cast entry and authored puppet, posed through the same
//! [`FigureState`] the stage and video use. The experiment only arranges him; the drawing is
//! `fables::art::puppet`.

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use fables::art::Svg;
use fables::art::figure;
use fables::art::figure::FigureState;
use fables::art::figure::Look;
use fables::art::figure::Mood;
use fables::art::figure::Pose;
use fables::art::pack::StylePack;
use fables::art::rig::Stance;
use fables::cast::Cast;
use fables::cast::Kind;

/// The `facing` the stage gives a figure turned towards the middle of the page: his full
/// three-quarter view.
pub const THREE_QUARTER: f32 = 0.35;

pub fn cast_file() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../worlds/kerfuffle/cast.toml")
}

/// Loads the cast and checks Stinky is drawn by an authored puppet, before anything is drawn.
pub fn check() -> Result<()> {
	let cast = Cast::load(&cast_file(), &StylePack::default_pack())?;
	let Kind::Figure(look) = cast.resolve("stinky")? else {
		bail!("stinky is a person in the cast file");
	};
	look.puppet
		.as_ref()
		.map(|_| ())
		.context("stinky has no `puppet` in the cast file")
}

/// Stinky's look from the cast file. [`check`] has already confirmed it loads.
pub fn look() -> &'static Look {
	static LOOK: OnceLock<Look> = OnceLock::new();
	LOOK.get_or_init(|| {
		let cast = Cast::load(&cast_file(), &StylePack::default_pack())
			.unwrap_or_else(|error| panic!("{error:#}"));

		match cast.resolve("stinky") {
			Ok(Kind::Figure(look)) => look,
			_ => unreachable!("check() confirmed stinky is a person"),
		}
	})
}

/// Stinky standing in `pose` with `mood`, turned by `facing`.
pub fn posed(mood: Mood, pose: Pose, facing: f32) -> FigureState {
	let frame = look().frame();
	let mut state = FigureState::new(&frame);
	state.mood = mood;
	state.facing = facing;
	state.hands = frame.hands(pose, facing);
	state.hand_shapes = pose.hand_shapes();
	state.legs = frame.limbs().stance(Stance::Stand, facing);
	state
}

/// Draws Stinky and his contact shadow with his soles at the origin.
pub fn draw(svg: &mut Svg, state: &FigureState) {
	figure::shadow(svg, look(), state, 0.0);
	figure::draw(svg, look(), state);
}
