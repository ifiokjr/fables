//! Renders the cartoon experiment to `out/experiments/cartoon/`.
//!
//! ```text
//! cargo run --release            # everything
//! cargo run --release -- sheet   # just the named pieces: sheet, preview, extras, turnaround,
//!                                # mouths, rig,
//!                                # compare, park, page, film
//! ```

mod film;
mod page;
mod park;
mod preview;
mod sheets;
mod stinky;

use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;

const PIECES: [&str; 10] = [
	"sheet",
	"preview",
	"extras",
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

	stinky::check()?;

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
			"preview" => {
				let mut sheets = preview::cast_preview(&out)?;
				let last = sheets.pop().unwrap_or_else(|| out.clone());

				for sheet in sheets {
					println!("wrote {}", sheet.display());
				}

				last
			}
			"extras" => sheets::extras_sheet(&out)?,
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
