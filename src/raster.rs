//! SVG to pixels, plus the paper grain that gives every render its printed feel.

use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;

use crate::art::dials::Tone;
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

	let width = pixmap.width() as usize;
	let scale = (strength * 255.0) as u32;
	let shifted = shift.wrapping_mul(0xCB1A_B31F);

	// Performance: row by row, so no pixel pays for a division to find its column; integer
	// maths only, since this runs over millions of pixels per video frame.
	for (y, row) in pixmap.data_mut().chunks_exact_mut(width * 4).enumerate() {
		let row_hash = (y as u32).wrapping_mul(0xD816_3841) ^ shifted;

		for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
			let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ row_hash;
			h ^= h >> 13;
			h = h.wrapping_mul(0x5bd1_e995);
			h ^= h >> 15;

			let speck = (h & 0xFF) * scale / 255;
			let keep = 255 - speck.min(255);

			for channel in &mut pixel[..3] {
				*channel = (u32::from(*channel) * keep / 255) as u8;
			}
		}
	}
}

/// Applies a style's finish to a rendered image: colour grade, paper mottling, then grain.
///
/// `grain` is the output's base grain (print, screen and video differ); the style scales it.
pub fn finish(pixmap: &mut Pixmap, style: &Style, grain_amount: f32, shift: u32) {
	let look = style.finish;

	if let Some(tone) = look.tone {
		screentone(pixmap, tone);
		return;
	}
	let grading = (look.saturation - 1.0).abs() > 0.01 || look.lift > 0.0 || look.mottle > 0.0;

	if grading {
		grade(pixmap, look.saturation, look.lift, look.mottle);
	}

	grain(pixmap, grain_amount * look.grain, shift);
}

/// Manga printing: black ink on white paper. Near-white becomes paper, near-black ink, and the
/// tones between become dot screens at 45 degrees, coarser for darker tones, the way sheets of
/// screentone are cut and laid on a page. The screen is fixed to the page, whatever is drawn.
pub fn screentone(pixmap: &mut Pixmap, tone: Tone) {
	const PAPER: f32 = 255.0;
	const INK: f32 = 22.0;
	let width = pixmap.width() as usize;
	// About 420 dots across the page (60 lines an inch on a printed page), whatever the
	// resolution, and never finer than three pixels.
	let cell = (width as f32 / 420.0).max(3.0);
	let soft = 0.7 / cell;

	for (index, pixel) in pixmap
		.data_mut()
		.as_chunks_mut::<4>()
		.0
		.iter_mut()
		.enumerate()
	{
		let (x, y) = ((index % width) as f32, (index / width) as f32);
		let [r, g, b] = [
			f32::from(pixel[0]),
			f32::from(pixel[1]),
			f32::from(pixel[2]),
		];
		let luma = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
		let coverage = match luma {
			l if l >= 0.82 => 0.0,
			l if l >= 0.64 => 0.14,
			l if l >= 0.44 => 0.3,
			l if l >= 0.2 => 0.52,
			_ => 1.0,
		};
		let ink = if coverage <= 0.0 || coverage >= 1.0 {
			coverage
		} else if tone == Tone::Lines {
			// Line tone: parallel lines at 45 degrees, thicker for darker tones.
			let u = (x + y) * std::f32::consts::FRAC_1_SQRT_2 / cell;
			let across = (u - u.floor() - 0.5).abs();
			((coverage / 2.0 + soft - across) / (2.0 * soft)).clamp(0.0, 1.0)
		} else {
			let u = (x + y) * std::f32::consts::FRAC_1_SQRT_2 / cell;
			let v = (x - y) * std::f32::consts::FRAC_1_SQRT_2 / cell;
			let (du, dv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
			let distance = du.hypot(dv);
			let radius = (coverage / std::f32::consts::PI).sqrt();
			((radius + soft - distance) / (2.0 * soft)).clamp(0.0, 1.0)
		};
		let value = (PAPER + (INK - PAPER) * ink).round() as u8;
		pixel[..3].fill(value);
	}
}

fn grade(pixmap: &mut Pixmap, saturation: f32, lift: f32, mottle: f32) {
	let width = pixmap.width() as usize;
	// Paper blooms are about a twentieth of the image wide, whatever the resolution.
	let cell = (width / 20).max(8) as f32;
	// Performance: the noise lattice is hashed once per cell rather than four times a pixel,
	// and each column's place in its cell is worked out once. The sums are the bilinear value
	// noise's own, in the same order, so every pixel comes out exactly as it did per pixel.
	let columns: Vec<(f32, f32)> = (0..width)
		.map(|x| {
			let x = x as f32 / cell;
			let x0 = x.floor();
			(x0, smoothstep(x - x0))
		})
		.collect();

	for (y, row) in pixmap.data_mut().chunks_exact_mut(width * 4).enumerate() {
		let y = y as f32 / cell;
		let y0 = y.floor();
		let sy = smoothstep(y - y0);
		let mut lattice = (f32::NAN, [0.0; 4]);

		for (pixel, &(x0, sx)) in row.as_chunks_mut::<4>().0.iter_mut().zip(&columns) {
			if lattice.0.to_bits() != x0.to_bits() {
				lattice = (
					x0,
					[
						lattice_value(x0, y0),
						lattice_value(x0 + 1.0, y0),
						lattice_value(x0, y0 + 1.0),
						lattice_value(x0 + 1.0, y0 + 1.0),
					],
				);
			}

			let [c00, c10, c01, c11] = lattice.1;
			let top = c00 + (c10 - c00) * sx;
			let bottom = c01 + (c11 - c01) * sx;
			let noise = top + (bottom - top) * sy;
			let bloom = 1.0 + (noise - 0.5) * mottle * 0.14;
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
}

/// The value noise's hashed lattice point at whole-numbered `(x, y)`, in 0..1.
fn lattice_value(x: f32, y: f32) -> f32 {
	let lattice =
		(x as i64 as u64).wrapping_mul(73_856_093) ^ (y as i64 as u64).wrapping_mul(19_349_663);

	f32::midpoint(crate::art::noise(0x5EED, lattice), 1.0)
}

/// Hermite easing from 0 to 1.
fn smoothstep(t: f32) -> f32 {
	t * t * (3.0 - 2.0 * t)
}

/// Encodes a pixmap as JPEG (RGB, alpha dropped onto white).
pub fn jpeg(pixmap: &Pixmap, quality: u8) -> Result<Vec<u8>> {
	encode_jpeg(pixmap, quality, false)
}

/// Encodes a pixmap as JPEG with Huffman tables fitted to the image: about 3% smaller, with
/// every decoded pixel the same, for a second pass over the image. Worth it for the pictures
/// sent to every reader, such as an ePub's pages, rather than files made once.
pub fn jpeg_compact(pixmap: &Pixmap, quality: u8) -> Result<Vec<u8>> {
	encode_jpeg(pixmap, quality, true)
}

fn encode_jpeg(pixmap: &Pixmap, quality: u8, fitted: bool) -> Result<Vec<u8>> {
	let mut out = Vec::new();
	let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality);
	encoder.set_optimized_huffman_tables(fitted);
	// Performance: the encoder reads RGBA as it is and ignores the alpha, so the pixels are
	// never copied into an RGB buffer first.
	encoder
		.encode(
			pixmap.data(),
			u16::try_from(pixmap.width()).context("image too wide for JPEG")?,
			u16::try_from(pixmap.height()).context("image too tall for JPEG")?,
			jpeg_encoder::ColorType::Rgba,
		)
		.context("JPEG encoding failed")?;

	Ok(out)
}

/// Encodes a pixmap as PNG.
///
/// An image with no transparency, which is nearly every page and frame, is written as RGB
/// rather than RGBA, and every image is deflated properly rather than at the fastest setting:
/// together that makes a grainy page over a third smaller, with every pixel exactly as it was.
pub fn png(pixmap: &Pixmap) -> Result<Vec<u8>> {
	encode_png(pixmap, png::DeflateCompression::Level(PNG_LEVEL))
}

/// Encodes a pixmap as PNG at the fastest setting, for a scratch file that is read back once
/// and thrown away, where the bytes saved would cost more time than they save.
pub fn png_scratch(pixmap: &Pixmap) -> Result<Vec<u8>> {
	encode_png(pixmap, png::DeflateCompression::FdeflateUltraFast)
}

fn encode_png(pixmap: &Pixmap, compression: png::DeflateCompression) -> Result<Vec<u8>> {
	let pixels = pixmap.pixels();
	let opaque = pixels.iter().all(|pixel| pixel.alpha() == u8::MAX);
	let (colour, data) = if opaque {
		let mut rgb = Vec::with_capacity(pixels.len() * 3);

		for pixel in pixmap.data().as_chunks::<4>().0 {
			rgb.extend_from_slice(&pixel[..3]);
		}

		(png::ColorType::Rgb, rgb)
	} else {
		let mut rgba = Vec::with_capacity(pixels.len() * 4);

		for pixel in pixels {
			let colour = pixel.demultiply();
			rgba.extend_from_slice(&[colour.red(), colour.green(), colour.blue(), colour.alpha()]);
		}

		(png::ColorType::Rgba, rgba)
	};

	let mut out = Vec::new();
	let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
	encoder.set_color(colour);
	encoder.set_depth(png::BitDepth::Eight);
	encoder.set_deflate_compression(compression);
	encoder.set_filter(png::Filter::Adaptive);
	encoder
		.write_header()
		.and_then(|mut writer| writer.write_image_data(&data))
		.context("PNG encoding failed")?;

	Ok(out)
}

/// The DEFLATE level PNGs are written at. On grainy pages, level 2 gets within 4% of level 6's
/// size in half its time; level 1 barely beats storing the grain.
const PNG_LEVEL: u8 = 2;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn screentone_prints_only_paper_ink_and_dots() {
		let mut pixmap = Pixmap::new(420, 60).unwrap();
		// White, a light tone, a mid tone and black, side by side.
		for (index, value) in [250_u8, 180, 120, 10].into_iter().enumerate() {
			let paint = resvg::tiny_skia::Paint {
				shader: resvg::tiny_skia::Shader::SolidColor(resvg::tiny_skia::Color::from_rgba8(
					value, value, value, 255,
				)),
				..Default::default()
			};
			let rect =
				resvg::tiny_skia::Rect::from_xywh(index as f32 * 105.0, 0.0, 105.0, 60.0).unwrap();
			pixmap.fill_rect(rect, &paint, resvg::tiny_skia::Transform::identity(), None);
		}

		screentone(&mut pixmap, Tone::Dots);
		let at = |x: u32, y: u32| pixmap.pixel(x, y).unwrap();
		let ink = |x0: u32| {
			(x0..x0 + 100)
				.flat_map(|x| (0..60).map(move |y| (x, y)))
				.filter(|&(x, y)| at(x, y).red() < 128)
				.count() as f32
				/ 6000.0
		};

		assert!(ink(0) < 0.01, "white stays paper");
		assert!(ink(315) > 0.99, "black stays ink");
		assert!((0.05..0.25).contains(&ink(105)), "light tone: {}", ink(105));
		assert!(ink(210) > ink(105), "darker tones print more dots");
		assert!((0..420).all(|x| at(x, 30).red() == at(x, 30).green()));
	}

	/// The finish as it was first written, a pixel at a time, to hold the faster loops to.
	fn reference_finish(
		pixmap: &mut Pixmap,
		saturation: f32,
		lift: f32,
		mottle: f32,
		strength: f32,
		shift: u32,
	) {
		let width = pixmap.width() as usize;
		let cell = (width / 20).max(8) as f32;
		let value_noise = |x: f32, y: f32| {
			let (x0, y0) = (x.floor(), y.floor());
			let (sx, sy) = (smoothstep(x - x0), smoothstep(y - y0));
			let top =
				lattice_value(x0, y0) + (lattice_value(x0 + 1.0, y0) - lattice_value(x0, y0)) * sx;
			let bottom = lattice_value(x0, y0 + 1.0)
				+ (lattice_value(x0 + 1.0, y0 + 1.0) - lattice_value(x0, y0 + 1.0)) * sx;

			top + (bottom - top) * sy
		};
		let scale = (strength * 255.0) as u32;

		for (index, pixel) in pixmap
			.data_mut()
			.as_chunks_mut::<4>()
			.0
			.iter_mut()
			.enumerate()
		{
			let (x, y) = (index % width, index / width);
			let bloom = 1.0 + (value_noise(x as f32 / cell, y as f32 / cell) - 0.5) * mottle * 0.14;
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

			let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
				^ (y as u32).wrapping_mul(0xD816_3841)
				^ shift.wrapping_mul(0xCB1A_B31F);
			h ^= h >> 13;
			h = h.wrapping_mul(0x5bd1_e995);
			h ^= h >> 15;
			let keep = 255 - ((h & 0xFF) * scale / 255).min(255);

			for channel in &mut pixel[..3] {
				*channel = (u32::from(*channel) * keep / 255) as u8;
			}
		}
	}

	/// A picture with every kind of pixel the finish meets: gradients in each channel.
	fn test_card(width: u32, height: u32) -> Pixmap {
		let mut pixmap = Pixmap::new(width, height).unwrap();

		for (index, pixel) in pixmap
			.data_mut()
			.as_chunks_mut::<4>()
			.0
			.iter_mut()
			.enumerate()
		{
			let (x, y) = (index as u32 % width, index as u32 / width);
			*pixel = [
				(x * 7 % 256) as u8,
				(y * 5 % 256) as u8,
				((x + y) * 3 % 256) as u8,
				255,
			];
		}

		pixmap
	}

	#[test]
	fn the_row_by_row_finish_matches_the_pixel_by_pixel_one() {
		for (width, height) in [(203, 117), (960, 54)] {
			for (saturation, lift, mottle, strength, shift) in [
				(0.8, 0.05, 1.0, 0.07, 0),
				(1.15, 0.0, 0.6, 0.03, 9),
				(1.0, 0.0, 0.0, 0.2, 3),
			] {
				let mut fast = test_card(width, height);
				let mut slow = fast.clone();
				grade(&mut fast, saturation, lift, mottle);
				grain(&mut fast, strength, shift);
				reference_finish(&mut slow, saturation, lift, mottle, strength, shift);
				assert!(
					fast.data() == slow.data(),
					"{width}x{height} {saturation} {lift} {mottle}"
				);
			}
		}
	}

	#[test]
	fn png_keeps_every_pixel_and_drops_alpha_only_when_opaque() {
		let decode = |bytes: &[u8]| {
			let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
				.read_info()
				.unwrap();
			let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
			let info = reader.next_frame(&mut buffer).unwrap();
			buffer.truncate(info.buffer_size());
			(info.color_type, buffer)
		};
		let opaque = test_card(37, 21);
		let (colour, rgb) = decode(&png(&opaque).unwrap());
		assert_eq!(colour, png::ColorType::Rgb);
		let expected: Vec<u8> = opaque
			.data()
			.as_chunks::<4>()
			.0
			.iter()
			.flat_map(|p| [p[0], p[1], p[2]])
			.collect();
		assert_eq!(rgb, expected);

		let mut clear = Pixmap::new(4, 1).unwrap();
		clear.pixels_mut()[1] =
			resvg::tiny_skia::PremultipliedColorU8::from_rgba(100, 50, 0, 200).unwrap();
		let (colour, rgba) = decode(&png(&clear).unwrap());
		assert_eq!(colour, png::ColorType::Rgba);
		let expected: Vec<u8> = clear
			.pixels()
			.iter()
			.flat_map(|p| {
				let c = p.demultiply();
				[c.red(), c.green(), c.blue(), c.alpha()]
			})
			.collect();
		assert_eq!(rgba, expected);
	}

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
