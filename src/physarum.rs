use std::f64::consts::{PI, TAU};
use std::time::Duration;

use crate::random::{seeded_index, size_seed, unit_from_seed};
use crate::{HalfBlockField, RgbColor, ScalarField, ScreenAnimationContext, ScreenFrameProducer};

pub const PHYSARUM_STYLE: &str = "physarum";
const MAX_AGENTS: usize = 6_000;
const SENSOR_ANGLE: f64 = PI / 8.0;
const DEPOSIT: f32 = 0.3;

struct Agent {
    x: f64,
    y: f64,
    heading: f64,
}

impl Agent {
    fn sense(&self, trail: &ScalarField, offset: f64, distance: f64) -> f32 {
        let angle = self.heading + offset;
        trail.sample_wrapped(
            (self.x + angle.cos() * distance).floor() as isize,
            (self.y + angle.sin() * distance).floor() as isize,
        )
    }

    fn move_forward(
        &mut self,
        width: usize,
        height: usize,
        occupied: &mut [bool],
        seed: &mut u64,
    ) -> bool {
        let wrap = |value: f64, extent: usize| {
            let extent = extent as f64;
            // rem_euclid can round a tiny negative remainder up to the extent.
            value.rem_euclid(extent).min(extent.next_down())
        };
        let x = wrap(self.x + self.heading.cos(), width);
        let y = wrap(self.y + self.heading.sin(), height);
        let next = y as usize * width + x as usize;
        let current = self.y as usize * width + self.x as usize;
        if next != current && occupied[next] {
            self.heading = unit_from_seed(seed) * TAU;
            return false;
        }
        occupied[current] = false;
        occupied[next] = true;
        self.x = x;
        self.y = y;
        true
    }
}

/// A visual approximation of chemotactic agents and their diffusing trails.
pub struct PhysarumAnimation {
    context: ScreenAnimationContext,
    agents: Vec<Agent>,
    occupied: Vec<bool>,
    seed: u64,
    trail: ScalarField,
    pixels: HalfBlockField,
}

impl PhysarumAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let width = context.inner_width;
        let height = context.resolved_height.saturating_mul(2);
        let mut seed = size_seed(width, height, 0x5048_5953_4152_554D);
        let mut trail = ScalarField::new(width, height);
        let mut occupied = vec![false; width.saturating_mul(height)];
        let agents = (0..agent_count(width, height))
            .map(|_| {
                let start = seeded_index(&mut seed, occupied.len());
                let index = (start..occupied.len())
                    .chain(0..start)
                    .find(|&index| !occupied[index])
                    .expect("population is smaller than field");
                occupied[index] = true;
                let agent = Agent {
                    x: (index % width) as f64 + 0.5,
                    y: (index / width) as f64 + 0.5,
                    heading: unit_from_seed(&mut seed) * TAU,
                };
                trail.deposit(agent.x as usize, agent.y as usize, DEPOSIT);
                agent
            })
            .collect();
        let mut animation = Self {
            context,
            agents,
            occupied,
            seed,
            trail,
            pixels: HalfBlockField::new(width, height),
        };
        animation.rasterize();
        animation
    }

    fn rasterize(&mut self) {
        self.trail
            .map_into(&mut self.pixels, |value| Some(trail_color(value)));
    }
}

impl ScreenFrameProducer for PhysarumAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.pixels.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        let width = self.context.inner_width;
        let height = self.context.resolved_height.saturating_mul(2);
        let distance = (width.min(height) as f64 / 4.0).clamp(1.0, 5.0);
        // Jones's sensor/motor model: doi:10.1162/artl.2010.16.2.16202, section 2.1.
        // Visual variant: normalized trails, terminal-scale sensors, sense then move per agent.
        // Randomized asynchronous updates and one agent per pixel prevent collapsed streams.
        for end in (1..self.agents.len()).rev() {
            self.agents.swap(end, seeded_index(&mut self.seed, end + 1));
        }
        for agent in &mut self.agents {
            let forward = agent.sense(&self.trail, 0.0, distance);
            let left = agent.sense(&self.trail, -SENSOR_ANGLE, distance);
            let right = agent.sense(&self.trail, SENSOR_ANGLE, distance);
            agent.heading =
                (agent.heading + turn(forward, left, right, &mut self.seed)).rem_euclid(TAU);
            if agent.move_forward(width, height, &mut self.occupied, &mut self.seed) {
                self.trail
                    .deposit(agent.x as usize, agent.y as usize, DEPOSIT);
            }
        }
        self.trail.blur_wrapped();
        self.trail.decay(0.9);
        self.rasterize();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn physarum_frame_delay() -> Duration {
    Duration::from_millis(40)
}

fn trail_color(value: f32) -> RgbColor {
    // Fixed exposure avoids whole-frame flashes from per-frame normalization.
    // 64 shades bound color pairs: the recording terminal's style table has 65,535 slots.
    let light = ((value * 1.5).min(1.0) * 63.0).round() / 63.0;
    RgbColor::new(
        (light * light * 230.0) as u8,
        (light * 220.0) as u8,
        (light * 165.0) as u8,
    )
}

fn agent_count(width: usize, height: usize) -> usize {
    width.saturating_mul(height).div_ceil(8).min(MAX_AGENTS)
}

fn turn(forward: f32, left: f32, right: f32, seed: &mut u64) -> f64 {
    let direction = if forward > left && forward > right {
        0.0
    } else if forward < left && forward < right {
        if unit_from_seed(seed) < 0.5 {
            -1.0
        } else {
            1.0
        }
    } else if left > right {
        -1.0
    } else if right > left {
        1.0
    } else {
        0.0
    };
    direction * PI / 4.0
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
    fn sensors_steer_toward_trails_and_wrap_across_edges() {
        let mut seed = 1;
        assert_eq!(turn(0.8, 0.2, 0.3, &mut seed), 0.0);
        assert_eq!(turn(0.5, 0.8, 0.2, &mut seed), -PI / 4.0);
        assert_eq!(turn(0.5, 0.2, 0.8, &mut seed), PI / 4.0);
        assert_eq!(turn(0.5, 0.5, 0.5, &mut seed), 0.0);
        let mut choices = [false; 2];
        for _ in 0..32 {
            let delta = turn(0.1, 0.8, 0.8, &mut seed);
            assert_eq!(delta.abs(), PI / 4.0);
            choices[usize::from(delta > 0.0)] = true;
        }
        assert_eq!(choices, [true, true]);
        let colors: std::collections::HashSet<_> = (0..=10_000)
            .map(|i| {
                let color = trail_color(i as f32 / 10_000.0);
                [color.red, color.green, color.blue]
            })
            .collect();
        assert!(colors.len() <= 64);

        let mut trail = ScalarField::new(10, 10);
        trail.deposit(0, 5, 0.75);
        let mut agent = Agent {
            x: 9.5,
            y: 5.0,
            heading: 0.0,
        };
        assert_eq!(agent.sense(&trail, 0.0, 1.0), 0.75);
        let mut occupied = vec![false; 100];
        occupied[59] = true;
        assert!(agent.move_forward(10, 10, &mut occupied, &mut seed));
        assert!((agent.x - 0.5).abs() < 1e-9);
        assert!(occupied[50] && !occupied[59]);
        occupied[51] = true;
        assert!(!agent.move_forward(10, 10, &mut occupied, &mut seed));
        assert!((agent.x - 0.5).abs() < 1e-9);
        agent.heading = PI;
        trail.clear();
        trail.deposit(9, 5, 0.5);
        assert_eq!(agent.sense(&trail, 0.0, 1.0), 0.5);

        // A tiny negative remainder can round up to the divisor on either axis.
        for (x, y, heading) in [
            (1.0_f64.next_down(), 9.5, PI),
            (9.5, 1.0_f64.next_down(), 3.0 * PI / 2.0),
        ] {
            let mut agent = Agent { x, y, heading };
            let mut occupied = vec![false; 100];
            occupied[y as usize * 10 + x as usize] = true;
            assert!(agent.move_forward(10, 10, &mut occupied, &mut seed));
            assert!((0.0..10.0).contains(&agent.x) && (0.0..10.0).contains(&agent.y));
            assert!(occupied[99]);
            assert_eq!(occupied.iter().filter(|&&v| v).count(), 1);
        }
    }

    #[test]
    fn trails_stay_deterministic_nonuniform_and_resize_safely() {
        assert_eq!(agent_count(usize::MAX, usize::MAX), MAX_AGENTS);
        let mut first = PhysarumAnimation::new(context(80, 24));
        let mut second = PhysarumAnimation::new(context(80, 24));
        let initial = first.render_frame();
        assert_eq!(initial, second.render_frame());
        assert!(first.agents.iter().any(|agent| {
            first
                .trail
                .sample(agent.x as usize, agent.y as usize)
                .unwrap()
                > 0.0
        }));

        for frame in 0..750 {
            first.advance_frame();
            second.advance_frame();
            if [74, 249, 749].contains(&frame) {
                assert_eq!(first.render_frame(), second.render_frame());
                let mut seen = vec![false; 80 * 48];
                for agent in &first.agents {
                    let index = agent.y as usize * 80 + agent.x as usize;
                    assert!(!seen[index]);
                    seen[index] = true;
                }
                assert_eq!(seen, first.occupied);
                let samples: Vec<_> = (0..48)
                    .flat_map(|y| {
                        let trail = &first.trail;
                        (0..80).map(move |x| trail.sample(x, y).unwrap())
                    })
                    .collect();
                assert!(
                    samples
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                );
                let low = samples.iter().filter(|&&v| v < 0.1).count();
                let high = samples.iter().filter(|&&v| v > 0.3).count();
                assert!(
                    low > samples.len() / 10 && high > samples.len() / 20,
                    "frame {frame}: low={low}, high={high}"
                );
            }
        }
        assert_ne!(initial, first.render_frame());
        let previous = first.render_frame();
        first.advance_frame();
        assert_ne!(previous, first.render_frame());
        assert!(first.agents.iter().all(|agent| {
            (0.0..80.0).contains(&agent.x)
                && (0.0..48.0).contains(&agent.y)
                && (0.0..TAU).contains(&agent.heading)
        }));

        for (width, height) in [(120, 40), (1, 1), (2, 1), (0, 0), (40, 12)] {
            first.resize(context(width, height));
            let mut fresh = PhysarumAnimation::new(context(width, height));
            assert_eq!(first.render_frame(), fresh.render_frame());
            first.advance_frame();
            fresh.advance_frame();
            let lines = first.render_frame();
            assert_eq!(lines, fresh.render_frame());
            assert_eq!(lines.len(), height);
            assert!(lines.iter().all(|line| visible_line_width(line) == width));
            assert!(first.agents.len() <= MAX_AGENTS);
            if width > 0 && height > 0 {
                for _ in 0..100 {
                    first.advance_frame();
                }
                assert!(first.agents.iter().any(|agent| {
                    first
                        .trail
                        .sample(agent.x as usize, agent.y as usize)
                        .unwrap()
                        > 0.01
                }));
            }
        }
    }
}
