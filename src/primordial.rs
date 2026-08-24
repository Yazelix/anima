use std::cmp::Ordering;
use std::f64::consts::{PI, TAU};
use std::time::Duration;

use crate::random::{seeded_index, size_seed, unit_from_seed};
use crate::{HalfBlockField, RgbColor, ScreenAnimationContext, ScreenFrameProducer};

pub const PRIMORDIAL_STYLE: &str = "primordial";

const MAX_PARTICLES: usize = 1_200;
const INTERACTION_RADIUS_SQUARED: f64 = 25.0;
const SPEED: f64 = 0.67;
const BETA: f64 = 17.0 * PI / 180.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Particle {
    x: f64,
    y: f64,
    heading: f64,
    neighbors: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Neighborhood {
    left: usize,
    right: usize,
}

impl Neighborhood {
    fn total(self) -> usize {
        self.left + self.right
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimordialAnimation {
    context: ScreenAnimationContext,
    particles: Vec<Particle>,
    order: Vec<usize>,
    random_seed: u64,
    field: HalfBlockField,
}

impl PrimordialAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let pixel_height = context.resolved_height.saturating_mul(2);
        let world_width = context.inner_width.max(1) as f64;
        let world_height = pixel_height.max(1) as f64;
        let count = particle_count(context.inner_width, pixel_height);
        let mut random_seed = size_seed(context.inner_width, pixel_height, 0x5052_494D_4F52_4449);
        let mut particles: Vec<_> = (0..count)
            .map(|_| Particle {
                x: unit_from_seed(&mut random_seed) * world_width,
                y: unit_from_seed(&mut random_seed) * world_height,
                heading: unit_from_seed(&mut random_seed) * TAU,
                neighbors: 0,
            })
            .collect();
        for index in 0..count {
            particles[index].neighbors =
                neighborhood(&particles, index, world_width, world_height).total();
        }
        let mut animation = Self {
            context,
            particles,
            order: (0..count).collect(),
            random_seed,
            field: HalfBlockField::new(context.inner_width, pixel_height),
        };
        animation.rasterize();
        animation
    }

    fn world_width(&self) -> f64 {
        self.context.inner_width.max(1) as f64
    }

    fn world_height(&self) -> f64 {
        self.context.resolved_height.saturating_mul(2).max(1) as f64
    }

    fn rasterize(&mut self) {
        let width = self.context.inner_width;
        let pixel_height = self.context.resolved_height.saturating_mul(2);
        self.field.clear();
        if width == 0 || pixel_height == 0 {
            return;
        }
        for particle in &self.particles {
            self.field.set(
                particle.x as usize,
                particle.y as usize,
                Some(particle_color(particle.neighbors)),
            );
        }
    }
}

impl ScreenFrameProducer for PrimordialAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.field.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        shuffle(&mut self.order, &mut self.random_seed);
        let world_width = self.world_width();
        let world_height = self.world_height();

        // Source: https://www.nature.com/articles/srep37969
        // ponytail: O(n²) scans are capped at 1,200 particles; add a spatial grid only if cadence misses.
        for &index in &self.order {
            let sensed = neighborhood(&self.particles, index, world_width, world_height);
            let particle = &mut self.particles[index];
            particle.neighbors = sensed.total();
            particle.heading =
                (particle.heading + turn_delta(sensed.left, sensed.right)).rem_euclid(TAU);
            particle.x = (particle.x + particle.heading.cos() * SPEED).rem_euclid(world_width);
            particle.y = (particle.y + particle.heading.sin() * SPEED).rem_euclid(world_height);
        }
        self.rasterize();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn primordial_frame_delay() -> Duration {
    Duration::from_millis(40)
}

fn particle_count(width: usize, pixel_height: usize) -> usize {
    width
        .saturating_mul(pixel_height)
        .div_ceil(12)
        .clamp(64, MAX_PARTICLES)
}

fn shuffle(order: &mut [usize], seed: &mut u64) {
    for end in (1..order.len()).rev() {
        order.swap(end, seeded_index(seed, end + 1));
    }
}

fn neighborhood(
    particles: &[Particle],
    index: usize,
    world_width: f64,
    world_height: f64,
) -> Neighborhood {
    let particle = particles[index];
    let heading_x = particle.heading.cos();
    let heading_y = particle.heading.sin();
    let mut result = Neighborhood::default();

    for (other_index, other) in particles.iter().enumerate() {
        if other_index == index {
            continue;
        }
        let dx = wrapped_offset(other.x - particle.x, world_width);
        let dy = wrapped_offset(other.y - particle.y, world_height);
        if dx * dx + dy * dy > INTERACTION_RADIUS_SQUARED {
            continue;
        }
        if heading_x * dy - heading_y * dx >= 0.0 {
            result.right += 1;
        } else {
            result.left += 1;
        }
    }
    result
}

fn wrapped_offset(offset: f64, extent: f64) -> f64 {
    offset - (offset / extent).round() * extent
}

fn turn_delta(left: usize, right: usize) -> f64 {
    let direction = match right.cmp(&left) {
        Ordering::Less => -1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => 1.0,
    };
    PI + BETA * (left + right) as f64 * direction
}

fn particle_color(neighbors: usize) -> RgbColor {
    match neighbors {
        0..=5 => RgbColor::new(0, 120, 95),
        6..=12 => RgbColor::new(0, 220, 175),
        13..=15 => RgbColor::new(210, 95, 55),
        16..=35 => RgbColor::new(30, 150, 255),
        _ => RgbColor::new(255, 225, 80),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScreenAnimationContext, ScreenFrameProducer, visible_line_width};

    fn context(width: usize, height: usize) -> ScreenAnimationContext {
        ScreenAnimationContext {
            resolved_width: width,
            resolved_height: height,
            inner_width: width,
            size_class: "wide",
        }
    }

    #[test]
    fn documented_turn_and_neighborhood_match_the_pps_model() {
        assert!((turn_delta(0, 0) - PI).abs() < 1e-12);
        assert!((turn_delta(2, 5) - (180.0_f64 + 17.0 * 7.0).to_radians()).abs() < 1e-12);
        assert!((turn_delta(5, 2) - (180.0_f64 - 17.0 * 7.0).to_radians()).abs() < 1e-12);
        assert!((turn_delta(4, 4) - PI).abs() < 1e-12);
        assert_eq!(wrapped_offset(9.0, 10.0), -1.0);
        assert_eq!(wrapped_offset(-9.0, 10.0), 1.0);

        let particle = |x, y| Particle {
            x,
            y,
            heading: 0.0,
            neighbors: 0,
        };
        let particles = [
            particle(9.0, 5.0),
            particle(1.0, 4.0),
            particle(1.0, 6.0),
            particle(8.0, 6.0),
        ];
        assert_eq!(
            neighborhood(&particles, 0, 10.0, 10.0),
            Neighborhood { left: 1, right: 2 }
        );
    }

    #[test]
    fn primordial_animation_is_bounded_deterministic_dense_and_resize_safe() {
        assert_eq!(particle_count(usize::MAX, usize::MAX), MAX_PARTICLES);

        let mut first = PrimordialAnimation::new(context(80, 24));
        let mut second = PrimordialAnimation::new(context(80, 24));
        assert_eq!(first.particles.len(), 320);
        assert_eq!(first, second);

        let dense_particle_count = |animation: &PrimordialAnimation| {
            animation
                .particles
                .iter()
                .filter(|particle| particle.neighbors >= 13)
                .count()
        };
        let initial_dense = dense_particle_count(&first);
        let frame = first.render_frame();
        assert_eq!(frame.len(), 24);
        assert!(frame.iter().all(|line| visible_line_width(line) == 80));
        assert!(
            frame
                .iter()
                .map(|line| line.matches('▀').count() + line.matches('▄').count())
                .sum::<usize>()
                > 150
        );

        let mut dense_checkpoints = [0; 3];
        for frame in 1..=750 {
            first.advance_frame();
            second.advance_frame();
            if let Some(checkpoint) = [75, 250, 750].iter().position(|&value| value == frame) {
                dense_checkpoints[checkpoint] = dense_particle_count(&first);
            }
        }

        assert_eq!(first, second);
        assert!(first.particles.iter().all(|particle| {
            particle.x.is_finite()
                && particle.y.is_finite()
                && particle.heading.is_finite()
                && (0.0..first.world_width()).contains(&particle.x)
                && (0.0..first.world_height()).contains(&particle.y)
        }));
        assert!(
            dense_checkpoints.iter().all(|&dense| dense > initial_dense),
            "dense particle counts at frames 75, 250, and 750: {dense_checkpoints:?}"
        );

        first.resize(context(120, 40));
        assert_eq!(first, PrimordialAnimation::new(context(120, 40)));
        assert_eq!(first.particles.len(), 800);
    }
}
