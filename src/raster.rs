//! SVG to pixels, plus the paper grain that gives every render its printed feel.

use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;

use crate::art::style::Style;

/// Fonts embedded in the binary so renders never depend on what is installed.
pub const BODY_FONT: &[u8] = include_bytes!("../assets/fonts/Andika-Regular.ttf");
pub const BODY_BOLD_FONT: &[u8] = include_bytes!("../assets/fonts/Andika-Bold.ttf");
pub const DISPLAY_FONT: &[u8] = include_bytes!("../assets/fonts/LilitaOne-Regular.ttf");

/// Family names as they appear inside the font files.
pub const BODY_FAMILY: &str = "Andika";
pub const DISPLAY_FAMILY: &str = "Lilita One";

/// The shared font database (loading fonts is slow, so it happens once).
pub fn fonts() -> Arc<usvg::fontdb::Database> {
	static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

	FONTS
		.get_or_init(|| {
			let mut db = usvg::fontdb::Database::new();
			db.load_font_data(BODY_FONT.to_vec());
			db.load_font_data(BODY_BOLD_FONT.to_vec());
			db.load_font_data(DISPLAY_FONT.to_vec());
			db.set_serif_family(BODY_FAMILY);
			db.set_sans_serif_family(BODY_FAMILY);
			Arc::new(db)
		})
		.clone()
}

/// Parsing options that resolve text against the embedded fonts.
pub fn options() -> usvg::Options<'static> {
	usvg::Options {
		fontdb: fonts(),
		..usvg::Options::default()
	}
}

/// Parses an SVG document.
pub fn parse(svg: &str) -> Result<usvg::Tree> {
	usvg::Tree::from_str(svg, &options()).context("generated SVG failed to parse")
}

/// Renders an SVG document to a pixmap at the document's own pixel size.
pub fn render(svg: &str) -> Result<Pixmap> {
	let tree = parse(svg)?;
	let size = tree.size().to_int_size();
	let mut pixmap =
		Pixmap::new(size.width(), size.height()).context("image size must be non-zero")?;
	resvg::render(
		&tree,
		resvg::tiny_skia::Transform::default(),
		&mut pixmap.as_mut(),
	);

	Ok(pixmap)
}

/// Darkens pixels with a fine, stable speckle, like ink sitting on uncoated paper.
///
/// `strength` is the maximum darkening (0.0 to 1.0). `shift` moves the grain so video can
/// make it crawl gently between frames instead of looking like a dirty lens.
pub fn grain(pixmap: &mut Pixmap, strength: f32, shift: u32) {
	if strength <= 0.0 {
		return;
	}

	let width = pixmap.width();
	let scale = (strength * 255.0) as u32;

	for (index, pixel) in pixmap
		.data_mut()
		.as_chunks_mut::<4>()
		.0
		.iter_mut()
		.enumerate()
	{
		let x = index as u32 % width;
		let y = index as u32 / width;
		let mut h = x.wrapping_mul(0x8DA6_B343)
			^ y.wrapping_mul(0xD816_3841)
			^ shift.wrapping_mul(0xCB1A_B31F);
		h ^= h >> 13;
		h = h.wrapping_mul(0x5bd1_e995);
		h ^= h >> 15;

		// Performance: integer maths only; this runs over millions of pixels per video frame.
		let speck = (h & 0xFF) * scale / 255;
		let keep = 255 - speck.min(255);

		for channel in &mut pixel[..3] {
			*channel = (u32::from(*channel) * keep / 255) as u8;
		}
	}
}

/// Applies a style's finish to a rendered image: colour grade, paper mottling, then grain.
///
/// `grain` is the output's base grain (print, screen and video differ); the style scales it.
pub fn finish(pixmap: &mut Pixmap, style: &Style, grain_amount: f32, shift: u32) {
	let look = style.finish;
	let grading = (look.saturation - 1.0).abs() > 0.01 || look.lift > 0.0 || look.mottle > 0.0;

	if grading {
		grade(pixmap, look.saturation, look.lift, look.mottle);
	}

	grain(pixmap, grain_amount * look.grain, shift);
}

fn grade(pixmap: &mut Pixmap, saturation: f32, lift: f32, mottle: f32) {
	let width = pixmap.width() as usize;
	// Paper blooms are about a twentieth of the image wide, whatever the resolution.
	let cell = (width / 20).max(8) as f32;

	for (index, pixel) in pixmap
		.data_mut()
		.as_chunks_mut::<4>()
		.0
		.iter_mut()
		.enumerate()
	{
		let x = (index % width) as f32 / cell;
		let y = (index / width) as f32 / cell;
		let bloom = 1.0 + (value_noise(x, y) - 0.5) * mottle * 0.14;
		let [r, g, b] = [
			f32::from(pixel[0]),
			f32::from(pixel[1]),
			f32::from(pixel[2]),
		];
		let luma = 0.299 * r + 0.587 * g + 0.114 * b;

		for (channel, value) in pixel[..3].iter_mut().zip([r, g, b]) {
			let saturated = luma + (value - luma) * saturation;
			let lifted = saturated + (255.0 - saturated) * lift;
			*channel = (lifted * bloom).clamp(0.0, 255.0) as u8;
		}
	}
}

/// Smooth 2D value noise in 0..1 (bilinear between hashed lattice points).
fn value_noise(x: f32, y: f32) -> f32 {
	let (x0, y0) = (x.floor(), y.floor());
	let (fx, fy) = (x - x0, y - y0);
	let corner = |dx: f32, dy: f32| {
		let lattice = ((x0 + dx) as i64 as u64).wrapping_mul(73_856_093)
			^ ((y0 + dy) as i64 as u64).wrapping_mul(19_349_663);
		f32::midpoint(crate::art::noise(0x5EED, lattice), 1.0)
	};
	let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
	let (sx, sy) = (smooth(fx), smooth(fy));
	let top = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * sx;
	let bottom = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * sx;

	top + (bottom - top) * sy
}

/// Encodes a pixmap as JPEG (RGB, alpha dropped onto white).
pub fn jpeg(pixmap: &Pixmap, quality: u8) -> Result<Vec<u8>> {
	let mut rgb = Vec::with_capacity((pixmap.width() * pixmap.height() * 3) as usize);

	for pixel in pixmap.data().as_chunks::<4>().0 {
		rgb.extend_from_slice(&pixel[..3]);
	}

	let mut out = Vec::new();
	let encoder = jpeg_encoder::Encoder::new(&mut out, quality);
	encoder
		.encode(
			&rgb,
			u16::try_from(pixmap.width()).context("image too wide for JPEG")?,
			u16::try_from(pixmap.height()).context("image too tall for JPEG")?,
			jpeg_encoder::ColorType::Rgb,
		)
		.context("JPEG encoding failed")?;

	Ok(out)
}

/// Encodes a pixmap as PNG.
pub fn png(pixmap: &Pixmap) -> Result<Vec<u8>> {
	pixmap.encode_png().context("PNG encoding failed")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn renders_simple_svg() {
		let pixmap = render(r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#ff0000"/></svg>"##).unwrap();
		assert_eq!(pixmap.width(), 4);
		assert_eq!(&pixmap.data()[..4], &[255, 0, 0, 255]);
	}

	#[test]
	fn grain_only_darkens() {
		let mut pixmap = Pixmap::new(32, 32).unwrap();
		pixmap.fill(resvg::tiny_skia::Color::WHITE);
		grain(&mut pixmap, 0.2, 0);
		assert!(
			pixmap
				.data()
				.as_chunks::<4>()
				.0
				.iter()
				.all(|p| p[0] >= 200 && p[3] == 255)
		);
		assert!(pixmap.data().as_chunks::<4>().0.iter().any(|p| p[0] < 255));
	}

	#[test]
	fn embedded_fonts_are_registered() {
		let db = fonts();
		let families: Vec<String> = db
			.faces()
			.flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
			.collect();
		assert!(
			families.iter().any(|name| name == BODY_FAMILY),
			"{families:?}"
		);
		assert!(
			families.iter().any(|name| name == DISPLAY_FAMILY),
			"{families:?}"
		);
	}
}
