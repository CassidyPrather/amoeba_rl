//! The code page 437 font atlas, and the one lookup that makes it usable.
//!
//! The sim hands out glyphs as `char`s; the atlas indexes them by their code
//! page 437 byte. For everything printable that is the same number, which is
//! why [`cp437`] is mostly the identity — the interesting part is the handful
//! of drawing characters the game reaches for, chiefly the arrows a charging
//! hunter shows.
//!
//! The bitmap is grayscale on black with no alpha channel, so tinting it would
//! do nothing: multiplying a grey glyph by green gives a dim green glyph on a
//! black square that hides whatever is underneath. [`Tileset::load`] rewrites
//! every pixel to white-with-luminance-as-alpha instead, after which a single
//! texture plus a colour tint draws any glyph in any colour over any
//! background.

use macroquad::color::Color;
use macroquad::math::{Rect, Vec2};
use macroquad::prelude::ImageFormat;
use macroquad::texture::{DrawTextureParams, FilterMode, Image, Texture2D, draw_texture_ex};

/// The atlas: 192×192 px, sixteen by sixteen glyphs, code page order.
const ATLAS: &[u8] = include_bytes!("../assets/terminal12x12_gs_ro.png");

/// Glyphs per atlas row, and per atlas column.
const GRID: u8 = 16;

/// Source pixels along one glyph's edge.
const GLYPH_PX: f32 = 12.0;

/// What an unmapped character draws as.
const UNKNOWN: u8 = b'?';

/// Every character the game draws that is not printable ASCII, with its code
/// page 437 index.
///
/// The first four are the arrows a hunter or scout shows while it is lining a
/// shot up (`view.rs` emits exactly these); the rest are the shading and block
/// characters this frontend uses for panel furniture. Anything else falls back
/// to `?`, which is what the original did with an unregistered entity too.
const EXTRAS: [(char, u8); 11] = [
    ('\u{2022}', 7),   // bullet
    ('\u{25BA}', 16),  // right-pointing arrow
    ('\u{25C4}', 17),  // left-pointing arrow
    ('\u{25B2}', 30),  // up-pointing arrow
    ('\u{25BC}', 31),  // down-pointing arrow
    ('\u{2591}', 176), // light shade
    ('\u{2592}', 177), // medium shade
    ('\u{2593}', 178), // dark shade
    ('\u{2588}', 219), // full block
    ('\u{2584}', 220), // lower half block
    ('\u{2580}', 223), // upper half block
];

/// Where `ch` lives in the code page.
///
/// Printable ASCII is code page 437's own first half, one index for one
/// codepoint; everything else has to be looked up, and anything unlisted
/// becomes `?` rather than an arbitrary box-drawing character.
#[must_use]
pub fn cp437(ch: char) -> u8 {
    if ch.is_ascii_graphic() || ch == ' ' {
        // In range by construction: `is_ascii_graphic` is 0x21..=0x7e.
        #[allow(clippy::cast_possible_truncation)]
        return ch as u8;
    }
    EXTRAS
        .iter()
        .find_map(|&(extra, index)| (extra == ch).then_some(index))
        .unwrap_or(UNKNOWN)
}

/// The loaded font atlas.
pub struct Tileset {
    texture: Texture2D,
}

impl Tileset {
    /// Decode the embedded atlas and turn it into a tintable texture.
    ///
    /// # Panics
    ///
    /// If the embedded PNG does not decode, which would mean the binary was
    /// built from a corrupt asset.
    #[must_use]
    pub fn load() -> Self {
        let mut image = Image::from_file_with_format(ATLAS, Some(ImageFormat::Png))
            .expect("the embedded font atlas decodes");
        for pixel in image.get_image_data_mut() {
            *pixel = [255, 255, 255, luminance(*pixel)];
        }
        let texture = Texture2D::from_image(&image);
        // Anything else turns a 12 px glyph blown up to 18 px into a smear.
        texture.set_filter(FilterMode::Nearest);
        Self { texture }
    }

    /// Draw one glyph in `color`, filling a `size`×`size` square at `(x, y)`.
    pub fn draw(&self, ch: char, x: f32, y: f32, size: f32, color: Color) {
        let index = cp437(ch);
        let source = Rect::new(
            f32::from(index % GRID) * GLYPH_PX,
            f32::from(index / GRID) * GLYPH_PX,
            GLYPH_PX,
            GLYPH_PX,
        );
        draw_texture_ex(
            &self.texture,
            x,
            y,
            color,
            DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                source: Some(source),
                ..DrawTextureParams::default()
            },
        );
    }

    /// Draw a string left to right on one baseline.
    pub fn draw_text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        for (i, ch) in text.chars().enumerate() {
            if ch != ' ' {
                self.draw(ch, size.mul_add(i as f32, x), y, size, color);
            }
        }
    }

    /// Draw a string centred on `cx`.
    pub fn draw_text_centred(&self, text: &str, cx: f32, y: f32, size: f32, color: Color) {
        let width = size * text.chars().count() as f32;
        self.draw_text(text, width.mul_add(-0.5, cx), y, size, color);
    }
}

/// Perceptual weight is beside the point here: the source is grey on black, so
/// the mean of the three channels is the glyph's coverage.
const fn luminance(pixel: [u8; 4]) -> u8 {
    let sum = pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16;
    #[allow(clippy::cast_possible_truncation)] // A third of 765 fits in a byte.
    {
        (sum / 3) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_ascii_is_its_own_index() {
        for byte in 0x20..=0x7e_u8 {
            assert_eq!(cp437(byte as char), byte);
        }
    }

    #[test]
    fn the_arrows_the_sim_emits_are_all_mapped() {
        // These four are the only non-ASCII glyphs `sim::view` ever produces.
        assert_eq!(cp437('\u{25BA}'), 16);
        assert_eq!(cp437('\u{25C4}'), 17);
        assert_eq!(cp437('\u{25B2}'), 30);
        assert_eq!(cp437('\u{25BC}'), 31);
    }

    #[test]
    fn the_blocks_the_frontend_draws_are_mapped() {
        assert_eq!(cp437('\u{2588}'), 219);
        assert_eq!(cp437('\u{2591}'), 176);
        assert_eq!(cp437('\u{2022}'), 7);
    }

    #[test]
    fn anything_else_becomes_a_question_mark() {
        for ch in ['\u{e9}', '\u{4e2d}', '\u{1f600}', '\n', '\t', '\u{0}'] {
            assert_eq!(cp437(ch), b'?');
        }
    }

    #[test]
    fn every_index_lands_inside_the_atlas() {
        // 16 by 16 glyphs, so any byte is addressable; this guards the row and
        // column arithmetic rather than the table.
        for byte in 0..=u8::MAX {
            let (row, col) = (byte / GRID, byte % GRID);
            assert!(row < GRID && col < GRID);
        }
    }

    #[test]
    fn the_extras_table_has_no_duplicates() {
        for (i, (ch, index)) in EXTRAS.iter().enumerate() {
            for (other, other_index) in &EXTRAS[i + 1..] {
                assert_ne!(ch, other);
                assert_ne!(index, other_index);
            }
        }
    }

    #[test]
    fn luminance_is_the_channel_mean() {
        assert_eq!(luminance([0, 0, 0, 255]), 0);
        assert_eq!(luminance([255, 255, 255, 255]), 255);
        assert_eq!(luminance([120, 120, 120, 0]), 120);
    }
}
