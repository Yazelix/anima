use std::{f64::consts::TAU, time::Duration};

use crate::random::{size_seed, unit_from_seed};
use crate::{HalfBlockField, RgbColor, ScreenAnimationContext, ScreenFrameProducer};

pub const AQUARIUM_STYLE: &str = "aquarium";

// Original pixel silhouettes and palette, drawn for Anima. No external sprites.
const MINNOW: &[&[u8]] = &[b"  hh ", b"fbbbe", b"  ss "];
const REEF: &[&[u8]] = &[
    b"     ff     ",
    b"   hhbbssb  ",
    b"ffhbbbssbeb ",
    b" fbbbbbssbb ",
    b"    sssf    ",
];
const ANGEL: &[&[u8]] = &[
    b"     f   ",
    b"    fh   ",
    b"   fhhb  ",
    b"  hbbssb ",
    b"ffbbbsebb",
    b" fbbbsbb ",
    b"  bbssb  ",
    b"   bsf   ",
    b"    f    ",
    b"    f    ",
];
// Body, highlight, stripe, fin. A fixed palette also bounds terminal style caches.
const FISH_COLORS: [[[u8; 3]; 4]; 4] = [
    [
        [232, 137, 66],
        [255, 211, 134],
        [125, 62, 54],
        [187, 83, 57],
    ],
    [
        [67, 190, 191],
        [160, 238, 216],
        [31, 103, 140],
        [44, 137, 172],
    ],
    [
        [226, 193, 85],
        [255, 237, 163],
        [96, 93, 62],
        [196, 138, 63],
    ],
    [
        [183, 126, 192],
        [241, 191, 218],
        [97, 71, 131],
        [144, 83, 154],
    ],
];

#[derive(Clone, Copy)]
enum FishKind {
    Minnow,
    Reef,
    Angel,
}

impl FishKind {
    fn art(self) -> &'static [&'static [u8]] {
        match self {
            Self::Minnow => MINNOW,
            Self::Reef => REEF,
            Self::Angel => ANGEL,
        }
    }
}

struct Fish {
    x: f64,
    y: f64,
    speed: f64,
    phase: f64,
    kind: FishKind,
    color: usize,
    depth: usize,
}

struct Bubble {
    x: f64,
    y: f64,
    speed: f64,
    large: bool,
}

struct Plant {
    x: f64,
    height: usize,
    lean: f64,
    phase: f64,
    front: bool,
    coral: bool,
}

pub struct AquariumAnimation {
    context: ScreenAnimationContext,
    pixels: HalfBlockField,
    fish: Vec<Fish>,
    bubbles: Vec<Bubble>,
    plants: Vec<Plant>,
    phase: f64,
}

impl AquariumAnimation {
    pub fn new(context: ScreenAnimationContext) -> Self {
        let width = context.inner_width;
        let height = context.resolved_height.saturating_mul(2);
        let area = width.saturating_mul(height);
        let mut seed = size_seed(width, height, 0x5245_4546);
        let mut random = || unit_from_seed(&mut seed);
        let fish = (0..if area == 0 {
            0
        } else {
            (area / 360).clamp(6, 36)
        })
            .map(|i| {
                let school = i < if area < 4_000 { 3 } else { 6 };
                Fish {
                    x: if school {
                        (width as f64 * 0.2 + (i / 2) as f64 * 7.0 + (i % 2) as f64 * 2.0)
                            % width as f64
                    } else {
                        ((i as f64 * 0.618 + random() * 0.09) % 1.0) * width as f64
                    },
                    y: if school {
                        height as f64 * 0.23 + (i % 2) as f64 * 4.0
                    } else {
                        height as f64 * (0.27 + ((i * 3) % 5) as f64 * 0.11 + random() * 0.04)
                    },
                    speed: if school {
                        0.22
                    } else {
                        (0.07 + random() * 0.10) * if i % 2 == 0 { 1.0 } else { -1.0 }
                    },
                    phase: if school { 0.0 } else { random() * TAU },
                    kind: if school || height < 16 || width < 30 {
                        FishKind::Minnow
                    } else if i % 3 == 0 {
                        FishKind::Angel
                    } else {
                        FishKind::Reef
                    },
                    color: if school { 1 } else { i % FISH_COLORS.len() },
                    depth: if school { 0 } else { 1 + i % 2 },
                }
            })
            .collect();
        let bubbles = (0..if area == 0 {
            0
        } else {
            (area / 220).clamp(4, 64)
        })
            .map(|i| Bubble {
                x: width as f64 * if i % 2 == 0 { 0.13 } else { 0.81 } + random() * 3.0,
                y: random() * height as f64,
                speed: 0.13 + random() * 0.16,
                large: i % 4 == 0,
            })
            .collect();
        let plants = (0..if area == 0 {
            0
        } else {
            (width / 7).clamp(4, 32)
        })
            .map(|i| Plant {
                x: width as f64
                    * if i % 2 == 0 {
                        random() * 0.30
                    } else {
                        0.70 + random() * 0.30
                    },
                height: ((height as f64 * (0.18 + random() * 0.35)) as usize).clamp(2, 42),
                lean: random() * 0.35 - 0.175,
                phase: random() * TAU,
                front: i % 3 == 0,
                coral: i % 4 == 0,
            })
            .collect();
        let mut scene = Self {
            pixels: HalfBlockField::new(width, height),
            context,
            fish,
            bubbles,
            plants,
            phase: 0.0,
        };
        scene.paint();
        scene
    }

    fn paint(&mut self) {
        let width = self.context.inner_width;
        let height = self.context.resolved_height.saturating_mul(2);
        if width == 0 || height == 0 {
            return;
        }
        let field = &mut self.pixels;
        for y in 0..height {
            let shade = (y * 16 / height).min(15) as u8;
            for x in 0..width {
                pixel(
                    field,
                    x as i32,
                    y as i32,
                    [5 + shade / 3, 17 + shade, 27 + shade],
                );
            }
        }
        // Soft broken surface ripples and a few suspended points, not a solid border.
        for x in 0..width {
            if (x + (self.phase / TAU * 26.0) as usize) % 13 < 6 {
                let y = 1 + ((x as f64 * 0.13 + self.phase * 2.0).sin() + 1.0) as i32;
                pixel(field, x as i32, y, [20, 60, 71]);
            }
            if x % 7 == 0 {
                let y = ((x * 17 + 9) % height) as i32;
                pixel(field, x as i32, y, [21, 48, 56]);
            }
        }
        for fish in self.fish.iter().filter(|fish| fish.depth == 0) {
            paint_fish(field, fish, self.phase);
        }
        for plant in self.plants.iter().filter(|plant| !plant.front) {
            paint_plant(field, plant, height, self.phase);
        }
        // A ray crosses the distant open water, then spends the rest of its loop offscreen.
        if width >= 60 && height >= 30 {
            let x = ((self.phase / TAU + 0.15) % 1.0) * (width as f64 * 3.0 + 70.0) - 45.0;
            paint_ray(field, x as i32, (height as f64 * 0.43) as i32, self.phase);
        }
        for bubble in &self.bubbles {
            let x = (bubble.x + (self.phase * 5.0 + bubble.y * 0.22).sin()) as i32;
            let y = bubble.y as i32;
            if bubble.large {
                for (dx, dy) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
                    pixel(field, x + dx, y + dy, [49, 99, 111]);
                }
                pixel(field, x, y - 1, [143, 197, 195]);
            } else {
                pixel(field, x, y, [69, 119, 129]);
            }
        }
        for fish in self.fish.iter().filter(|fish| fish.depth == 1) {
            paint_fish(field, fish, self.phase);
        }
        for x in 0..width {
            let rise = 2 + ((x as f64 * 0.11).sin() + 1.0) as usize;
            for y in height.saturating_sub(rise)..height {
                let color = match (x * 13 + y * 7) % 11 {
                    0 | 1 => [101, 105, 77],
                    2..=4 => [63, 78, 66],
                    _ => [35, 53, 52],
                };
                pixel(field, x as i32, y as i32, color);
            }
        }
        // Two low rock outcrops anchor the plants and leave an open central channel.
        for (fraction, radius) in [(0.18, 0.11), (0.83, 0.14)] {
            let rx = (width as f64 * radius).clamp(2.0, 16.0) as i32;
            let ry = (height as f64 * 0.09).clamp(1.0, 7.0) as i32;
            for dy in -ry..=0 {
                for dx in -rx..=rx {
                    if (dx * dx) as f64 / (rx * rx) as f64 + (dy * dy) as f64 / (ry * ry) as f64
                        <= 1.0
                    {
                        let color = if dy < -ry / 2 && (dx - dy) % 3 != 0 {
                            [67, 94, 88]
                        } else {
                            [30, 58, 61]
                        };
                        pixel(
                            field,
                            (width as f64 * fraction) as i32 + dx,
                            height as i32 - 3 + dy,
                            color,
                        );
                    }
                }
            }
        }
        for plant in self.plants.iter().filter(|plant| plant.front) {
            paint_plant(field, plant, height, self.phase);
        }
        if width >= 30 && height >= 16 {
            // A small sand crab travels through the channel between the outcrops.
            let x = (width as f64 * (0.5 + 0.16 * (self.phase * 2.0).sin())) as i32;
            let y = height as i32 - 4;
            for dx in -2..=2 {
                pixel(field, x + dx, y, [195, 119, 83]);
                pixel(field, x + dx, y + 1, [133, 78, 67]);
            }
            for sign in [-1, 1] {
                pixel(field, x + sign, y - 1, [231, 197, 143]);
                pixel(field, x + sign * 3, y - 1, [195, 119, 83]);
                let step = ((self.phase * 24.0).sin() * sign as f64).round() as i32;
                pixel(field, x + sign * 3 + step, y + 2, [133, 78, 67]);
            }
        }
        for fish in self.fish.iter().filter(|fish| fish.depth == 2) {
            paint_fish(field, fish, self.phase);
        }
    }
}

impl ScreenFrameProducer for AquariumAnimation {
    fn render_frame(&self) -> Vec<String> {
        self.pixels.render_lines(self.context.resolved_width)
    }

    fn advance_frame(&mut self) {
        self.phase = (self.phase + TAU / 1_200.0) % TAU;
        let width = self.context.inner_width as f64;
        let height = self.context.resolved_height.saturating_mul(2) as f64;
        for fish in &mut self.fish {
            fish.x = (fish.x + fish.speed + 16.0).rem_euclid(width + 32.0) - 16.0;
        }
        for bubble in &mut self.bubbles {
            bubble.y = (bubble.y - bubble.speed).rem_euclid(height);
        }
        self.paint();
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        *self = Self::new(context);
    }
}

pub fn aquarium_frame_delay() -> Duration {
    Duration::from_millis(40)
}

fn pixel(field: &mut HalfBlockField, x: i32, y: i32, color: [u8; 3]) {
    let [r, g, b] = color;
    field.set(x as usize, y as usize, Some(RgbColor::new(r, g, b)));
}

fn paint_fish(field: &mut HalfBlockField, fish: &Fish, phase: f64) {
    let art = fish.kind.art();
    let width = art[0].len() as i32;
    let y = fish.y.round() as i32 + (phase.mul_add(4.0, fish.phase).sin() * 1.4) as i32;
    for (dy, row) in art.iter().enumerate() {
        for (dx, part) in row.iter().enumerate() {
            let tone = match part {
                b'b' => 0,
                b'h' => 1,
                b's' => 2,
                b'f' => 3,
                b'e' => 4,
                _ => continue,
            };
            // Tail/fin tips tuck in and fan out independently of forward motion.
            if *part == b'f' && (phase * 28.0 + fish.phase).sin() < -0.2 && dx % 2 == 0 {
                continue;
            }
            let color = if tone == 4 {
                [13, 28, 37]
            } else {
                FISH_COLORS[fish.color][tone]
            };
            let color = color.map(|channel| {
                if fish.depth == 0 {
                    channel / 3 * 2
                } else {
                    channel
                }
            });
            let dx = if fish.speed < 0.0 {
                width - 1 - dx as i32
            } else {
                dx as i32
            };
            pixel(
                field,
                fish.x.round() as i32 + dx - width / 2,
                y + dy as i32 - art.len() as i32 / 2,
                color,
            );
        }
    }
}

fn paint_plant(field: &mut HalfBlockField, plant: &Plant, height: usize, phase: f64) {
    let color = if plant.coral {
        [169, 93, 110]
    } else if plant.front {
        [47, 142, 105]
    } else {
        [21, 70, 67]
    };
    let stalk = if plant.coral {
        plant.height.min(12)
    } else {
        plant.height
    };
    for rise in 0..stalk {
        let progress = rise as f64 / stalk as f64;
        let x = plant.x
            + rise as f64 * plant.lean
            + (phase * 2.0 + plant.phase + progress * 2.0).sin() * progress * 3.0;
        let y = height as i32 - 3 - rise as i32;
        pixel(field, x.round() as i32, y, color);
        if rise % 3 == 1 {
            let direction = if rise % 2 == 0 { 1 } else { -1 };
            let reach = if plant.coral { 5 } else { 3 };
            for leaf in 1..=reach {
                pixel(
                    field,
                    x.round() as i32 + direction * leaf,
                    y - leaf / 2,
                    color,
                );
            }
            if plant.coral {
                pixel(
                    field,
                    x.round() as i32 + direction * reach,
                    y - reach / 2 - 1,
                    [228, 151, 151],
                );
            } else if plant.front {
                pixel(field, x.round() as i32 - direction, y, [96, 183, 119]);
            }
        }
    }
}

fn paint_ray(field: &mut HalfBlockField, x: i32, y: i32, phase: f64) {
    let wing = 5 + ((phase * 8.0).sin() * 2.0) as i32;
    for dy in -wing..=wing {
        let radius = 8 - dy.abs() * 3 / 4;
        for dx in -radius..=radius {
            let color = if dy.abs() < 2 {
                [91, 131, 140]
            } else {
                [43, 81, 101]
            };
            pixel(field, x + dx - dy.abs() / 2, y + dy, color);
        }
    }
    for tail in 8..24 {
        pixel(
            field,
            x - tail,
            y + ((phase * 8.0 + tail as f64 * 0.16).sin() * 1.5) as i32,
            [43, 81, 101],
        );
    }
    for dy in [-1, 1] {
        pixel(field, x + 5, y + dy, [11, 29, 39]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visible_line_width;

    fn context(width: usize, height: usize) -> ScreenAnimationContext {
        ScreenAnimationContext {
            resolved_width: width + 4,
            inner_width: width,
            resolved_height: height,
            size_class: "medium",
        }
    }

    #[test]
    fn aquarium_is_populated_moving_deterministic_and_resize_safe() {
        let mut first = AquariumAnimation::new(context(80, 24));
        let mut second = AquariumAnimation::new(context(80, 24));
        let initial = first.render_frame();
        assert!(
            initial
                .iter()
                .map(|line| line.matches('▀').count())
                .sum::<usize>()
                > 400
        );
        assert_eq!(initial, second.render_frame());
        let populations = (first.fish.len(), first.bubbles.len(), first.plants.len());
        assert!(first.fish.iter().any(|fish| fish.speed < 0.0));
        assert!(first.fish.iter().any(|fish| fish.speed > 0.0));
        assert!(first.plants.iter().any(|plant| plant.front));
        assert!(first.plants.iter().any(|plant| !plant.front));
        let fish_pixels = first
            .pixels
            .cells
            .iter()
            .flatten()
            .filter(|pixel| {
                pixel.is_some_and(|pixel| {
                    FISH_COLORS
                        .iter()
                        .flatten()
                        .any(|rgb| [pixel.red, pixel.green, pixel.blue] == *rgb)
                })
            })
            .count();
        assert!(
            fish_pixels > 60,
            "the water background alone is not a populated tank"
        );
        for frame in 1..=3_000 {
            first.advance_frame();
            second.advance_frame();
            if frame % 250 == 0 {
                let rendered = first.render_frame();
                assert_ne!(rendered, initial);
                assert_eq!(rendered, second.render_frame());
                assert_eq!(rendered.len(), 24);
                assert!(rendered.iter().all(|line| visible_line_width(line) == 84));
                assert_eq!(
                    (first.fish.len(), first.bubbles.len(), first.plants.len()),
                    populations
                );
                assert!(
                    first
                        .fish
                        .iter()
                        .all(|fish| fish.x.is_finite() && (-16.0..96.0).contains(&fish.x))
                );
                assert!(
                    first
                        .bubbles
                        .iter()
                        .all(|bubble| (0.0..48.0).contains(&bubble.y))
                );
            }
        }
        for (width, height) in [
            (0, 0),
            (0, 12),
            (10, 0),
            (1, 1),
            (2, 1),
            (17, 5),
            (80, 24),
            (200, 60),
        ] {
            first.resize(context(width, height));
            let fresh = AquariumAnimation::new(context(width, height));
            let rendered = first.render_frame();
            assert_eq!(rendered, fresh.render_frame());
            assert_eq!(rendered.len(), if width == 0 { 0 } else { height });
            assert!(
                rendered
                    .iter()
                    .all(|line| visible_line_width(line) == width + 4)
            );
            assert!(
                first.fish.len() <= 36 && first.bubbles.len() <= 64 && first.plants.len() <= 32
            );
            first.advance_frame();
        }
    }

    #[test]
    fn fish_face_their_motion_clip_at_edges_and_obey_depth() {
        let fish = |x, speed, depth, color| Fish {
            x,
            y: 32.0,
            speed,
            phase: 0.0,
            kind: FishKind::Reef,
            color,
            depth,
        };
        let mut scene = AquariumAnimation::new(context(80, 24));
        scene.plants.clear();
        scene.bubbles.clear();
        // Foreground first in storage: depth, not vector order, must decide overlap.
        scene.fish = vec![fish(40.0, 0.1, 2, 0), fish(40.0, 0.1, 1, 1)];
        scene.paint();
        let mut right = HalfBlockField::new(80, 48);
        let mut left = HalfBlockField::new(80, 48);
        paint_fish(&mut right, &fish(40.0, 0.1, 2, 0), 0.0);
        paint_fish(&mut left, &fish(40.0, -0.1, 2, 0), 0.0);
        let at = |field: &HalfBlockField, x: usize, y: usize| field.cells[(y / 2) * 80 + x][y % 2];
        for y in 0..48 {
            for x in 0..80 {
                assert_eq!(at(&right, x, y), at(&left, 79 - x, y));
                if at(&right, x, y).is_some() {
                    assert_eq!(at(&scene.pixels, x, y), at(&right, x, y));
                }
            }
        }
        right.clear();
        paint_fish(&mut right, &fish(-3.0, 0.1, 2, 0), 0.0);
        assert!(right.cells.iter().flatten().any(Option::is_some));
        for y in 0..48 {
            assert!((3..80).all(|x| at(&right, x, y).is_none()));
        }
    }
}
