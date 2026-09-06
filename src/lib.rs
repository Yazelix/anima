//! Terminal screen primitives shared by Yazelix front-door animation surfaces.

mod aquarium;
mod boids;
mod chladni;
mod friends_and_enemies;
mod game_of_life;
mod kitty_frames;
mod mandelbrot;
mod matrix;
mod physarum;
mod plasma;
mod primordial;
mod random;
mod scalar_field;
mod screen_runner;
mod terminal_control;

use crossterm::terminal;
use std::io::{self, Write};

pub use aquarium::{AQUARIUM_STYLE, AquariumAnimation, aquarium_frame_delay};
pub use boids::{BoidsAnimation, BoidsVariant, is_boids_style};
pub use chladni::{CHLADNI_STYLE, ChladniAnimation, chladni_frame_delay};
pub use friends_and_enemies::{FRIENDS_AND_ENEMIES_STYLE, FriendsAndEnemiesAnimation};
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
pub use matrix::{MATRIX_STYLE, MatrixAnimation, matrix_frame_delay};
pub use physarum::{PHYSARUM_STYLE, PhysarumAnimation, physarum_frame_delay};
pub use plasma::{PLASMA_STYLE, PlasmaAnimation, plasma_frame_delay};
pub use primordial::{PRIMORDIAL_STYLE, PrimordialAnimation, primordial_frame_delay};
pub use random::{
    BOIDS_RANDOM_STYLES, GAME_OF_LIFE_RANDOM_STYLES, MANDELBROT_STYLE, random_animation_slot_count,
    random_animation_styles, resolve_random_animation_style,
};
pub use scalar_field::ScalarField;
pub use screen_runner::{
    ASCIQUARIUM_STYLE, LOGO_STYLE, SCREEN_RANDOM_STYLES, SCREEN_STYLES, STATIC_STYLE,
    run_screen_cli,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenCell {
    pub glyph: char,
    pub color_x: usize,
    pub color_y: usize,
    foreground: Option<RgbColor>,
    background: Option<RgbColor>,
}

impl ScreenCell {
    pub const fn indexed(glyph: char, color_x: usize, color_y: usize) -> Self {
        Self {
            glyph,
            color_x,
            color_y,
            foreground: None,
            background: None,
        }
    }

    pub const fn truecolor(
        glyph: char,
        foreground: RgbColor,
        background: Option<RgbColor>,
    ) -> Self {
        Self {
            glyph,
            color_x: 0,
            color_y: 0,
            foreground: Some(foreground),
            background,
        }
    }
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
                let mut line = Vec::new();
                for x in 0..self.width {
                    match self.cells[y * self.width + x] {
                        Some(cell) => match cell.foreground {
                            Some(foreground) => terminal_control::write_truecolor(
                                &mut line,
                                cell.glyph,
                                foreground,
                                cell.background,
                            )
                            .expect("crossterm command writes to memory"),
                            None => line.extend(render_cell(cell).bytes()),
                        },
                        None => line.push(b' '),
                    }
                }
                String::from_utf8(line).expect("screen cells render UTF-8")
            })
            .collect();
        center_frame_lines(lines, resolved_width)
    }
}

/// A reusable RGB pixel field packed into terminal half-block cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfBlockField {
    width: usize,
    pixel_height: usize,
    cells: Vec<[Option<RgbColor>; 2]>,
}

impl HalfBlockField {
    pub fn new(width: usize, pixel_height: usize) -> Self {
        Self {
            width,
            pixel_height,
            cells: vec![[None; 2]; width.saturating_mul(pixel_height.div_ceil(2))],
        }
    }

    /// Sets a pixel, or removes it when `color` is `None`. Out-of-bounds writes are ignored.
    pub fn set(&mut self, x: usize, y: usize, color: Option<RgbColor>) {
        if x >= self.width || y >= self.pixel_height {
            return;
        }
        self.cells[(y / 2) * self.width + x][y % 2] = color;
    }

    pub fn clear(&mut self) {
        self.cells.fill([None; 2]);
    }

    /// Resizes the field and clears all samples, retaining existing allocation when possible.
    pub fn resize(&mut self, width: usize, pixel_height: usize) {
        self.width = width;
        self.pixel_height = pixel_height;
        self.cells
            .resize(width.saturating_mul(pixel_height.div_ceil(2)), [None; 2]);
        self.clear();
    }

    pub fn render_lines(&self, resolved_width: usize) -> Vec<String> {
        if self.width == 0 || self.pixel_height == 0 {
            return Vec::new();
        }

        let lines = self
            .cells
            .chunks_exact(self.width)
            .map(|row| {
                let mut line = Vec::new();
                for cell in row {
                    match *cell {
                        [Some(upper), lower] => {
                            terminal_control::write_truecolor(&mut line, '▀', upper, lower)
                                .expect("crossterm command writes to memory")
                        }
                        [None, Some(lower)] => {
                            terminal_control::write_truecolor(&mut line, '▄', lower, None)
                                .expect("crossterm command writes to memory")
                        }
                        [None, None] => line.push(b' '),
                    }
                }
                String::from_utf8(line).expect("crossterm commands emit UTF-8")
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
        .map(|line| {
            if visible_line_width(&line) >= width {
                line
            } else {
                center_text(&line, width)
            }
        })
        .collect()
}

pub fn screen_frame_output(frame: &[String]) -> String {
    terminal_control::screen_frame_output(frame)
}

pub fn flush_stdout() -> io::Result<()> {
    io::stdout().flush()
}

pub fn render_screen_frame(frame: &[String]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    terminal_control::render_screen_frame(&mut stdout, frame, "")
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

    // Regression: one large multi-row frame must stay inside one synchronized update without newline-driven wrapping.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn render_screen_frame_is_one_synchronized_update() {
        const BEGIN: &str = "\x1b[?2026h";
        const END: &str = "\x1b[?2026l";
        const RESET: &str = "\x1b[0m";
        let frame = vec!["a".repeat(4_096), "b".repeat(4_096), "c".to_string()];
        let mut output = Vec::new();

        terminal_control::render_screen_frame(&mut output, &frame, "").unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.len() > 8 * 1_024);
        assert!(output.starts_with(&format!("{BEGIN}{RESET}")));
        assert!(output.ends_with(&format!("{RESET}{END}")));
        assert_eq!(output.matches(BEGIN).count(), 1);
        assert_eq!(output.matches(END).count(), 1);
        assert_eq!(output.matches(RESET).count(), frame.len() + 1);
        assert!(terminal_control::clear_screen_sequence().starts_with(RESET));
        assert!(!output.contains('\n'));
        // A terminal may present an incomplete write despite synchronized output.
        // Redrawing a row must not first erase the rest of the previous frame.
        assert!(!output.contains("\x1b[2J"));
        assert!(output.contains("\x1b[1;1H\x1b[2K"));
        assert!(output.contains("\x1b[3;1H\x1b[Jc"));
        assert!(screen_frame_output(&[]).contains("\x1b[2J"));
        assert!(output.contains(&frame[0]));
        assert!(output.contains(&frame[1]));

        let overlay = "\x1b[2;3Hname and credit\x1b[0m";
        let mut composed = Vec::new();
        terminal_control::render_screen_frame(&mut composed, &frame, overlay).unwrap();
        let composed = String::from_utf8(composed).unwrap();
        assert_eq!(composed.matches(BEGIN).count(), 1);
        assert_eq!(composed.matches(END).count(), 1);
        assert!(composed.ends_with(&format!("{overlay}{END}")));
        assert!(composed.find(&frame[1]).unwrap() < composed.find(overlay).unwrap());
    }

    #[test]
    fn half_block_field_handles_colors_absence_bounds_and_resizes() {
        let red = RgbColor::new(255, 0, 0);
        let blue = RgbColor::new(0, 0, 255);
        let green = RgbColor::new(0, 255, 0);
        let yellow = RgbColor::new(255, 255, 0);
        let purple = RgbColor::new(127, 0, 255);
        let mut field = HalfBlockField::new(2, 3);

        field.set(0, 0, Some(red));
        field.set(0, 1, Some(blue));
        field.set(1, 0, Some(green));
        field.set(0, 2, Some(yellow));
        field.set(2, 0, Some(purple));
        field.set(0, 3, Some(purple));

        let lines = field.render_lines(2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| visible_line_width(line) == 2));
        assert_eq!(
            lines[0],
            "\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀\x1b[49m\x1b[39m\x1b[38;2;0;255;0m▀\x1b[49m\x1b[39m"
        );
        assert_eq!(lines[1], "\x1b[38;2;255;255;0m▀\x1b[49m\x1b[39m ");

        field.clear();
        field.set(1, 1, Some(purple));
        let lines = field.render_lines(2);
        assert_eq!(lines[0], " \x1b[38;2;127;0;255m▄\x1b[49m\x1b[39m");
        assert_eq!(lines[1], "  ");

        field.resize(1, 1);
        assert_eq!(field.render_lines(1), vec![" "]);
        field.set(0, 0, Some(red));
        field.set(0, 0, None);
        assert_eq!(field.render_lines(1), vec![" "]);

        assert!(HalfBlockField::new(0, 3).render_lines(80).is_empty());
        assert!(HalfBlockField::new(3, 0).render_lines(80).is_empty());
    }

    #[test]
    fn screen_cells_route_indexed_and_truecolor_rendering() {
        let mut frame = ScreenFrame::new(2, 1);
        frame.set(0, 0, ScreenCell::indexed('x', 3, 4));
        frame.set(
            1,
            0,
            ScreenCell::truecolor('▀', RgbColor::new(1, 2, 3), Some(RgbColor::new(4, 5, 6))),
        );

        assert_eq!(
            frame.render_lines(2, |cell| {
                assert_eq!(cell.glyph, 'x');
                assert_eq!((cell.color_x, cell.color_y), (3, 4));
                terminal_control::styled(cell.glyph, crossterm::style::Color::Red)
            }),
            vec![format!(
                "{}\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[49m\x1b[39m",
                terminal_control::styled('x', crossterm::style::Color::Red)
            )]
        );
    }
}
