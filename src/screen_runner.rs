use crate::random::system_random_index;
use crate::{
    AQUARIUM_STYLE, AquariumAnimation, BoidsAnimation, BoidsVariant, CHLADNI_STYLE,
    ChladniAnimation, FRIENDS_AND_ENEMIES_STYLE, FriendsAndEnemiesAnimation,
    GAME_OF_LIFE_RANDOM_STYLES, GameOfLifeAnimation, GameOfLifeCellStyle, MANDELBROT_STYLE,
    MATRIX_STYLE, MandelbrotAnimation, MatrixAnimation, PHYSARUM_STYLE, PLASMA_STYLE,
    PRIMORDIAL_STYLE, PhysarumAnimation, PlasmaAnimation, PrimordialAnimation, RawModeGuard,
    ScreenAnimationContext, ScreenFrameProducer, aquarium_frame_delay, center_frame_lines,
    center_text, chladni_frame_delay, enter_screen_mode, game_of_life_spec, leave_screen_mode,
    mandelbrot_frame_delay, matrix_frame_delay, physarum_frame_delay, plasma_frame_delay,
    primordial_frame_delay, render_screen_frame, terminal_height, terminal_width,
};
use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};
#[cfg(unix)]
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Compatibility spelling for the native Aquarium.
pub const ASCIQUARIUM_STYLE: &str = "asciiquarium";
pub const STATIC_STYLE: &str = "static";
pub const LOGO_STYLE: &str = "logo";
pub const SCREEN_STYLES: &[&str] = &[
    STATIC_STYLE,
    LOGO_STYLE,
    AQUARIUM_STYLE,
    "boids",
    "boids_predator",
    "boids_schools",
    FRIENDS_AND_ENEMIES_STYLE,
    PRIMORDIAL_STYLE,
    PHYSARUM_STYLE,
    CHLADNI_STYLE,
    PLASMA_STYLE,
    MANDELBROT_STYLE,
    MATRIX_STYLE,
    "game_of_life_gliders",
    "game_of_life_tumblers",
];
pub const SCREEN_RANDOM_STYLES: &[&str] = &[
    AQUARIUM_STYLE,
    "boids",
    "boids_predator",
    "boids_schools",
    FRIENDS_AND_ENEMIES_STYLE,
    PRIMORDIAL_STYLE,
    PHYSARUM_STYLE,
    CHLADNI_STYLE,
    PLASMA_STYLE,
    MANDELBROT_STYLE,
    MATRIX_STYLE,
    "game_of_life_gliders",
    "game_of_life_tumblers",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenStyle {
    Static,
    Logo,
    Animation(AnimationStyle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimationStyle {
    Aquarium,
    Boids(BoidsVariant),
    GameOfLife(&'static str),
    FriendsAndEnemies,
    Primordial,
    Physarum,
    Chladni,
    Plasma,
    Mandelbrot,
    Matrix,
}

const ANIMATION_STYLES: &[AnimationStyle] = &[
    AnimationStyle::Aquarium,
    AnimationStyle::Boids(BoidsVariant::Predator),
    AnimationStyle::Boids(BoidsVariant::Schools),
    AnimationStyle::FriendsAndEnemies,
    AnimationStyle::Primordial,
    AnimationStyle::Physarum,
    AnimationStyle::Chladni,
    AnimationStyle::Plasma,
    AnimationStyle::Mandelbrot,
    AnimationStyle::Matrix,
    AnimationStyle::GameOfLife(GAME_OF_LIFE_RANDOM_STYLES[0]),
    AnimationStyle::GameOfLife(GAME_OF_LIFE_RANDOM_STYLES[1]),
];

const BROWSE_HINT: &str = "←/h previous · l/→ next";
const CARD_LIFETIME: Duration = Duration::from_secs(4);
const CARD_FRAME_DELAY: Duration = Duration::from_millis(33);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputAction {
    Exit,
    Previous,
    Next,
}

struct ScreenArgs {
    style: String,
    cell_style: GameOfLifeCellStyle,
    duration: Option<Duration>,
    help: bool,
}

struct ScreenModeGuard;

// Process-lifetime CLI handlers: signal-hook cannot restore default handlers by
// unregistering them. Library frame producers never install these handlers.
#[cfg(unix)]
static TERMINATED: OnceLock<Result<Arc<AtomicBool>, String>> = OnceLock::new();

fn install_termination_handlers() -> Result<(), String> {
    #[cfg(unix)]
    TERMINATED
        .get_or_init(|| {
            let stopped = Arc::new(AtomicBool::new(false));
            use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
            for signal in [SIGHUP, SIGINT, SIGTERM] {
                signal_hook::flag::register(signal, Arc::clone(&stopped))
                    .map_err(|error| format!("could not handle terminal shutdown: {error}"))?;
            }
            Ok(stopped)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    Ok(())
}

impl ScreenModeGuard {
    fn new() -> Result<Self, String> {
        enter_screen_mode()
            .map_err(|error| format!("could not enter alternate screen: {error}"))?;
        Ok(Self)
    }
}

impl Drop for ScreenModeGuard {
    fn drop(&mut self) {
        let _ = leave_screen_mode();
    }
}

pub fn run_screen_cli(
    args: impl IntoIterator<Item = String>,
    command_name: &str,
) -> Result<(), String> {
    let parsed = parse_screen_args(args, command_name)?;
    if parsed.help {
        print_screen_help(command_name)?;
        return Ok(());
    }

    match resolve_style(&parsed.style, None, command_name)? {
        ScreenStyle::Static => run_in_screen_mode(|| run_static(parsed.duration)),
        ScreenStyle::Logo => run_in_screen_mode(|| run_logo(parsed.duration)),
        ScreenStyle::Animation(style) => {
            let timing = parsed.duration.map(|duration| (Instant::now(), duration));
            install_termination_handlers()?;
            run_in_screen_mode(|| run_animation(style, parsed.cell_style, timing))
        }
    }
}

fn run_in_screen_mode(run: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let _raw = RawModeGuard::new().map_err(|error| format!("could not enter raw mode: {error}"))?;
    let _screen = ScreenModeGuard::new()?;
    run()
}

fn parse_screen_args(
    args: impl IntoIterator<Item = String>,
    command_name: &str,
) -> Result<ScreenArgs, String> {
    let mut help = false;
    let mut style = None;
    let mut cell_style = GameOfLifeCellStyle::FullBlock;
    let mut duration = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" | "help" => help = true,
            "--cell-style" => {
                let Some(raw) = iter.next() else {
                    return Err("missing value after --cell-style".into());
                };
                cell_style = GameOfLifeCellStyle::parse(&raw).map_err(|error| {
                    format!(
                        "invalid --cell-style value `{}`. Expected full_block or dotted",
                        error.normalized()
                    )
                })?;
            }
            "--duration-seconds" => {
                let Some(raw) = iter.next() else {
                    return Err("missing value after --duration-seconds".into());
                };
                let seconds = raw.trim().parse::<u64>().map_err(|_| {
                    format!("invalid --duration-seconds value `{raw}`. Expected positive integer")
                })?;
                if seconds == 0 {
                    return Err(
                        "invalid --duration-seconds value `0`. Expected positive integer".into(),
                    );
                }
                duration = Some(Duration::from_secs(seconds));
            }
            other if style.is_none() => style = Some(other.to_string()),
            other => {
                return Err(format!(
                    "unexpected argument `{other}`. Try `{command_name} --help`"
                ));
            }
        }
    }

    Ok(ScreenArgs {
        style: style.unwrap_or_else(|| "random".to_string()),
        cell_style,
        duration,
        help,
    })
}

fn print_screen_help(command_name: &str) -> Result<(), String> {
    let help = format!(
        "Show Yazelix terminal screen animations\n\nUsage:\n  {command_name} [STYLE] [--cell-style full_block|dotted] [--duration-seconds N]\n\nStyles:\n  {}\n  random\n\nNotes:\n  asciiquarium is a compatibility alias for native aquarium (not the classic renderer)\n  Animations, including Aquarium: Left/h/p = previous; Right/l/n = next; any other key = exit\n  Static and logo: any key = exit\n",
        SCREEN_STYLES.join("\n  ")
    );
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout
        .write_all(help.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("could not write help: {error}"))
}

fn resolve_style(
    raw: &str,
    random_index: Option<usize>,
    command_name: &str,
) -> Result<ScreenStyle, String> {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized == "random" {
        return resolve_style(random_screen_style(random_index), None, command_name);
    }
    if normalized == STATIC_STYLE {
        return Ok(ScreenStyle::Static);
    }
    if normalized == LOGO_STYLE {
        return Ok(ScreenStyle::Logo);
    }
    if normalized == AQUARIUM_STYLE || normalized == ASCIQUARIUM_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Aquarium));
    }
    if let Some(variant) = BoidsVariant::from_style_name(&normalized) {
        return Ok(ScreenStyle::Animation(AnimationStyle::Boids(variant)));
    }
    if normalized == MANDELBROT_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Mandelbrot));
    }
    if normalized == FRIENDS_AND_ENEMIES_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::FriendsAndEnemies));
    }
    if normalized == PRIMORDIAL_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Primordial));
    }
    if normalized == PHYSARUM_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Physarum));
    }
    if normalized == CHLADNI_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Chladni));
    }
    if normalized == PLASMA_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Plasma));
    }
    if normalized == MATRIX_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Matrix));
    }
    if let Some(style) = GAME_OF_LIFE_RANDOM_STYLES
        .iter()
        .find(|candidate| **candidate == normalized)
        .copied()
    {
        return Ok(ScreenStyle::Animation(AnimationStyle::GameOfLife(style)));
    }

    Err(format!(
        "unsupported screen style `{normalized}`. Try `{command_name} --help`"
    ))
}

fn random_screen_style(random_index: Option<usize>) -> &'static str {
    let index = random_index.unwrap_or_else(|| system_random_index(SCREEN_RANDOM_STYLES.len()));
    SCREEN_RANDOM_STYLES[index % SCREEN_RANDOM_STYLES.len()]
}

fn remaining(timing: Option<(Instant, Duration)>) -> Option<Duration> {
    #[cfg(unix)]
    if TERMINATED.get().is_some_and(|state| {
        state
            .as_ref()
            .is_ok_and(|stopped| stopped.load(Ordering::Relaxed))
    }) {
        return Some(Duration::ZERO);
    }
    timing.map(|(started, duration)| duration.saturating_sub(started.elapsed()))
}

fn run_static(duration: Option<Duration>) -> Result<(), String> {
    let mut width = terminal_width();
    let mut height = terminal_height();
    render_centered_static(width, height)?;
    if let Some(duration) = duration {
        return wait_for_duration(duration);
    }

    loop {
        if poll_for_input(Duration::from_millis(250))?.is_some() {
            return Ok(());
        }
        let current = (terminal_width(), terminal_height());
        if current != (width, height) {
            (width, height) = current;
            render_centered_static(width, height)?;
        }
    }
}

fn run_logo(duration: Option<Duration>) -> Result<(), String> {
    let mut width = terminal_width();
    let mut height = terminal_height();
    let mut frames = logo_frames(width, height);
    if let Some(duration) = duration {
        let delay = duration / frames.len() as u32;
        for frame in frames {
            render_screen_frame(&frame)
                .map_err(|error| format!("could not render logo: {error}"))?;
            if poll_for_input(delay)?.is_some() {
                break;
            }
        }
        return Ok(());
    }

    let mut index = 0usize;
    loop {
        render_screen_frame(&frames[index % frames.len()])
            .map_err(|error| format!("could not render logo frame: {error}"))?;
        if poll_for_input(Duration::from_millis(180))?.is_some() {
            return Ok(());
        }
        let current = (terminal_width(), terminal_height());
        if current != (width, height) {
            (width, height) = current;
            frames = logo_frames(width, height);
            index = 0;
        } else {
            index += 1;
        }
    }
}

fn run_animation(
    mut style: AnimationStyle,
    cell_style: GameOfLifeCellStyle,
    timing: Option<(Instant, Duration)>,
) -> Result<(), String> {
    let mut width = terminal_width();
    let mut height = terminal_height();
    let mut animation = build_animation(style, width, height, cell_style);
    let mut cadence = frame_delay(style);
    let mut frame = animation.render_frame();
    let mut card_started = Instant::now();
    let mut next_frame = card_started + cadence;
    let card_lifetime = || {
        timing.map_or(CARD_LIFETIME, |(started, duration)| {
            CARD_LIFETIME.min(duration.saturating_sub(started.elapsed()))
        })
    };
    let mut lifetime = card_lifetime();

    loop {
        if remaining(timing) == Some(Duration::ZERO) {
            return Ok(());
        }

        // Presentation ticks do not advance the simulation. Cache its last frame
        // so slow styles can fade smoothly without extra simulation/render work.
        if Instant::now() >= next_frame {
            animation.advance_frame();
            frame = animation.render_frame();
            next_frame = Instant::now() + cadence;
        }
        let elapsed = card_started.elapsed();
        let (intensity, card_delay) = card_timing(elapsed, lifetime);
        let card = identity_card(style, width, height, intensity);
        crate::terminal_control::render_screen_frame(&mut io::stdout().lock(), &frame, &card)
            .map_err(|error| format!("could not render screen frame: {error}"))?;
        let mut delay = next_frame.saturating_duration_since(Instant::now());
        if !card.is_empty() {
            delay = delay.min(card_delay);
        }
        let delay = timing.map_or(delay, |(started, duration)| {
            delay.min(duration.saturating_sub(started.elapsed()))
        });
        let action = poll_for_input(delay)?;
        let current = (terminal_width(), terminal_height());
        match action {
            Some(InputAction::Exit) => return Ok(()),
            Some(action @ (InputAction::Previous | InputAction::Next)) => {
                style = browse_style(style, action);
                (width, height) = current;
                animation = build_animation(style, width, height, cell_style);
                cadence = frame_delay(style);
                frame = animation.render_frame();
                card_started = Instant::now();
                lifetime = card_lifetime();
                next_frame = card_started + cadence;
            }
            None if current != (width, height) => {
                (width, height) = current;
                animation.resize(context_for_style(style, width, height));
                frame = animation.render_frame();
            }
            None => {}
        }
    }
}

// Attribution sources and the distinction between models and visual adaptations
// live in README's Special Thanks section. These credits name the original role,
// not authorship of Anima's Rust implementation.
fn animation_credit(style: AnimationStyle) -> (&'static str, &'static str) {
    match style {
        AnimationStyle::Aquarium => ("Aquarium", "Original art and motion by Anima"),
        AnimationStyle::Boids(BoidsVariant::Predator) => {
            ("Boids: Predator", "Model by Craig Reynolds")
        }
        AnimationStyle::Boids(BoidsVariant::Schools) => {
            ("Boids: Schools", "Model by Craig Reynolds")
        }
        AnimationStyle::GameOfLife("game_of_life_gliders") => {
            ("Game of Life: Gliders", "Created by John Conway")
        }
        AnimationStyle::GameOfLife("game_of_life_tumblers") => {
            ("Game of Life: Tumblers", "Created by John Conway")
        }
        AnimationStyle::GameOfLife(_) => unreachable!("resolved Game of Life variant"),
        AnimationStyle::FriendsAndEnemies => {
            ("Friends and Enemies", "Particle rule by Simon Woods")
        }
        AnimationStyle::Primordial => (
            "Primordial",
            "Model by Thomas Schmickl, Martin Stefanec and Karl Crailsheim",
        ),
        AnimationStyle::Physarum => ("Physarum", "Based on the model by Jeff Jones"),
        AnimationStyle::Chladni => ("Chladni", "Equations documented by Paul Bourke"),
        AnimationStyle::Plasma => ("Plasma", "Technique documented by Lode Vandevenne"),
        AnimationStyle::Mandelbrot => ("Mandelbrot", "Fractal research by Benoît Mandelbrot"),
        AnimationStyle::Matrix => ("Matrix", "Original rain design by Simon Whiteley"),
    }
}

fn card_timing(elapsed: Duration, lifetime: Duration) -> (f64, Duration) {
    if elapsed >= lifetime {
        return (0.0, Duration::MAX);
    }
    let edge =
        (elapsed.min(lifetime - elapsed).as_secs_f64() * 4.0 / lifetime.as_secs_f64()).min(1.0);
    // During the hold, only the simulation needs redraws. Wake at the fade-out
    // boundary so slow animations do not postpone the card's next transition.
    let delay = if edge < 1.0 {
        CARD_FRAME_DELAY.min(lifetime - elapsed)
    } else {
        lifetime * 3 / 4 - elapsed
    };
    (edge * edge * (3.0 - 2.0 * edge), delay)
}

fn identity_card(style: AnimationStyle, width: usize, height: usize, intensity: f64) -> String {
    // Leave two columns on each side, plus the border and inner padding.
    let available = width.saturating_sub(8).min(52);
    if intensity <= 0.0 || available == 0 {
        return String::new();
    }
    let (title, credit) = animation_credit(style);
    let mut lines = Vec::new();
    for (text, color) in [
        (title, [235, 241, 250]),
        (credit, [176, 190, 210]),
        (BROWSE_HINT, [94, 183, 194]),
    ] {
        let mut line = String::new();
        for word in text.split_whitespace() {
            if word.chars().count() > available {
                return String::new(); // Never cut a creator's name in half.
            }
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > available {
                lines.push((line, color));
                line = String::new();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push((line, color));
    }
    if lines.len() + 4 > height {
        return String::new();
    }
    let inner = lines
        .iter()
        .map(|(line, _)| line.chars().count())
        .max()
        .unwrap();
    let border = [64, 88, 112];
    let mut rows = vec![(format!("╭{}╮", "─".repeat(inner + 2)), border)];
    rows.extend(
        lines
            .into_iter()
            .map(|(line, color)| (format!("│\x1b[48;2;0;0;0m {line:<inner$} \x1b[49m│"), color)),
    );
    rows.push((format!("╰{}╯", "─".repeat(inner + 2)), border));
    let mut output = String::new();
    // Terminal cells have no alpha channel. Only the interior gets black backing;
    // default-background border cells keep it from spilling outside the outline.
    for (row, (line, color)) in rows.into_iter().enumerate() {
        let [r, g, b] = color.map(|channel| (channel as f64 * intensity).round() as u8);
        write!(
            output,
            "\x1b[{};3H\x1b[0m\x1b[38;2;{r};{g};{b}m{line}\x1b[0m",
            row + 2
        )
        .expect("writing to a String");
    }
    output
}

fn wait_for_duration(duration: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < duration {
        let remaining = duration.saturating_sub(started.elapsed());
        if poll_for_input(Duration::from_millis(100).min(remaining))?.is_some() {
            break;
        }
    }
    Ok(())
}

fn build_animation(
    style: AnimationStyle,
    width: usize,
    height: usize,
    cell_style: GameOfLifeCellStyle,
) -> Box<dyn ScreenFrameProducer> {
    let context = context_for_style(style, width, height);
    match style {
        AnimationStyle::Aquarium => Box::new(AquariumAnimation::new(context)),
        AnimationStyle::Boids(variant) => {
            Box::new(BoidsAnimation::with_variant(context, cell_style, variant))
        }
        AnimationStyle::GameOfLife(style_name) => {
            Box::new(GameOfLifeAnimation::new(style_name, context, cell_style))
        }
        AnimationStyle::FriendsAndEnemies => Box::new(FriendsAndEnemiesAnimation::new(context)),
        AnimationStyle::Primordial => Box::new(PrimordialAnimation::new(context)),
        AnimationStyle::Physarum => Box::new(PhysarumAnimation::new(context)),
        AnimationStyle::Chladni => Box::new(ChladniAnimation::new(context)),
        AnimationStyle::Plasma => Box::new(PlasmaAnimation::new(context)),
        AnimationStyle::Mandelbrot => Box::new(MandelbrotAnimation::new(context)),
        AnimationStyle::Matrix => Box::new(MatrixAnimation::new(context)),
    }
}

fn context_for_style(style: AnimationStyle, width: usize, height: usize) -> ScreenAnimationContext {
    match style {
        AnimationStyle::GameOfLife(_) => game_of_life_context(width, height),
        AnimationStyle::Aquarium
        | AnimationStyle::Boids(_)
        | AnimationStyle::FriendsAndEnemies
        | AnimationStyle::Primordial
        | AnimationStyle::Physarum
        | AnimationStyle::Chladni
        | AnimationStyle::Plasma
        | AnimationStyle::Mandelbrot
        | AnimationStyle::Matrix => full_screen_context(width, height),
    }
}

fn game_of_life_context(width: usize, height: usize) -> ScreenAnimationContext {
    let size_class = size_class(width);
    let spec = game_of_life_spec(size_class);
    ScreenAnimationContext {
        resolved_width: width,
        resolved_height: height,
        inner_width: width.saturating_sub(6).max(spec.minimum_inner_width),
        size_class,
    }
}

fn full_screen_context(width: usize, height: usize) -> ScreenAnimationContext {
    ScreenAnimationContext {
        resolved_width: width,
        resolved_height: height,
        inner_width: width,
        size_class: size_class(width),
    }
}

fn frame_delay(style: AnimationStyle) -> Duration {
    match style {
        AnimationStyle::Aquarium => aquarium_frame_delay(),
        AnimationStyle::Boids(_) => Duration::from_millis(70),
        AnimationStyle::FriendsAndEnemies => Duration::from_millis(55),
        AnimationStyle::Primordial => primordial_frame_delay(),
        AnimationStyle::Physarum => physarum_frame_delay(),
        AnimationStyle::Chladni => chladni_frame_delay(),
        AnimationStyle::Plasma => plasma_frame_delay(),
        AnimationStyle::Mandelbrot => mandelbrot_frame_delay(),
        AnimationStyle::Matrix => matrix_frame_delay(),
        AnimationStyle::GameOfLife(_) => Duration::from_millis(160),
    }
}

fn browse_style(style: AnimationStyle, action: InputAction) -> AnimationStyle {
    let count = ANIMATION_STYLES.len();
    let offset = match action {
        InputAction::Previous => count - 1,
        InputAction::Next => 1,
        InputAction::Exit => return style,
    };
    let index = ANIMATION_STYLES
        .iter()
        .position(|candidate| *candidate == style)
        .expect("native style is browsable");
    ANIMATION_STYLES[(index + offset) % count]
}

fn key_action(key: KeyEvent) -> Option<InputAction> {
    match (key.kind, key.modifiers.is_empty(), key.code) {
        (KeyEventKind::Release, _, _) => None,
        (_, true, KeyCode::Left | KeyCode::Char('h' | 'p')) => Some(InputAction::Previous),
        (_, true, KeyCode::Right | KeyCode::Char('l' | 'n')) => Some(InputAction::Next),
        _ => Some(InputAction::Exit),
    }
}

fn poll_for_input(timeout: Duration) -> Result<Option<InputAction>, String> {
    let started = Instant::now();
    let mut wait = timeout;
    let had_terminal = io::stdin().is_terminal();
    let poll_interval = Duration::from_millis(50);

    loop {
        // Long static/logo waits must still notice terminal loss and shutdown.
        let ready = event::poll(wait.min(poll_interval))
            .map_err(|error| format!("could not poll terminal input: {error}"))?;
        if remaining(None) == Some(Duration::ZERO) || (had_terminal && !io::stdin().is_terminal()) {
            return Ok(Some(InputAction::Exit));
        }
        if !ready {
            if wait <= poll_interval {
                return Ok(None);
            }
            wait = timeout.saturating_sub(started.elapsed());
            continue;
        }
        let key = event::read()
            .map_err(|error| format!("could not read terminal input: {error}"))?
            .as_key_event();
        if let Some(action) = key.and_then(key_action) {
            return Ok(Some(action));
        }
        wait = key.map_or(Duration::ZERO, |_| {
            timeout.saturating_sub(started.elapsed())
        });
    }
}

fn render_centered_static(width: usize, height: usize) -> Result<(), String> {
    render_screen_frame(&center_vertically(static_card(width), height))
        .map_err(|error| format!("could not render static welcome: {error}"))
}

fn logo_frames(width: usize, height: usize) -> Vec<Vec<String>> {
    let spec = WelcomeSpec::for_width(width);
    let full = static_card_lines(spec, spec.body.len(), true, width);
    let title = static_card_lines(spec, 0, false, width);
    let first = static_card_lines(spec, 1, true, width);
    [title, first, full]
        .into_iter()
        .map(|frame| center_vertically(frame, height))
        .collect()
}

fn static_card(width: usize) -> Vec<String> {
    let spec = WelcomeSpec::for_width(width);
    static_card_lines(spec, spec.body.len(), true, width)
}

fn static_card_lines(
    spec: WelcomeSpec,
    body_count: usize,
    full_title: bool,
    width: usize,
) -> Vec<String> {
    let content_width = spec.inner_width + 2;
    let title = if full_title { "YAZELIX" } else { "YZS" };
    let mut lines = vec![
        magenta(format!("╭{}╮", "─".repeat(spec.inner_width))),
        if full_title {
            colorize_logo(&center_text(title, content_width))
        } else {
            dim(center_text(title, content_width))
        },
    ];
    for (index, body) in spec.body.iter().enumerate() {
        let line = if spec.center_body {
            center_text(body, content_width)
        } else {
            format!("{body:<content_width$}")
        };
        if index < body_count {
            lines.push(colorize_body(&line));
        } else {
            lines.push(dim(" ".repeat(content_width)));
        }
    }
    lines.push(yellow(center_text("welcome to yazelix", content_width)));
    lines.push(magenta(format!("╰{}╯", "─".repeat(spec.inner_width))));
    center_frame_lines(lines, width)
}

fn center_vertically(lines: Vec<String>, height: usize) -> Vec<String> {
    let top = height.saturating_sub(lines.len()) / 2;
    let mut out = vec![String::new(); top];
    out.extend(lines);
    out
}

#[derive(Debug, Clone, Copy)]
struct WelcomeSpec {
    inner_width: usize,
    center_body: bool,
    body: &'static [&'static str],
}

impl WelcomeSpec {
    fn for_width(width: usize) -> Self {
        match size_class(width) {
            "narrow" => Self {
                inner_width: 22,
                center_body: false,
                body: &["yazi zellij helix", "one shell. one flow."],
            },
            "medium" => Self {
                inner_width: 34,
                center_body: false,
                body: &[
                    "your reproducible terminal IDE",
                    "zero-conflict helix/zellij keys",
                    "top terminals, shells, and packs",
                ],
            },
            "wide" => Self {
                inner_width: 58,
                center_body: true,
                body: &[
                    "your reproducible, declarative terminal IDE",
                    "zero-conflict keybindings between helix and zellij",
                    "supports all top terminals and shells",
                    "curated program packs (all configurable)",
                ],
            },
            _ => Self {
                inner_width: 58,
                center_body: true,
                body: &[
                    "your reproducible, declarative terminal IDE",
                    "zero-conflict keybindings between helix and zellij",
                    "supports all top terminals and shells",
                    "curated program packs (all configurable)",
                    "shines over SSH",
                ],
            },
        }
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

fn colorize_logo(text: &str) -> String {
    const COLORS: &[&str] = &["31", "32", "33", "34", "35"];
    text.chars()
        .enumerate()
        .map(|(index, ch)| {
            if ch == ' ' {
                " ".to_string()
            } else {
                ansi(COLORS[index % COLORS.len()], ch.to_string())
            }
        })
        .collect()
}

fn colorize_body(text: &str) -> String {
    let mut out = String::new();
    let mut remaining = text;
    let accents = [
        "reproducible",
        "declarative",
        "helix",
        "zellij",
        "terminals",
        "shells",
        "packs",
        "SSH",
    ];
    while let Some((index, accent)) = accents
        .iter()
        .filter_map(|&accent| remaining.find(accent).map(|index| (index, accent)))
        .min_by_key(|(index, _)| *index)
    {
        out.push_str(&green(&remaining[..index]));
        out.push_str(&blue(accent));
        remaining = &remaining[index + accent.len()..];
    }
    out.push_str(&green(remaining));
    out
}

fn green(text: impl AsRef<str>) -> String {
    ansi("32", text.as_ref())
}

fn blue(text: impl AsRef<str>) -> String {
    ansi("34", text.as_ref())
}

fn yellow(text: impl AsRef<str>) -> String {
    ansi("33", text.as_ref())
}

fn magenta(text: impl AsRef<str>) -> String {
    ansi("35", text.as_ref())
}

fn dim(text: impl AsRef<str>) -> String {
    ansi("2", text.as_ref())
}

fn ansi(code: &str, text: impl AsRef<str>) -> String {
    format!("\x1b[{code}m{}\x1b[0m", text.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    // Test lane: default

    #[test]
    fn identity_card_fades_wraps_and_preserves_complete_credits() {
        for lifetime in [
            Duration::from_secs(4),
            Duration::from_secs(1),
            Duration::ZERO,
        ] {
            assert_eq!(card_timing(Duration::ZERO, lifetime).0, 0.0);
            assert_eq!(card_timing(lifetime, lifetime), (0.0, Duration::MAX));
            assert_eq!(
                card_timing(lifetime + Duration::from_secs(1), lifetime),
                (0.0, Duration::MAX)
            );
            if !lifetime.is_zero() {
                assert_eq!(card_timing(lifetime / 4, lifetime), (1.0, lifetime / 2));
                assert_eq!(card_timing(lifetime / 2, lifetime), (1.0, lifetime / 4));
                assert_eq!(card_timing(lifetime / 8, lifetime), (0.5, CARD_FRAME_DELAY));
                assert_eq!(
                    card_timing(lifetime * 7 / 8, lifetime),
                    (0.5, CARD_FRAME_DELAY)
                );
                let just_before_fade = lifetime * 3 / 4 - Duration::from_millis(1);
                assert_eq!(
                    card_timing(just_before_fade, lifetime),
                    (1.0, Duration::from_millis(1))
                );
            }
        }
        assert_eq!(strip_ansi("\x1b[2;3HH\x1b[0m\x1b[3;3HHello"), "\nH\nHello");
        for &style in ANIMATION_STYLES {
            let (title, credit) = animation_credit(style);
            for width in [24, 40, 80, 160] {
                let card = identity_card(style, width, 24, 1.0);
                let plain = strip_ansi(&card);
                let lines: Vec<_> = plain.lines().skip(1).collect();
                assert!(!lines.is_empty());
                assert!(lines.iter().all(|line| line.chars().count() <= width - 4));
                let text = lines.join(" ").replace(['│', '╭', '╮', '╰', '╯', '─'], " ");
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                assert_eq!(text, format!("{title} {credit} {BROWSE_HINT}"));

                // Interpret the emitted background state: glyphs cannot clip
                // their rectangular cells, so black must stay inside the border.
                let mut black = true;
                for sequence in card.split("\x1b[").skip(1) {
                    let end = sequence.find(|ch: char| ch.is_ascii_alphabetic()).unwrap();
                    let (parameters, content) = sequence.split_at(end);
                    if content.starts_with('m') {
                        match parameters {
                            "0" | "49" => black = false,
                            "48;2;0;0;0" => black = true,
                            _ => {}
                        }
                    }
                    for glyph in content[1..].chars() {
                        let border = "│╭╮╰╯─".contains(glyph);
                        assert_eq!(black, !border, "background behind {glyph:?}");
                    }
                }
                assert!(!black, "the card must restore the background");
            }
            for (width, height) in [(0, 0), (1, 1), (8, 24), (80, 3)] {
                assert!(identity_card(style, width, height, 1.0).is_empty());
            }
            assert!(identity_card(style, 80, 24, 0.0).is_empty());
            let dim = identity_card(style, 80, 24, 0.5);
            let bright = identity_card(style, 80, 24, 1.0);
            assert_ne!(dim, bright);
            assert_eq!(strip_ansi(&dim), strip_ansi(&bright));
        }
    }

    // Defends: every advertised style is executable.
    #[test]
    fn supported_styles_resolve() {
        for style in SCREEN_STYLES {
            assert!(resolve_style(style, None, "anima").is_ok());
        }
        assert_eq!(
            resolve_style(" AQUARIUM ", None, "anima"),
            Ok(ScreenStyle::Animation(AnimationStyle::Aquarium))
        );
        assert_eq!(
            resolve_style(" ASCIIQUARIUM ", None, "anima"),
            resolve_style(AQUARIUM_STYLE, None, "anima")
        );
        assert_eq!(
            build_animation(
                AnimationStyle::Aquarium,
                40,
                12,
                GameOfLifeCellStyle::FullBlock
            )
            .render_frame(),
            AquariumAnimation::new(full_screen_context(40, 12)).render_frame()
        );
        assert!(matches!(
            resolve_style(" PHYSARUM ", None, "anima"),
            Ok(ScreenStyle::Animation(AnimationStyle::Physarum))
        ));
        assert_eq!(
            build_animation(
                AnimationStyle::Physarum,
                40,
                12,
                GameOfLifeCellStyle::FullBlock
            )
            .render_frame(),
            PhysarumAnimation::new(full_screen_context(40, 12)).render_frame()
        );
        assert!(matches!(
            resolve_style(" CHLADNI ", None, "anima"),
            Ok(ScreenStyle::Animation(AnimationStyle::Chladni))
        ));
        assert_eq!(
            build_animation(
                AnimationStyle::Chladni,
                40,
                12,
                GameOfLifeCellStyle::FullBlock
            )
            .render_frame(),
            ChladniAnimation::new(full_screen_context(40, 12)).render_frame()
        );
        for style in ["game_of_life_oscillators", "game_of_life_bloom"] {
            assert!(resolve_style(style, None, "anima").is_err());
        }
        assert!(matches!(
            resolve_style(" PLASMA ", None, "anima"),
            Ok(ScreenStyle::Animation(AnimationStyle::Plasma))
        ));
        assert_eq!(
            build_animation(
                AnimationStyle::Plasma,
                40,
                12,
                GameOfLifeCellStyle::FullBlock
            )
            .render_frame(),
            PlasmaAnimation::new(full_screen_context(40, 12)).render_frame()
        );
    }

    // Defends: every current animation resolves from random; static/logo never do.
    #[test]
    fn random_pool_resolves_current_animations_with_existing_alias_weight() {
        let mut native = Vec::new();
        for index in 0..SCREEN_RANDOM_STYLES.len() {
            match resolve_style("random", Some(index), "anima").unwrap() {
                ScreenStyle::Animation(style) => native.push(style),
                _ => panic!("random selected a non-animation"),
            }
        }
        assert_eq!(native.len(), ANIMATION_STYLES.len() + 1);
        for style in ANIMATION_STYLES {
            // `boids` remains an additional alias slot for the predator variant.
            let weight = if *style == AnimationStyle::Boids(BoidsVariant::Predator) {
                2
            } else {
                1
            };
            assert_eq!(
                native
                    .iter()
                    .filter(|candidate| *candidate == style)
                    .count(),
                weight,
                "{style:?}"
            );
        }
    }

    #[test]
    fn animation_navigation_maps_keys_and_wraps_through_aquarium() {
        let action = |code| key_action(KeyEvent::new(code, KeyModifiers::NONE));
        for code in [KeyCode::Left, KeyCode::Char('h'), KeyCode::Char('p')] {
            assert_eq!(action(code), Some(InputAction::Previous));
        }
        for code in [KeyCode::Right, KeyCode::Char('l'), KeyCode::Char('n')] {
            assert_eq!(action(code), Some(InputAction::Next));
        }
        for key in [
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
        ] {
            assert_eq!(key_action(key), Some(InputAction::Exit));
        }
        assert_eq!(
            key_action(KeyEvent::new_with_kind(
                KeyCode::Char('l'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            )),
            None
        );

        assert_eq!(
            ANIMATION_STYLES,
            &[
                AnimationStyle::Aquarium,
                AnimationStyle::Boids(BoidsVariant::Predator),
                AnimationStyle::Boids(BoidsVariant::Schools),
                AnimationStyle::FriendsAndEnemies,
                AnimationStyle::Primordial,
                AnimationStyle::Physarum,
                AnimationStyle::Chladni,
                AnimationStyle::Plasma,
                AnimationStyle::Mandelbrot,
                AnimationStyle::Matrix,
                AnimationStyle::GameOfLife(GAME_OF_LIFE_RANDOM_STYLES[0]),
                AnimationStyle::GameOfLife(GAME_OF_LIFE_RANDOM_STYLES[1]),
            ]
        );
        let cycle = ANIMATION_STYLES;
        for index in 0..cycle.len() {
            let current = cycle[index];
            assert_eq!(
                browse_style(current, InputAction::Next),
                cycle[(index + 1) % cycle.len()]
            );
            assert_eq!(
                browse_style(current, InputAction::Previous),
                cycle[(index + cycle.len() - 1) % cycle.len()]
            );
            assert_eq!(browse_style(current, InputAction::Exit), current);
        }
    }

    // Defends: CLI parsing keeps the package usable standalone while exposing the timed mode needed by integrated welcome.
    #[test]
    fn parse_args_accepts_style_cell_style_and_duration() {
        let parsed = parse_screen_args(
            [
                "game_of_life_gliders".to_string(),
                "--cell-style".to_string(),
                "dotted".to_string(),
                "--duration-seconds".to_string(),
                "3".to_string(),
            ],
            "anima",
        )
        .unwrap();

        assert_eq!(parsed.style, "game_of_life_gliders");
        assert_eq!(parsed.cell_style, GameOfLifeCellStyle::Dotted);
        assert_eq!(parsed.duration, Some(Duration::from_secs(3)));
        assert!(!parsed.help);
    }

    // Defends: the static card copy matches the Yazelix welcome copy and omits the main-runtime trailing prompt.
    #[test]
    fn static_card_uses_yazelix_welcome_copy_without_extra_prompt() {
        let frame = strip_ansi(&static_card(140).join("\n"));
        assert!(frame.contains("YAZELIX"));
        assert!(frame.contains("your reproducible, declarative terminal IDE"));
        assert!(frame.contains("welcome to yazelix"));
        assert!(!frame.contains("just"));

        let narrow = static_card(20).join("\n");
        assert!(narrow.contains(&blue("zellij")));
        assert!(narrow.contains(&blue("helix")));
    }

    // Defends: Game of Life keeps the same minimum-width sizing contract as the integrated screen renderer.
    #[test]
    fn game_of_life_context_preserves_inner_width_floor() {
        let context = game_of_life_context(20, 10);

        assert_eq!(context.size_class, "narrow");
        assert_eq!(
            context.inner_width,
            game_of_life_spec("narrow").minimum_inner_width
        );
        assert_eq!(context.resolved_height, 10);
    }

    fn strip_ansi(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for inner in chars.by_ref() {
                    if inner == 'H' {
                        out.push('\n');
                    }
                    if inner.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
}
