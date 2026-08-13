use crate::random::{seeded_index, size_seed, unit_from_seed};
use crate::{HalfBlockField, RgbColor, ScreenAnimationContext, ScreenFrameProducer};

pub const FRIENDS_AND_ENEMIES_STYLE: &str = "friends_and_enemies";

const MAX_PARTICLES: usize = 1_600;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FriendsAndEnemiesAnimation {
    context: ScreenAnimationContext,
    positions: Vec<Point>,
    next_positions: Vec<Point>,
    relations: Vec<(usize, usize)>,
    random_seed: u64,
    density: Vec<u8>,
    field: HalfBlockField,
}

impl FriendsAndEnemiesAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let width = context.inner_width;
        let pixel_height = context.resolved_height.saturating_mul(2);
        let count = particle_count(width, context.resolved_height);
        let mut random_seed = size_seed(width, pixel_height, 0x4652_4945_4E44);
        let aspect = width.max(1) as f64 / pixel_height.max(1) as f64;
        let positions: Vec<_> = (0..count)
            .map(|_| Point {
                x: (unit_from_seed(&mut random_seed) * 2.0 - 1.0) * aspect,
                y: unit_from_seed(&mut random_seed) * 2.0 - 1.0,
            })
            .collect();
        let mut relations = vec![(0, 0); count];
        for (friend, _) in &mut relations {
            *friend = seeded_index(&mut random_seed, count);
        }
        for (_, enemy) in &mut relations {
            *enemy = seeded_index(&mut random_seed, count);
        }
        let mut animation = Self {
            context,
            next_positions: positions.clone(),
            positions,
            relations,
            random_seed,
            density: vec![0; width.saturating_mul(pixel_height)],
            field: HalfBlockField::new(width, pixel_height),
        };
        animation.rasterize();
        animation
    }

    fn rasterize(&mut self) {
        let width = self.context.inner_width;
        let pixel_height = self.context.resolved_height.saturating_mul(2);
        let view_half_height = view_half_height(&self.positions, width, pixel_height);
        self.density.fill(0);
        self.field.clear();
        for &point in &self.positions {
            if let Some(index) = sample_index(point, width, pixel_height, view_half_height) {
                let density = self.density[index].saturating_add(1);
                self.density[index] = density;
                self.field
                    .set(index % width, index / width, Some(density_color(density)));
            }
        }
    }
}

impl ScreenFrameProducer for FriendsAndEnemiesAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.field.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        // The source rewires periodically; at terminal cadence, a 75% gate prevents collapse.
        // Source: https://community.wolfram.com/groups/-/m/t/122095
        if seeded_index(&mut self.random_seed, 4) != 0 {
            let particle = seeded_index(&mut self.random_seed, self.positions.len());
            let friend = seeded_index(&mut self.random_seed, self.positions.len());
            let enemy = seeded_index(&mut self.random_seed, self.positions.len());
            self.relations[particle] = (friend, enemy);
        }
        for (index, next) in self.next_positions.iter_mut().enumerate() {
            *next = next_position(index, &self.positions, &self.relations);
        }
        std::mem::swap(&mut self.positions, &mut self.next_positions);
        self.rasterize();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

fn particle_count(width: usize, height: usize) -> usize {
    let area = width.saturating_mul(height);
    area.div_ceil(2).clamp(500, MAX_PARTICLES)
}

fn next_position(index: usize, positions: &[Point], relations: &[(usize, usize)]) -> Point {
    let point = positions[index];
    let (friend, enemy) = relations[index];
    let friend = normalized_offset(point, positions[friend]);
    let enemy = normalized_offset(point, positions[enemy]);
    Point {
        x: point.x * 0.995 + friend.x * 0.02 - enemy.x * 0.01,
        y: point.y * 0.995 + friend.y * 0.02 - enemy.y * 0.01,
    }
}

fn normalized_offset(from: Point, to: Point) -> Point {
    let x = to.x - from.x;
    let y = to.y - from.y;
    let scale = 1.0 / (0.01 + (x * x + y * y).sqrt());
    Point {
        x: x * scale,
        y: y * scale,
    }
}

fn view_half_height(positions: &[Point], width: usize, pixel_height: usize) -> f64 {
    let aspect = width.max(1) as f64 / pixel_height.max(1) as f64;
    let mean_square_radius = positions
        .iter()
        .map(|point| (point.x / aspect).powi(2) + point.y.powi(2))
        .sum::<f64>()
        / positions.len() as f64;
    (mean_square_radius.sqrt() * 2.25).clamp(0.2, 2.0)
}

fn sample_index(point: Point, width: usize, pixel_height: usize, half_y: f64) -> Option<usize> {
    if width == 0 || pixel_height == 0 {
        return None;
    }
    let half_x = half_y * width as f64 / pixel_height as f64;
    let x = (point.x / half_x + 1.0) * 0.5 * width as f64;
    let y = (point.y / half_y + 1.0) * 0.5 * pixel_height as f64;
    if x < 0.0 || x >= width as f64 || y < 0.0 || y >= pixel_height as f64 {
        None
    } else {
        Some(y as usize * width + x as usize)
    }
}

fn density_color(density: u8) -> RgbColor {
    match density {
        1 => RgbColor::new(0, 150, 145),
        2 => RgbColor::new(0, 220, 205),
        3 => RgbColor::new(110, 245, 225),
        _ => RgbColor::new(240, 255, 250),
    }
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
    fn friends_and_enemies_contract() {
        assert_eq!(particle_count(usize::MAX, usize::MAX), MAX_PARTICLES);

        let mut first = FriendsAndEnemiesAnimation::new(context(80, 24));
        let mut second = FriendsAndEnemiesAnimation::new(context(80, 24));
        assert_eq!(first, second);
        assert!(first.positions.len() >= 500);
        assert!(first.relations.iter().all(
            |&(friend, enemy)| friend < first.positions.len() && enemy < first.positions.len()
        ));

        let occupied = first.density.iter().filter(|&&value| value > 0).count();
        assert!(occupied > 300);
        assert!(first.density.iter().any(|&value| value > 1));
        assert_ne!(density_color(1), density_color(2));
        let frame = first.render_frame();
        assert_eq!(frame.len(), 24);
        assert!(frame.iter().all(|line| visible_line_width(line) == 80));
        assert!(
            frame
                .iter()
                .map(|line| line.matches('▀').count() + line.matches('▄').count())
                .sum::<usize>()
                > 250
        );

        let original_relations = first.relations.clone();
        for _ in 0..600 {
            first.advance_frame();
            second.advance_frame();
        }
        assert_eq!(first, second);
        assert_ne!(first.relations, original_relations);
        assert!(first.density.iter().filter(|&&value| value > 0).count() > 150);
        assert!(
            first
                .positions
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );

        first.resize(context(120, 40));
        assert_eq!(first, FriendsAndEnemiesAnimation::new(context(120, 40)));
        assert!(first.positions.len() >= 1_000);
    }

    #[test]
    fn documented_update_is_simultaneous_and_order_independent() {
        let positions = [
            Point { x: 1.0, y: 2.0 },
            Point { x: 4.0, y: 6.0 },
            Point { x: -2.0, y: 2.0 },
        ];
        let relations = [(1, 2), (2, 0), (0, 1)];
        let mut forward = positions;
        let mut reverse = positions;

        for (index, next) in forward.iter_mut().enumerate() {
            *next = next_position(index, &positions, &relations);
        }
        for index in (0..reverse.len()).rev() {
            reverse[index] = next_position(index, &positions, &relations);
        }

        assert_eq!(forward, reverse);
        let expected = Point {
            x: 0.995 + 0.02 * (3.0 / 5.01) + 0.01 * (3.0 / 3.01),
            y: 1.99 + 0.02 * (4.0 / 5.01),
        };
        assert!((forward[0].x - expected.x).abs() < 1e-12);
        assert!((forward[0].y - expected.y).abs() < 1e-12);
    }
}
