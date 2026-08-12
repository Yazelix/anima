use std::time::Duration;

use crate::random::unit_from_seed;
use crate::{ScreenAnimationContext, ScreenCell, ScreenFrame, ScreenFrameProducer};
use crossterm::style::Color;

pub const MATRIX_STYLE: &str = "matrix";

const MATRIX_GLYPHS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ@#$%&*+=-:;.";

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
    frame_index: usize,
    seed: u64,
    columns: Vec<MatrixColumn>,
}

impl MatrixAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let mut seed = (context.inner_width as u64)
            .wrapping_mul(1_103_515_245)
            .wrapping_add((context.resolved_height as u64).wrapping_mul(12_345))
            .wrapping_add(0x4D41_5452_4958);
        let columns = (0..context.inner_width)
            .map(|_| new_column(&mut seed, context.resolved_height, true))
            .collect();
        Self {
            context,
            frame_index: 0,
            seed,
            columns,
        }
    }
}

impl ScreenFrameProducer for MatrixAnimation {
    fn render_frame(&self) -> Vec<String> {
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
        let height = self.context.resolved_height;
        for column in &mut self.columns {
            if self
                .frame_index
                .wrapping_add(column.phase)
                .is_multiple_of(column.speed)
            {
                column.head += 1;
            }
            if column.head as i64 - column.length as i64 >= height as i64 {
                *column = new_column(&mut self.seed, height, false);
            }
        }
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn matrix_frame_delay() -> Duration {
    Duration::from_millis(55)
}

fn new_column(seed: &mut u64, height: usize, warm: bool) -> MatrixColumn {
    let minimum_length = height.clamp(1, 4);
    let maximum_length = (height.saturating_mul(2) / 3).clamp(minimum_length, 24);
    let length = minimum_length + random_index(seed, maximum_length - minimum_length + 1);
    let speed = 1 + random_index(seed, 3);
    let head = if warm {
        random_index(seed, height.max(1)) as i32
    } else {
        -(random_index(seed, (height / 4).max(1)) as i32) - 1
    };

    MatrixColumn {
        head,
        length,
        speed,
        phase: random_index(seed, speed),
        glyph_seed: *seed,
    }
}

fn random_index(seed: &mut u64, length: usize) -> usize {
    (unit_from_seed(seed) * length as f64) as usize
}

fn trail_tone(age: usize, length: usize) -> usize {
    length.saturating_sub(age) * 4 / length
}

fn matrix_glyph(seed: u64, x: usize, y: usize, frame_index: usize) -> char {
    let mut hash = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ ((frame_index / 2) as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^= hash >> 27;
    MATRIX_GLYPHS[(hash as usize) % MATRIX_GLYPHS.len()] as char
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

    fn visible_glyphs(frame: &[String]) -> usize {
        frame
            .iter()
            .map(|line| line.matches("\u{1b}[39m").count())
            .sum()
    }
}
