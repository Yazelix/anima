use std::time::Duration;

use crate::random::{seeded_index, size_seed};
use crate::{ScreenAnimationContext, ScreenCell, ScreenFrame, ScreenFrameProducer};
use crossterm::style::Color;

pub const MATRIX_STYLE: &str = "matrix";
pub const ACCELERANDO_STYLE: &str = "accelerando";

const MATRIX_GLYPHS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ@#$%&*+=-:;.";
const GLYPH_WIDTH: usize = 2;
const BIG_GLYPH_WIDTH: usize = 8;
const BIG_GLYPH_HEIGHT: usize = 4;
const ACCELERANDO_BITMAPS: &[u8; 8192 * 8] = include_bytes!("accelerando-glyphs.bin");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixVariant {
    Classic,
    Accelerando,
}

impl MatrixVariant {
    fn dimensions(self, context: ScreenAnimationContext, big: bool) -> (usize, usize) {
        match self {
            Self::Classic => (context.inner_width, context.resolved_height),
            Self::Accelerando => (
                context
                    .inner_width
                    .div_ceil(if big { BIG_GLYPH_WIDTH } else { GLYPH_WIDTH }),
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
    symbols: bool,
    big: bool,
    frame_index: usize,
    seed: u64,
    columns: Vec<MatrixColumn>,
}

impl MatrixAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        Self::with_variant(context, MatrixVariant::Classic)
    }

    pub fn with_variant(context: ScreenAnimationContext, variant: MatrixVariant) -> Self {
        Self::configured(context, variant, false, false)
    }

    fn configured(
        context: ScreenAnimationContext,
        variant: MatrixVariant,
        symbols: bool,
        big: bool,
    ) -> Self {
        let (width, height) = variant.dimensions(context, big);
        let pitch = if big { BIG_GLYPH_HEIGHT } else { 1 };
        let mut seed = size_seed(width, height, 0x4D41_5452_4958);
        let columns = (0..width)
            .map(|_| new_column(&mut seed, height, true, variant, pitch))
            .collect();
        Self {
            context,
            variant,
            symbols,
            big,
            frame_index: 0,
            seed,
            columns,
        }
    }

    pub fn with_symbols(mut self, symbols: bool) -> Self {
        self.symbols = symbols;
        self
    }

    /// Draw Accelerando with large solid pixels; ordinary Matrix keeps its size.
    pub fn with_big(self, big: bool) -> Self {
        if self.big == big || self.variant == MatrixVariant::Classic {
            return self;
        }
        Self::configured(self.context, self.variant, self.symbols, big)
    }

    fn render_accelerando(&self) -> Vec<String> {
        let width = self.context.inner_width;
        let (_, height) = self.variant.dimensions(self.context, self.big);
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut frame = ScreenFrame::new(width, height);
        let pitch = if self.big { BIG_GLYPH_HEIGHT } else { 1 };
        for (x, column) in self.columns.iter().enumerate() {
            for age in 0..column.length {
                let top = column.head - (age * pitch) as i32;
                if top + pitch as i32 <= 0 || top >= height as i32 {
                    continue;
                }
                let mutation = self
                    .frame_index
                    .wrapping_add(x.wrapping_mul(17))
                    .wrapping_add(age * 11);
                let hash = matrix_glyph_hash(column.glyph_seed, x, age, mutation);
                let tone = trail_tone(age, column.length);
                let symbol = self.symbols && (hash >> 32).is_multiple_of(4);
                if self.big {
                    let bits = if symbol {
                        symbol_bitmap(hash as usize % MATRIX_GLYPHS.len())
                    } else {
                        accelerando_bitmap(hash as usize % 8192)
                    };
                    paint_bitmap(&mut frame, bits, x * BIG_GLYPH_WIDTH, top, tone);
                    continue;
                }
                let glyph = if symbol {
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
        let (_, height) = self.variant.dimensions(self.context, self.big);
        let pitch = if self.big { BIG_GLYPH_HEIGHT } else { 1 };
        for column in &mut self.columns {
            if self
                .frame_index
                .wrapping_add(column.phase)
                .is_multiple_of(column.speed)
            {
                column.head += 1;
            }
            if column.head as i64 - (column.length * pitch) as i64 >= height as i64 {
                *column = new_column(&mut self.seed, height, false, self.variant, pitch);
            }
        }
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::configured(context, self.variant, self.symbols, self.big);
    }
}

pub fn matrix_frame_delay() -> Duration {
    Duration::from_millis(55)
}

fn new_column(
    seed: &mut u64,
    height: usize,
    warm: bool,
    variant: MatrixVariant,
    pitch: usize,
) -> MatrixColumn {
    let glyph_height = height.div_ceil(pitch);
    let (minimum_length, maximum_length) = match variant {
        MatrixVariant::Classic => {
            let minimum = height.clamp(1, 4);
            (minimum, (height.saturating_mul(2) / 3).clamp(minimum, 24))
        }
        MatrixVariant::Accelerando => {
            let minimum = (glyph_height / 2).clamp(1, 24);
            (
                minimum,
                (glyph_height.saturating_mul(3) / 2).clamp(minimum, 72),
            )
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

fn accelerando_bitmap(id: usize) -> u64 {
    u64::from_le_bytes(
        ACCELERANDO_BITMAPS[id * 8..(id + 1) * 8]
            .try_into()
            .unwrap(),
    )
}

fn paint_bitmap(frame: &mut ScreenFrame, bits: u64, left: usize, top: i32, tone: usize) {
    for y in 0..BIG_GLYPH_HEIGHT {
        let row = top + y as i32;
        if row < 0 {
            continue;
        }
        for x in 0..7 {
            let upper = bits & (1 << (y * 14 + x)) != 0;
            let lower = bits & (1 << (y * 14 + 7 + x)) != 0;
            let glyph = match (upper, lower) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => continue,
            };
            frame.set(
                left.saturating_add(x),
                row as usize,
                ScreenCell::indexed(glyph, 0, tone),
            );
        }
    }
}

fn symbol_bitmap(index: usize) -> u64 {
    MATRIX_BITMAPS[index]
        .iter()
        .enumerate()
        .fold(0, |mut bits, (y, row)| {
            for x in 0..5 {
                if row & (1 << (4 - x)) != 0 {
                    bits |= 1 << (y * 7 + x + 1);
                }
            }
            bits
        })
}

// Original 5x7 block lettering, in MATRIX_GLYPHS order, centered in the 7x7 grid.
const MATRIX_BITMAPS: [[u8; 7]; MATRIX_GLYPHS.len()] = [
    [14, 17, 19, 21, 25, 17, 14], // 0
    [4, 12, 4, 4, 4, 4, 14],      // 1
    [14, 17, 1, 2, 4, 8, 31],     // 2
    [30, 1, 1, 14, 1, 1, 30],     // 3
    [2, 6, 10, 18, 31, 2, 2],     // 4
    [31, 16, 16, 30, 1, 1, 30],   // 5
    [6, 8, 16, 30, 17, 17, 14],   // 6
    [31, 1, 2, 4, 8, 8, 8],       // 7
    [14, 17, 17, 14, 17, 17, 14], // 8
    [14, 17, 17, 15, 1, 2, 12],   // 9
    [14, 17, 17, 31, 17, 17, 17], // A
    [30, 17, 17, 30, 17, 17, 30], // B
    [14, 17, 16, 16, 16, 17, 14], // C
    [30, 17, 17, 17, 17, 17, 30], // D
    [31, 16, 16, 30, 16, 16, 31], // E
    [31, 16, 16, 30, 16, 16, 16], // F
    [14, 17, 16, 23, 17, 17, 15], // G
    [17, 17, 17, 31, 17, 17, 17], // H
    [14, 4, 4, 4, 4, 4, 14],      // I
    [7, 2, 2, 2, 18, 18, 12],     // J
    [17, 18, 20, 24, 20, 18, 17], // K
    [16, 16, 16, 16, 16, 16, 31], // L
    [17, 27, 21, 21, 17, 17, 17], // M
    [17, 25, 25, 21, 19, 19, 17], // N
    [14, 17, 17, 17, 17, 17, 14], // O
    [30, 17, 17, 30, 16, 16, 16], // P
    [14, 17, 17, 17, 21, 18, 13], // Q
    [30, 17, 17, 30, 20, 18, 17], // R
    [15, 16, 16, 14, 1, 1, 30],   // S
    [31, 4, 4, 4, 4, 4, 4],       // T
    [17, 17, 17, 17, 17, 17, 14], // U
    [17, 17, 17, 17, 17, 10, 4],  // V
    [17, 17, 17, 21, 21, 27, 17], // W
    [17, 17, 10, 4, 10, 17, 17],  // X
    [17, 17, 10, 4, 4, 4, 4],     // Y
    [31, 1, 2, 4, 8, 16, 31],     // Z
    [14, 17, 23, 21, 23, 16, 14], // @
    [10, 10, 31, 10, 31, 10, 10], // #
    [4, 15, 20, 14, 5, 30, 4],    // $
    [24, 25, 2, 4, 8, 19, 3],     // %
    [12, 18, 20, 8, 21, 18, 13],  // &
    [0, 21, 14, 31, 14, 21, 0],   // *
    [0, 4, 4, 31, 4, 4, 0],       // +
    [0, 0, 31, 0, 31, 0, 0],      // =
    [0, 0, 0, 31, 0, 0, 0],       // -
    [0, 4, 4, 0, 4, 4, 0],        // :
    [0, 4, 4, 0, 4, 4, 8],        // ;
    [0, 0, 0, 0, 0, 4, 4],        // .
];

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
        assert_eq!(
            initial
                .iter()
                .flat_map(|line| line.chars())
                .filter(|glyph| glyphs.contains(glyph))
                .count(),
            visible_glyphs(&initial)
        );
        let mut mixed = first.clone().with_symbols(true);
        let mixed_frame = mixed.render_frame();
        assert_ne!(mixed_frame, initial);
        assert!(MATRIX_GLYPHS.iter().any(|&glyph| {
            mixed_frame
                .iter()
                .any(|line| line.contains(&format!("m{}\u{1b}[39m", glyph as char)))
        }));
        mixed.resize(context(80, 24));
        assert_eq!(mixed.render_frame(), mixed_frame);
        assert_eq!(mixed.with_symbols(false).render_frame(), initial);
        assert_eq!(
            MatrixAnimation::new(context(40, 12)).render_frame(),
            MatrixAnimation::new(context(40, 12))
                .with_symbols(true)
                .render_frame()
        );
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

    #[test]
    fn big_accelerando_preserves_geometry_symbols_and_resize() {
        let bitmaps: std::collections::HashSet<_> = (0..8192).map(accelerando_bitmap).collect();
        assert_eq!(bitmaps.len(), 8192);
        assert!(bitmaps.iter().all(|&bits| bits > 0 && bits < (1 << 49)));
        for bits in &bitmaps {
            let mut frame = ScreenFrame::new(7, 4);
            paint_bitmap(&mut frame, *bits, 0, 0, 4);
            let mut recovered = 0u64;
            for (y, line) in frame
                .render_lines(7, |cell| cell.glyph.to_string())
                .iter()
                .enumerate()
            {
                for (x, glyph) in line.chars().enumerate() {
                    if matches!(glyph, '▀' | '█') {
                        recovered |= 1 << (y * 14 + x);
                    }
                    if matches!(glyph, '▄' | '█') {
                        recovered |= 1 << (y * 14 + 7 + x);
                    }
                }
            }
            assert_eq!(recovered, *bits);
        }
        let mut pixels = ScreenFrame::new(7, 4);
        paint_bitmap(&mut pixels, 1 | (1 << 48), 0, 0, 4);
        let plain = pixels.render_lines(7, |cell| cell.glyph.to_string());
        assert_eq!(plain, ["▀      ", "       ", "       ", "      ▀"]);
        paint_bitmap(&mut pixels, 1 | (1 << 48), usize::MAX, -3, 4);
        assert_eq!(pixels.render_lines(7, |cell| cell.glyph.to_string()), plain);
        let mut clipped = ScreenFrame::new(1, 1);
        paint_bitmap(&mut clipped, 1 << 42, 0, -3, 4);
        assert_eq!(
            clipped.render_lines(1, |cell| cell.glyph.to_string()),
            ["▀"]
        );
        let symbols: std::collections::HashSet<_> =
            (0..MATRIX_GLYPHS.len()).map(symbol_bitmap).collect();
        assert_eq!(symbols.len(), MATRIX_GLYPHS.len());
        assert!(symbols.iter().all(|&bits| bits > 0 && bits < (1 << 49)));

        let native = MatrixAnimation::with_variant(context(80, 24), MatrixVariant::Accelerando);
        let mut big = native.clone().with_big(true);
        let initial = big.render_frame();
        assert_ne!(initial, native.render_frame());
        assert!(
            initial
                .iter()
                .any(|line| line.contains('▀') || line.contains('▄'))
        );
        assert_eq!(big.columns.len(), 10);
        assert_ne!(initial, big.clone().with_symbols(true).render_frame());
        for _ in 0..360 {
            big.advance_frame();
        }
        assert_ne!(big.render_frame(), initial);
        for symbols in [false, true] {
            big = big.with_symbols(symbols);
            for (width, height) in [(0, 0), (0, 7), (9, 0), (1, 1), (17, 5), (80, 24)] {
                big.resize(context(width, height));
                big.advance_frame();
                let frame = big.render_frame();
                assert!(big.big);
                assert_eq!(big.symbols, symbols);
                if width == 0 || height == 0 {
                    assert!(frame.is_empty());
                } else {
                    assert_eq!(frame.len(), height);
                    assert!(frame.iter().all(|line| visible_line_width(line) == width));
                }
            }
        }
        assert_eq!(native.clone().with_big(true).with_big(false), native);
        assert_eq!(
            MatrixAnimation::new(context(40, 12))
                .with_big(true)
                .render_frame(),
            MatrixAnimation::new(context(40, 12)).render_frame()
        );
    }
}
