//! `fables mcp` end to end: the real binary on stdio, driven the way an agent drives it. It
//! starts a world, adds a character, redraws a part and gets the model sheet back as an image,
//! without touching a file itself.

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
