//! Terminal screen primitives shared by Yazelix front-door animation surfaces.

mod boids;
mod game_of_life;
mod kitty_frames;
mod mandelbrot;
mod random;
mod screen_runner;
mod terminal_control;

use crossterm::terminal;
use std::io::{self, Write};

pub use boids::{BoidsAnimation, BoidsVariant, is_boids_style};
pub use game_of_life::{
    GameOfLifeAnimation, GameOfLifeCellStyle, GameOfLifeCellStyleParseError, GameOfLifeScreenState,
    GameOfLifeSpec, ScreenAnimationContext, ScreenFrameProducer, build_game_of_life_screen_lines,
    build_game_of_life_screen_state, build_live_game_of_life_seed, game_of_life_grid_height,
    game_of_life_grid_width, game_of_life_spec, is_game_of_life_style,
    render_game_of_life_screen_state, resolve_game_of_life_body_height,
    resolve_game_of_life_screen_body_height, step_game_of_life_cells,
    step_game_of_life_screen_state,
};
pub use kitty_frames::{
    KittyFrameLayout, KittyFrameSequence, cleanup_kitty_image, draw_kitty_png_frame,
    kitty_delete_image_command, kitty_frame_layout, kitty_png_file_command,
    play_kitty_png_frame_sequence,
};
pub use mandelbrot::{
    MandelbrotAnimation, mandelbrot_escape_iterations, mandelbrot_frame_delay,
    mandelbrot_max_iterations,
};
pub use random::{
    BOIDS_RANDOM_STYLES, GAME_OF_LIFE_RANDOM_STYLES, MANDELBROT_STYLE, random_animation_slot_count,
    random_animation_styles, resolve_random_animation_style,
};
pub use screen_runner::{
    ASCIQUARIUM_STYLE, LOGO_STYLE, SCREEN_RANDOM_STYLES, SCREEN_STYLES, STATIC_STYLE,
    run_screen_cli,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenCell {
    pub glyph: char,
    pub color_x: usize,
    pub color_y: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFrame {
    width: usize,
    height: usize,
    cells: Vec<Option<ScreenCell>>,
}

impl ScreenFrame {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![None; width.saturating_mul(height)],
        }
    }

    pub fn set(&mut self, x: usize, y: usize, cell: ScreenCell) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.cells[y * self.width + x] = Some(cell);
    }

    pub fn render_lines<F>(&self, resolved_width: usize, render_cell: F) -> Vec<String>
    where
        F: Fn(ScreenCell) -> String,
    {
        let lines = (0..self.height)
            .map(|y| {
                let mut line = String::new();
                for x in 0..self.width {
                    match self.cells[y * self.width + x] {
                        Some(cell) => line.push_str(&render_cell(cell)),
                        None => line.push(' '),
                    }
                }
                line
            })
            .collect();
        center_frame_lines(lines, resolved_width)
    }
}

pub fn terminal_width() -> usize {
    std::env::var("YAZELIX_WELCOME_WIDTH")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|width| *width > 0)
        .or_else(|| terminal::size().ok().map(|(width, _)| width as usize))
        .unwrap_or(80)
}

pub fn terminal_height() -> usize {
    std::env::var("YAZELIX_WELCOME_HEIGHT")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|height| *height > 0)
        .or_else(|| terminal::size().ok().map(|(_, height)| height as usize))
        .unwrap_or(24)
}

pub fn visible_line_width(line: &str) -> usize {
    let mut count = 0;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            for inner in chars.by_ref() {
                if inner.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        count += 1;
    }
    count
}

pub fn center_text(text: &str, width: usize) -> String {
    let visible_width = visible_line_width(text);
    if visible_width >= width {
        return text.to_string();
    }

    let left = (width - visible_width) / 2;
    let right = width - visible_width - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

pub fn center_frame_lines(lines: Vec<String>, width: usize) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| center_text(&line, width))
        .collect()
}

pub fn screen_frame_output(frame: &[String]) -> String {
    terminal_control::screen_frame_output(frame)
}

pub fn flush_stdout() -> io::Result<()> {
    io::stdout().flush()
}

pub fn render_screen_frame(frame: &[String]) -> io::Result<()> {
    print!("{}", screen_frame_output(frame));
    flush_stdout()
}

pub fn enter_screen_mode() -> io::Result<()> {
    terminal_control::enter_screen_mode()
}

pub fn leave_screen_mode() -> io::Result<()> {
    terminal_control::leave_screen_mode()
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test lane: default

    // Regression: raw alternate-screen rendering must not rely on newlines after full-width lines, which can wrap into every other row.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn screen_frame_output_addresses_rows_without_newlines() {
        let output = screen_frame_output(&["aaaaaaaa".to_string(), "bbbbbbbb".to_string()]);
        assert!(!output.contains('\n'));
        assert!(output.contains("aaaaaaaa"));
        assert!(output.contains("bbbbbbbb"));
        assert_eq!(visible_line_width(&output), "aaaaaaaabbbbbbbb".len());
    }
}
