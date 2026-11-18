//! `fables mcp` end to end: the real binary on stdio, driven the way an agent drives it. It
//! starts a world, adds a character, redraws a part, gets the model sheet back as an image, and
//! writes and renders a page of a book, without touching a file itself.

use std::io::BufRead as _;
use std::io::BufReader;
use std::io::Write as _;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use serde_json::Value;
use serde_json::json;

struct Session {
	child: Child,
	input: ChildStdin,
	output: BufReader<ChildStdout>,
	next: u64,
}

impl Session {
	fn start(root: &std::path::Path) -> Session {
		let mut child = Command::new(env!("CARGO_BIN_EXE_fables"))
			.args(["mcp", "--root"])
			.arg(root)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.expect("fables runs");
		let input = child.stdin.take().unwrap();
		let output = BufReader::new(child.stdout.take().unwrap());
		let mut session = Session {
			child,
			input,
			output,
			next: 1,
		};
		let hello = session.request(
			"initialize",
			&json!({
				"protocolVersion": "2025-06-18",
				"capabilities": {},
				"clientInfo": { "name": "test", "version": "0" }
			}),
		);
		assert!(
			hello["instructions"]
				.as_str()
				.unwrap()
				.contains("character")
		);
		session.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
		session
	}

	fn send(&mut self, message: &Value) {
		writeln!(self.input, "{message}").unwrap();
		self.input.flush().unwrap();
	}

	fn request(&mut self, method: &str, params: &Value) -> Value {
		let id = self.next;
		self.next += 1;
		self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));

		loop {
			let mut line = String::new();
			assert!(
				self.output.read_line(&mut line).unwrap() > 0,
				"the server hung up"
			);
			let message: Value = serde_json::from_str(&line).unwrap();

			if message["id"] == id {
				assert!(message.get("error").is_none(), "{message}");
				return message["result"].clone();
			}
		}
	}

	/// Calls a tool and returns its content, failing on a tool error.
	fn call(&mut self, tool: &str, arguments: &Value) -> Vec<Value> {
		let result = self.request(
			"tools/call",
			&json!({ "name": tool, "arguments": arguments }),
		);
		assert_ne!(result["isError"], json!(true), "{tool}: {result}");
		result["content"].as_array().unwrap().clone()
	}

	fn json(&mut self, tool: &str, arguments: &Value) -> Value {
		let content = self.call(tool, arguments);
		serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap()
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

#[test]
fn an_agent_designs_a_character_without_touching_files() {
	let root = tempfile::tempdir().unwrap();
	let mut session = Session::start(root.path());

	let tools = session.request("tools/list", &json!({}));
	let names: Vec<&str> = tools["tools"]
		.as_array()
		.unwrap()
		.iter()
		.map(|tool| tool["name"].as_str().unwrap())
		.collect();
	assert!(names.contains(&"character_set_parts"), "{names:?}");

	let world = session.json(
		"world_create",
		&json!({ "name": "Nibbleton", "about": "A town of small habits." }),
	);
	assert_eq!(world["folder"], "nibbleton");

	let style = session.json("style_describe", &json!({ "style": "cartoon" }));
	assert!(style["rules"].as_array().unwrap().len() > 3);

	let nell = session.json(
		"character_create",
		&json!({
			"world": "nibbleton",
			"character": {
				"id": "nell",
				"name": "Nell Nibbles",
				"habit": "Nibbles her nails.",
				"colours": {
					"skin": "#f4c766", "hair": "#7a4a35", "top": "#ec7aa6", "trim": "#fff8ef",
					"bottom": "#2e9a90", "shoes": "#7a5cb8", "eyes": "#5d9e5a"
				}
			}
		}),
	);
	assert_eq!(nell["puppet"], "puppets/nell.svg");

	let ear = session.json(
		"character_parts",
		&json!({ "world": "nibbleton", "id": "nell", "names": ["ear"] }),
	);
	assert_eq!(ear[0]["from"], "kit");

	let update = session.json(
		"character_set_parts",
		&json!({
			"world": "nibbleton",
			"id": "nell",
			"parts": [{ "name": "ear", "d": "M0 -20 C-16 -32 -36 -20 -34 2 C-32 22 -12 28 2 18 Z" }]
		}),
	);
	assert_eq!(update["written"], true, "{update}");

	let sheet = session.call(
		"character_sheet",
		&json!({ "world": "nibbleton", "id": "nell", "sheets": ["model"] }),
	);
	let image = sheet
		.iter()
		.find(|block| block["type"] == "image")
		.expect("the sheet comes back as an image");
	assert_eq!(image["mimeType"], "image/png");
	assert!(image["data"].as_str().unwrap().len() > 10_000);

	// Then a one-page book with her in it, checked against the cast, and its page rendered.
	let report = session.json(
		"story_write",
		&json!({
			"world": "nibbleton",
			"story": "nell-nibbles",
			"script": "+++\ntitle = \"Nell Nibbles\"\nauthor = \"A\"\nstar = \"nell\"\n+++\n# park\n@nell x=0.5 happy wave\nNell waved.\n"
		}),
	);
	assert_eq!(report["script_pages"], 1);
	let page = session.call(
		"page_render",
		&json!({ "world": "nibbleton", "story": "nell-nibbles", "page": 4 }),
	);
	assert!(page.iter().any(|block| block["type"] == "image"));

	// A mistake comes back as a tool error the agent can read, not a broken session.
	let missing = session.request(
		"tools/call",
		&json!({ "name": "world_describe", "arguments": { "world": "atlantis" } }),
	);
	assert_eq!(missing["isError"], true);
	assert!(
		missing["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("world_list")
	);
}

/// The seeing and directing tools, over the protocol.
#[test]
fn an_agent_reads_the_guide_reviews_and_sees_the_video() {
	let root = tempfile::tempdir().unwrap();
	let mut session = Session::start(root.path());

	// The guide: overview with the topic list, one topic, an unknown topic's error.
	let overview = session.call("guide", &json!({}));
	let text = overview[0]["text"].as_str().unwrap();
	assert!(text.contains("three passes"), "{text}");
	assert!(text.contains("Topics:"), "{text}");

	let topic = session.call("guide", &json!({ "topic": "mixing" }));
	assert!(
		topic[0]["text"].as_str().unwrap().contains("guests"),
		"{}",
		topic[0]["text"]
	);

	let bad = session.request(
		"tools/call",
		&json!({ "name": "guide", "arguments": { "topic": "lenses" } }),
	);
	assert_eq!(bad["isError"], true);
	assert!(
		bad["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("workflow")
	);

	// A world, a character, a book: then review it and see it.
	session.json(
		"world_create",
		&json!({ "name": "Nibbleton", "about": "A town of small habits." }),
	);
	session.json(
		"character_create",
		&json!({
			"world": "nibbleton",
			"character": {
				"id": "nell", "name": "Nell Nibbles", "habit": "Nibbles her nails.",
				"colours": {
					"skin": "#f4c766", "hair": "#7a4a35", "top": "#ec7aa6", "trim": "#fff8ef",
					"bottom": "#2e9a90", "shoes": "#7a5cb8", "eyes": "#5d9e5a"
				}
			}
		}),
	);
	session.json(
		"story_write",
		&json!({
			"world": "nibbleton",
			"story": "nell-nibbles",
			"script": "+++\ntitle = \"Nell Nibbles\"\nauthor = \"A\"\nstar = \"nell\"\n+++\n# park\n@nell x=0.5 happy wave\nNell nibbled her nails.\n"
		}),
	);

	let findings = session.json(
		"story_review",
		&json!({ "world": "nibbleton", "story": "nell-nibbles" }),
	);
	let list = findings.as_array().unwrap();
	assert!(!list.is_empty(), "a four-word book has something to say");
	assert!(list[0]["line"].as_u64().unwrap() > 0, "{list:?}");
	assert!(
		list[0]["suggestion"]
			.as_str()
			.unwrap()
			.contains(char::is_alphabetic)
	);

	let board = session.call(
		"storyboard_render",
		&json!({ "world": "nibbleton", "story": "nell-nibbles" }),
	);
	assert!(
		board.iter().any(|block| {
			block["type"] == "image" && block["data"].as_str().unwrap().len() > 10_000
		}),
		"the board comes back as an image"
	);

	let frames = session.call(
		"video_frames",
		&json!({ "world": "nibbleton", "story": "nell-nibbles", "every": 2 }),
	);
	assert!(frames.iter().any(|block| block["type"] == "image"));

	// Frames outside the video are an error that says where the video ends.
	let outside = session.request(
		"tools/call",
		&json!({ "name": "video_frames", "arguments": {
			"world": "nibbleton", "story": "nell-nibbles", "times": [999.0]
		}}),
	);
	assert_eq!(outside["isError"], true);
	assert!(
		outside["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("outside the video")
	);

	let lights = session.json("light_list", &json!({}));
	assert_eq!(lights.as_array().unwrap().len(), 10);
	assert!(
		lights[9]["name"] == "candle" && lights[9]["about"].as_str().unwrap().contains("light"),
		"{lights:?}"
	);

	let shots = session.json("shot_list", &json!({}));
	assert_eq!(shots["sizes"].as_array().unwrap().len(), 5);
	assert_eq!(shots["moves"].as_array().unwrap().len(), 7);
	assert_eq!(shots["transitions"].as_array().unwrap().len(), 6);
	assert_eq!(shots["sizes"][4]["name"], "extreme-close");
}

/// A guest from another world, invited over the protocol and drawn in the host's book.
#[test]
fn an_agent_invites_a_guest_from_another_world() {
	let root = tempfile::tempdir().unwrap();
	let mut session = Session::start(root.path());

	session.json(
		"world_create",
		&json!({ "name": "Harbour", "about": "Kites.", "style": "anime" }),
	);
	session.json(
		"character_create",
		&json!({
			"world": "harbour",
			"character": {
				"id": "hana", "name": "Hana", "habit": "Holds the string.",
				"colours": {
					"skin": "#f8dcc8", "hair": "#2b2f5e", "top": "#ffffff", "trim": "#2f6fb8",
					"bottom": "#2f4f8a", "shoes": "#7a3b2e", "eyes": "#3f8fd0"
				}
			}
		}),
	);
	session.json("world_create", &json!({ "name": "Hill", "about": "Wind." }));
	session.json(
		"character_create",
		&json!({
			"world": "hill",
			"character": {
				"id": "mo", "name": "Mo", "habit": "Mumbles.",
				"colours": {
					"skin": "#d99a6c", "hair": "#3b2a24", "top": "#008080", "trim": "#fff8ef",
					"bottom": "#3a4a7a", "shoes": "#ff8a3d", "eyes": "#4a3426"
				}
			}
		}),
	);

	// The host world lists its neighbour and who could be invited.
	let described = session.json("world_describe", &json!({ "world": "hill" }));
	assert_eq!(described["visitors"].as_array().unwrap().len(), 1);
	let visitor = &described["visitors"][0];
	assert_eq!(visitor["folder"], "harbour");
	assert_eq!(visitor["style"], "anime");
	assert!(
		visitor["cast"]
			.as_array()
			.unwrap()
			.iter()
			.any(|m| m["id"] == "hana")
	);

	// A script with a guest is checked against both casts and renders.
	let report = session.json(
		"story_write",
		&json!({
			"world": "hill",
			"story": "the-visitor",
			"script": "+++\ntitle = \"The Visitor\"\nauthor = \"A\"\nstar = \"mo\"\nguests = [\"harbour/hana\"]\n+++\n# park\n@mo x=0.3 worried\n@hana enter=right x=0.7 wave talk\n\"Hello!\" said Hana.\n@mo shocked\nMo had never met anybody from the harbour.\n"
		}),
	);
	assert_eq!(report["script_pages"], 1);

	let page = session.call(
		"page_render",
		&json!({ "world": "hill", "story": "the-visitor", "page": 4 }),
	);
	assert!(
		page.iter().any(|block| block["type"] == "image"),
		"the guest's page comes back as an image"
	);

	// A guest nobody can find is refused, naming the worlds that exist.
	let refused = session.request(
		"tools/call",
		&json!({ "name": "story_write", "arguments": {
			"world": "hill",
			"story": "nobody",
			"script": "+++\ntitle = \"Nobody\"\nauthor = \"A\"\nguests = [\"atlantis/pip\"]\n+++\n# park\n@pip x=0.5\nHi.\n"
		}}),
	);
	assert_eq!(refused["isError"], true);
	let error = refused["content"][0]["text"].as_str().unwrap();
	assert!(
		error.contains("atlantis") && error.contains("harbour"),
		"{error}"
	);

	let kept = session.request(
		"tools/call",
		&json!({ "name": "story_read", "arguments": { "world": "hill", "story": "nobody" } }),
	);
	assert_eq!(kept["isError"], true, "the refused story is not kept");
}

/// A settings file outside the root, with the parts a setting needs, so only the sandbox can
/// refuse copying it in.
const DECOY_SETTING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-160 -160 2720 2720" data-ground="2080">
  <title>Decoy</title>
  <path id="near-secret-vault" d="M0 0 L100 100 Z" fill="moss"/>
</svg>"#;

const MINIMAL_SCRIPT: &str = "+++\ntitle = \"Probe\"\nauthor = \"A\"\n+++\n# park\nHi.\n";

/// Every tool that takes a world, story, setting or character name, or a `from` setting.
fn calls_taking_a_name(name: &str) -> Vec<(&'static str, Value)> {
	let colours = json!({
		"skin": "#f4c766", "hair": "#7a4a35", "top": "#ec7aa6", "trim": "#fff8ef",
		"bottom": "#2e9a90", "shoes": "#7a5cb8", "eyes": "#5d9e5a"
	});

	vec![
		("world_describe", json!({ "world": name })),
		("setting_list", json!({ "world": name })),
		(
			"character_create",
			json!({ "world": "nibbleton", "character": { "id": name, "name": "Probe", "colours": colours } }),
		),
		(
			"character_parts",
			json!({ "world": "nibbleton", "id": name }),
		),
		(
			"character_set_parts",
			json!({ "world": "nibbleton", "id": name, "parts": [] }),
		),
		(
			"character_sheet",
			json!({ "world": "nibbleton", "id": name }),
		),
		(
			"voice_cast",
			json!({ "world": "nibbleton", "id": name, "voice": { "provider": "say", "name": "Daniel" } }),
		),
		(
			"voice_audition",
			json!({ "world": "nibbleton", "who": name, "line": "Hi." }),
		),
		(
			"story_write",
			json!({ "world": "nibbleton", "story": name, "script": MINIMAL_SCRIPT }),
		),
		("story_read", json!({ "world": "nibbleton", "story": name })),
		(
			"story_check",
			json!({ "world": "nibbleton", "story": name }),
		),
		(
			"story_review",
			json!({ "world": "nibbleton", "story": name }),
		),
		(
			"book_render",
			json!({ "world": "nibbleton", "story": name }),
		),
		(
			"page_render",
			json!({ "world": "nibbleton", "story": name, "page": 1 }),
		),
		(
			"storyboard_render",
			json!({ "world": "nibbleton", "story": name }),
		),
		(
			"video_frames",
			json!({ "world": "nibbleton", "story": name, "times": [0.0] }),
		),
		(
			"video_render",
			json!({ "world": "nibbleton", "story": name, "preview": true }),
		),
		(
			"setting_create",
			json!({ "world": "nibbleton", "setting": name, "about": "Probe" }),
		),
		(
			"setting_parts",
			json!({ "world": "nibbleton", "setting": name }),
		),
		(
			"setting_set_parts",
			json!({ "world": "nibbleton", "setting": name, "parts": [] }),
		),
		(
			"setting_render",
			json!({ "world": "nibbleton", "setting": name }),
		),
	]
}

/// The sandbox the readme promises: a name is a slug, never a path, and nothing outside the
/// root is read or written, whichever tool is asked.
#[test]
fn a_path_outside_the_studio_root_is_refused_by_every_tool() {
	// The root and the decoy are siblings, as in the report's reproduction, so the relative
	// `..` forms really do leave the root.
	let base = tempfile::tempdir().unwrap();
	let root = base.path().join("root");
	let outside = base.path().join("outside");
	std::fs::create_dir_all(&root).unwrap();
	std::fs::create_dir_all(&outside).unwrap();

	// A decoy setting, and a decoy world with a book, both outside the root.
	let decoy = outside.join("decoy.svg");
	std::fs::write(&decoy, DECOY_SETTING).unwrap();
	let away = outside.join("worlds/away");
	std::fs::create_dir_all(away.join("stories/book")).unwrap();
	std::fs::write(away.join("world.toml"), "name = \"Away\"\n").unwrap();
	std::fs::write(away.join("stories/book/story.md"), MINIMAL_SCRIPT).unwrap();

	let mut session = Session::start(&root);
	session.json("world_create", &json!({ "name": "Nibbleton" }));
	session.json(
		"character_create",
		&json!({
			"world": "nibbleton",
			"character": {
				"id": "nell", "name": "Nell Nibbles",
				"colours": {
					"skin": "#f4c766", "hair": "#7a4a35", "top": "#ec7aa6", "trim": "#fff8ef",
					"bottom": "#2e9a90", "shoes": "#7a5cb8", "eyes": "#5d9e5a"
				}
			}
		}),
	);
	session.json(
		"story_write",
		&json!({ "world": "nibbleton", "story": "nell-nibbles", "script": MINIMAL_SCRIPT }),
	);
	// A setting of the world's own, so its `settings/` folder exists: a relative `..` needs it.
	session.json(
		"setting_create",
		&json!({ "world": "nibbleton", "setting": "harbour", "about": "The harbour wall" }),
	);

	let forms = [
		// `setting_create` appends `.svg`, so the reported exploit names the decoy without it.
		decoy.with_extension("").display().to_string(),
		decoy.display().to_string(),
		"../../../outside/decoy".into(),
		"../../../../outside/decoy".into(),
		"../worlds/away".into(),
		"/etc/passwd".into(),
		"..".into(),
	];

	for form in &forms {
		for (tool, arguments) in calls_taking_a_name(form) {
			let result = session.request(
				"tools/call",
				&json!({ "name": tool, "arguments": arguments }),
			);

			assert_eq!(
				result["isError"],
				json!(true),
				"{tool} accepted `{form}`: {result}"
			);
		}

		// `setting_create`'s `from` is the reported hole: a copy of a file outside the root.
		let result = session.request(
			"tools/call",
			&json!({
				"name": "setting_create",
				"arguments": {
					"world": "nibbleton", "setting": "stolen", "about": "Probe", "from": form
				}
			}),
		);
		assert_eq!(
			result["isError"],
			json!(true),
			"setting_create copied `{form}`: {result}"
		);
	}

	// A display name is slugified rather than refused, and lands under `worlds/`.
	let created = session.request(
		"tools/call",
		&json!({ "name": "world_create", "arguments": { "name": "../../outside" } }),
	);
	assert!(
		created["isError"] == json!(true)
			|| created["content"][0]["text"]
				.as_str()
				.is_some_and(|text| text.contains("\"folder\": \"outside\"")),
		"{created}"
	);
	assert!(
		!outside.join("outside").exists(),
		"no world was made outside the root"
	);
	assert!(
		!root.join("outside").exists(),
		"no world was made beside the root"
	);

	// Nothing outside the root was read into it or written to.
	assert_eq!(
		std::fs::read_to_string(&decoy).unwrap(),
		DECOY_SETTING,
		"the decoy is untouched"
	);
	assert!(
		!root.join("worlds/nibbleton/settings/stolen.svg").exists(),
		"nothing was copied into the world"
	);
}

/// A story written over MCP belongs to the world it is written into, so its front matter may
/// not name another world.
#[test]
fn a_story_written_over_mcp_cannot_name_its_own_world() {
	let root = tempfile::tempdir().unwrap();
	let mut session = Session::start(root.path());
	session.json("world_create", &json!({ "name": "Nibbleton" }));

	let refused = session.request(
		"tools/call",
		&json!({
			"name": "story_write",
			"arguments": {
				"world": "nibbleton",
				"story": "elsewhere",
				"script": "+++\ntitle = \"Elsewhere\"\nauthor = \"A\"\nworld = \"../elsewhere\"\n+++\n# plain\nHi.\n"
			}
		}),
	);
	assert_eq!(refused["isError"], true);
	let error = refused["content"][0]["text"].as_str().unwrap();
	assert!(
		error.contains("world") && error.contains("written into"),
		"{error}"
	);

	let kept = session.request(
		"tools/call",
		&json!({ "name": "story_read", "arguments": { "world": "nibbleton", "story": "elsewhere" } }),
	);
	assert_eq!(kept["isError"], true, "the refused story is not kept");
}
