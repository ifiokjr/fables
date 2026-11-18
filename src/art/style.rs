//! Art styles: different skins over the same puppets and settings.
//!
//! Every drawing call (`inked`, `shaded`, `inked_noodle`, `outline`, detail strokes) asks the
//! active [`Style`] how to put ink on paper. The characters, staging and animation are
//! identical in every style; only the rendering changes, so a series can switch looks with one
//! line in a script and every book stays on-model.

use super::Color;
use super::Pt;
use super::dials::Palette;
use super::dials::Tone;
use super::palette;
use super::pt;

/// The available looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleName {
	/// Two-colour riso print: flat inks, wobbly line, fills slightly off register, dot screens.
	Riso,
	/// Modern preschool TV: no outlines, clean flat shapes, soft gradient shading.
	Tv,
	/// Classic TV cartoon: heavy black outlines, hard cel shadows, glossy highlights.
	Bold,
	/// Cut-paper collage: textured paper shapes with torn edges, each casting a shadow.
	CutPaper,
	/// Crayon: scribbled colouring that strays over the lines, waxy texture.
	Crayon,
	/// Storybook: pencil lines over soft watercolour washes on textured paper.
	Storybook,
	/// Clear line: one even outline everywhere, flat colour, no shading or texture.
	Clear,
	/// Shadow theatre: the setting is ink shadows on a lit screen, paler the farther away.
	Shadow,
	/// Paper theatre: every shape is card with a white cut edge and a hard shadow, and each
	/// layer of the setting stands in front of the last as a sheet of its own.
	Card,
}

impl StyleName {
	pub const ALL: [(&'static str, StyleName); 9] = [
		("riso", StyleName::Riso),
		("tv", StyleName::Tv),
		("bold", StyleName::Bold),
		("cut-paper", StyleName::CutPaper),
		("crayon", StyleName::Crayon),
		("storybook", StyleName::Storybook),
		("clear", StyleName::Clear),
		("shadow", StyleName::Shadow),
		("card", StyleName::Card),
	];

	pub fn parse(name: &str) -> Option<StyleName> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, style)| *style)
	}

	pub fn name(self) -> &'static str {
		Self::ALL
			.iter()
			.find(|(_, style)| *style == self)
			.map_or("riso", |(name, _)| *name)
	}
}

/// How a shape's single tone of shadow is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shading {
	/// A flat crescent in one extra ink.
	Crescent,
	/// A soft radial gradient.
	Gradient,
	/// A hard crescent plus a glossy highlight.
	Cel,
	/// A translucent watercolour glaze.
	Wash,
	/// Scribbled hatching.
	Hatch,
	/// None: flat colour.
	Flat,
}

/// A surface texture laid over fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
	None,
	/// Paper fibres and speckles.
	Paper,
	/// Crayon scribbles.
	Hatch,
	/// Watercolour blooms.
	Mottle,
}

/// Raster finishing applied after rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
	/// Multiplier on each output's base grain.
	pub grain: f32,
	/// Strength of low-frequency paper mottling (0..1).
	pub mottle: f32,
	/// Colour saturation multiplier.
	pub saturation: f32,
	/// Lift towards white (0..1), for airy watercolour.
	pub lift: f32,
	/// Black ink on white paper, with every colour turned into this screentone (manga).
	pub tone: Option<Tone>,
	/// Every colour snapped to a limited palette.
	pub palette: Option<Palette>,
}

/// What a page's words sit on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Card {
	/// A cream paper card with a wobbly edge, inked the way the page skin inks.
	#[default]
	Paper,
	/// A plain dark band with light letters, as a caption sits under a film.
	Caption,
}

impl Card {
	pub const ALL: [(&'static str, Card); 2] = [("paper", Card::Paper), ("caption", Card::Caption)];

	pub fn parse(name: &str) -> Option<Card> {
		Self::ALL
			.iter()
			.find(|(key, _)| *key == name)
			.map(|(_, card)| *card)
	}

	pub fn name(self) -> &'static str {
		match self {
			Card::Paper => "paper",
			Card::Caption => "caption",
		}
	}
}

/// The shadow theatre's screen in flat daylight, top and bottom: lamplight through paper.
/// Under a scene light the screen is the light's own sky.
pub const SCREEN: (Color, Color) = (Color(0xF2_C2_7C), Color(0xFB_E8_BE));

/// Everything that decides how ink meets paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
	pub name: StyleName,
	/// Silhouette outline weight multiplier; 0 means no outlines at all.
	pub outline: f32,
	pub ink: Color,
	/// Detail line (faces, folds) weight multiplier.
	pub detail: f32,
	/// Fill offset from the outline.
	pub registration: Pt,
	pub fill_opacity: f32,
	/// The colour of silhouette outlines, if not the ink: the white core of cut card.
	pub edge: Option<Color>,
	/// Shadow cast by every shape: offset and opacity.
	pub shadow: Option<(Pt, f32)>,
	/// The shadow each layer of a setting casts on the one behind it, as sheets of card
	/// standing apart: offset and opacity.
	pub planes: Option<(Pt, f32)>,
	/// Settings are painted as ink shadows on a lit screen (see [`Solid`](super::Solid)).
	pub silhouette: bool,
	/// What the words sit on.
	pub card: Card,
	pub shading: Shading,
	pub texture: Texture,
	/// Sketchy lines: extra passes and how far they wander.
	pub sketch: Option<(u8, f32)>,
	/// Darkening just inside a shape's edge, as watercolour pigment pools there.
	pub pooling: f32,
	/// Riso dot screens in backgrounds.
	pub halftone: bool,
	/// Edge roughening (stage units) for torn paper, crayon and paint.
	pub roughen: f32,
	/// How busy the roughening is: low for broad torn edges, higher for waxy crayon.
	pub roughen_frequency: f32,
	pub finish: Finish,
}

impl Default for Style {
	fn default() -> Self {
		Self::named(StyleName::Riso)
	}
}

impl Style {
	pub fn named(name: StyleName) -> Style {
		let plain = Finish {
			grain: 1.0,
			mottle: 0.0,
			saturation: 1.0,
			lift: 0.0,
			tone: None,
			palette: None,
		};

		match name {
			StyleName::Riso => {
				Style {
					name,
					outline: 1.0,
					ink: palette::INK,
					detail: 1.0,
					registration: pt(5.0, 4.0),
					fill_opacity: 1.0,
					shadow: None,
					shading: Shading::Crescent,
					texture: Texture::None,
					sketch: None,
					pooling: 0.0,
					halftone: true,
					roughen: 0.0,
					roughen_frequency: 0.0,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: plain,
				}
			}
			StyleName::Tv => {
				Style {
					name,
					outline: 0.0,
					ink: palette::INK,
					detail: 0.9,
					registration: pt(0.0, 0.0),
					fill_opacity: 1.0,
					shadow: None,
					shading: Shading::Gradient,
					texture: Texture::None,
					sketch: None,
					pooling: 0.0,
					halftone: false,
					roughen: 0.0,
					roughen_frequency: 0.0,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 0.0,
						saturation: 1.08,
						..plain
					},
				}
			}
			StyleName::Bold => {
				Style {
					name,
					outline: 2.1,
					ink: Color(0x16_12_1C),
					detail: 1.3,
					registration: pt(0.0, 0.0),
					fill_opacity: 1.0,
					shadow: None,
					shading: Shading::Cel,
					texture: Texture::None,
					sketch: None,
					pooling: 0.0,
					halftone: false,
					roughen: 0.0,
					roughen_frequency: 0.0,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 0.0,
						saturation: 1.18,
						..plain
					},
				}
			}
			StyleName::CutPaper => {
				Style {
					name,
					outline: 0.0,
					ink: palette::INK,
					detail: 0.9,
					registration: pt(0.0, 0.0),
					fill_opacity: 1.0,
					shadow: Some((pt(7.0, 11.0), 0.3)),
					shading: Shading::Crescent,
					texture: Texture::Paper,
					sketch: None,
					pooling: 0.0,
					halftone: false,
					roughen: 3.2,
					roughen_frequency: 0.011,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 0.9,
						mottle: 0.35,
						saturation: 0.95,
						lift: 0.0,
						tone: None,
						palette: None,
					},
				}
			}
			StyleName::Crayon => {
				Style {
					name,
					outline: 1.1,
					ink: Color(0x34_2C_3E),
					detail: 1.0,
					registration: pt(7.0, 6.0),
					fill_opacity: 0.88,
					shadow: None,
					shading: Shading::Hatch,
					texture: Texture::Hatch,
					sketch: Some((2, 3.5)),
					pooling: 0.0,
					halftone: false,
					roughen: 1.7,
					roughen_frequency: 0.045,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 2.2,
						mottle: 0.25,
						saturation: 1.12,
						lift: 0.0,
						tone: None,
						palette: None,
					},
				}
			}
			StyleName::Storybook => {
				Style {
					name,
					outline: 0.62,
					ink: Color(0x4A_42_55),
					detail: 0.8,
					registration: pt(3.0, 2.0),
					fill_opacity: 0.84,
					shadow: None,
					shading: Shading::Wash,
					texture: Texture::Mottle,
					sketch: Some((2, 2.2)),
					pooling: 0.4,
					halftone: false,
					roughen: 1.5,
					roughen_frequency: 0.018,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 1.1,
						mottle: 0.7,
						saturation: 0.8,
						lift: 0.05,
						tone: None,
						palette: None,
					},
				}
			}
			StyleName::Clear => {
				Style {
					name,
					outline: 0.8,
					ink: palette::INK,
					// Clear line gives every line the same weight, inside and out.
					detail: 1.0,
					registration: pt(0.0, 0.0),
					fill_opacity: 1.0,
					shadow: None,
					shading: Shading::Flat,
					texture: Texture::None,
					sketch: None,
					pooling: 0.0,
					halftone: false,
					roughen: 0.0,
					roughen_frequency: 0.0,
					edge: None,
					planes: None,
					silhouette: false,
					card: Card::Paper,
					finish: Finish {
						grain: 0.0,
						saturation: 1.06,
						..plain
					},
				}
			}
			StyleName::Shadow => {
				Style {
					name,
					outline: 0.0,
					silhouette: true,
					shading: Shading::Flat,
					halftone: false,
					registration: pt(0.0, 0.0),
					finish: Finish {
						grain: 0.6,
						..plain
					},
					..Style::named(StyleName::Riso)
				}
			}
			StyleName::Card => {
				Style {
					name,
					outline: 1.15,
					edge: Some(palette::WHITE),
					registration: pt(0.0, 0.0),
					shadow: Some((pt(8.0, 12.0), 0.26)),
					planes: Some((pt(22.0, 30.0), 0.24)),
					texture: Texture::Paper,
					shading: Shading::Flat,
					halftone: false,
					finish: Finish {
						grain: 0.5,
						..plain
					},
					..Style::named(StyleName::Riso)
				}
			}
		}
	}

	pub fn parse(name: &str) -> Option<Style> {
		StyleName::parse(name).map(Style::named)
	}
}

/// The SVG `<pattern>` for a texture, tiled in stage units.
pub fn texture_pattern(texture: Texture, id: &str) -> String {
	use std::fmt::Write as _;

	use super::noise;

	let ink = palette::INK;
	let mut out = String::new();

	match texture {
		Texture::None => {}
		Texture::Paper => {
			let size = 180.0;
			let _ = write!(
				out,
				r#"<pattern id="{id}" patternUnits="userSpaceOnUse" width="{size}" height="{size}">"#
			);

			for i in 0..30 {
				let x = (noise(501, i) + 1.0) * size / 2.0;
				let y = (noise(502, i) + 1.0) * size / 2.0;
				let dx = noise(503, i) * 22.0;
				let dy = noise(504, i) * 22.0;
				let (colour, opacity) = if i % 4 == 0 {
					(ink, 0.07)
				} else {
					(palette::WHITE, 0.32)
				};
				let _ = write!(
					out,
					r#"<path d="M{x:.1} {y:.1}q{:.1} {:.1} {dx:.1} {dy:.1}" fill="none" stroke="{colour}" stroke-opacity="{opacity}" stroke-width="2.2" stroke-linecap="round"/>"#,
					dx * 0.5 + 4.0,
					dy * 0.5 - 4.0
				);
			}

			for i in 0..14 {
				let x = (noise(505, i) + 1.0) * size / 2.0;
				let y = (noise(506, i) + 1.0) * size / 2.0;
				let _ = write!(
					out,
					r#"<circle cx="{x:.1}" cy="{y:.1}" r="1.8" fill="{ink}" fill-opacity="0.12"/>"#
				);
			}

			out.push_str("</pattern>");
		}
		Texture::Hatch => {
			let size = 26.0;
			let _ = write!(
				out,
				r#"<pattern id="{id}" patternUnits="userSpaceOnUse" width="{size}" height="{size}" patternTransform="rotate(32)"><path d="M-2 6 Q13 4 28 7" fill="none" stroke="{ink}" stroke-opacity="0.16" stroke-width="5" stroke-linecap="round"/><path d="M-2 18 Q13 21 28 17" fill="none" stroke="{white}" stroke-opacity="0.22" stroke-width="3" stroke-linecap="round"/></pattern>"#,
				white = palette::WHITE
			);
		}
		Texture::Mottle => {
			// Blooms fade softly to nothing at their rims, like pigment drying unevenly.
			let size = 420.0;
			let white = palette::WHITE;
			let _ = write!(
				out,
				r#"<radialGradient id="{id}-light"><stop offset="0" stop-color="{white}" stop-opacity="0.2"/><stop offset="1" stop-color="{white}" stop-opacity="0"/></radialGradient><radialGradient id="{id}-dark"><stop offset="0.55" stop-color="{ink}" stop-opacity="0"/><stop offset="0.9" stop-color="{ink}" stop-opacity="0.07"/><stop offset="1" stop-color="{ink}" stop-opacity="0"/></radialGradient><pattern id="{id}" patternUnits="userSpaceOnUse" width="{size}" height="{size}">"#
			);

			for i in 0..16 {
				let x = (noise(511, i) + 1.0) * size / 2.0;
				let y = (noise(512, i) + 1.0) * size / 2.0;
				let r = 40.0 + (noise(513, i) + 1.0) * 45.0;
				let tone = if i % 3 == 0 { "dark" } else { "light" };
				let _ = write!(
					out,
					r#"<circle cx="{x:.1}" cy="{y:.1}" r="{r:.1}" fill="url(#{id}-{tone})"/>"#
				);
			}

			out.push_str("</pattern>");
		}
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_style_round_trips_by_name() {
		for (name, style) in StyleName::ALL {
			assert_eq!(StyleName::parse(name), Some(style));
			assert_eq!(style.name(), name);
			assert_eq!(Style::named(style).name, style);
		}
	}

	#[test]
	fn outline_free_styles_still_draw_faces() {
		for name in [StyleName::Tv, StyleName::CutPaper] {
			let style = Style::named(name);
			assert!(style.outline.abs() < f32::EPSILON);
			assert!(style.detail > 0.5, "{name:?} must keep eyes and mouths");
		}
	}
}
