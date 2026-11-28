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
use crate::studio::Frames;
use crate::studio::Image;
use crate::studio::Output;
use crate::studio::PartData;
use crate::studio::PropPart;
use crate::studio::SettingPart;
use crate::studio::SettingWeather;
use crate::studio::SheetKind;
use crate::studio::StoryBrief;
use crate::studio::Studio;
use crate::studio::VideoShape;
use crate::studio::VoiceChoice;
use crate::studio::WorldBrief;

/// The director's guide ([`docs/directing.md`](../../docs/directing.md)), compiled in so the
/// tool and the file can never disagree.
const GUIDE: &str = include_str!("../docs/directing.md");

/// The guide's topics, which are its `##` headings.
const GUIDE_TOPICS: [&str; 9] = [
	"overview", "story", "staging", "camera", "light", "acting", "mixing", "sound", "workflow",
];

/// The guide's `##` sections, as (heading, body), in file order.
fn guide_sections() -> impl Iterator<Item = (&'static str, &'static str)> {
	GUIDE.split("\n## ").skip(1).map(|section| {
		match section.split_once('\n') {
			Some((heading, body)) => (heading.trim(), body.trim()),
			None => (section.trim(), ""),
		}
	})
}

/// One `##` section of the guide, by topic.
fn guide_section(topic: &str) -> Option<&'static str> {
	guide_sections()
		.find(|(heading, _)| heading.eq_ignore_ascii_case(topic))
		.map(|(_, body)| body)
}

/// What the server tells a model before it calls anything.
const INSTRUCTIONS: &str =
	"\
fables is a studio for illustrated books and cartoons. A world (a series) has a cast of characters \
	 drawn as cut-out puppets from vector parts, in a drawing style.

To design a character:
1. `world_list`, or `world_create` for a new series.
2. `style_describe` to learn the parts a figure is drawn with, the space each is drawn in, and the \
	 hair, brows, noses and builds each body offers.
3. `character_create` with who they are, their colours, and their `kind` (kid or grown-up), \
	 `hair`, `build`, `brows` and `nose`. Their parts start as the style's template for their \
	 body (a grown-up's are cut for a grown-up) with those variants; without a `hair` or `build`, \
	 one is picked from their id and written into the cast.
4. `character_sheet` to see them. `character_parts` reads parts back as SVG path data (name kit \
	 parts to see the shared ones).
5. `character_set_parts` to redraw parts, then look at the sheet again. Repeat until they read as \
	 themselves: a clear silhouette, a face that shows every mood.

Changes that would break a figure are refused with the part check's findings. A person signs off \
	 every character's model sheet before the character is used in a book.

To make a book:
1. `guide` for the director's rules: topics overview, story, staging, camera, light, acting, \
	 mixing, sound and workflow.
2. `voice_cast` gives characters their voices; `voice_audition` lets you hear them. With \
	 ELEVENLABS_API_KEY set, `voice_library` lists ElevenLabs voices to cast, and `voice_design` \
	 then `voice_keep` make a new one from a casting note.
3. `flow_list` shows the story flows a book can take, with each one's page and word budgets; \
	 `flow_describe` prints one flow's beats. `story_start` returns the whole laid-out story.md \
	 for a flow, with the star and friend staged; fill in the [bracketed] lines and `story_write` \
	 it.
4. `story_write` saves a script (a whole story.md: TOML front matter between `+++` lines, then \
	 pages that start `# setting`, with narration and `@actor` stage directions). It is checked \
	 against the cast and refused, with the line to fix, if it doesn't work. `story_read` reads \
	 one back, for example a world's existing books, to learn the format.
5. `story_review` critiques it the way a director would: findings with line numbers and \
	 suggestions, never a refusal. `storyboard_render` returns the board and its text shot list; \
	 `storyboard_pin` writes that plan into the script as editable `@camera` lines; \
	 `animatic_render` holds each shot for its real duration over the soundtrack; `video_frames` \
	 shows moments in time.
6. `page_render` shows one page; `book_render` makes proofs (with a contact sheet of every page), \
	 the ePub and KDP print files; `video_render` narrates and animates it.";

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
pub struct GuideParams {
	/// The topic: overview, story, staging, camera, light, acting, mixing, sound or workflow. Leave
	/// it out for the overview and the topic list.
	pub topic: Option<String>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LibraryParams {
	/// Only voices whose name, description or labels match, e.g. `british` or `grandad`.
	pub search: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DesignParams {
	pub world: String,
	/// A cast id, or `narrator`.
	pub who: String,
	/// A casting note of 20 to 1,000 characters: "A warm, gravelly grandad with a soft Welsh
	/// lilt, slow and kind". Describe an adult performing a child, as studios cast them.
	pub description: String,
	/// A line of 100 to 1,000 characters for the previews to read; left out, `ElevenLabs` writes
	/// one.
	pub sample: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeepParams {
	pub world: String,
	/// A cast id.
	pub who: String,
	/// The designed voice's number, from `voice_design`.
	pub number: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StoryParams {
	pub world: String,
	/// The book's folder under the world's `stories/`, e.g. `ren-runs`.
	pub story: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteStoryParams {
	pub world: String,
	/// The book's folder under the world's `stories/`: lowercase words joined by hyphens.
	pub story: String,
	/// The whole story.md.
	pub script: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FlowParams {
	pub world: String,
	/// The flow's name, from `flow_list`: `three-tries`, `pile-up` and the rest.
	pub flow: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartStoryParams {
	pub world: String,
	/// The book's title.
	pub title: String,
	/// The cast id of the star.
	pub star: String,
	/// The cast id of the second character, for a flow that stages two or a crowd.
	pub friend: Option<String>,
	/// The flow to start from, from `flow_list`; the world's `recipe.md` when left out.
	pub flow: Option<String>,
	/// The author on the title page; the world's author when left out.
	pub author: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BookParams {
	pub world: String,
	pub story: String,
	/// Which outputs: `proofs` (the default), `ebook`, `print`.
	#[serde(default)]
	pub outputs: Vec<Output>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PageParams {
	pub world: String,
	pub story: String,
	/// Page 1 is the front cover, then every page in order, ending with the back cover.
	pub page: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VideoParams {
	pub world: String,
	pub story: String,
	/// `portrait` (9:16, the default) or `landscape` (16:9).
	#[serde(default)]
	pub shape: VideoShape,
	/// Half size at 15 frames a second, for checking timing quickly.
	#[serde(default)]
	pub preview: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StoryboardParams {
	pub world: String,
	pub story: String,
	/// `landscape` (16:9, the default) or `portrait` (9:16).
	#[serde(default)]
	pub shape: Option<VideoShape>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StoryboardPinParams {
	pub world: String,
	pub story: String,
	/// Pages to pin, counting from one; leave empty for every page.
	#[serde(default)]
	pub pages: Vec<usize>,
	/// Remove the directions a previous pin wrote instead of pinning.
	#[serde(default)]
	pub unpin: bool,
	/// Whose plan to pin: `landscape` (the default) or `portrait`.
	#[serde(default)]
	pub shape: Option<VideoShape>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnimaticParams {
	pub world: String,
	pub story: String,
	/// `landscape` (16:9, the default) or `portrait` (9:16).
	#[serde(default)]
	pub shape: Option<VideoShape>,
	/// Leave every line silent, timed by its estimate, so nothing is recorded and no voice is
	/// needed: a quick timing check.
	#[serde(default)]
	pub silent: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VideoFramesParams {
	pub world: String,
	pub story: String,
	/// Seconds to look at, from the start of the video, e.g. [3.5, 12].
	#[serde(default)]
	pub times: Vec<f32>,
	/// Look every n seconds instead of naming times. At most 12 frames come back.
	pub every: Option<f32>,
	/// `portrait` (9:16, the default) or `landscape` (16:9).
	#[serde(default)]
	pub shape: VideoShape,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SettingParams {
	pub world: String,
	/// The setting's name, as a page heading uses it (`# park`).
	pub setting: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSettingParams {
	pub world: String,
	/// The new setting's name: lowercase words joined by hyphens.
	pub setting: String,
	/// A line on what the place is.
	pub about: String,
	/// A setting to start from (to repaint it); leave out for a sky over the ground.
	pub from: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSettingPartsParams {
	pub world: String,
	pub setting: String,
	/// Parts to write or replace, each one SVG element.
	#[serde(default)]
	pub parts: Vec<SettingPart>,
	/// Part ids to remove.
	#[serde(default)]
	pub remove: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderSettingParams {
	pub world: String,
	pub setting: String,
	#[serde(default, flatten)]
	pub weather: SettingWeather,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PropParams {
	pub world: String,
	/// The prop's name, as `holds=` or a stage direction uses it (`holds=kite`).
	pub prop: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePropParams {
	pub world: String,
	/// The new prop's name: lowercase words joined by hyphens.
	pub prop: String,
	/// A line on what the thing is.
	pub about: String,
	/// A prop to start from (to redraw it); leave out for a plain shape.
	pub from: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPropPartsParams {
	pub world: String,
	pub prop: String,
	/// Parts to write or replace, each one SVG element.
	#[serde(default)]
	pub parts: Vec<PropPart>,
	/// Part ids to remove.
	#[serde(default)]
	pub remove: Vec<String>,
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
		description = "Describe a drawing style: its finish, its bodies and the hair, brows, \
		               noses and builds each offers, every part a figure is drawn with (views, \
		               whether it's required, the coordinate space) and the rules for drawing \
		               parts. Read this before drawing."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn style_describe(&self, Parameters(params): Parameters<StyleParams>) -> CallToolResult {
		json(Studio::describe_style(&params.style))
	}

	#[tool(
		description = "Show a style as images: its template character on a model sheet, its kit \
		               and template parts, and every hair, brows, nose and build it offers."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn style_sheet(&self, Parameters(params): Parameters<StyleParams>) -> CallToolResult {
		images(Studio::style_sheets(&params.style, crate::studio::SHEET_PX))
	}

	#[tool(
		description = "The director's guide, by topic: overview, story, staging, camera, light, \
		               acting, mixing, sound or workflow. No topic returns the overview and the \
		               topic list. Read it before directing a book or a cartoon."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn guide(&self, Parameters(params): Parameters<GuideParams>) -> CallToolResult {
		let Some(topic) = params.topic.as_deref() else {
			let overview = guide_section("overview").unwrap_or_default();

			return CallToolResult::success(vec![ContentBlock::text(format!(
				"{overview}\n\nTopics: {}",
				GUIDE_TOPICS.join(", ")
			))]);
		};

		let Some(section) = guide_section(topic) else {
			return failed(&anyhow::anyhow!(
				"unknown topic `{topic}`; the topics are {}",
				GUIDE_TOPICS.join(", ")
			));
		};

		CallToolResult::success(vec![ContentBlock::text(format!("## {topic}\n\n{section}"))])
	}

	#[tool(
		description = "Add a character to a world's cast: their id, name, kind (kid or grown-up), \
		               who they are, their habit and colours, and the hair, build (slim, plain or \
		               stout), brows and nose style_describe lists for their body. Their drawing \
		               starts as the style's template for their body with those (any left out are \
		               picked from the ones the cast uses least, and written into the cast); \
		               redraw it with character_set_parts."
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
		               parts. The whole figure is checked, and posed on their body to make sure \
		               its parts still join; changes that leave errors are not written. Returns \
		               whether it was written and the findings."
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

	#[tool(description = "List the settings a world's books can use, built-in and its own.")]
	fn setting_list(&self, Parameters(params): Parameters<WorldParams>) -> CallToolResult {
		json(self.studio.settings(&params.world))
	}

	#[tool(
		description = "Start a world's own setting: a copy of one it can use (to repaint it), or \
		               a sky over the ground. Settings are SVG part files on a 2400-unit stage; \
		               setting_parts shows the format."
	)]
	fn setting_create(
		&self,
		Parameters(params): Parameters<CreateSettingParams>,
	) -> CallToolResult {
		json(self.studio.create_setting(
			&params.world,
			&params.setting,
			&params.about,
			params.from.as_deref(),
		))
	}

	#[tool(
		description = "Read a setting's parts as SVG elements. Ids start far- (sky, moves least), \
		               near- (behind the actors) or front- (in front). Fills are palette names; \
		               data-night, data-wilt, data-depth, data-time, data-drift and data-swing \
		               say how a part reacts to the scene."
	)]
	fn setting_parts(&self, Parameters(params): Parameters<SettingParams>) -> CallToolResult {
		json(self.studio.setting_parts(&params.world, &params.setting))
	}

	#[tool(
		description = "Write or replace parts of a world's own setting, each one SVG element, or \
		               remove parts. A file that wouldn't read is not written."
	)]
	fn setting_set_parts(
		&self,
		Parameters(params): Parameters<SetSettingPartsParams>,
	) -> CallToolResult {
		json(self.studio.set_setting_parts(
			&params.world,
			&params.setting,
			&params.parts,
			&params.remove,
		))
	}

	#[tool(description = "Show a setting as an image, by day or night, wilted or not.")]
	fn setting_render(
		&self,
		Parameters(params): Parameters<RenderSettingParams>,
	) -> CallToolResult {
		images(
			self.studio
				.render_setting(&params.world, &params.setting, params.weather)
				.map(|image| vec![image]),
		)
	}

	#[tool(
		description = "List the props a world's books can use, built-in and its own, with how \
		               each is held: its hands, its grip and its carry (hand, up, front, string \
		               or head)."
	)]
	fn prop_list(&self, Parameters(params): Parameters<WorldParams>) -> CallToolResult {
		json(self.studio.props(&params.world))
	}

	#[tool(
		description = "Start a world's own prop: a copy of one it can use (to redraw it), or a \
		               plain shape. Props are SVG part files with a data-grip; prop_parts shows \
		               the format."
	)]
	fn prop_create(&self, Parameters(params): Parameters<CreatePropParams>) -> CallToolResult {
		json(self.studio.create_prop(
			&params.world,
			&params.prop,
			&params.about,
			params.from.as_deref(),
		))
	}

	#[tool(
		description = "Read a prop's parts as SVG elements. The root data-grip is the point in \
		               the fist, data-hands says one or two, data-angle leans it in a relaxed \
		               hand, and data-use=\"brush\" is a toothbrush's behaviour. Fills are \
		               palette names or hex; data-shade, data-line and data-opacity tune each \
		               part."
	)]
	fn prop_parts(&self, Parameters(params): Parameters<PropParams>) -> CallToolResult {
		json(self.studio.prop_parts(&params.world, &params.prop))
	}

	#[tool(
		description = "Write or replace parts of a world's own prop, each one SVG element, or \
		               remove parts. A file that wouldn't read is not written."
	)]
	fn prop_set_parts(&self, Parameters(params): Parameters<SetPropPartsParams>) -> CallToolResult {
		json(
			self.studio
				.set_prop_parts(&params.world, &params.prop, &params.parts, &params.remove),
		)
	}

	#[tool(description = "Show a prop as an image, alone and fitted to the sheet, with its grip.")]
	fn prop_render(&self, Parameters(params): Parameters<PropParams>) -> CallToolResult {
		images(
			self.studio
				.render_prop(&params.world, &params.prop)
				.map(|image| vec![image]),
		)
	}

	#[tool(
		description = "List the story flows a world's new books can start from: each one's name, \
		               what it is for, how many characters it stages, its page and word budgets \
		               and its beats. The world's own flows come before the built-ins."
	)]
	fn flow_list(&self, Parameters(params): Parameters<WorldParams>) -> CallToolResult {
		json(self.studio.flows(&params.world))
	}

	#[tool(
		description = "Describe one story flow: its about, what it is good for, the characters it \
		               needs, its page and word budgets, and its beats as a table of what happens \
		               over how many pages."
	)]
	fn flow_describe(&self, Parameters(params): Parameters<FlowParams>) -> CallToolResult {
		json(self.studio.describe_flow(&params.world, &params.flow))
	}

	#[tool(
		description = "Start a book from a story flow (or the world's recipe): returns the whole \
		               story.md scaffold with the star and friend staged and every page laid out. \
		               Fill in the [bracketed] lines, then write it with story_write."
	)]
	fn story_start(&self, Parameters(params): Parameters<StartStoryParams>) -> CallToolResult {
		let brief = StoryBrief {
			title: params.title,
			star: params.star,
			friend: params.friend,
			flow: params.flow,
			author: params.author,
		};

		match self.studio.start_story(&params.world, &brief) {
			Ok(script) => CallToolResult::success(vec![ContentBlock::text(script)]),
			Err(error) => failed(&error),
		}
	}

	#[tool(
		description = "Write a book's script (the whole story.md) into a world and check it \
		               against the cast. A script that doesn't parse or rehearse is not kept; the \
		               error says what to fix, by line. The story belongs to the world it is \
		               written into, so its front matter may not name a `world`."
	)]
	fn story_write(&self, Parameters(params): Parameters<WriteStoryParams>) -> CallToolResult {
		json(
			self.studio
				.write_story(&params.world, &params.story, &params.script),
		)
	}

	#[tool(description = "Read a book's script (its story.md).")]
	fn story_read(&self, Parameters(params): Parameters<StoryParams>) -> CallToolResult {
		match self.studio.read_story(&params.world, &params.story) {
			Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
			Err(error) => failed(&error),
		}
	}

	#[tool(description = "Check a book against its cast: page and word counts, or what to fix.")]
	fn story_check(&self, Parameters(params): Parameters<StoryParams>) -> CallToolResult {
		json(self.studio.check_story(&params.world, &params.story))
	}

	#[tool(
		description = "Critique a book the way a director would: findings with a severity (`fix` \
		               or `consider`), the script line, an observation and a suggestion in script \
		               vocabulary. Advice, never a refusal; run it before rendering video."
	)]
	fn story_review(&self, Parameters(params): Parameters<StoryParams>) -> CallToolResult {
		json(self.studio.review_story(&params.world, &params.story))
	}

	#[tool(
		description = "Make a book's outputs: proofs (every page, returned as a contact sheet \
		               image), the ePub, and KDP print files. Returns the files written."
	)]
	fn book_render(&self, Parameters(params): Parameters<BookParams>) -> CallToolResult {
		match self
			.studio
			.render_book(&params.world, &params.story, &params.outputs)
		{
			Ok((rendered, contact)) => {
				let mut content = match serde_json::to_string_pretty(&rendered) {
					Ok(text) => vec![ContentBlock::text(text)],
					Err(error) => return failed(&error.into()),
				};

				if let Some(sheet) = contact {
					content.push(ContentBlock::text(sheet.label));
					content.push(ContentBlock::image(
						base64::engine::general_purpose::STANDARD.encode(&sheet.png),
						"image/png",
					));
				}

				CallToolResult::success(content)
			}
			Err(error) => failed(&error),
		}
	}

	#[tool(description = "Show one page of a book as an image (page 1 is the front cover).")]
	fn page_render(&self, Parameters(params): Parameters<PageParams>) -> CallToolResult {
		images(
			self.studio
				.render_page(&params.world, &params.story, params.page)
				.map(|image| vec![image]),
		)
	}

	#[tool(
		description = "Narrate and animate a book as an MP4, lip-synced, in its world's voices. \
		               Takes minutes; a preview is quicker. Returns the files written."
	)]
	fn video_render(&self, Parameters(params): Parameters<VideoParams>) -> CallToolResult {
		json(
			self.studio
				.render_video(&params.world, &params.story, params.shape, params.preview),
		)
	}

	#[tool(
		description = "The storyboard: the video's camera plan as a comic strip, one labelled \
		               panel per shot, and the same plan as a text shot list (page, script line, \
		               timing, shot size, subjects, move, entry, who chose it, words spoken). \
		               Never records a voice or encodes a video, so it runs offline in a second \
		               or two."
	)]
	fn storyboard_render(
		&self,
		Parameters(params): Parameters<StoryboardParams>,
	) -> CallToolResult {
		let shape = params.shape.unwrap_or(VideoShape::Landscape);

		match self.studio.storyboard(&params.world, &params.story, shape) {
			Ok((text, images)) => {
				let mut content = vec![ContentBlock::text(text)];

				for image in images {
					content.push(ContentBlock::text(image.label));
					content.push(ContentBlock::image(
						base64::engine::general_purpose::STANDARD.encode(&image.png),
						"image/png",
					));
				}

				CallToolResult::success(content)
			}
			Err(error) => failed(&error),
		}
	}

	#[tool(
		description = "Pin the director's shot list into a book's script as editable `@camera` \
		               directions, one before the line or direction that starts each shot, or \
		               remove the ones pinned before (`unpin`). A page that already directs its \
		               own camera is left as written. `shape` picks whose plan is pinned \
		               (landscape by default); pages where the other shape plans other shots are \
		               named in one note. Returns the new script."
	)]
	fn storyboard_pin(
		&self,
		Parameters(params): Parameters<StoryboardPinParams>,
	) -> CallToolResult {
		let pages = (!params.pages.is_empty()).then_some(params.pages.as_slice());

		json(self.studio.pin_story(
			&params.world,
			&params.story,
			pages,
			params.unpin,
			params.shape.unwrap_or(VideoShape::Landscape),
		))
	}

	#[tool(
		description = "Cut an animatic: every storyboard shot held for its real duration over the \
		               video's soundtrack, at half size. It draws each panel once and lets ffmpeg \
		               hold it, so a ninety-second film takes seconds, not minutes. Returns the \
		               MP4 and the shot list. Narrates any missing line once, then reuses the \
		               voice cache. A line whose voice cannot speak here (no `say`, no API key, \
		               no command) is left silent and timed by its estimate, and the shot list \
		               says so; `silent: true` does that for every line, recording nothing."
	)]
	fn animatic_render(&self, Parameters(params): Parameters<AnimaticParams>) -> CallToolResult {
		let shape = params.shape.unwrap_or(VideoShape::Landscape);

		json(
			self.studio
				.render_animatic(&params.world, &params.story, shape, params.silent),
		)
	}

	#[tool(
		description = "Frames from a story's video at named times (`times`) or every n seconds \
		               (`every`), as one tiled image. Rendered straight from the frame renderer \
		               with estimated timings, so no voice is recorded and no video encoded. At \
		               most 12 frames."
	)]
	fn video_frames(&self, Parameters(params): Parameters<VideoFramesParams>) -> CallToolResult {
		let sample = match (params.every, params.times.is_empty()) {
			(Some(every), _) => Frames::Every(every),
			(None, false) => Frames::Times(params.times),
			(None, true) => {
				return failed(&anyhow::anyhow!(
					"name moments to look at (`times`: seconds from the start) or an interval \
					 (`every`: seconds between frames)"
				));
			}
		};

		images(
			self.studio
				.video_frames(&params.world, &params.story, &sample, params.shape)
				.map(|image| vec![image]),
		)
	}

	#[tool(description = "The light presets, with a sentence on what each feels like.")]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn light_list(&self) -> CallToolResult {
		json(Ok(Studio::lights()))
	}

	#[tool(
		description = "The camera vocabulary: shot sizes, moves, how a camera arrives (`cut`, \
		               `ease`), and page transitions, with a sentence on what each says."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn shot_list(&self) -> CallToolResult {
		json(Ok(Studio::shots()))
	}

	#[tool(
		description = "List the voices in the ElevenLabs account's library (ids to cast with \
		               voice_cast), optionally only those matching a search. Needs \
		               ELEVENLABS_API_KEY."
	)]
	#[expect(
		clippy::unused_self,
		reason = "every tool is a method, so the router can call it"
	)]
	fn voice_library(&self, Parameters(params): Parameters<LibraryParams>) -> CallToolResult {
		json(Studio::elevenlabs_voices(params.search.as_deref()))
	}

	#[tool(
		description = "Design ElevenLabs voices for a character (or the narrator) from a casting \
		               note. Returns each numbered preview as audio; listen, then keep one with \
		               voice_keep. Needs ELEVENLABS_API_KEY."
	)]
	fn voice_design(&self, Parameters(params): Parameters<DesignParams>) -> CallToolResult {
		match self.studio.design_voice(
			&params.world,
			&params.who,
			&params.description,
			params.sample.as_deref(),
		) {
			Ok(designed) => {
				let mut blocks = Vec::new();

				for voice in designed {
					blocks.push(ContentBlock::text(format!(
						"Voice {} ({:.1}s)",
						voice.number, voice.seconds
					)));
					blocks.push(ContentBlock::audio(
						base64::engine::general_purpose::STANDARD.encode(&voice.mp3),
						"audio/mpeg",
					));
				}

				CallToolResult::success(blocks)
			}
			Err(error) => failed(&error),
		}
	}

	#[tool(
		description = "Keep a designed voice by its number: it is saved to the ElevenLabs library \
		               and cast as the character's voice, with their old macOS voice kept as the \
		               fallback."
	)]
	fn voice_keep(&self, Parameters(params): Parameters<KeepParams>) -> CallToolResult {
		json(
			self.studio
				.keep_voice(&params.world, &params.who, params.number),
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
			"guide",
			"character_create",
			"character_parts",
			"character_set_parts",
			"character_sheet",
			"voice_providers",
			"voice_list",
			"voice_cast",
			"voice_library",
			"voice_design",
			"voice_keep",
			"voice_audition",
			"story_write",
			"story_read",
			"story_check",
			"story_review",
			"flow_list",
			"flow_describe",
			"story_start",
			"book_render",
			"page_render",
			"video_render",
			"storyboard_render",
			"storyboard_pin",
			"animatic_render",
			"video_frames",
			"light_list",
			"shot_list",
			"setting_list",
			"setting_create",
			"setting_parts",
			"setting_set_parts",
			"setting_render",
			"prop_list",
			"prop_create",
			"prop_parts",
			"prop_set_parts",
			"prop_render",
		] {
			assert!(
				names.iter().any(|listed| listed == name),
				"{name}: {names:?}"
			);
		}
	}

	#[test]
	fn the_guide_serves_every_topic_and_only_the_topics() {
		let sections: Vec<&str> = guide_sections().map(|(heading, _)| heading).collect();

		for topic in GUIDE_TOPICS {
			let lowercase = topic.to_ascii_lowercase();
			assert!(
				sections
					.iter()
					.any(|heading| heading.eq_ignore_ascii_case(&lowercase)),
				"the guide has no {topic} section"
			);
			assert!(
				guide_section(topic).is_some_and(|body| body.len() > 200),
				"the {topic} section has something to say"
			);
		}

		for heading in sections {
			assert!(
				GUIDE_TOPICS
					.iter()
					.any(|topic| heading.eq_ignore_ascii_case(topic)),
				"the guide's `{heading}` section is not a topic"
			);
		}
	}

	#[test]
	fn an_unknown_guide_topic_is_an_error_listing_the_topics() {
		let server = StudioServer::new(Path::new("."));
		let result = server.guide(Parameters(GuideParams {
			topic: Some("lenses".into()),
		}));

		assert_eq!(result.is_error, Some(true));
		match &result.content[0] {
			ContentBlock::Text(text) => {
				assert!(text.text.contains("staging"), "{}", text.text);
				assert!(text.text.contains("workflow"), "{}", text.text);
			}
			other => panic!("the error comes back as text: {other:?}"),
		}
	}

	#[test]
	fn a_flow_lists_and_starts_a_book() {
		let server = StudioServer::new(Path::new("."));
		let listed = server.flow_list(Parameters(WorldParams {
			world: "mumbleton".into(),
		}));
		assert_eq!(listed.is_error, Some(false));

		match &listed.content[0] {
			ContentBlock::Text(text) => {
				assert!(text.text.contains("three-tries"), "{}", text.text);
				assert!(text.text.contains("good_for"), "{}", text.text);
			}
			other => panic!("the flow list comes back as JSON text: {other:?}"),
		}

		let started = server.story_start(Parameters(StartStoryParams {
			world: "mumbleton".into(),
			title: "A Test Tale".into(),
			star: "mo".into(),
			friend: None,
			flow: Some("three-tries".into()),
			author: None,
		}));
		assert_eq!(started.is_error, Some(false));

		match &started.content[0] {
			ContentBlock::Text(text) => {
				assert!(
					text.text.contains("flow = \"three-tries\""),
					"{}",
					text.text
				);
				assert!(text.text.contains("@mo"), "{}", text.text);
				assert!(
					text.text.contains("[The star says what they want.]"),
					"{}",
					text.text
				);
				assert_eq!(
					text.text.matches("\nage").count(),
					1,
					"one age line: {}",
					text.text
				);
				assert!(text.text.contains("age = \"3-5\""), "{}", text.text);
			}
			other => panic!("the scaffold comes back as text: {other:?}"),
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
