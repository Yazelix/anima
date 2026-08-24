use crate::random::{size_seed, unit_from_seed};
use crate::{
    GameOfLifeCellStyle, ScreenAnimationContext, ScreenCell, ScreenFrame, ScreenFrameProducer,
};
use crossterm::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoidsVariant {
    Predator,
    Schools,
}

impl BoidsVariant {
    pub fn from_style_name(style: &str) -> Option<Self> {
        match style {
            "boids" | "boids_predator" => Some(Self::Predator),
            "boids_schools" => Some(Self::Schools),
            _ => None,
        }
    }

    pub fn canonical_style_name(self) -> &'static str {
        match self {
            Self::Predator => "boids_predator",
            Self::Schools => "boids_schools",
        }
    }
}

pub fn is_boids_style(style: &str) -> bool {
    BoidsVariant::from_style_name(style).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn limit(self, max: f64) -> Self {
        let length = self.length();
        if length <= max || length == 0.0 {
            self
        } else {
            self.scale(max / length)
        }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    fn normalized(self) -> Self {
        let length = self.length();
        if length == 0.0 {
            Self::zero()
        } else {
            self.scale(1.0 / length)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoidRole {
    Flock,
    Predator,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Boid {
    position: Vec2,
    velocity: Vec2,
    species: usize,
    role: BoidRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoidsAnimation {
    context: ScreenAnimationContext,
    cell_style: GameOfLifeCellStyle,
    variant: BoidsVariant,
    boids: Vec<Boid>,
}

impl BoidsAnimation {
    pub fn new(context: ScreenAnimationContext, cell_style: GameOfLifeCellStyle) -> Self {
        Self::with_variant(context, cell_style, BoidsVariant::Predator)
    }

    pub fn with_variant(
        context: ScreenAnimationContext,
        cell_style: GameOfLifeCellStyle,
        variant: BoidsVariant,
    ) -> Self {
        let boids = seed_boids(context, variant);
        Self {
            context,
            cell_style,
            variant,
            boids,
        }
    }

    fn grid_width(&self) -> usize {
        (self.context.inner_width / 2).max(1)
    }

    fn grid_height(&self) -> usize {
        self.context.resolved_height.max(1)
    }
}

impl ScreenFrameProducer for BoidsAnimation {
    fn render_frame(&self) -> Vec<String> {
        let grid_width = self.grid_width();
        let grid_height = self.grid_height();
        let mut frame = ScreenFrame::new(self.context.inner_width, grid_height);
        for role_pass in [BoidRole::Flock, BoidRole::Predator] {
            for boid in &self.boids {
                if boid.role != role_pass {
                    continue;
                }
                let x = wrapped_index(boid.position.x.round(), grid_width);
                let y = wrapped_index(boid.position.y.round(), grid_height);
                let sprite = boid_sprite_cells(self.cell_style, boid.role, boid.velocity);
                let color_x = boid_color_index(boid, self.variant);
                let origin_x = x * 2;
                for cell in sprite {
                    frame.set(
                        origin_x + cell.dx,
                        y + cell.dy,
                        ScreenCell::indexed(
                            cell.glyph,
                            color_x,
                            pack_boid_tones(cell.tone, cell.background_tone),
                        ),
                    );
                }
            }
        }
        frame.render_lines(self.context.resolved_width, |cell| {
            colorize_boid_cell(cell.color_x, cell.color_y, cell.glyph)
        })
    }

    fn advance_frame(&mut self) {
        let grid_width = self.grid_width() as f64;
        let grid_height = self.grid_height() as f64;
        step_boids(&mut self.boids, grid_width, grid_height, self.variant);
    }

    fn resize(&mut self, context: ScreenAnimationContext) {
        self.context = context;
        self.boids = seed_boids(context, self.variant);
    }
}

fn seed_boids(context: ScreenAnimationContext, variant: BoidsVariant) -> Vec<Boid> {
    let grid_width = (context.inner_width / 2).max(1);
    let grid_height = context.resolved_height.max(1);
    let area = grid_width.saturating_mul(grid_height);
    let count = (area / 90).clamp(8, 32);
    let mut seed = size_seed(grid_width, grid_height, 0x5EED);

    (0..count)
        .map(|index| {
            let role = if variant == BoidsVariant::Predator && index == 0 {
                BoidRole::Predator
            } else {
                BoidRole::Flock
            };
            let (px, py) = if role == BoidRole::Predator {
                (grid_width as f64 * 0.5, grid_height as f64 * 0.5)
            } else {
                (
                    unit_from_seed(&mut seed) * grid_width as f64,
                    unit_from_seed(&mut seed) * grid_height as f64,
                )
            };
            let angle = unit_from_seed(&mut seed) * std::f64::consts::TAU;
            let speed = if role == BoidRole::Predator {
                1.08
            } else {
                0.62 + (index % 5) as f64 * 0.07
            };
            Boid {
                position: Vec2::new(px, py),
                velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
                species: match variant {
                    BoidsVariant::Schools => index % 3,
                    _ => 0,
                },
                role,
            }
        })
        .collect()
}

fn step_boids(boids: &mut [Boid], width: f64, height: f64, variant: BoidsVariant) {
    let previous = boids.to_vec();

    for (index, boid) in boids.iter_mut().enumerate() {
        let mut velocity = match (variant, boid.role) {
            (BoidsVariant::Predator, BoidRole::Predator) => predator_velocity(index, &previous),
            _ => flock_velocity(index, &previous, variant),
        };
        let max_speed = match (variant, boid.role) {
            (BoidsVariant::Predator, BoidRole::Predator) => 1.70,
            (BoidsVariant::Schools, BoidRole::Flock) => 1.28,
            _ => 1.35,
        };
        velocity = velocity.limit(max_speed);
        boid.velocity = velocity;
        boid.position = wrap_position(boid.position.add(boid.velocity), width, height);
    }
}

fn flock_velocity(index: usize, previous: &[Boid], variant: BoidsVariant) -> Vec2 {
    let boid = previous[index];
    let perception = match variant {
        BoidsVariant::Schools => 10.0,
        _ => 8.0,
    };
    let separation_distance = 3.0;
    let mut predator_flee = Vec2::zero();
    let mut separation = Vec2::zero();
    let mut alignment = Vec2::zero();
    let mut cohesion = Vec2::zero();
    let mut neighbors = 0.0;

    for (other_index, other) in previous.iter().enumerate() {
        if index == other_index {
            continue;
        }
        let offset = other.position.sub(boid.position);
        let distance = offset.length();
        if distance == 0.0 || distance > perception {
            continue;
        }

        if variant == BoidsVariant::Predator && other.role == BoidRole::Predator {
            predator_flee = predator_flee.sub(offset.scale(1.4 / distance.max(0.4)));
            continue;
        }

        if variant == BoidsVariant::Schools
            && other.role == BoidRole::Flock
            && other.species != boid.species
        {
            separation = separation.sub(offset.scale(0.55 / distance.max(0.4)));
            continue;
        }

        if distance < separation_distance {
            separation = separation.sub(offset.scale(1.0 / distance.max(0.2)));
        }
        alignment = alignment.add(other.velocity);
        cohesion = cohesion.add(other.position);
        neighbors += 1.0;
    }

    let mut velocity = boid.velocity;
    if neighbors > 0.0 {
        alignment = alignment.scale(1.0 / neighbors).sub(boid.velocity);
        cohesion = cohesion.scale(1.0 / neighbors).sub(boid.position);
        let (separation_weight, alignment_weight, cohesion_weight) = match variant {
            BoidsVariant::Schools => (0.14, 0.07, 0.012),
            BoidsVariant::Predator => (0.12, 0.04, 0.006),
        };
        velocity = velocity
            .add(separation.scale(separation_weight))
            .add(alignment.scale(alignment_weight))
            .add(cohesion.scale(cohesion_weight));
    }

    velocity.add(predator_flee.scale(0.22))
}

fn predator_velocity(index: usize, previous: &[Boid]) -> Vec2 {
    let predator = previous[index];
    let mut nearest: Option<(f64, Vec2)> = None;
    for other in previous.iter().filter(|boid| boid.role == BoidRole::Flock) {
        let offset = other.position.sub(predator.position);
        let distance = offset.length();
        if distance == 0.0 {
            continue;
        }
        if nearest
            .as_ref()
            .is_none_or(|(nearest_distance, _)| distance < *nearest_distance)
        {
            nearest = Some((distance, offset));
        }
    }

    let Some((_, offset)) = nearest else {
        return predator.velocity;
    };
    let desired = offset.normalized().scale(1.70);
    predator
        .velocity
        .add(desired.sub(predator.velocity).limit(0.28))
}

fn boid_color_index(boid: &Boid, variant: BoidsVariant) -> usize {
    match (variant, boid.role) {
        (_, BoidRole::Predator) => 0,
        (BoidsVariant::Predator, BoidRole::Flock) => 1,
        (BoidsVariant::Schools, BoidRole::Flock) => 2 + boid.species % 3,
    }
}

fn wrap_position(position: Vec2, width: f64, height: f64) -> Vec2 {
    Vec2::new(position.x.rem_euclid(width), position.y.rem_euclid(height))
}

fn wrapped_index(value: f64, limit: usize) -> usize {
    value.rem_euclid(limit as f64) as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoidDirection {
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoidSpriteTone {
    Tail,
    Body,
    Fin,
    Head,
}

impl BoidSpriteTone {
    const fn as_index(self) -> usize {
        match self {
            Self::Tail => 0,
            Self::Body => 1,
            Self::Fin => 2,
            Self::Head => 3,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Tail,
            2 => Self::Fin,
            3 => Self::Head,
            _ => Self::Body,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoidSpriteCell {
    dx: usize,
    dy: usize,
    tone: BoidSpriteTone,
    background_tone: Option<BoidSpriteTone>,
    glyph: char,
}

const fn sprite_cell(dx: usize, dy: usize, tone: BoidSpriteTone, glyph: char) -> BoidSpriteCell {
    BoidSpriteCell {
        dx,
        dy,
        tone,
        background_tone: None,
        glyph,
    }
}

const fn sprite_cell_with_background(
    dx: usize,
    dy: usize,
    tone: BoidSpriteTone,
    background_tone: BoidSpriteTone,
    glyph: char,
) -> BoidSpriteCell {
    BoidSpriteCell {
        dx,
        dy,
        tone,
        background_tone: Some(background_tone),
        glyph,
    }
}

const BOID_BLOCK: char = '█';
const BOID_LEFT_HALF: char = '▌';
const BOID_RIGHT_HALF: char = '▐';
const BOID_UPPER_HALF: char = '▀';
const BOID_LOWER_HALF: char = '▄';
const BOID_TOP_LEFT: char = '▘';
const BOID_TOP_RIGHT: char = '▝';
const BOID_BOTTOM_LEFT: char = '▖';
const BOID_BOTTOM_RIGHT: char = '▗';
const TAIL: BoidSpriteTone = BoidSpriteTone::Tail;
const BODY: BoidSpriteTone = BoidSpriteTone::Body;
const FIN: BoidSpriteTone = BoidSpriteTone::Fin;
const HEAD: BoidSpriteTone = BoidSpriteTone::Head;

const PREY_EAST: [BoidSpriteCell; 11] = [
    sprite_cell(0, 0, TAIL, BOID_LOWER_HALF),
    sprite_cell(4, 0, FIN, BOID_LOWER_HALF),
    sprite_cell_with_background(0, 1, BODY, TAIL, BOID_UPPER_HALF),
    sprite_cell(1, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(2, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 1, BODY, BOID_UPPER_HALF),
    sprite_cell_with_background(4, 1, BODY, FIN, BOID_UPPER_HALF),
    sprite_cell(5, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(6, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(7, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(8, 1, HEAD, BOID_TOP_RIGHT),
];
const PREY_SOUTH_EAST: [BoidSpriteCell; 8] = [
    sprite_cell(0, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(1, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(0, 1, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(1, 2, FIN, BOID_BLOCK),
    sprite_cell(2, 2, BODY, BOID_LEFT_HALF),
    sprite_cell(2, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 3, HEAD, BOID_BOTTOM_RIGHT),
];
const PREY_SOUTH: [BoidSpriteCell; 9] = [
    sprite_cell(0, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(1, 0, TAIL, BOID_LOWER_HALF),
    sprite_cell(2, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(1, 1, TAIL, BOID_BLOCK),
    sprite_cell(0, 2, FIN, BOID_RIGHT_HALF),
    sprite_cell(1, 2, FIN, BOID_BLOCK),
    sprite_cell(2, 2, FIN, BOID_LEFT_HALF),
    sprite_cell(1, 3, BODY, BOID_BLOCK),
    sprite_cell(1, 4, HEAD, BOID_LOWER_HALF),
];
const PREY_SOUTH_WEST: [BoidSpriteCell; 8] = [
    sprite_cell(2, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(3, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(3, 1, TAIL, BOID_LEFT_HALF),
    sprite_cell(2, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(1, 2, BODY, BOID_RIGHT_HALF),
    sprite_cell(2, 2, FIN, BOID_BLOCK),
    sprite_cell(0, 3, HEAD, BOID_BOTTOM_LEFT),
    sprite_cell(1, 3, BODY, BOID_UPPER_HALF),
];
const PREY_WEST: [BoidSpriteCell; 11] = [
    sprite_cell(4, 0, FIN, BOID_LOWER_HALF),
    sprite_cell(8, 0, TAIL, BOID_LOWER_HALF),
    sprite_cell(0, 1, HEAD, BOID_TOP_LEFT),
    sprite_cell(1, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(2, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 1, BODY, BOID_UPPER_HALF),
    sprite_cell_with_background(4, 1, BODY, FIN, BOID_UPPER_HALF),
    sprite_cell(5, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(6, 1, BODY, BOID_UPPER_HALF),
    sprite_cell(7, 1, BODY, BOID_UPPER_HALF),
    sprite_cell_with_background(8, 1, BODY, TAIL, BOID_UPPER_HALF),
];
const PREY_NORTH_WEST: [BoidSpriteCell; 8] = [
    sprite_cell(0, 0, HEAD, BOID_TOP_LEFT),
    sprite_cell(1, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(1, 1, BODY, BOID_RIGHT_HALF),
    sprite_cell(2, 1, FIN, BOID_BLOCK),
    sprite_cell(2, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 2, TAIL, BOID_LEFT_HALF),
    sprite_cell(2, 3, TAIL, BOID_TOP_RIGHT),
    sprite_cell(3, 3, TAIL, BOID_TOP_LEFT),
];
const PREY_NORTH: [BoidSpriteCell; 9] = [
    sprite_cell(1, 0, HEAD, BOID_UPPER_HALF),
    sprite_cell(1, 1, BODY, BOID_BLOCK),
    sprite_cell(0, 2, FIN, BOID_RIGHT_HALF),
    sprite_cell(1, 2, FIN, BOID_BLOCK),
    sprite_cell(2, 2, FIN, BOID_LEFT_HALF),
    sprite_cell(1, 3, TAIL, BOID_BLOCK),
    sprite_cell(0, 4, TAIL, BOID_TOP_RIGHT),
    sprite_cell(1, 4, TAIL, BOID_UPPER_HALF),
    sprite_cell(2, 4, TAIL, BOID_TOP_LEFT),
];
const PREY_NORTH_EAST: [BoidSpriteCell; 8] = [
    sprite_cell(2, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(3, 0, HEAD, BOID_TOP_RIGHT),
    sprite_cell(1, 1, FIN, BOID_BLOCK),
    sprite_cell(2, 1, BODY, BOID_LEFT_HALF),
    sprite_cell(0, 2, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(0, 3, TAIL, BOID_TOP_RIGHT),
    sprite_cell(1, 3, TAIL, BOID_TOP_LEFT),
];

const PREDATOR_EAST: [BoidSpriteCell; 32] = [
    sprite_cell(0, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(1, 0, TAIL, BOID_LOWER_HALF),
    sprite_cell(5, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(6, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(7, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(8, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(9, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(10, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(11, 0, HEAD, BOID_BOTTOM_RIGHT),
    sprite_cell(0, 1, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 1, TAIL, BOID_BLOCK),
    sprite_cell(2, 1, TAIL, BOID_RIGHT_HALF),
    sprite_cell(3, 1, BODY, BOID_BLOCK),
    sprite_cell(4, 1, BODY, BOID_BLOCK),
    sprite_cell(5, 1, BODY, BOID_BLOCK),
    sprite_cell(6, 1, BODY, BOID_BLOCK),
    sprite_cell(7, 1, BODY, BOID_BLOCK),
    sprite_cell(8, 1, BODY, BOID_BLOCK),
    sprite_cell(9, 1, BODY, BOID_BLOCK),
    sprite_cell(10, 1, BODY, BOID_BLOCK),
    sprite_cell(11, 1, BODY, BOID_BLOCK),
    sprite_cell(12, 1, HEAD, BOID_BLOCK),
    sprite_cell(13, 1, HEAD, BOID_RIGHT_HALF),
    sprite_cell(0, 2, TAIL, BOID_TOP_RIGHT),
    sprite_cell(1, 2, TAIL, BOID_UPPER_HALF),
    sprite_cell(5, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(6, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(7, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(8, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(9, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(10, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(11, 2, HEAD, BOID_TOP_RIGHT),
];
const PREDATOR_SOUTH_EAST: [BoidSpriteCell; 14] = [
    sprite_cell(0, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(0, 1, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 1, TAIL, BOID_LOWER_HALF),
    sprite_cell(1, 2, TAIL, BOID_RIGHT_HALF),
    sprite_cell(2, 2, BODY, BOID_BLOCK),
    sprite_cell(3, 2, BODY, BOID_LOWER_HALF),
    sprite_cell(2, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 3, BODY, BOID_BLOCK),
    sprite_cell(4, 3, BODY, BOID_LEFT_HALF),
    sprite_cell(3, 4, BODY, BOID_UPPER_HALF),
    sprite_cell(4, 4, BODY, BOID_BLOCK),
    sprite_cell(5, 4, BODY, BOID_LEFT_HALF),
    sprite_cell(4, 5, BODY, BOID_UPPER_HALF),
    sprite_cell(5, 5, HEAD, BOID_BOTTOM_RIGHT),
];
const PREDATOR_SOUTH: [BoidSpriteCell; 15] = [
    sprite_cell(1, 0, TAIL, BOID_BOTTOM_RIGHT),
    sprite_cell(2, 0, TAIL, BOID_UPPER_HALF),
    sprite_cell(3, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(1, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(2, 1, BODY, BOID_BLOCK),
    sprite_cell(3, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(0, 2, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 2, BODY, BOID_BLOCK),
    sprite_cell(2, 2, BODY, BOID_BLOCK),
    sprite_cell(3, 2, BODY, BOID_BLOCK),
    sprite_cell(4, 2, TAIL, BOID_LEFT_HALF),
    sprite_cell(1, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(2, 3, BODY, BOID_BLOCK),
    sprite_cell(3, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(2, 4, HEAD, BOID_LOWER_HALF),
];
const PREDATOR_SOUTH_WEST: [BoidSpriteCell; 14] = [
    sprite_cell(5, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(4, 1, TAIL, BOID_LOWER_HALF),
    sprite_cell(5, 1, TAIL, BOID_LEFT_HALF),
    sprite_cell(2, 2, BODY, BOID_LOWER_HALF),
    sprite_cell(3, 2, BODY, BOID_BLOCK),
    sprite_cell(4, 2, TAIL, BOID_LEFT_HALF),
    sprite_cell(1, 3, BODY, BOID_RIGHT_HALF),
    sprite_cell(2, 3, BODY, BOID_BLOCK),
    sprite_cell(3, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(0, 4, BODY, BOID_RIGHT_HALF),
    sprite_cell(1, 4, BODY, BOID_BLOCK),
    sprite_cell(2, 4, BODY, BOID_UPPER_HALF),
    sprite_cell(0, 5, HEAD, BOID_BOTTOM_LEFT),
    sprite_cell(1, 5, BODY, BOID_UPPER_HALF),
];
const PREDATOR_WEST: [BoidSpriteCell; 32] = [
    sprite_cell(2, 0, HEAD, BOID_BOTTOM_LEFT),
    sprite_cell(3, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(4, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(5, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(6, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(7, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(8, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(12, 0, TAIL, BOID_LOWER_HALF),
    sprite_cell(13, 0, TAIL, BOID_BOTTOM_LEFT),
    sprite_cell(0, 1, HEAD, BOID_LEFT_HALF),
    sprite_cell(1, 1, HEAD, BOID_BLOCK),
    sprite_cell(2, 1, BODY, BOID_BLOCK),
    sprite_cell(3, 1, BODY, BOID_BLOCK),
    sprite_cell(4, 1, BODY, BOID_BLOCK),
    sprite_cell(5, 1, BODY, BOID_BLOCK),
    sprite_cell(6, 1, BODY, BOID_BLOCK),
    sprite_cell(7, 1, BODY, BOID_BLOCK),
    sprite_cell(8, 1, BODY, BOID_BLOCK),
    sprite_cell(9, 1, BODY, BOID_BLOCK),
    sprite_cell(10, 1, BODY, BOID_BLOCK),
    sprite_cell(11, 1, TAIL, BOID_LEFT_HALF),
    sprite_cell(12, 1, TAIL, BOID_BLOCK),
    sprite_cell(13, 1, TAIL, BOID_LEFT_HALF),
    sprite_cell(2, 2, HEAD, BOID_TOP_LEFT),
    sprite_cell(3, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(4, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(5, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(6, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(7, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(8, 2, BODY, BOID_UPPER_HALF),
    sprite_cell(12, 2, TAIL, BOID_UPPER_HALF),
    sprite_cell(13, 2, TAIL, BOID_TOP_LEFT),
];
const PREDATOR_NORTH_WEST: [BoidSpriteCell; 14] = [
    sprite_cell(0, 0, HEAD, BOID_TOP_LEFT),
    sprite_cell(1, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(0, 1, BODY, BOID_RIGHT_HALF),
    sprite_cell(1, 1, BODY, BOID_BLOCK),
    sprite_cell(2, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(1, 2, BODY, BOID_RIGHT_HALF),
    sprite_cell(2, 2, BODY, BOID_BLOCK),
    sprite_cell(3, 2, BODY, BOID_LOWER_HALF),
    sprite_cell(2, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(3, 3, BODY, BOID_BLOCK),
    sprite_cell(4, 3, TAIL, BOID_LEFT_HALF),
    sprite_cell(4, 4, TAIL, BOID_UPPER_HALF),
    sprite_cell(5, 4, TAIL, BOID_LEFT_HALF),
    sprite_cell(5, 5, TAIL, BOID_TOP_LEFT),
];
const PREDATOR_NORTH: [BoidSpriteCell; 15] = [
    sprite_cell(2, 0, HEAD, BOID_UPPER_HALF),
    sprite_cell(1, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(2, 1, BODY, BOID_BLOCK),
    sprite_cell(3, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(0, 2, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 2, BODY, BOID_BLOCK),
    sprite_cell(2, 2, BODY, BOID_BLOCK),
    sprite_cell(3, 2, BODY, BOID_BLOCK),
    sprite_cell(4, 2, TAIL, BOID_LEFT_HALF),
    sprite_cell(1, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(2, 3, BODY, BOID_BLOCK),
    sprite_cell(3, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(1, 4, TAIL, BOID_TOP_RIGHT),
    sprite_cell(2, 4, TAIL, BOID_LOWER_HALF),
    sprite_cell(3, 4, TAIL, BOID_TOP_LEFT),
];
const PREDATOR_NORTH_EAST: [BoidSpriteCell; 14] = [
    sprite_cell(4, 0, BODY, BOID_LOWER_HALF),
    sprite_cell(5, 0, HEAD, BOID_TOP_RIGHT),
    sprite_cell(3, 1, BODY, BOID_LOWER_HALF),
    sprite_cell(4, 1, BODY, BOID_BLOCK),
    sprite_cell(5, 1, BODY, BOID_LEFT_HALF),
    sprite_cell(2, 2, BODY, BOID_LOWER_HALF),
    sprite_cell(3, 2, BODY, BOID_BLOCK),
    sprite_cell(4, 2, BODY, BOID_LEFT_HALF),
    sprite_cell(1, 3, TAIL, BOID_RIGHT_HALF),
    sprite_cell(2, 3, BODY, BOID_BLOCK),
    sprite_cell(3, 3, BODY, BOID_UPPER_HALF),
    sprite_cell(0, 4, TAIL, BOID_RIGHT_HALF),
    sprite_cell(1, 4, TAIL, BOID_UPPER_HALF),
    sprite_cell(0, 5, TAIL, BOID_TOP_RIGHT),
];

fn prey_sprite_cells(direction: BoidDirection) -> &'static [BoidSpriteCell] {
    match direction {
        BoidDirection::East => &PREY_EAST,
        BoidDirection::SouthEast => &PREY_SOUTH_EAST,
        BoidDirection::South => &PREY_SOUTH,
        BoidDirection::SouthWest => &PREY_SOUTH_WEST,
        BoidDirection::West => &PREY_WEST,
        BoidDirection::NorthWest => &PREY_NORTH_WEST,
        BoidDirection::North => &PREY_NORTH,
        BoidDirection::NorthEast => &PREY_NORTH_EAST,
    }
}

fn predator_sprite_cells(direction: BoidDirection) -> &'static [BoidSpriteCell] {
    match direction {
        BoidDirection::East => &PREDATOR_EAST,
        BoidDirection::SouthEast => &PREDATOR_SOUTH_EAST,
        BoidDirection::South => &PREDATOR_SOUTH,
        BoidDirection::SouthWest => &PREDATOR_SOUTH_WEST,
        BoidDirection::West => &PREDATOR_WEST,
        BoidDirection::NorthWest => &PREDATOR_NORTH_WEST,
        BoidDirection::North => &PREDATOR_NORTH,
        BoidDirection::NorthEast => &PREDATOR_NORTH_EAST,
    }
}

fn boid_direction(velocity: Vec2) -> BoidDirection {
    if velocity.length() == 0.0 {
        return BoidDirection::East;
    }

    let sector = (velocity.y.atan2(velocity.x) / std::f64::consts::FRAC_PI_4).round() as i32;
    match sector.rem_euclid(8) {
        0 => BoidDirection::East,
        1 => BoidDirection::SouthEast,
        2 => BoidDirection::South,
        3 => BoidDirection::SouthWest,
        4 => BoidDirection::West,
        5 => BoidDirection::NorthWest,
        6 => BoidDirection::North,
        _ => BoidDirection::NorthEast,
    }
}

fn boid_sprite_cells(
    _cell_style: GameOfLifeCellStyle,
    role: BoidRole,
    velocity: Vec2,
) -> &'static [BoidSpriteCell] {
    let direction = boid_direction(velocity);
    match role {
        BoidRole::Flock => prey_sprite_cells(direction),
        BoidRole::Predator => predator_sprite_cells(direction),
    }
}

const BOID_BACKGROUND_TONE_OFFSET: usize = 1;
const BOID_BACKGROUND_TONE_SHIFT: usize = 8;

fn pack_boid_tones(tone: BoidSpriteTone, background_tone: Option<BoidSpriteTone>) -> usize {
    tone.as_index()
        | (background_tone
            .map(|tone| tone.as_index() + BOID_BACKGROUND_TONE_OFFSET)
            .unwrap_or_default()
            << BOID_BACKGROUND_TONE_SHIFT)
}

fn unpack_boid_tones(tone_index: usize) -> (BoidSpriteTone, Option<BoidSpriteTone>) {
    let tone = BoidSpriteTone::from_index(tone_index & 0xff);
    let background_tone_index = tone_index >> BOID_BACKGROUND_TONE_SHIFT;
    let background_tone = background_tone_index
        .checked_sub(BOID_BACKGROUND_TONE_OFFSET)
        .map(BoidSpriteTone::from_index);
    (tone, background_tone)
}

fn boid_tone_color(index: usize, tone: BoidSpriteTone) -> Color {
    let body_palette = [
        Color::Red,
        Color::Cyan,
        Color::Blue,
        Color::Magenta,
        Color::Green,
        Color::Yellow,
    ];
    let tail_palette = [
        Color::DarkRed,
        Color::Rgb {
            r: 48,
            g: 192,
            b: 128,
        },
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkGreen,
        Color::DarkYellow,
    ];
    let head_palette = [
        Color::Yellow,
        Color::White,
        Color::Cyan,
        Color::White,
        Color::Yellow,
        Color::White,
    ];
    let palette_index = index % body_palette.len();
    match tone {
        BoidSpriteTone::Tail => tail_palette[palette_index],
        BoidSpriteTone::Body => body_palette[palette_index],
        BoidSpriteTone::Fin => Color::DarkBlue,
        BoidSpriteTone::Head => head_palette[palette_index],
    }
}

fn colorize_boid_cell(index: usize, tone_index: usize, glyph: char) -> String {
    let (tone, background_tone) = unpack_boid_tones(tone_index);
    let color = boid_tone_color(index, tone);
    if let Some(background_tone) = background_tone {
        crate::terminal_control::styled_with_background(
            glyph,
            color,
            boid_tone_color(index, background_tone),
        )
    } else {
        crate::terminal_control::styled(glyph, color)
    }
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

    fn tone_char(tone: BoidSpriteTone) -> char {
        match tone {
            BoidSpriteTone::Tail => 't',
            BoidSpriteTone::Body => 'b',
            BoidSpriteTone::Fin => 'f',
            BoidSpriteTone::Head => 'h',
        }
    }

    fn sprite_extent(cells: &[BoidSpriteCell]) -> (usize, usize) {
        let width = cells.iter().map(|cell| cell.dx).max().unwrap_or(0) + 1;
        let height = cells.iter().map(|cell| cell.dy).max().unwrap_or(0) + 1;
        (width, height)
    }

    fn column_has_no_full_blocks(cells: &[BoidSpriteCell], column: usize) -> bool {
        cells
            .iter()
            .filter(|cell| cell.dx == column)
            .all(|cell| cell.glyph != BOID_BLOCK)
    }

    fn tone_glyphs_in_row(
        cells: &[BoidSpriteCell],
        row: usize,
        tone: BoidSpriteTone,
    ) -> Vec<(usize, char)> {
        let mut row_cells = cells
            .iter()
            .filter(|cell| cell.dy == row && cell.tone == tone)
            .map(|cell| (cell.dx, cell.glyph))
            .collect::<Vec<_>>();
        row_cells.sort_by_key(|(dx, _)| *dx);
        row_cells
    }

    fn assert_tone_rows_mirror(cells: &[BoidSpriteCell], tone: BoidSpriteTone) {
        let (_, height) = sprite_extent(cells);
        let top = tone_glyphs_in_row(cells, 0, tone)
            .into_iter()
            .map(|(dx, glyph)| (dx, mirror_glyph_row(glyph)))
            .collect::<Vec<_>>();
        let bottom = tone_glyphs_in_row(cells, height - 1, tone);
        assert_eq!(top, bottom);
    }

    fn sprite_signature_for_cells(cells: &[BoidSpriteCell]) -> Vec<String> {
        let (width, height) = sprite_extent(cells);
        let mut rows = vec![vec!['.'; width]; height];
        for cell in cells {
            rows[cell.dy][cell.dx] = tone_char(cell.tone);
        }
        rows.into_iter()
            .map(|row| row.into_iter().collect())
            .collect()
    }

    fn glyph_signature_for_cells(cells: &[BoidSpriteCell]) -> Vec<String> {
        let (width, height) = sprite_extent(cells);
        let mut rows = vec![vec!['.'; width]; height];
        for cell in cells {
            rows[cell.dy][cell.dx] = cell.glyph;
        }
        rows.into_iter()
            .map(|row| row.into_iter().collect())
            .collect()
    }

    fn mirror_columns(rows: &[String]) -> Vec<String> {
        rows.iter().map(|row| row.chars().rev().collect()).collect()
    }

    fn mirror_rows(rows: &[String]) -> Vec<String> {
        rows.iter().rev().cloned().collect()
    }

    fn mirror_glyph_column(glyph: char) -> char {
        match glyph {
            BOID_LEFT_HALF => BOID_RIGHT_HALF,
            BOID_RIGHT_HALF => BOID_LEFT_HALF,
            BOID_TOP_LEFT => BOID_TOP_RIGHT,
            BOID_TOP_RIGHT => BOID_TOP_LEFT,
            BOID_BOTTOM_LEFT => BOID_BOTTOM_RIGHT,
            BOID_BOTTOM_RIGHT => BOID_BOTTOM_LEFT,
            _ => glyph,
        }
    }

    fn mirror_glyph_row(glyph: char) -> char {
        match glyph {
            BOID_UPPER_HALF => BOID_LOWER_HALF,
            BOID_LOWER_HALF => BOID_UPPER_HALF,
            BOID_TOP_LEFT => BOID_BOTTOM_LEFT,
            BOID_TOP_RIGHT => BOID_BOTTOM_RIGHT,
            BOID_BOTTOM_LEFT => BOID_TOP_LEFT,
            BOID_BOTTOM_RIGHT => BOID_TOP_RIGHT,
            _ => glyph,
        }
    }

    fn mirror_glyph_columns(rows: &[String]) -> Vec<String> {
        rows.iter()
            .map(|row| {
                row.chars()
                    .rev()
                    .map(mirror_glyph_column)
                    .collect::<String>()
            })
            .collect()
    }

    fn mirror_glyph_rows(rows: &[String]) -> Vec<String> {
        rows.iter()
            .rev()
            .map(|row| row.chars().map(mirror_glyph_row).collect())
            .collect()
    }

    fn sprite_signature(role: BoidRole, velocity: Vec2) -> Vec<String> {
        sprite_signature_for_cells(boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            role,
            velocity,
        ))
    }

    fn glyph_signature(role: BoidRole, velocity: Vec2) -> Vec<String> {
        glyph_signature_for_cells(boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            role,
            velocity,
        ))
    }

    fn is_boid_block_glyph(glyph: char) -> bool {
        matches!(
            glyph,
            BOID_BLOCK
                | BOID_LEFT_HALF
                | BOID_RIGHT_HALF
                | BOID_UPPER_HALF
                | BOID_LOWER_HALF
                | BOID_TOP_LEFT
                | BOID_TOP_RIGHT
                | BOID_BOTTOM_LEFT
                | BOID_BOTTOM_RIGHT
        )
    }

    fn assert_painted_boid_frame(visible: &[String], width: usize, height: usize) {
        assert_eq!(visible.len(), height);
        assert!(visible.iter().all(|line| line.chars().count() == width));
        let forbidden = [
            "→", "←", "↑", "↓", "▶", "◀", "▲", "▼", "⠐", "⠶", "⠈", "⢆", "◤", "◥", "◢", "◣", "●",
            "⬤", "/", "\\", "<", ">", "^", "v",
        ];
        assert!(
            visible
                .iter()
                .all(|line| { !forbidden.into_iter().any(|glyph| line.contains(glyph)) })
        );

        let mut painted_cells = 0;
        for line in visible {
            for cell in line.chars().filter(|cell| *cell != ' ') {
                painted_cells += 1;
                assert!(is_boid_block_glyph(cell));
            }
        }
        assert!(painted_cells > 0);
    }

    // Defends: public boids style names include the legacy alias and retained behavior-backed variants, with flow removed from the live surface.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_style_names_resolve_to_variants() {
        assert_eq!(
            BoidsVariant::from_style_name("boids"),
            Some(BoidsVariant::Predator)
        );
        assert_eq!(
            BoidsVariant::from_style_name("boids_predator"),
            Some(BoidsVariant::Predator)
        );
        assert_eq!(
            BoidsVariant::from_style_name("boids_schools"),
            Some(BoidsVariant::Schools)
        );
        assert_eq!(BoidsVariant::from_style_name("boids_flow"), None);
        assert_eq!(BoidsVariant::from_style_name("game_of_life_tumblers"), None);
    }

    // Defends: boids variants alter simulation behavior, not just labels or colors.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_variants_apply_distinct_motion_rules() {
        let starting_boids = vec![
            Boid {
                position: Vec2::new(4.0, 5.0),
                velocity: Vec2::new(0.25, 0.0),
                species: 0,
                role: BoidRole::Predator,
            },
            Boid {
                position: Vec2::new(8.0, 5.0),
                velocity: Vec2::new(-0.1, 0.0),
                species: 0,
                role: BoidRole::Flock,
            },
            Boid {
                position: Vec2::new(9.0, 5.0),
                velocity: Vec2::new(-0.1, 0.1),
                species: 1,
                role: BoidRole::Flock,
            },
        ];
        let mut predator = starting_boids.clone();
        let mut schools = starting_boids.clone();

        step_boids(&mut predator, 40.0, 20.0, BoidsVariant::Predator);
        step_boids(&mut schools, 40.0, 20.0, BoidsVariant::Schools);

        assert!(predator[0].velocity.x > starting_boids[0].velocity.x);
        assert_ne!(schools[1].velocity, starting_boids[1].velocity);
        assert_ne!(predator, schools);
    }

    // Defends: boids colors map predator roles and school species to stable visual identities.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_variant_colors_follow_roles_and_species() {
        let predator = Boid {
            position: Vec2::zero(),
            velocity: Vec2::zero(),
            species: 0,
            role: BoidRole::Predator,
        };
        let first_school = Boid {
            position: Vec2::zero(),
            velocity: Vec2::zero(),
            species: 0,
            role: BoidRole::Flock,
        };
        let second_school = Boid {
            species: 1,
            ..first_school
        };
        let other_prey = Boid {
            species: 3,
            ..first_school
        };

        assert_ne!(
            boid_color_index(&predator, BoidsVariant::Predator),
            boid_color_index(&first_school, BoidsVariant::Predator)
        );
        assert_eq!(
            boid_color_index(&first_school, BoidsVariant::Predator),
            boid_color_index(&other_prey, BoidsVariant::Predator)
        );
        assert_ne!(
            boid_color_index(&first_school, BoidsVariant::Schools),
            boid_color_index(&second_school, BoidsVariant::Schools)
        );
    }

    // Defends: boids use deterministic in-house state updates instead of host randomness.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_animation_is_deterministic_and_advances() {
        let mut first = BoidsAnimation::new(context(80, 24), GameOfLifeCellStyle::FullBlock);
        let mut second = BoidsAnimation::new(context(80, 24), GameOfLifeCellStyle::FullBlock);
        assert_eq!(first, second);

        first.advance_frame();
        second.advance_frame();
        assert_eq!(first, second);
        assert_ne!(
            BoidsAnimation::new(context(80, 24), GameOfLifeCellStyle::FullBlock),
            first
        );
    }

    // Regression: boids render as painted block-cell sprites, not font-sensitive symbolic glyphs, braille blobs, or corner triangles.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_render_painted_cells_without_inserted_rows() {
        let animation = BoidsAnimation::new(context(80, 24), GameOfLifeCellStyle::FullBlock);
        let visible = animation
            .render_frame()
            .into_iter()
            .map(|line| strip_ansi_codes(&line))
            .collect::<Vec<_>>();

        assert_painted_boid_frame(&visible, 80, 24);
    }

    // Defends: boids obey the shared frame-producer resize contract.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boids_resize_preserves_frame_dimensions() {
        let mut animation = BoidsAnimation::new(context(80, 24), GameOfLifeCellStyle::Dotted);
        animation.resize(context(60, 12));
        let visible = animation
            .render_frame()
            .into_iter()
            .map(|line| strip_ansi_codes(&line))
            .collect::<Vec<_>>();

        assert_painted_boid_frame(&visible, 60, 12);
    }

    // Regression: boid units must not collapse into identical pulsing blocks; diagonal movement gets real painted silhouettes.
    // Strength: defect=2 behavior=2 resilience=2 cost=1 uniqueness=2 total=9/10
    #[test]
    fn boid_visual_identity_is_stable_and_directional() {
        let directions = [
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(0.0, -1.0),
            Vec2::new(1.0, -1.0),
        ];
        let prey_signatures = directions
            .into_iter()
            .map(|velocity| sprite_signature(BoidRole::Flock, velocity).join("\n"))
            .collect::<std::collections::BTreeSet<_>>();
        let predator_signatures = directions
            .into_iter()
            .map(|velocity| sprite_signature(BoidRole::Predator, velocity).join("\n"))
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(prey_signatures.len(), 8);
        assert_eq!(predator_signatures.len(), 8);

        let prey_east = sprite_signature(BoidRole::Flock, Vec2::new(1.0, 0.0));
        let prey_west = sprite_signature(BoidRole::Flock, Vec2::new(-1.0, 0.0));
        let prey_north = sprite_signature(BoidRole::Flock, Vec2::new(0.0, -1.0));
        let prey_south = sprite_signature(BoidRole::Flock, Vec2::new(0.0, 1.0));
        let prey_north_east = sprite_signature(BoidRole::Flock, Vec2::new(1.0, -1.0));
        let prey_north_west = sprite_signature(BoidRole::Flock, Vec2::new(-1.0, -1.0));
        let prey_south_east = sprite_signature(BoidRole::Flock, Vec2::new(1.0, 1.0));
        let prey_south_west = sprite_signature(BoidRole::Flock, Vec2::new(-1.0, 1.0));

        assert_eq!(prey_east, vec!["t...f....", "bbbbbbbbh"]);
        let prey_east_cells = boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            BoidRole::Flock,
            Vec2::new(1.0, 0.0),
        );
        let prey_east_body_blocks = prey_east_cells
            .iter()
            .filter(|cell| cell.tone == BODY && cell.glyph == BOID_BLOCK)
            .map(|cell| (cell.dx, cell.dy))
            .collect::<Vec<_>>();
        assert_eq!(prey_east_body_blocks, Vec::<(usize, usize)>::new());
        assert!(
            prey_east_cells
                .iter()
                .filter(|cell| cell.tone == BODY && cell.glyph != BOID_BLOCK)
                .all(|cell| matches!(cell.glyph, BOID_UPPER_HALF | BOID_LOWER_HALF))
        );
        let prey_east_fin_cells = prey_east_cells
            .iter()
            .filter(|cell| cell.tone == FIN)
            .map(|cell| (cell.dx, cell.dy, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(prey_east_fin_cells, vec![(4, 0, BOID_LOWER_HALF)]);
        let prey_east_fin_background_cells = prey_east_cells
            .iter()
            .filter(|cell| cell.background_tone == Some(FIN))
            .map(|cell| (cell.dx, cell.dy, cell.tone, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(
            prey_east_fin_background_cells,
            vec![(4, 1, BODY, BOID_UPPER_HALF)]
        );
        let prey_east_tail_cells = prey_east_cells
            .iter()
            .filter(|cell| cell.tone == TAIL)
            .map(|cell| (cell.dx, cell.dy, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(prey_east_tail_cells, vec![(0, 0, BOID_LOWER_HALF)]);
        let prey_east_tail_background_cells = prey_east_cells
            .iter()
            .filter(|cell| cell.background_tone == Some(TAIL))
            .map(|cell| (cell.dx, cell.dy, cell.tone, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(
            prey_east_tail_background_cells,
            vec![(0, 1, BODY, BOID_UPPER_HALF)]
        );
        let prey_west_cells = boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            BoidRole::Flock,
            Vec2::new(-1.0, 0.0),
        );
        let prey_west_body_blocks = prey_west_cells
            .iter()
            .filter(|cell| cell.tone == BODY && cell.glyph == BOID_BLOCK)
            .map(|cell| (cell.dx, cell.dy))
            .collect::<Vec<_>>();
        assert_eq!(prey_west_body_blocks, Vec::<(usize, usize)>::new());
        assert!(
            prey_west_cells
                .iter()
                .filter(|cell| cell.tone == BODY && cell.glyph != BOID_BLOCK)
                .all(|cell| matches!(cell.glyph, BOID_UPPER_HALF | BOID_LOWER_HALF))
        );
        let prey_west_fin_cells = prey_west_cells
            .iter()
            .filter(|cell| cell.tone == FIN)
            .map(|cell| (cell.dx, cell.dy, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(prey_west_fin_cells, vec![(4, 0, BOID_LOWER_HALF)]);
        let prey_west_fin_background_cells = prey_west_cells
            .iter()
            .filter(|cell| cell.background_tone == Some(FIN))
            .map(|cell| (cell.dx, cell.dy, cell.tone, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(
            prey_west_fin_background_cells,
            vec![(4, 1, BODY, BOID_UPPER_HALF)]
        );
        let prey_west_tail_cells = prey_west_cells
            .iter()
            .filter(|cell| cell.tone == TAIL)
            .map(|cell| (cell.dx, cell.dy, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(prey_west_tail_cells, vec![(8, 0, BOID_LOWER_HALF)]);
        let prey_west_tail_background_cells = prey_west_cells
            .iter()
            .filter(|cell| cell.background_tone == Some(TAIL))
            .map(|cell| (cell.dx, cell.dy, cell.tone, cell.glyph))
            .collect::<Vec<_>>();
        assert_eq!(
            prey_west_tail_background_cells,
            vec![(8, 1, BODY, BOID_UPPER_HALF)]
        );
        assert_eq!(prey_north, vec![".h.", ".b.", "fff", ".t.", "ttt"]);
        assert_eq!(prey_north_east, vec!["..bh", ".fb.", "tb..", "tt.."]);
        assert_eq!(prey_north_west, mirror_columns(&prey_north_east));
        assert_eq!(prey_south_east, mirror_rows(&prey_north_east));
        assert_eq!(prey_south_west, mirror_rows(&prey_north_west));
        assert_eq!(prey_west, mirror_columns(&prey_east));
        assert_eq!(prey_south, mirror_rows(&prey_north));

        let predator_east = sprite_signature(BoidRole::Predator, Vec2::new(1.0, 0.0));
        let predator_west = sprite_signature(BoidRole::Predator, Vec2::new(-1.0, 0.0));
        let predator_north = sprite_signature(BoidRole::Predator, Vec2::new(0.0, -1.0));
        let predator_south = sprite_signature(BoidRole::Predator, Vec2::new(0.0, 1.0));
        let predator_north_east = sprite_signature(BoidRole::Predator, Vec2::new(1.0, -1.0));
        let predator_north_west = sprite_signature(BoidRole::Predator, Vec2::new(-1.0, -1.0));
        let predator_south_east = sprite_signature(BoidRole::Predator, Vec2::new(1.0, 1.0));
        let predator_south_west = sprite_signature(BoidRole::Predator, Vec2::new(-1.0, 1.0));

        assert_eq!(
            predator_east,
            vec!["tt...bbbbbbh..", "tttbbbbbbbbbhh", "tt...bbbbbbh..",]
        );
        assert_eq!(
            predator_north_east,
            vec!["....bh", "...bbb", "..bbb.", ".tbb..", "tt....", "t....."]
        );
        assert_eq!(predator_west, mirror_columns(&predator_east));
        assert_eq!(predator_south, mirror_rows(&predator_north));
        assert_eq!(predator_north_west, mirror_columns(&predator_north_east));
        assert_eq!(predator_south_east, mirror_rows(&predator_north_east));
        assert_eq!(predator_south_west, mirror_rows(&predator_north_west));

        for role in [BoidRole::Flock, BoidRole::Predator] {
            let east = glyph_signature(role, Vec2::new(1.0, 0.0));
            let west = glyph_signature(role, Vec2::new(-1.0, 0.0));
            let north = glyph_signature(role, Vec2::new(0.0, -1.0));
            let south = glyph_signature(role, Vec2::new(0.0, 1.0));
            let north_east = glyph_signature(role, Vec2::new(1.0, -1.0));
            let north_west = glyph_signature(role, Vec2::new(-1.0, -1.0));
            let south_east = glyph_signature(role, Vec2::new(1.0, 1.0));
            let south_west = glyph_signature(role, Vec2::new(-1.0, 1.0));

            assert_eq!(west, mirror_glyph_columns(&east));
            assert_eq!(south, mirror_glyph_rows(&north));
            assert_eq!(north_west, mirror_glyph_columns(&north_east));
            assert_eq!(south_east, mirror_glyph_rows(&north_east));
            assert_eq!(south_west, mirror_glyph_rows(&north_west));

            for velocity in directions {
                let cells = boid_sprite_cells(GameOfLifeCellStyle::FullBlock, role, velocity);
                assert!(cells.iter().all(|cell| is_boid_block_glyph(cell.glyph)));
                assert!(cells.iter().any(|cell| cell.glyph != BOID_BLOCK));
            }
        }

        for role in [BoidRole::Flock, BoidRole::Predator] {
            let east = boid_sprite_cells(GameOfLifeCellStyle::FullBlock, role, Vec2::new(1.0, 0.0));
            let west =
                boid_sprite_cells(GameOfLifeCellStyle::FullBlock, role, Vec2::new(-1.0, 0.0));
            let (east_width, east_height) = sprite_extent(east);
            let (west_width, west_height) = sprite_extent(west);

            assert!(east_width >= east_height * 3);
            assert!(west_width >= west_height * 3);
            if role == BoidRole::Flock {
                assert!(column_has_no_full_blocks(east, east_width - 1));
                assert!(column_has_no_full_blocks(west, 0));
            } else {
                assert!(column_has_no_full_blocks(east, 0));
                assert!(column_has_no_full_blocks(east, east_width - 1));
                assert!(column_has_no_full_blocks(west, 0));
                assert!(column_has_no_full_blocks(west, west_width - 1));
                assert_tone_rows_mirror(east, TAIL);
                assert_tone_rows_mirror(east, HEAD);
                assert_tone_rows_mirror(west, TAIL);
                assert_tone_rows_mirror(west, HEAD);
            }
        }
        assert!(predator_east[0].len() >= prey_east[0].len() + 4);
        assert_eq!(
            colorize_boid_cell(3, BODY.as_index(), BOID_BLOCK),
            colorize_boid_cell(3, BODY.as_index(), BOID_BLOCK)
        );
        assert_ne!(
            colorize_boid_cell(0, BODY.as_index(), BOID_BLOCK),
            colorize_boid_cell(1, BODY.as_index(), BOID_BLOCK)
        );
        assert_ne!(
            colorize_boid_cell(1, TAIL.as_index(), BOID_BLOCK),
            colorize_boid_cell(1, HEAD.as_index(), BOID_BLOCK)
        );
        assert_ne!(
            colorize_boid_cell(1, BODY.as_index(), BOID_BLOCK),
            colorize_boid_cell(1, FIN.as_index(), BOID_BLOCK)
        );
        assert_ne!(
            colorize_boid_cell(1, BODY.as_index(), BOID_UPPER_HALF),
            colorize_boid_cell(1, pack_boid_tones(BODY, Some(FIN)), BOID_UPPER_HALF)
        );
    }

    // Defends: predators remain larger than prey while using the same terminal-safe painted-cell language.
    // Strength: defect=2 behavior=2 resilience=1 cost=1 uniqueness=2 total=8/10
    #[test]
    fn predator_sprite_is_larger_than_prey_sprite() {
        let prey = boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            BoidRole::Flock,
            Vec2::new(1.0, -1.0),
        );
        let predator = boid_sprite_cells(
            GameOfLifeCellStyle::FullBlock,
            BoidRole::Predator,
            Vec2::new(1.0, -1.0),
        );
        let (prey_width, prey_height) = sprite_extent(prey);
        let (predator_width, predator_height) = sprite_extent(predator);

        assert!(predator.len() > prey.len());
        assert!(predator_width > prey_width);
        assert!(predator_height > prey_height);
        assert!(
            prey.iter()
                .all(|cell| matches!(cell.tone, TAIL | BODY | FIN | HEAD))
        );
        assert!(
            predator
                .iter()
                .all(|cell| matches!(cell.tone, TAIL | BODY | FIN | HEAD))
        );
        assert_eq!(
            sprite_signature_for_cells(prey),
            vec!["..bh", ".fb.", "tb..", "tt.."]
        );
        assert_eq!(
            sprite_signature_for_cells(predator),
            vec!["....bh", "...bbb", "..bbb.", ".tbb..", "tt....", "t....."]
        );
    }

    // Defends: the faster boids tuning moves creatures far enough per frame to read as intentional animation.
    // Strength: defect=2 behavior=2 resilience=1 cost=1 uniqueness=2 total=8/10
    #[test]
    fn boids_motion_uses_faster_velocity_floor() {
        let animation = BoidsAnimation::with_variant(
            context(80, 24),
            GameOfLifeCellStyle::FullBlock,
            BoidsVariant::Predator,
        );
        let slowest_prey = animation
            .boids
            .iter()
            .filter(|boid| boid.role == BoidRole::Flock)
            .map(|boid| boid.velocity.length())
            .fold(f64::INFINITY, f64::min);

        assert!(slowest_prey >= 0.62);
    }
}
