//! `fables mcp`: the studio served over the Model Context Protocol, on stdio.
//!
//! The tools follow the loop that drew every Kerfuffle character: start a character from the
//! style's template, redraw parts as SVG path data, look at the model sheet, fix, repeat. Sheets
//! come back as images, so the model sees what it drew. Each tool is a thin wrapper over
//! [`Studio`]; failures come back as tool errors the model can read and act on.

use std::path::Path;

use base64::Engine as _;
use rmcp::ServerHandler;
use rmcp::ServiceExt as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::ContentBlock;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerConfig;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::studio::Audition;
use crate::studio::CharacterBrief;
use crate::studio::Image;
use crate::studio::PartData;
use crate::studio::SheetKind;
use crate::studio::Studio;
use crate::studio::VoiceChoice;
use crate::studio::WorldBrief;

/// What the server tells a model before it calls anything.
const INSTRUCTIONS: &str = "\
fables is a studio for illustrated books and cartoons. A world (a series) has a cast of characters \
                            drawn as cut-out puppets from vector parts, in a drawing style.

To design a character:
1. `world_list`, or `world_create` for a new series.
2. `style_describe` to learn the parts a figure is drawn with and the space each is drawn in.
3. `character_create` with who they are and their colours. Their parts start as the style's \
                            template.
4. `character_sheet` to see them. `character_parts` reads parts back as SVG path data (name kit \
                            parts to see the shared ones).
5. `character_set_parts` to redraw parts, then look at the sheet again. Repeat until they read as \
                            themselves: a clear silhouette, a face that shows every mood.

Changes that would break a figure are refused with the part check's findings. A person signs off \
                            every character's model sheet before the character is used in a book.";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorldParams {
	/// The world's folder, from `world_list`.
	pub world: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StyleParams {
	/// The style's name, from `style_list`.
	pub style: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCharacterParams {
	pub world: String,
	pub character: CharacterBrief,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CharacterPartsParams {
	pub world: String,
	/// The character's cast id.
	pub id: String,
	/// Parts to read, from the character's file or the kit. Leave empty for all their own parts.
	#[serde(default)]
	pub names: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPartsParams {
	pub world: String,
	pub id: String,
	/// Parts to write or replace in the character's own file.
	#[serde(default)]
	pub parts: Vec<PartData>,
	/// Own parts to remove, so the kit's drawing is used again (or an optional part goes).
	#[serde(default)]
	pub remove: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SheetParams {
	pub world: String,
	pub id: String,
	/// Which sheets: `model` (the default), `parts` or `moods`.
	#[serde(default)]
	pub sheets: Vec<SheetKind>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CastVoiceParams {
	pub world: String,
	/// The character's cast id.
	pub id: String,
	pub voice: VoiceChoice,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditionParams {
	pub world: String,
	/// A cast id, or `narrator`.
	pub who: String,
	/// The line to speak.
	pub line: String,
	/// A mood to deliver it in, e.g. `cross`, `happy`, `sad`.
	pub mood: Option<String>,
}

#[derive(Serialize)]
struct SayVoice {
	name: String,
	language: String,
}

#[derive(Serialize)]
struct StyleLine {
	name: String,
	description: String,
}

/// The studio's MCP server.
#[derive(Clone, Debug)]
pub struct StudioServer {
	studio: Studio,
	tool_router: ToolRouter<StudioServer>,
}

/// A JSON result, or the error as text the model can read.
fn json<T: Serialize>(result: anyhow::Result<T>) -> CallToolResult {
	match result.and_then(|value| Ok(serde_json::to_string_pretty(&value)?)) {
		Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
		Err(error) => failed(&error),
	}
}

/// Images, each after a line saying what it is.
fn images(result: anyhow::Result<Vec<Image>>) -> CallToolResult {
	match result {
		Ok(images) => {
			let content = images
				.into_iter()
				.flat_map(|image| {
					[
						ContentBlock::text(image.label),
						ContentBlock::image(
							base64::engine::general_purpose::STANDARD.encode(&image.png),
							"image/png",
						),
					]
				})
				.collect();
			CallToolResult::success(content)
		}
		Err(error) => failed(&error),
	}
}

/// A spoken line as audio, after a line saying who said it.
fn audio(result: anyhow::Result<Audition>) -> CallToolResult {
	match result {
		Ok(audition) => {
			CallToolResult::success(vec![
				ContentBlock::text(format!("{} ({:.1}s)", audition.label, audition.seconds)),
				ContentBlock::audio(
					base64::engine::general_purpose::STANDARD.encode(&audition.wav),
					"audio/wav",
				),
			])
		}
		Err(error) => failed(&error),
	}
}

fn failed(error: &anyhow::Error) -> CallToolResult {
	CallToolResult::error(vec![ContentBlock::text(format!("{error:#}"))])
}

#[tool_router]
impl StudioServer {
	/// A server for the worlds under `root/worlds/`.
	pub fn new(root: &Path) -> StudioServer {
		StudioServer {
			studio: Studio::new(root),
			tool_router: Self::tool_router(),
		}
	}

	#[tool(description = "List the worlds (series) in the studio: folder, name, style and about.")]
	fn world_list(&self) -> CallToolResult {
		json(self.studio.worlds())
	}

	#[tool(
		description = "Start a new world (series) from a name, a few sentences about it, and a \
		               drawing style. Returns its folder, which the other tools use."
	)]
	fn world_create(&self, Parameters(brief): Parameters<WorldBrief>) -> CallToolResult {
		json(self.studio.create_world(&brief))
	}

	#[tool(description = "Describe a world: its style, narrator, cast and books.")]
	fn world_describe(&self, Parameters(params): Parameters<WorldParams>) -> CallToolResult {
		json(self.studio.describe_world(&params.world))
	}

	#[tool(description = "List the drawing styles, with a sentence on each look.")]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn style_list(&self) -> CallToolResult {
		json(Studio::styles().map(|styles| {
			styles
				.into_iter()
				.map(|(name, description)| StyleLine { name, description })
				.collect::<Vec<_>>()
		}))
	}

	#[tool(
		description = "Describe a drawing style: its finish, its bodies, every part a figure is \
		               drawn with (views, whether it's required, the coordinate space) and the \
		               rules for drawing parts. Read this before drawing."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn style_describe(&self, Parameters(params): Parameters<StyleParams>) -> CallToolResult {
		json(Studio::describe_style(&params.style))
	}

	#[tool(
		description = "Show a style as images: its template character on a model sheet, and its \
		               kit and template parts."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn style_sheet(&self, Parameters(params): Parameters<StyleParams>) -> CallToolResult {
		images(Studio::style_sheets(&params.style, crate::studio::SHEET_PX))
	}

	#[tool(
		description = "Add a character to a world's cast: their id, name, build (kid or \
		               grown-up), who they are, their habit and colours. Their drawing starts as \
		               the style's template; redraw it with character_set_parts."
	)]
	fn character_create(
		&self,
		Parameters(params): Parameters<CreateCharacterParams>,
	) -> CallToolResult {
		json(
			self.studio
				.create_character(&params.world, &params.character),
		)
	}

	#[tool(
		description = "Read a character's parts as SVG path data: all their own parts, or the \
		               named parts from their file or the style's kit."
	)]
	fn character_parts(
		&self,
		Parameters(params): Parameters<CharacterPartsParams>,
	) -> CallToolResult {
		json(
			self.studio
				.character_parts(&params.world, &params.id, &params.names),
		)
	}

	#[tool(
		description = "Write or replace a character's parts as SVG path data, or remove their own \
		               parts. The whole figure is checked; changes that leave errors are not \
		               written. Returns whether it was written and the findings."
	)]
	fn character_set_parts(
		&self,
		Parameters(params): Parameters<SetPartsParams>,
	) -> CallToolResult {
		json(
			self.studio
				.set_parts(&params.world, &params.id, &params.parts, &params.remove),
		)
	}

	#[tool(
		description = "Render a character as images: the model sheet (five beats, front and \
		               three-quarters), the parts sheet, or all fifteen moods."
	)]
	fn character_sheet(&self, Parameters(params): Parameters<SheetParams>) -> CallToolResult {
		images(
			self.studio
				.character_sheets(&params.world, &params.id, &params.sheets),
		)
	}

	#[tool(
		description = "List the voice providers: which run offline, which can speak right now \
		               (their program is here or their key is set), and what each needs."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn voice_providers(&self) -> CallToolResult {
		json(Ok(Studio::voice_providers()))
	}

	#[tool(description = "List the macOS voices the offline `say` provider can speak with.")]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn voice_list(&self) -> CallToolResult {
		json(Studio::say_voices().map(|voices| {
			voices
				.into_iter()
				.map(|(name, language)| SayVoice { name, language })
				.collect::<Vec<_>>()
		}))
	}

	#[tool(
		description = "Give a character their own voice for dialogue: a provider and a voice (a \
		               name for say and openai, an id for elevenlabs, cartesia and hume), with \
		               optional acting style. Replaces any voice they had."
	)]
	fn voice_cast(&self, Parameters(params): Parameters<CastVoiceParams>) -> CallToolResult {
		json(
			self.studio
				.cast_voice(&params.world, &params.id, &params.voice),
		)
	}

	#[tool(
		description = "Hear a line in a character's voice (or the narrator's), delivered in a \
		               mood. Returns the audio. Takes are cached, so a line is only paid for once."
	)]
	fn voice_audition(&self, Parameters(params): Parameters<AuditionParams>) -> CallToolResult {
		audio(self.studio.audition(
			&params.world,
			&params.who,
			&params.line,
			params.mood.as_deref(),
		))
	}
}

#[expect(
	clippy::unused_async_trait_impl,
	reason = "rmcp's tool_handler generates the async methods"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for StudioServer {
	fn get_info(&self) -> ServerConfig {
		ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
			.with_instructions(INSTRUCTIONS)
	}
}

/// Serves the studio under `root` on stdin and stdout until the client hangs up.
pub fn serve(root: &Path) -> anyhow::Result<()> {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()?;

	runtime.block_on(async {
		let service = StudioServer::new(root)
			.serve(rmcp::transport::stdio())
			.await?;
		service.waiting().await?;
		Ok(())
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_studio_tool_is_listed_with_a_schema() {
		let server = StudioServer::new(Path::new("."));
		let names: Vec<String> = server
			.tool_router
			.list_all()
			.iter()
			.map(|tool| tool.name.to_string())
			.collect();

		for name in [
			"world_list",
			"world_create",
			"world_describe",
			"style_list",
			"style_describe",
			"style_sheet",
			"character_create",
			"character_parts",
			"character_set_parts",
			"character_sheet",
			"voice_providers",
			"voice_list",
			"voice_cast",
			"voice_audition",
		] {
			assert!(
				names.iter().any(|listed| listed == name),
				"{name}: {names:?}"
			);
		}
	}

	#[test]
	fn failures_come_back_as_tool_errors() {
		let server = StudioServer::new(Path::new("/nonexistent"));
		let result = server.world_describe(Parameters(WorldParams {
			world: "nowhere".into(),
		}));
		assert_eq!(result.is_error, Some(true));
	}
}
