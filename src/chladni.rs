use std::f64::consts::PI;
use std::time::Duration;

use crate::{HalfBlockField, RgbColor, ScreenAnimationContext, ScreenFrameProducer};

pub const CHLADNI_STYLE: &str = "chladni";
const MODES: [(usize, usize); 5] = [(1, 2), (1, 3), (2, 3), (2, 5), (3, 5)];
const MODE_FRAMES: usize = 180;

/// A visual blend of Chladni nodal modes, not a physical plate solver.
pub struct ChladniAnimation {
    context: ScreenAnimationContext,
    x_terms: Vec<[f64; 5]>,
    y_terms: Vec<[f64; 5]>,
    frame: usize,
    pixels: HalfBlockField,
}

impl ChladniAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let width = context.inner_width;
        let height = context.resolved_height.saturating_mul(2);
        // Half-block pixels assume 2:1 terminal cells. A shared scale avoids stretching;
        // the minimum keeps high modes below Nyquist even in very thin viewports.
        let scale = width.min(height).max(16) as f64;
        let mut animation = Self {
            context,
            x_terms: axis_terms(width, scale),
            y_terms: axis_terms(height, scale),
            frame: 0,
            pixels: HalfBlockField::new(width, height),
        };
        animation.rasterize();
        animation
    }

    fn rasterize(&mut self) {
        let mode = self.frame / MODE_FRAMES;
        let t = (self.frame % MODE_FRAMES) as f64 / MODE_FRAMES as f64;
        let blend = (1.0 - (PI * t).cos()) * 0.5;
        for (y, y_terms) in self.y_terms.iter().enumerate() {
            for (x, x_terms) in self.x_terms.iter().enumerate() {
                let from = mode_amplitude(x_terms, y_terms, MODES[mode]);
                let to = mode_amplitude(x_terms, y_terms, MODES[(mode + 1) % MODES.len()]);
                let value = from * (1.0 - blend) + to * blend;
                self.pixels.set(x, y, Some(nodal_color(value)));
            }
        }
    }
}

impl ScreenFrameProducer for ChladniAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.pixels.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        self.frame = (self.frame + 1) % (MODE_FRAMES * MODES.len());
        self.rasterize();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn chladni_frame_delay() -> Duration {
    Duration::from_millis(40)
}

fn axis_terms(length: usize, scale: f64) -> Vec<[f64; 5]> {
    (0..length)
        .map(|position| {
            let angle = PI * (2.0 * position as f64 + 1.0 - length as f64) / scale;
            std::array::from_fn(|mode| ((mode + 1) as f64 * angle).cos())
        })
        .collect()
}

fn mode_amplitude(x: &[f64; 5], y: &[f64; 5], (m, n): (usize, usize)) -> f64 {
    // Square-plate cosine difference: https://www.paulbourke.net/geometry/chladni/
    // Ordered distinct modes avoid the identically zero m=n field and opposite pairs.
    (x[m - 1] * y[n - 1] - x[n - 1] * y[m - 1]) * 0.5
}

fn nodal_color(value: f64) -> RgbColor {
    // Two fixed 32-shade ramps meet at the nodes; <=64 colors also bound terminal styles.
    let light = (31.0 / (1.0 + (value * 10.0).powi(2))).round() / 31.0;
    let dark = if value < 0.0 {
        [4.0, 10.0, 24.0]
    } else {
        [18.0, 6.0, 24.0]
    };
    let channel = |i: usize, peak: f64| (dark[i] + (peak - dark[i]) * light) as u8;
    RgbColor::new(channel(0, 245.0), channel(1, 228.0), channel(2, 180.0))
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
            size_class: "wide",
        }
    }

    #[test]
    fn plate_modes_preserve_nodes_symmetry_and_square_pixel_spacing() {
        let axis = axis_terms(5, 4.0);
        assert_eq!(axis.len(), 5);
        assert!((mode_amplitude(&axis[0], &axis[2], (1, 2)) + 1.0).abs() < 1e-12);
        for mode in MODES {
            for (i, x) in axis.iter().enumerate() {
                assert_eq!(mode_amplitude(x, x, mode), 0.0);
                for y in &axis {
                    let value = mode_amplitude(x, y, mode);
                    assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
                    assert!((value + mode_amplitude(y, x, mode)).abs() < 1e-12);
                    assert!((value - mode_amplitude(&axis[4 - i], y, mode)).abs() < 1e-12);
                }
            }
        }
        // One horizontal pixel and one half-cell vertical pixel use the same scale.
        let wide = axis_terms(9, 4.0);
        assert_eq!(axis[3], wide[5]);
        assert_eq!(axis_terms(1, 16.0), vec![[1.0; 5]]);
        assert!(axis_terms(0, 16.0).is_empty());
    }

    #[test]
    fn nodal_animation_stays_bounded_nonblank_and_resizes_deterministically() {
        let colors: std::collections::HashSet<_> = (-10_000..=10_000)
            .map(|i| {
                let color = nodal_color(i as f64 / 10_000.0);
                [color.red, color.green, color.blue]
            })
            .collect();
        assert!(colors.len() <= 64);
        assert_eq!(nodal_color(0.0), nodal_color(-0.0));
        assert_ne!(nodal_color(0.0), nodal_color(1.0));
        assert_ne!(nodal_color(-1.0), nodal_color(1.0));

        let mut animation = ChladniAnimation::new(context(48, 24));
        let initial = animation.render_frame();
        let mut previous_mean: Option<f64> = None;
        for frame in 0..MODE_FRAMES * MODES.len() {
            let colors: Vec<_> = animation
                .pixels
                .cells
                .iter()
                .flatten()
                .copied()
                .flatten()
                .collect();
            assert_eq!(colors.len(), 48 * 48);
            assert!(colors.iter().filter(|color| color.red > 120).count() > colors.len() / 100);
            assert!(colors.iter().filter(|color| color.red < 60).count() > colors.len() / 5);
            let mean =
                colors.iter().map(|color| color.red as f64).sum::<f64>() / colors.len() as f64;
            if let Some(previous) = previous_mean {
                assert!(
                    (mean - previous).abs() < 5.0,
                    "brightness jump at frame {frame}"
                );
            }
            previous_mean = Some(mean);
            if frame % MODE_FRAMES == MODE_FRAMES / 2 {
                assert_ne!(initial, animation.render_frame());
            }
            animation.advance_frame();
        }
        assert_eq!(animation.render_frame(), initial);

        for (width, height) in [
            (1, 1),
            (1, 24),
            (40, 12),
            (120, 40),
            (0, 24),
            (80, 0),
            (80, 24),
        ] {
            animation.resize(context(width, height));
            let mut fresh = ChladniAnimation::new(context(width, height));
            for _ in 0..3 {
                let lines = animation.render_frame();
                assert_eq!(lines, fresh.render_frame());
                assert_eq!(lines.len(), if width == 0 { 0 } else { height });
                assert!(lines.iter().all(|line| visible_line_width(line) == width));
                animation.advance_frame();
                fresh.advance_frame();
            }
        }
    }
}
