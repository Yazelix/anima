use std::time::Duration;

use crate::random::{seeded_index, size_seed};
use crate::{ScreenAnimationContext, ScreenCell, ScreenFrame, ScreenFrameProducer};
use crossterm::style::Color;

pub const MATRIX_STYLE: &str = "matrix";
pub const ACCELERANDO_STYLE: &str = "accelerando";

const MATRIX_GLYPHS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ@#$%&*+=-:;.";
const GLYPH_WIDTH: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixVariant {
    Classic,
    Accelerando,
}

impl MatrixVariant {
    fn dimensions(self, context: ScreenAnimationContext) -> (usize, usize) {
        match self {
            Self::Classic => (context.inner_width, context.resolved_height),
            Self::Accelerando => (
                context.inner_width.div_ceil(GLYPH_WIDTH),
                context.resolved_height,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MatrixColumn {
    head: i32,
    length: usize,
    speed: usize,
    phase: usize,
    glyph_seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixAnimation {
    context: ScreenAnimationContext,
    variant: MatrixVariant,
    frame_index: usize,
    seed: u64,
    columns: Vec<MatrixColumn>,
}

impl MatrixAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        Self::with_variant(context, MatrixVariant::Classic)
    }

    pub fn with_variant(context: ScreenAnimationContext, variant: MatrixVariant) -> Self {
        let (width, height) = variant.dimensions(context);
        let mut seed = size_seed(width, height, 0x4D41_5452_4958);
        let columns = (0..width)
            .map(|_| new_column(&mut seed, height, true, variant))
            .collect();
        Self {
            context,
            variant,
            frame_index: 0,
            seed,
            columns,
        }
    }

    fn render_accelerando(&self) -> Vec<String> {
        let width = self.context.inner_width;
        let (_, height) = self.variant.dimensions(self.context);
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut frame = ScreenFrame::new(width, height);
        for (x, column) in self.columns.iter().enumerate() {
            for age in 0..column.length {
                let top = column.head - age as i32;
                if top < 0 || top >= height as i32 {
                    continue;
                }
                let mutation = self
                    .frame_index
                    .wrapping_add(x.wrapping_mul(17))
                    .wrapping_add(age * 11);
                let hash = matrix_glyph_hash(column.glyph_seed, x, age, mutation);
                let tone = trail_tone(age, column.length);
                let glyph = if (hash >> 32).is_multiple_of(4) {
                    matrix_glyph(column.glyph_seed, x, age, mutation)
                } else {
                    accelerando_glyph(hash as usize)
                };
                frame.set(
                    x * GLYPH_WIDTH,
                    top as usize,
                    ScreenCell::indexed(glyph, 0, tone),
                );
            }
        }
        frame.render_lines(self.context.resolved_width, colorize_matrix_cell)
    }
}

impl ScreenFrameProducer for MatrixAnimation {
    fn render_frame(&self) -> Vec<String> {
        if self.variant == MatrixVariant::Accelerando {
            return self.render_accelerando();
        }
        let width = self.context.inner_width;
        let height = self.context.resolved_height;
        let mut frame = ScreenFrame::new(width, height);

        for (x, column) in self.columns.iter().enumerate() {
            for age in 0..column.length {
                let y = column.head - age as i32;
                if y >= 0 && y < height as i32 {
                    frame.set(
                        x,
                        y as usize,
                        ScreenCell::indexed(
                            matrix_glyph(column.glyph_seed, x, y as usize, self.frame_index),
                            0,
                            trail_tone(age, column.length),
                        ),
                    );
                }
            }
        }

        frame.render_lines(self.context.resolved_width, colorize_matrix_cell)
    }

    fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
        let (_, height) = self.variant.dimensions(self.context);
        for column in &mut self.columns {
            if self
                .frame_index
                .wrapping_add(column.phase)
                .is_multiple_of(column.speed)
            {
                column.head += 1;
            }
            if column.head as i64 - column.length as i64 >= height as i64 {
                *column = new_column(&mut self.seed, height, false, self.variant);
            }
        }
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::with_variant(context, self.variant);
    }
}

pub fn matrix_frame_delay() -> Duration {
    Duration::from_millis(55)
}

fn new_column(seed: &mut u64, height: usize, warm: bool, variant: MatrixVariant) -> MatrixColumn {
    let (minimum_length, maximum_length) = match variant {
        MatrixVariant::Classic => {
            let minimum = height.clamp(1, 4);
            (minimum, (height.saturating_mul(2) / 3).clamp(minimum, 24))
        }
        MatrixVariant::Accelerando => {
            let minimum = (height / 2).clamp(1, 24);
            (minimum, (height.saturating_mul(3) / 2).clamp(minimum, 72))
        }
    };
    let length = minimum_length + seeded_index(seed, maximum_length - minimum_length + 1);
    let speed = 1 + seeded_index(seed, 3);
    let head = if warm {
        seeded_index(seed, height.max(1)) as i32
    } else {
        -(seeded_index(seed, (height / 4).max(1)) as i32) - 1
    };

    MatrixColumn {
        head,
        length,
        speed,
        phase: seeded_index(seed, speed),
        glyph_seed: *seed,
    }
}

fn trail_tone(age: usize, length: usize) -> usize {
    length.saturating_sub(age) * 4 / length
}

fn matrix_glyph(seed: u64, x: usize, y: usize, frame_index: usize) -> char {
    MATRIX_GLYPHS[(matrix_glyph_hash(seed, x, y, frame_index) as usize) % MATRIX_GLYPHS.len()]
        as char
}

fn matrix_glyph_hash(seed: u64, x: usize, y: usize, frame_index: usize) -> u64 {
    let mut hash = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ ((frame_index / 2) as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^= hash >> 27;
    hash
}

fn accelerando_glyph(id: usize) -> char {
    // Supplementary-B avoids Eon's Nerd Font range; the bundled face owns these cells.
    char::from_u32(0x100000 + (id % 8192) as u32).unwrap()
}

fn colorize_matrix_cell(cell: ScreenCell) -> String {
    let color = match cell.color_y {
        4 => Color::White,
        3 => Color::AnsiValue(120),
        2 => Color::AnsiValue(46),
        1 => Color::AnsiValue(28),
        _ => Color::AnsiValue(22),
    };
    crate::terminal_control::styled(cell.glyph, color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visible_line_width;

    fn context(width: usize, height: usize) -> ScreenAnimationContext {
        ScreenAnimationContext {
            resolved_width: width,
            resolved_height: height,
            inner_width: width,
            size_class: "medium",
        }
    }

    // Defends: Matrix rain stays deterministic, dense, varied, fading, one-column-safe, bounded, and resize-safe.
    #[test]
    fn matrix_animation_contract() {
        let mut first = MatrixAnimation::new(context(40, 12));
        let mut second = MatrixAnimation::new(context(40, 12));
        let initial = first.render_frame();

        assert_eq!(first, second);
        assert_eq!(initial.len(), 12);
        assert!(initial.iter().all(|line| visible_line_width(line) == 40));
        assert!(visible_glyphs(&initial) > 80);
        assert_eq!(first.columns.len(), 40);
        assert!(
            first
                .columns
                .iter()
                .any(|column| column.speed != first.columns[0].speed)
        );
        assert!(MATRIX_GLYPHS.iter().all(u8::is_ascii_graphic));
        assert!((1..12).all(|age| trail_tone(age - 1, 12) >= trail_tone(age, 12)));
        assert!((1..32).any(|frame| matrix_glyph(7, 3, 5, frame) != matrix_glyph(7, 3, 5, 0)));

        for _ in 0..120 {
            first.advance_frame();
            second.advance_frame();
        }
        assert_eq!(first, second);
        assert_ne!(first.render_frame(), initial);
        assert!(visible_glyphs(&first.render_frame()) > 40);
        assert_eq!(first.columns.len(), 40);

        for (width, height) in [(1, 1), (17, 5), (52, 15)] {
            first.resize(context(width, height));
            let frame = first.render_frame();
            assert_eq!(first.columns.len(), width);
            assert_eq!(frame.len(), height);
            assert!(frame.iter().all(|line| visible_line_width(line) == width));
        }

        first.columns[0].phase = 1;
        first.frame_index = usize::MAX - 1;
        first.advance_frame();
        assert_eq!(first.frame_index, usize::MAX);
    }

    #[test]
    fn accelerando_renders_native_glyphs_symbols_and_long_bounded_rain() {
        let glyphs: std::collections::HashSet<_> = (0..8192).map(accelerando_glyph).collect();
        assert_eq!(glyphs.len(), 8192);
        assert_eq!(accelerando_glyph(0), '\u{100000}');
        assert_eq!(accelerando_glyph(8191), '\u{101fff}');
        assert!(
            glyphs
                .iter()
                .all(|&glyph| ('\u{100000}'..='\u{101fff}').contains(&glyph))
        );

        let mut first = MatrixAnimation::with_variant(context(80, 24), MatrixVariant::Accelerando);
        let mut second = first.clone();
        let initial = first.render_frame();
        assert_eq!(first.columns.len(), 40);
        assert!(visible_glyphs(&initial) > 400);
        assert!(
            initial
                .iter()
                .flat_map(|line| line.chars())
                .any(|glyph| glyphs.contains(&glyph))
        );
        assert!(
            !initial
                .iter()
                .flat_map(|line| line.chars())
                .any(|glyph| ('\u{2800}'..='\u{28ff}').contains(&glyph))
        );
        assert!(
            first
                .columns
                .iter()
                .all(|column| (12..=36).contains(&column.length))
        );
        assert!(first.columns.iter().any(|column| column.length > 24));
        assert!(MATRIX_GLYPHS.iter().any(|&glyph| {
            initial
                .iter()
                .any(|line| line.contains(&format!("m{}\u{1b}[39m", glyph as char)))
        }));
        assert!(initial.join("").contains("38;5;15"));
        assert!(initial.join("").contains("38;5;28"));
        let mut shimmer = first.clone();
        shimmer.columns.truncate(1);
        shimmer.columns[0].head = 0;
        shimmer.columns[0].length = 1;
        let patterns: std::collections::HashSet<_> = (0..24)
            .map(|frame| {
                shimmer.frame_index = frame;
                shimmer.render_frame()
            })
            .collect();
        assert!(patterns.len() >= 10);
        for _ in 0..360 {
            first.advance_frame();
            second.advance_frame();
            assert!(
                first
                    .columns
                    .iter()
                    .all(|column| (12..=36).contains(&column.length))
            );
        }
        assert_eq!(first, second);
        assert_ne!(first.render_frame(), initial);

        for (width, height) in [(0, 0), (0, 7), (9, 0), (1, 1), (17, 5), (80, 24)] {
            first.resize(context(width, height));
            first.advance_frame();
            let frame = first.render_frame();
            if width == 0 || height == 0 {
                assert!(frame.is_empty());
            } else {
                assert_eq!(frame.len(), height);
                assert!(frame.iter().all(|line| visible_line_width(line) == width));
            }
            assert_eq!(first.variant, MatrixVariant::Accelerando);
        }
        first.frame_index = usize::MAX;
        first.advance_frame();
        assert_eq!(first.frame_index, 0);
    }

    fn visible_glyphs(frame: &[String]) -> usize {
        frame
            .iter()
            .map(|line| line.matches("\u{1b}[39m").count())
            .sum()
    }
}
