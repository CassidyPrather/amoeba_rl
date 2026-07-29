//! Placeholder frontend.
//!
//! The simulation in [`amoeba_rl::sim`] is complete enough to play headless;
//! the terminal-style renderer that turns a
//! [`RenderView`](amoeba_rl::sim::RenderView) into glyphs and key presses into
//! [`Command`](amoeba_rl::sim::Command)s is still to come. Until then this
//! window exists only so `cargo run` does something honest.

use macroquad::color::Color;
use macroquad::text::draw_text;
use macroquad::window::{Conf, clear_background, next_frame, screen_height, screen_width};

use amoeba_rl::VERSION;

const BACKGROUND: Color = Color::new(0.05, 0.05, 0.07, 1.0);
const OVERLAY: Color = Color::new(0.85, 0.85, 0.90, 0.75);
const TEXT_SIZE: f32 = 24.0;

fn window_conf() -> Conf {
    Conf {
        window_title: env!("CARGO_PKG_NAME").to_owned(),
        window_width: 1032,
        window_height: 708,
        high_dpi: true,
        ..Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let line = format!("amoeba-rl {VERSION} — frontend pending");
    loop {
        clear_background(BACKGROUND);
        draw_text(
            &line,
            (line.len() as f32 * TEXT_SIZE)
                .mul_add(-0.5, screen_width())
                .max(0.0)
                * 0.5,
            screen_height() * 0.5,
            TEXT_SIZE,
            OVERLAY,
        );
        next_frame().await;
    }
}
