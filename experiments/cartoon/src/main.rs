//! Renders the cartoon experiment to `out/experiments/cartoon/`.
//!
//! ```text
//! cargo run --release            # everything
//! cargo run --release -- sheet   # just the named pieces: sheet, turnaround, mouths, rig,
//!                                # compare, park, page, film
//! ```

mod film;
mod page;
mod park;
mod sheets;
mod stinky;

use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;

const PIECES: [&str; 8] = [
	"sheet",
	"turnaround",
	"mouths",
	"rig",
	"compare",
	"park",
	"page",
	"film",
];

fn main() -> Result<()> {
	let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../out/experiments/cartoon");
	std::fs::create_dir_all(&out)?;
	let wanted: Vec<String> = std::env::args().skip(1).collect();

	for name in &wanted {
		if !PIECES.contains(&name.as_str()) {
			bail!("unknown piece {name:?}; choose from {}", PIECES.join(", "));
		}
	}

	for piece in PIECES {
		if !wanted.is_empty() && !wanted.iter().any(|name| name == piece) {
			continue;
		}

		let path = match piece {
			"sheet" => sheets::model_sheet(&out)?,
			"turnaround" => sheets::turnaround(&out)?,
			"mouths" => sheets::mouth_chart(&out)?,
			"rig" => sheets::rig_sheet(&out)?,
			"compare" => sheets::comparison(&out)?,
			"park" => page::park_plate(&out)?,
			"page" => {
				let page = page::book_page(&out)?;
				println!("wrote {}", page.display());
				page::page_comparison(&out, &page)?
			}
			"film" => {
				println!("wrote {}", film::strip(&out)?.display());
				film::film(&out)?
			}
			_ => continue,
		};
		println!("wrote {}", path.display());
	}

	Ok(())
}
