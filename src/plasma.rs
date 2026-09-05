use std::f64::consts::TAU;
use std::time::Duration;

use crate::{HalfBlockField, RgbColor, ScreenAnimationContext, ScreenFrameProducer};

pub const PLASMA_STYLE: &str = "plasma";
const CYCLE_FRAMES: usize = 1200;

/// Moving sine interference bands with a circular color palette.
pub struct PlasmaAnimation {
    context: ScreenAnimationContext,
    spatial: Vec<[f64; 4]>,
    palette: [RgbColor; 64],
    frame: usize,
    pixels: HalfBlockField,
}

impl PlasmaAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let width = context.inner_width;
        let height = context.resolved_height.saturating_mul(2);
        // Half-block pixels assume 2:1 terminal cells. Use one scale on both axes;
        // the floor keeps the waves broad even in thin or tiny viewports.
        let scale = width.min(height).max(16) as f64;
        let mut spatial = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            let y = TAU * (y as f64 + 0.5 - height as f64 * 0.5) / scale;
            for x in 0..width {
                let x = TAU * (x as f64 + 0.5 - width as f64 * 0.5) / scale;
                spatial.push([
                    1.1 * x,
                    1.3 * y,
                    0.8 * (x + y),
                    1.6 * (x - 1.5).hypot(y + 0.8),
                ]);
            }
        }
        // A fixed circular palette bounds terminal color-pair growth across the loop.
        let palette = std::array::from_fn(|index| {
            let angle = TAU * index as f64 / 64.0;
            let channel = |offset: f64| (128.0 + 104.0 * (angle + offset).cos()).round() as u8;
            RgbColor::new(channel(0.0), channel(TAU / 3.0), channel(2.0 * TAU / 3.0))
        });
        let mut animation = Self {
            context,
            spatial,
            palette,
            frame: 0,
            pixels: HalfBlockField::new(width, height),
        };
        animation.rasterize();
        animation
    }

    fn rasterize(&mut self) {
        let phase = TAU * self.frame as f64 / CYCLE_FRAMES as f64;
        for (index, spatial) in self.spatial.iter().enumerate() {
            let position = (1.5 * wave(spatial, phase) + phase / TAU).fract();
            let color = self.palette[(position * self.palette.len() as f64) as usize];
            self.pixels.set(
                index % self.context.inner_width,
                index / self.context.inner_width,
                Some(color),
            );
        }
    }
}

impl ScreenFrameProducer for PlasmaAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.pixels.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        self.frame = (self.frame + 1) % CYCLE_FRAMES;
        self.rasterize();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn plasma_frame_delay() -> Duration {
    Duration::from_millis(40)
}

fn wave(spatial: &[f64; 4], phase: f64) -> f64 {
    // Classic sine-sum technique: https://lodev.org/cgtutor/plasma.html
    // Local frequencies and integer phase speeds give a continuous bounded loop.
    0.5 + 0.125
        * ((spatial[0] + 3.0 * phase).sin()
            + (spatial[1] - 2.0 * phase).sin()
            + (spatial[2] + 2.0 * phase).sin()
            + (spatial[3] - 3.0 * phase).sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visible_line_width;
    use std::f64::consts::FRAC_PI_2;

    fn context(width: usize, height: usize) -> ScreenAnimationContext {
        ScreenAnimationContext {
            resolved_width: width,
            resolved_height: height,
            inner_width: width,
            size_class: "wide",
        }
    }

    #[test]
    fn sine_field_is_normalized_and_periodic() {
        assert_eq!(wave(&[0.0; 4], 0.0), 0.5);
        assert_eq!(wave(&[FRAC_PI_2; 4], 0.0), 1.0);
        assert_eq!(wave(&[-FRAC_PI_2; 4], 0.0), 0.0);
        for step in 0..=1200 {
            let phase = TAU * step as f64 / 1200.0;
            for spatial in [[0.0; 4], [1.2, -3.5, 7.1, 2.4]] {
                let value = wave(&spatial, phase);
                assert!(value.is_finite() && (0.0..=1.0).contains(&value));
                assert!((value - wave(&spatial, phase + TAU)).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn plasma_moves_without_flashes_and_resizes_deterministically() {
        let mut animation = PlasmaAnimation::new(context(48, 24));
        let initial = animation.render_frame();
        let dx = (animation.spatial[1][0] - animation.spatial[0][0]) / 1.1;
        let dy = (animation.spatial[48][1] - animation.spatial[0][1]) / 1.3;
        assert!((dx - dy).abs() < 1e-12);
        let mut previous = animation.pixels.clone();
        let mut colors = std::collections::HashSet::new();
        for frame in 1..=CYCLE_FRAMES {
            animation.advance_frame();
            let mut frame_colors = std::collections::HashSet::new();
            for (cell, old) in animation.pixels.cells.iter().zip(&previous.cells) {
                for (color, old) in cell.iter().zip(old) {
                    let color = color.unwrap();
                    let old = old.unwrap();
                    let channels = [color.red, color.green, color.blue];
                    frame_colors.insert(channels);
                    assert!(channels.iter().all(|channel| *channel >= 20));
                    for (current, old) in channels.into_iter().zip([old.red, old.green, old.blue]) {
                        assert!(current.abs_diff(old) <= 24, "color jump at frame {frame}");
                    }
                }
            }
            assert!(frame_colors.len() >= 24, "flat field at frame {frame}");
            colors.extend(frame_colors);
            previous.clone_from(&animation.pixels);
            if frame % 75 == 0 && frame < CYCLE_FRAMES {
                assert_ne!(animation.render_frame(), initial);
            }
        }
        assert!(colors.len() <= 64);
        assert_eq!(animation.render_frame(), initial);
        for (width, height) in [(1, 1), (1, 24), (40, 12), (200, 60), (0, 24), (80, 0)] {
            let mut resized = context(width, height);
            resized.resolved_width += 6; // Library callers may request horizontal padding.
            animation.resize(resized);
            let mut fresh = PlasmaAnimation::new(resized);
            for _ in 0..3 {
                let lines = animation.render_frame();
                assert_eq!(lines, fresh.render_frame());
                assert_eq!(lines.len(), if width == 0 { 0 } else { height });
                assert!(
                    lines
                        .iter()
                        .all(|line| visible_line_width(line) == width + 6)
                );
                animation.advance_frame();
                fresh.advance_frame();
            }
        }
    }
}
