use crate::{
    BoidsAnimation, BoidsVariant, GAME_OF_LIFE_RANDOM_STYLES, GameOfLifeAnimation,
    GameOfLifeCellStyle, MANDELBROT_STYLE, MandelbrotAnimation, RawModeGuard,
    ScreenAnimationContext, ScreenFrameProducer, center_frame_lines, center_text,
    enter_screen_mode, game_of_life_spec, leave_screen_mode, mandelbrot_frame_delay,
    render_screen_frame, terminal_height, terminal_width,
};
use crossterm::event::{self, Event};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const ASCIQUARIUM_STYLE: &str = "asciiquarium";
pub const STATIC_STYLE: &str = "static";
pub const LOGO_STYLE: &str = "logo";
pub const SCREEN_STYLES: &[&str] = &[
    STATIC_STYLE,
    LOGO_STYLE,
    ASCIQUARIUM_STYLE,
    "boids",
    "boids_predator",
    "boids_schools",
    MANDELBROT_STYLE,
    "game_of_life_gliders",
    "game_of_life_oscillators",
    "game_of_life_bloom",
];
pub const SCREEN_RANDOM_STYLES: &[&str] = &[
    ASCIQUARIUM_STYLE,
    "boids",
    "boids_predator",
    "boids_schools",
    MANDELBROT_STYLE,
    "game_of_life_gliders",
    "game_of_life_oscillators",
    "game_of_life_bloom",
];

#[derive(Clone, Copy)]
enum ScreenStyle {
    Static,
    Logo,
    Asciiquarium,
    Animation(AnimationStyle),
}

#[derive(Clone, Copy)]
enum AnimationStyle {
    Boids(BoidsVariant),
    GameOfLife(&'static str),
    Mandelbrot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenArgs {
    style: String,
    cell_style: GameOfLifeCellStyle,
    duration: Option<Duration>,
    help: bool,
}

struct ScreenModeGuard;

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
        print_screen_help(command_name);
        return Ok(());
    }

    match resolve_style(&parsed.style, None, command_name)? {
        ScreenStyle::Asciiquarium => run_asciiquarium(parsed.duration),
        ScreenStyle::Static => run_in_screen_mode(|| run_static(parsed.duration)),
        ScreenStyle::Logo => run_in_screen_mode(|| run_logo(parsed.duration)),
        ScreenStyle::Animation(style) => {
            run_in_screen_mode(|| run_animation(style, parsed.cell_style, parsed.duration))
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

fn print_screen_help(command_name: &str) {
    println!("Show Yazelix terminal screen animations");
    println!();
    println!("Usage:");
    println!("  {command_name} [STYLE] [--cell-style full_block|dotted] [--duration-seconds N]");
    println!();
    println!("Styles:");
    for style in SCREEN_STYLES {
        println!("  {style}");
    }
    println!("  random");
    println!();
    println!("Notes:");
    println!("  Press any key to exit");
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
    if normalized == ASCIQUARIUM_STYLE {
        return Ok(ScreenStyle::Asciiquarium);
    }
    if let Some(variant) = BoidsVariant::from_style_name(&normalized) {
        return Ok(ScreenStyle::Animation(AnimationStyle::Boids(variant)));
    }
    if normalized == MANDELBROT_STYLE {
        return Ok(ScreenStyle::Animation(AnimationStyle::Mandelbrot));
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
    let index = random_index.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize
    });
    SCREEN_RANDOM_STYLES[index % SCREEN_RANDOM_STYLES.len()]
}

fn run_asciiquarium(duration: Option<Duration>) -> Result<(), String> {
    let mut command = Command::new(option_env!("YZS_ASCIQUARIUM_BIN").unwrap_or("asciiquarium-rs"));
    command.arg("--exit-on-any-key");
    if let Some(duration) = duration {
        command
            .arg("--duration-seconds")
            .arg(duration.as_secs().to_string());
    }
    let status = command
        .status()
        .map_err(|error| format!("could not launch asciiquarium: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("asciiquarium exited with {status}"))
    }
}

fn run_static(duration: Option<Duration>) -> Result<(), String> {
    let mut width = terminal_width();
    let mut height = terminal_height();
    render_centered_static(width, height)?;
    if let Some(duration) = duration {
        return wait_for_duration(duration);
    }

    loop {
        if poll_for_keypress(Duration::from_millis(250))? {
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
            if poll_for_keypress(delay)? {
                break;
            }
        }
        return Ok(());
    }

    let mut index = 0usize;
    loop {
        render_screen_frame(&frames[index % frames.len()])
            .map_err(|error| format!("could not render logo frame: {error}"))?;
        if poll_for_keypress(Duration::from_millis(180))? {
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
    style: AnimationStyle,
    cell_style: GameOfLifeCellStyle,
    duration: Option<Duration>,
) -> Result<(), String> {
    let started = duration.map(|_| Instant::now());
    let mut width = terminal_width();
    let mut height = terminal_height();
    let mut animation = build_animation(style, width, height, cell_style);
    let frame_delay = frame_delay(style);

    loop {
        if let (Some(started), Some(duration)) = (started, duration)
            && started.elapsed() >= duration
        {
            return Ok(());
        }

        render_screen_frame(&animation.render_frame())
            .map_err(|error| format!("could not render screen frame: {error}"))?;
        let delay = if let (Some(started), Some(duration)) = (started, duration) {
            frame_delay.min(duration.saturating_sub(started.elapsed()))
        } else {
            frame_delay
        };
        if poll_for_keypress(delay)? {
            return Ok(());
        }

        let current = (terminal_width(), terminal_height());
        if current != (width, height) {
            (width, height) = current;
            animation.resize(context_for_style(style, width, height));
        } else {
            animation.advance_frame();
        }
    }
}

fn wait_for_duration(duration: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < duration {
        let remaining = duration.saturating_sub(started.elapsed());
        if poll_for_keypress(Duration::from_millis(100).min(remaining))? {
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
        AnimationStyle::Boids(variant) => {
            Box::new(BoidsAnimation::with_variant(context, cell_style, variant))
        }
        AnimationStyle::GameOfLife(style_name) => {
            Box::new(GameOfLifeAnimation::new(style_name, context, cell_style))
        }
        AnimationStyle::Mandelbrot => Box::new(MandelbrotAnimation::new(context)),
    }
}

fn context_for_style(style: AnimationStyle, width: usize, height: usize) -> ScreenAnimationContext {
    match style {
        AnimationStyle::GameOfLife(_) => game_of_life_context(width, height),
        AnimationStyle::Boids(_) | AnimationStyle::Mandelbrot => full_screen_context(width, height),
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
        AnimationStyle::Boids(_) => Duration::from_millis(70),
        AnimationStyle::Mandelbrot => mandelbrot_frame_delay(),
        AnimationStyle::GameOfLife(_) => Duration::from_millis(160),
    }
}

fn poll_for_keypress(timeout: Duration) -> Result<bool, String> {
    if !event::poll(timeout).map_err(|error| format!("could not poll terminal input: {error}"))? {
        return Ok(false);
    }

    loop {
        match event::read().map_err(|error| format!("could not read terminal input: {error}"))? {
            Event::Key(_) => return Ok(true),
            _ => {
                if !event::poll(Duration::from_millis(0))
                    .map_err(|error| format!("could not poll terminal input: {error}"))?
                {
                    return Ok(false);
                }
            }
        }
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
    for accent in [
        "reproducible",
        "declarative",
        "helix",
        "zellij",
        "terminals",
        "shells",
        "packs",
        "SSH",
    ] {
        if let Some(index) = remaining.find(accent) {
            out.push_str(&green(&remaining[..index]));
            out.push_str(&blue(accent));
            remaining = &remaining[index + accent.len()..];
        }
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

    // Test lane: default

    // Defends: every advertised style is executable.
    #[test]
    fn supported_styles_resolve() {
        for style in SCREEN_STYLES {
            assert!(resolve_style(style, None, "yzs").is_ok());
        }
    }

    // Defends: random welcome avoids card-like styles while keeping them explicitly selectable.
    #[test]
    fn random_pool_skips_static_and_logo() {
        for style in [STATIC_STYLE, LOGO_STYLE] {
            assert!(!SCREEN_RANDOM_STYLES.contains(&style));
            assert!(resolve_style(style, None, "yzs").is_ok());
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
            "yzs",
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
    }

    // Regression: the deleted image-backed magician style must not return through random selection.
    #[test]
    fn random_style_skips_magician() {
        assert!(!SCREEN_RANDOM_STYLES.contains(&"magician"));
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
