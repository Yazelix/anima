use std::{io, thread, time::Duration};

use yazelix_screen::{
    AQUARIUM_STYLE, AquariumAnimation, BoidsAnimation, BoidsVariant, CHLADNI_STYLE,
    ChladniAnimation, FRIENDS_AND_ENEMIES_STYLE, FriendsAndEnemiesAnimation,
    GAME_OF_LIFE_RANDOM_STYLES, GameOfLifeAnimation, GameOfLifeCellStyle, MANDELBROT_STYLE,
    MATRIX_STYLE, MandelbrotAnimation, MatrixAnimation, PHYSARUM_STYLE, PLASMA_STYLE,
    PRIMORDIAL_STYLE, PhysarumAnimation, PlasmaAnimation, PrimordialAnimation,
    ScreenAnimationContext, ScreenFrameProducer, aquarium_frame_delay, chladni_frame_delay,
    enter_screen_mode, game_of_life_spec, leave_screen_mode, mandelbrot_frame_delay,
    matrix_frame_delay, physarum_frame_delay, plasma_frame_delay, primordial_frame_delay,
    render_screen_frame, terminal_height, terminal_width,
};

#[derive(Debug, Clone, Copy)]
enum ExampleStyle {
    Aquarium,
    Boids(BoidsVariant),
    FriendsAndEnemies,
    GameOfLife(&'static str),
    Mandelbrot,
    Matrix,
    Primordial,
    Physarum,
    Chladni,
    Plasma,
}

struct ScreenModeGuard;

impl ScreenModeGuard {
    fn new() -> io::Result<Self> {
        enter_screen_mode()?;
        Ok(Self)
    }
}

impl Drop for ScreenModeGuard {
    fn drop(&mut self) {
        let _ = leave_screen_mode();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let raw_style = args.next().unwrap_or_else(|| MANDELBROT_STYLE.to_string());
    let frame_count = args
        .next()
        .map(|raw| raw.parse::<usize>())
        .transpose()?
        .unwrap_or(90);
    let style = resolve_style(&raw_style)?;
    let mut animation = build_animation(style);
    let frame_delay = frame_delay(style);

    let _screen = ScreenModeGuard::new()?;
    for _ in 0..frame_count {
        render_screen_frame(&animation.render_frame())?;
        animation.advance_frame();
        thread::sleep(frame_delay);
    }

    Ok(())
}

fn resolve_style(raw: &str) -> Result<ExampleStyle, io::Error> {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized == AQUARIUM_STYLE {
        return Ok(ExampleStyle::Aquarium);
    }
    if let Some(variant) = BoidsVariant::from_style_name(&normalized) {
        return Ok(ExampleStyle::Boids(variant));
    }
    if normalized == FRIENDS_AND_ENEMIES_STYLE {
        return Ok(ExampleStyle::FriendsAndEnemies);
    }
    if normalized == MANDELBROT_STYLE {
        return Ok(ExampleStyle::Mandelbrot);
    }
    if normalized == MATRIX_STYLE {
        return Ok(ExampleStyle::Matrix);
    }
    if normalized == PRIMORDIAL_STYLE {
        return Ok(ExampleStyle::Primordial);
    }
    if normalized == PHYSARUM_STYLE {
        return Ok(ExampleStyle::Physarum);
    }
    if normalized == CHLADNI_STYLE {
        return Ok(ExampleStyle::Chladni);
    }
    if normalized == PLASMA_STYLE {
        return Ok(ExampleStyle::Plasma);
    }
    if let Some(style) = GAME_OF_LIFE_RANDOM_STYLES
        .iter()
        .find(|candidate| **candidate == normalized)
        .copied()
    {
        return Ok(ExampleStyle::GameOfLife(style));
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "unsupported style `{normalized}`; expected aquarium, boids, boids_predator, boids_schools, friends_and_enemies, primordial, physarum, chladni, plasma, mandelbrot, matrix, game_of_life_gliders, or game_of_life_tumblers"
        ),
    ))
}

fn build_animation(style: ExampleStyle) -> Box<dyn ScreenFrameProducer> {
    let context = context_for_style(style, terminal_width(), terminal_height());
    match style {
        ExampleStyle::Aquarium => Box::new(AquariumAnimation::new(context)),
        ExampleStyle::Boids(variant) => Box::new(BoidsAnimation::with_variant(
            context,
            GameOfLifeCellStyle::FullBlock,
            variant,
        )),
        ExampleStyle::GameOfLife(style_name) => Box::new(GameOfLifeAnimation::new(
            style_name,
            context,
            GameOfLifeCellStyle::FullBlock,
        )),
        ExampleStyle::FriendsAndEnemies => Box::new(FriendsAndEnemiesAnimation::new(context)),
        ExampleStyle::Mandelbrot => Box::new(MandelbrotAnimation::new(context)),
        ExampleStyle::Matrix => Box::new(MatrixAnimation::new(context)),
        ExampleStyle::Primordial => Box::new(PrimordialAnimation::new(context)),
        ExampleStyle::Physarum => Box::new(PhysarumAnimation::new(context)),
        ExampleStyle::Chladni => Box::new(ChladniAnimation::new(context)),
        ExampleStyle::Plasma => Box::new(PlasmaAnimation::new(context)),
    }
}

fn context_for_style(style: ExampleStyle, width: usize, height: usize) -> ScreenAnimationContext {
    let size_class = size_class(width);
    let inner_width = match style {
        ExampleStyle::GameOfLife(_) => {
            let spec = game_of_life_spec(size_class);
            width.saturating_sub(6).max(spec.minimum_inner_width)
        }
        ExampleStyle::Aquarium
        | ExampleStyle::Boids(_)
        | ExampleStyle::FriendsAndEnemies
        | ExampleStyle::Primordial
        | ExampleStyle::Physarum
        | ExampleStyle::Chladni
        | ExampleStyle::Plasma
        | ExampleStyle::Mandelbrot
        | ExampleStyle::Matrix => width,
    };

    ScreenAnimationContext {
        resolved_width: width,
        resolved_height: height,
        inner_width,
        size_class,
    }
}

fn size_class(width: usize) -> &'static str {
    if width < 44 {
        "narrow"
    } else if width < 72 {
        "medium"
    } else if width < 120 {
        "wide"
    } else {
        "hero"
    }
}

fn frame_delay(style: ExampleStyle) -> Duration {
    match style {
        ExampleStyle::Aquarium => aquarium_frame_delay(),
        ExampleStyle::Boids(_) => Duration::from_millis(70),
        ExampleStyle::FriendsAndEnemies => Duration::from_millis(55),
        ExampleStyle::Mandelbrot => mandelbrot_frame_delay(),
        ExampleStyle::Matrix => matrix_frame_delay(),
        ExampleStyle::Primordial => primordial_frame_delay(),
        ExampleStyle::Physarum => physarum_frame_delay(),
        ExampleStyle::Chladni => chladni_frame_delay(),
        ExampleStyle::Plasma => plasma_frame_delay(),
        ExampleStyle::GameOfLife(_) => Duration::from_millis(160),
    }
}
