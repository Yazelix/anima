use std::time::Duration;

use crate::{ScreenAnimationContext, ScreenCell, ScreenFrame, ScreenFrameProducer};
use crossterm::style::Color;

const MANDELBROT_LOOP_FRAMES: usize = 120;
const MANDELBROT_RECURSIVE_PORTAL_START_PROGRESS: f64 = 0.68;
const MANDELBROT_TARGET_CENTER: Complex64 = Complex64 {
    re: -0.743_643_887_037_151,
    im: 0.131_825_904_205_33,
};
const MANDELBROT_START_SCALE_X: f64 = 3.15;
const MANDELBROT_END_SCALE_X: f64 = 0.000_01;
const MANDELBROT_MIN_ITERATIONS: usize = 64;
const MANDELBROT_MAX_ITERATIONS: usize = 1_800;
const MANDELBROT_FRAME_DELAY_MS: u64 = 55;

#[derive(Debug, Clone, PartialEq)]
pub struct MandelbrotAnimation {
    context: ScreenAnimationContext,
    frame_index: usize,
}

impl MandelbrotAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        Self {
            context,
            frame_index: 0,
        }
    }
}

impl ScreenFrameProducer for MandelbrotAnimation {
    fn render_frame(&self) -> Vec<String> {
        render_mandelbrot_frame(self.context, self.frame_index)
    }

    fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        self.context = context;
        self.frame_index = 0;
    }
}

pub fn mandelbrot_frame_delay() -> Duration {
    Duration::from_millis(MANDELBROT_FRAME_DELAY_MS)
}

pub fn mandelbrot_max_iterations(width: usize, height: usize) -> usize {
    (42 + width.saturating_mul(height) / 320).clamp(48, 96)
}

fn mandelbrot_max_iterations_for_zoom(width: usize, height: usize, zoom: f64) -> usize {
    let base_iterations = mandelbrot_max_iterations(width, height);
    let zoom_iterations = zoom.max(1.0).log2().mul_add(48.0, 0.0).round() as usize;
    base_iterations
        .saturating_add(zoom_iterations)
        .clamp(MANDELBROT_MIN_ITERATIONS, MANDELBROT_MAX_ITERATIONS)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Complex64 {
    re: f64,
    im: f64,
}

impl std::ops::Add for Complex64 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl std::ops::Mul for Complex64 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MandelbrotEscape {
    iterations: usize,
    distance_estimate: Option<f64>,
}

pub fn mandelbrot_escape_iterations(cx: f64, cy: f64, max_iterations: usize) -> usize {
    mandelbrot_escape(cx, cy, max_iterations).iterations
}

fn mandelbrot_escape(cx: f64, cy: f64, max_iterations: usize) -> MandelbrotEscape {
    if is_known_mandelbrot_interior(cx, cy) {
        return MandelbrotEscape {
            iterations: max_iterations,
            distance_estimate: None,
        };
    }

    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut derivative_x = 0.0;
    let mut derivative_y = 0.0;

    for iteration in 0..max_iterations {
        let magnitude_squared = zx * zx + zy * zy;
        if magnitude_squared > 4.0 {
            return MandelbrotEscape {
                iterations: iteration,
                distance_estimate: mandelbrot_distance_estimate(
                    magnitude_squared,
                    derivative_x,
                    derivative_y,
                ),
            };
        }
        let next_derivative_x = 2.0 * (zx * derivative_x - zy * derivative_y) + 1.0;
        let next_derivative_y = 2.0 * (zx * derivative_y + zy * derivative_x);
        let next_x = zx * zx - zy * zy + cx;
        zy = 2.0 * zx * zy + cy;
        zx = next_x;
        derivative_x = next_derivative_x;
        derivative_y = next_derivative_y;
    }

    MandelbrotEscape {
        iterations: max_iterations,
        distance_estimate: None,
    }
}

fn is_known_mandelbrot_interior(cx: f64, cy: f64) -> bool {
    let cardioid_x = cx - 0.25;
    let q = cardioid_x * cardioid_x + cy * cy;
    let in_main_cardioid = q * (q + cardioid_x) <= 0.25 * cy * cy;
    let period_two_x = cx + 1.0;
    let in_period_two_bulb = period_two_x * period_two_x + cy * cy <= 0.0625;

    in_main_cardioid || in_period_two_bulb
}

fn mandelbrot_distance_estimate(
    magnitude_squared: f64,
    derivative_x: f64,
    derivative_y: f64,
) -> Option<f64> {
    let derivative_magnitude = derivative_x.hypot(derivative_y);
    if derivative_magnitude <= f64::EPSILON {
        return None;
    }

    let magnitude = magnitude_squared.sqrt();
    Some(0.5 * magnitude * magnitude.ln() / derivative_magnitude)
}

fn render_mandelbrot_frame(context: ScreenAnimationContext, frame_index: usize) -> Vec<String> {
    let width = context.inner_width.max(1);
    let height = context.resolved_height.max(1);
    let view = mandelbrot_view(frame_index);
    let max_iterations = mandelbrot_max_iterations_for_zoom(width, height, view.zoom);
    let mut frame = ScreenFrame::new(width, height);
    populate_mandelbrot_frame(&mut frame, width, height, frame_index, view, max_iterations);

    frame.render_lines(context.resolved_width, colorize_mandelbrot_cell)
}

fn populate_mandelbrot_frame(
    frame: &mut ScreenFrame,
    width: usize,
    height: usize,
    frame_index: usize,
    view: MandelbrotView,
    max_iterations: usize,
) {
    let portal_progress =
        mandelbrot_recursive_portal_progress(mandelbrot_loop_progress(frame_index));
    let nested_view = mandelbrot_view_for_progress(0.0);
    let nested_max_iterations = mandelbrot_max_iterations_for_zoom(width, height, nested_view.zoom);

    for y in 0..height {
        for x in 0..width {
            let nx = normalized_axis_position(x, width);
            let ny = normalized_axis_position(y, height);

            if let Some((portal_x, portal_y)) =
                portal_progress.and_then(|progress| mandelbrot_portal_coordinates(nx, ny, progress))
            {
                if let Some(cell) = mandelbrot_sample_at(
                    portal_x,
                    portal_y,
                    width,
                    nested_view,
                    nested_max_iterations,
                )
                .and_then(|score| mandelbrot_cell(score, nested_view))
                {
                    frame.set(x, y, cell);
                }
                continue;
            }

            if let Some(cell) = mandelbrot_sample_at(nx, ny, width, view, max_iterations)
                .and_then(|score| mandelbrot_cell(score, view))
            {
                frame.set(x, y, cell);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MandelbrotView {
    center: Complex64,
    multiplier: Complex64,
    base_scale_x: f64,
    scale_x: f64,
    zoom: f64,
}

fn mandelbrot_view(frame_index: usize) -> MandelbrotView {
    let progress = mandelbrot_loop_progress(frame_index);
    mandelbrot_view_for_progress(progress)
}

fn mandelbrot_view_for_progress(progress: f64) -> MandelbrotView {
    let dive_progress = mandelbrot_dive_progress(progress);
    let scale_x = mandelbrot_scale_x(dive_progress);
    let scale_ratio = scale_x / MANDELBROT_START_SCALE_X;

    MandelbrotView {
        center: MANDELBROT_TARGET_CENTER,
        multiplier: Complex64 {
            re: scale_ratio,
            im: 0.0,
        },
        base_scale_x: MANDELBROT_START_SCALE_X,
        scale_x,
        zoom: MANDELBROT_START_SCALE_X / scale_x,
    }
}

fn smoothstep(progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoothstep_with_tail(progress: f64, tail_weight: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    let tail = tail_weight.clamp(0.0, 1.0);
    smoothstep(t) * (1.0 - tail) + t * tail
}

fn mandelbrot_dive_progress(progress: f64) -> f64 {
    smoothstep_with_tail(progress, 0.20).powf(1.10)
}

fn mandelbrot_scale_x(dive_progress: f64) -> f64 {
    let log_start = MANDELBROT_START_SCALE_X.ln();
    let log_end = MANDELBROT_END_SCALE_X.ln();
    (log_start + (log_end - log_start) * dive_progress.clamp(0.0, 1.0)).exp()
}

fn mandelbrot_loop_progress(frame_index: usize) -> f64 {
    if MANDELBROT_LOOP_FRAMES <= 1 {
        0.0
    } else {
        (frame_index % MANDELBROT_LOOP_FRAMES) as f64 / (MANDELBROT_LOOP_FRAMES - 1) as f64
    }
}

fn normalized_axis_position(position: usize, length: usize) -> f64 {
    if length <= 1 {
        0.5
    } else {
        position as f64 / (length - 1) as f64
    }
}

fn mandelbrot_point(nx: f64, ny: f64, view: MandelbrotView) -> (f64, f64) {
    let base = Complex64 {
        re: (nx - 0.5) * view.base_scale_x,
        im: (ny - 0.5) * view.base_scale_x * 0.64,
    };
    let point = view.center + view.multiplier * base;
    (point.re, point.im)
}

fn mandelbrot_sample_at(
    nx: f64,
    ny: f64,
    width: usize,
    view: MandelbrotView,
    max_iterations: usize,
) -> Option<f64> {
    let (cx, cy) = mandelbrot_point(nx, ny, view);
    let escape = mandelbrot_escape(cx, cy, max_iterations);
    mandelbrot_sample(escape, max_iterations, view, width)
}

fn mandelbrot_recursive_portal_progress(progress: f64) -> Option<f64> {
    if progress < MANDELBROT_RECURSIVE_PORTAL_START_PROGRESS {
        return None;
    }

    Some(smoothstep_with_tail(
        (progress - MANDELBROT_RECURSIVE_PORTAL_START_PROGRESS)
            / (1.0 - MANDELBROT_RECURSIVE_PORTAL_START_PROGRESS),
        0.34,
    ))
}

fn mandelbrot_portal_coordinates(nx: f64, ny: f64, progress: f64) -> Option<(f64, f64)> {
    let scale = 0.12 + progress * 0.88;
    let local_x = (nx - 0.5) / scale;
    let local_y = (ny - 0.5) / scale;
    let distance = (local_x * local_x + (local_y * 1.20) * (local_y * 1.20)).sqrt();
    let hole_radius = 0.08 + progress * 0.74;
    if distance > hole_radius {
        return None;
    }

    Some((0.5 + local_x, 0.5 + local_y))
}

fn mandelbrot_sample(
    escape: MandelbrotEscape,
    max_iterations: usize,
    view: MandelbrotView,
    width: usize,
) -> Option<f64> {
    let iterations = escape.iterations;
    if iterations <= 1 || iterations == max_iterations {
        return None;
    }

    let pixel_scale = view.scale_x / width.max(1) as f64;
    let distance_pixels = escape.distance_estimate.map_or(0.0, |distance| {
        distance / pixel_scale.max(f64::MIN_POSITIVE)
    });
    let boundary_weight = (1.0 / (1.0 + distance_pixels.max(0.0).powf(0.7))).clamp(0.0, 1.0);
    let dwell_weight = (iterations as f64 / max_iterations as f64).powf(0.35);

    Some(boundary_weight * 0.70 + dwell_weight * 0.30)
}

fn mandelbrot_cell(score: f64, view: MandelbrotView) -> Option<ScreenCell> {
    let intensity = score.clamp(0.0, 1.0);
    let (glyph, intensity_bucket) = if intensity < 0.28 {
        return None;
    } else if intensity < 0.44 {
        ('░', 1)
    } else if intensity < 0.64 {
        ('▒', 2)
    } else if intensity < 0.82 {
        ('▓', 4)
    } else {
        ('█', 6)
    };

    let zoom_band = (view.zoom.max(1.0).log2() / 5.0).floor().max(0.0) as usize;
    Some(ScreenCell {
        glyph,
        color_x: zoom_band,
        color_y: intensity_bucket,
    })
}

fn colorize_mandelbrot_cell(cell: ScreenCell) -> String {
    let phase = cell.color_x % 3;
    let color = match (cell.color_y, phase) {
        (0 | 1, 0) => Color::AnsiValue(33),
        (0 | 1, _) | (2 | 3, 0) => Color::AnsiValue(129),
        (4 | 5, 2) => Color::AnsiValue(208),
        (2 | 3, _) | (4 | 5, _) => Color::AnsiValue(201),
        (_, 2) => Color::AnsiValue(226),
        _ => Color::AnsiValue(208),
    };
    crate::terminal_control::styled(cell.glyph, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test lane: default

    fn context(width: usize, height: usize) -> ScreenAnimationContext {
        ScreenAnimationContext {
            resolved_width: width,
            resolved_height: height,
            inner_width: width,
            size_class: "test",
        }
    }

    fn strip_ansi_codes(line: &str) -> String {
        let mut visible = String::new();
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for code_ch in chars.by_ref() {
                    if code_ch.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
            visible.push(ch);
        }
        visible
    }

    fn strip_ansi_from_frame(frame: Vec<String>) -> Vec<String> {
        frame
            .into_iter()
            .map(|line| strip_ansi_codes(&line))
            .collect()
    }

    fn render_test_frame(context: ScreenAnimationContext, frame_index: usize) -> Vec<String> {
        render_mandelbrot_frame(context, frame_index)
    }

    fn visible_frame_similarity(first: &[String], second: &[String]) -> f64 {
        let mut matching_cells = 0;
        let mut total_cells = 0;

        for (first_line, second_line) in first.iter().zip(second.iter()) {
            for (first_cell, second_cell) in first_line.chars().zip(second_line.chars()) {
                if first_cell == second_cell {
                    matching_cells += 1;
                }
                total_cells += 1;
            }
        }

        matching_cells as f64 / total_cells as f64
    }

    fn dominant_structural_glyph_fraction(frame: &[String]) -> f64 {
        let mut counts = std::collections::BTreeMap::new();
        let mut total_cells = 0;

        for line in frame {
            for cell in line.chars() {
                if cell == ' ' {
                    continue;
                }
                *counts.entry(cell).or_insert(0) += 1;
                total_cells += 1;
            }
        }

        if total_cells == 0 {
            return 1.0;
        }

        counts.values().copied().max().unwrap_or(0) as f64 / total_cells as f64
    }

    fn visible_glyph_fraction(frame: &[String], predicate: impl Fn(char) -> bool) -> f64 {
        let mut matching_cells = 0;
        let mut total_cells = 0;

        for line in frame {
            for cell in line.chars() {
                if predicate(cell) {
                    matching_cells += 1;
                }
                total_cells += 1;
            }
        }

        matching_cells as f64 / total_cells as f64
    }

    // Defends: Mandelbrot uses deterministic in-house CPU frames without host randomness or external engines.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_animation_is_deterministic_and_advances() {
        let mut first = MandelbrotAnimation::new(context(48, 16));
        let mut second = MandelbrotAnimation::new(context(48, 16));
        assert_eq!(first.render_frame(), second.render_frame());

        let initial = first.render_frame();
        for _ in 0..4 {
            first.advance_frame();
            second.advance_frame();
        }

        assert_eq!(first.render_frame(), second.render_frame());
        assert_ne!(initial, first.render_frame());
    }

    // Regression: Mandelbrot must visibly zoom through fractal structure, not merely pulse color.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_loop_changes_visible_fractal_structure() {
        let initial = strip_ansi_from_frame(render_test_frame(context(64, 20), 0));
        let deep_zoom = strip_ansi_from_frame(render_test_frame(
            context(64, 20),
            MANDELBROT_LOOP_FRAMES * 3 / 8,
        ));

        assert_ne!(initial, deep_zoom);
        assert!(visible_frame_similarity(&initial, &deep_zoom) <= 0.65);
    }

    // Regression: Mandelbrot must not spend sampled loop points as uniform low-detail screens.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_sampled_frames_keep_visible_variation() {
        for frame_index in [
            0,
            MANDELBROT_LOOP_FRAMES / 8,
            MANDELBROT_LOOP_FRAMES / 4,
            MANDELBROT_LOOP_FRAMES / 2,
            MANDELBROT_LOOP_FRAMES * 3 / 4,
            MANDELBROT_LOOP_FRAMES - 1,
        ] {
            let visible = strip_ansi_from_frame(render_test_frame(context(64, 20), frame_index));

            assert!(
                dominant_structural_glyph_fraction(&visible) <= 0.72,
                "frame {frame_index} collapsed to one visible glyph"
            );
        }
    }

    // Regression: Mandelbrot should use dark interior space while keeping visible fractal structure.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_frames_use_negative_space_not_sparse_dots_or_solid_fill() {
        for frame_index in [
            0,
            MANDELBROT_LOOP_FRAMES / 4,
            MANDELBROT_LOOP_FRAMES / 2,
            MANDELBROT_LOOP_FRAMES * 3 / 4,
            MANDELBROT_LOOP_FRAMES - 1,
        ] {
            let visible = strip_ansi_from_frame(render_test_frame(context(64, 20), frame_index));
            let dotted_fraction =
                visible_glyph_fraction(&visible, |cell| matches!(cell, '.' | '·' | ':'));
            let structural_fraction =
                visible_glyph_fraction(&visible, |cell| matches!(cell, '░' | '▒' | '▓' | '█'));

            assert_eq!(dotted_fraction, 0.0);
            assert!(
                structural_fraction >= 0.04,
                "frame {frame_index} is too sparse: structural fraction {structural_fraction}"
            );
        }
    }

    // Defends: the bounded recursion phase repeats so long-running sessions do not drain into empty precision limits.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_cycle_boundary_repeats_first_frame_exactly() {
        assert_eq!(
            render_test_frame(context(48, 16), 0),
            render_test_frame(context(48, 16), MANDELBROT_LOOP_FRAMES)
        );
    }

    // Regression: the frame before wrap must already reveal the next cycle's overview.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_recursive_portal_hides_cycle_reset() {
        assert_eq!(
            render_test_frame(context(48, 16), 0),
            render_test_frame(context(48, 16), MANDELBROT_LOOP_FRAMES - 1)
        );
    }

    // Regression: the recursive portal should carve a growing hole instead of overlapping the parent layer.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_recursive_portal_carves_parent_layer_and_keeps_moving() {
        let start =
            mandelbrot_recursive_portal_progress(MANDELBROT_RECURSIVE_PORTAL_START_PROGRESS)
                .expect("portal starts");
        let before_end = mandelbrot_recursive_portal_progress(0.95).expect("portal active");
        let near_end = mandelbrot_recursive_portal_progress(0.99).expect("portal active");
        let end = mandelbrot_recursive_portal_progress(1.0).expect("portal active");

        assert!(mandelbrot_portal_coordinates(0.5, 0.5, start).is_some());
        assert!(mandelbrot_portal_coordinates(0.0, 0.0, start).is_none());
        assert!(
            near_end - before_end > 0.04,
            "portal slowed too much near the cycle boundary"
        );
        assert_eq!(
            mandelbrot_portal_coordinates(0.0, 0.0, end),
            Some((0.0, 0.0))
        );
        assert_eq!(
            mandelbrot_portal_coordinates(1.0, 1.0, end),
            Some((1.0, 1.0))
        );
    }

    // Defends: Mandelbrot starts from a recognizable overview, dives deeply, then recurses into the next overview.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_view_dives_from_overview_to_recursive_boundary() {
        let home = mandelbrot_view(0);
        let quarter = mandelbrot_view(MANDELBROT_LOOP_FRAMES / 4);
        let half = mandelbrot_view(MANDELBROT_LOOP_FRAMES / 2);
        let deep = mandelbrot_view(MANDELBROT_LOOP_FRAMES * 4 / 5);
        let seam = mandelbrot_view(MANDELBROT_LOOP_FRAMES - 1);
        let next_cycle = mandelbrot_view(MANDELBROT_LOOP_FRAMES);

        assert_eq!(home.center, MANDELBROT_TARGET_CENTER);
        assert!(quarter.zoom > home.zoom);
        assert!(half.zoom > quarter.zoom);
        assert!(deep.zoom > half.zoom);
        assert!(deep.zoom > home.zoom * 10_000.0);
        assert!(deep.scale_x < home.scale_x / 10_000.0);
        assert_eq!(deep.center, MANDELBROT_TARGET_CENTER);
        assert!(seam.zoom > deep.zoom);
        assert!(seam.zoom > home.zoom * 100_000.0);
        assert_eq!(next_cycle, home);
        assert_eq!(seam.center, MANDELBROT_TARGET_CENTER);
        assert!((home.scale_x - MANDELBROT_START_SCALE_X).abs() < f64::EPSILON);
        assert!((home.multiplier.re - 1.0).abs() < f64::EPSILON);
        assert!(home.multiplier.im.abs() < f64::EPSILON);
    }

    // Regression: Mandelbrot should zoom through the Seahorse target instead of panning around it.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_camera_stays_fixed_on_target_boundary_without_rotating() {
        let home = mandelbrot_view(0);
        let early = mandelbrot_view(MANDELBROT_LOOP_FRAMES / 6);
        let mid = mandelbrot_view(MANDELBROT_LOOP_FRAMES / 2);
        let deep = mandelbrot_view(MANDELBROT_LOOP_FRAMES * 4 / 5);

        assert_eq!(home.center, MANDELBROT_TARGET_CENTER);
        assert_eq!(early.center, MANDELBROT_TARGET_CENTER);
        assert_eq!(mid.center, MANDELBROT_TARGET_CENTER);
        assert_eq!(deep.center, MANDELBROT_TARGET_CENTER);
        assert!(home.multiplier.im.abs() < f64::EPSILON);
        assert!(early.multiplier.im.abs() < f64::EPSILON);
        assert!(mid.multiplier.im.abs() < f64::EPSILON);
        assert!(deep.multiplier.im.abs() < f64::EPSILON);
    }

    // Regression: the dive should keep changing near maximum zoom instead of freezing before the portal takes over.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_dive_has_no_hold_plateau_before_recursion() {
        let late_dive = mandelbrot_view((MANDELBROT_LOOP_FRAMES as f64 * 0.76).round() as usize);
        let later_dive = mandelbrot_view((MANDELBROT_LOOP_FRAMES as f64 * 0.80).round() as usize);
        let final_dive = mandelbrot_view((MANDELBROT_LOOP_FRAMES as f64 * 0.84).round() as usize);
        let still_diving = mandelbrot_view((MANDELBROT_LOOP_FRAMES as f64 * 0.90).round() as usize);

        assert!(later_dive.zoom > late_dive.zoom);
        assert!(final_dive.zoom > later_dive.zoom);
        assert!(still_diving.zoom > final_dive.zoom);
    }

    // Regression: the endless standalone animation should remain populated after multiple phase repeats.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_later_cycles_stay_populated() {
        for frame_index in [
            MANDELBROT_LOOP_FRAMES * 2 + MANDELBROT_LOOP_FRAMES / 4,
            MANDELBROT_LOOP_FRAMES * 2 + MANDELBROT_LOOP_FRAMES / 2,
            MANDELBROT_LOOP_FRAMES * 2 + MANDELBROT_LOOP_FRAMES * 3 / 4,
        ] {
            let visible = strip_ansi_from_frame(render_test_frame(context(64, 20), frame_index));
            let structural_fraction =
                visible_glyph_fraction(&visible, |cell| matches!(cell, '░' | '▒' | '▓' | '█'));

            assert!(
                structural_fraction >= 0.04,
                "frame {frame_index} is too sparse: structural fraction {structural_fraction}"
            );
        }
    }

    // Regression: nearby frames should evolve smoothly rather than shimmer from unstable per-frame remapping.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_adjacent_frames_do_not_flicker_apart() {
        for frame_index in [
            MANDELBROT_LOOP_FRAMES / 8,
            MANDELBROT_LOOP_FRAMES / 3,
            MANDELBROT_LOOP_FRAMES * 2 / 3,
        ] {
            let first = strip_ansi_from_frame(render_test_frame(context(72, 22), frame_index));
            let second = strip_ansi_from_frame(render_test_frame(context(72, 22), frame_index + 1));
            let similarity = visible_frame_similarity(&first, &second);
            assert!(
                similarity >= 0.45,
                "frame {frame_index} adjacent similarity was {similarity}"
            );
        }
    }

    // Defends: narrow terminals still get a complete frame with no skipped or over-wide rows.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_renders_narrow_frames_at_exact_dimensions() {
        let visible = MandelbrotAnimation::new(context(16, 8))
            .render_frame()
            .into_iter()
            .map(|line| strip_ansi_codes(&line))
            .collect::<Vec<_>>();

        assert_eq!(visible.len(), 8);
        assert!(visible.iter().all(|line| line.chars().count() == 16));
        assert!(
            visible
                .iter()
                .any(|line| { line.chars().any(|ch| matches!(ch, '░' | '▒' | '▓' | '█')) })
        );
    }

    // Defends: core Mandelbrot math keeps known outside and inside points stable.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn mandelbrot_escape_iterations_classify_known_points() {
        let max_iterations = 64;

        assert_eq!(
            mandelbrot_escape_iterations(0.0, 0.0, max_iterations),
            max_iterations
        );
        assert!(mandelbrot_escape_iterations(2.0, 2.0, max_iterations) < 3);
    }

    // Defends: CPU work is bounded by a small deterministic iteration budget across practical terminal sizes.
    // Strength: defect=1 behavior=2 resilience=2 cost=1 uniqueness=2 total=8/10
    #[test]
    fn mandelbrot_iteration_budget_is_bounded() {
        assert_eq!(mandelbrot_max_iterations(1, 1), 48);
        assert_eq!(mandelbrot_max_iterations(120, 40), 57);
        assert_eq!(mandelbrot_max_iterations(300, 120), 96);
        assert_eq!(
            mandelbrot_max_iterations_for_zoom(64, 20, 1.0),
            MANDELBROT_MIN_ITERATIONS
        );
        assert_eq!(mandelbrot_max_iterations_for_zoom(300, 120, 1_450.0), 600);
        assert_eq!(
            mandelbrot_max_iterations_for_zoom(300, 120, 1_000_000_000_000.0),
            MANDELBROT_MAX_ITERATIONS
        );
    }
}
