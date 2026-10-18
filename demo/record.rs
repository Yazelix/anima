use kinestra::{Error, Recorder, Result, Size};
use std::{
    env, fs,
    path::Path,
    process::{Command, ExitCode, Stdio},
    time::Duration,
};

fn record(r: &mut Recorder) -> Result<()> {
    let root = env::current_dir()?;
    let demo = root.join("demo");
    if !demo.join("record.rs").is_file() {
        return Err(Error::Invalid(
            "run record-demo from the Anima repository root".into(),
        ));
    }
    let assets = root.join("assets");
    let gallery = assets.join("animations");
    let rio = env::var_os("ANIMA_RIO").expect("Nix supplies ANIMA_RIO");
    let anima = env::var_os("ANIMA_BIN").expect("Nix supplies ANIMA_BIN");
    let config = fs::read_to_string(
        env::var_os("ANIMA_RIO_CONFIG").expect("Nix supplies ANIMA_RIO_CONFIG"),
    )?;
    if config.matches("size = 14.0").count() != 1
        || config.matches("family = \"JetBrains Mono\"").count() != 1
    {
        return Err(Error::Invalid(
            "demo/rio/config.toml must set one 14.0 JetBrains Mono font for the Accelerando override".into(),
        ));
    }
    let work = r.work().to_path_buf();
    let small_font = work.join("rio");
    let large_font = work.join("rio-accelerando");
    for (home, text) in [
        (&small_font, config.clone()),
        (
            &large_font,
            config.replace("size = 14.0", "size = 28.0").replace(
                "family = \"JetBrains Mono\"",
                "family = \"Accelerando Terminal\"",
            ),
        ),
    ] {
        fs::create_dir_all(home)?;
        let path = home.join("config.toml");
        fs::write(&path, text)?;
        // A rejected config would otherwise fall back to defaults and record a misconfigured terminal.
        let check = Command::new(&rio)
            .arg("--config-editor")
            .stdin(fs::File::open(&path)?)
            .stdout(Stdio::null())
            .output()?;
        if !check.status.success() {
            return Err(Error::Invalid(format!(
                "Nova Rio rejected {}: {}",
                path.display(),
                String::from_utf8_lossy(&check.stderr).trim()
            )));
        }
    }
    let recordings = demo.join(".work");
    fs::create_dir_all(&gallery)?;
    fs::create_dir_all(&recordings)?;
    // Warm-up seconds: allow trails to develop and the Aquarium whale to arrive.
    let styles = [
        ("aquarium", 23),
        ("boids_predator", 4),
        ("boids_schools", 4),
        ("friends_and_enemies", 4),
        ("primordial", 4),
        ("physarum", 15),
        ("chladni", 4),
        ("plasma", 4),
        ("mandelbrot", 4),
        ("matrix", 4),
        ("accelerando", 4),
        ("game_of_life_gliders", 4),
        ("game_of_life_tumblers", 4),
    ];
    let help = r.output(Command::new(&anima).arg("--help"))?;
    let listed = help
        .lines()
        .skip_while(|line| *line != "Styles:")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(str::trim)
        .filter(|style| !["static", "boids", "random"].contains(style));
    if !listed.eq(styles.iter().map(|(style, _)| *style)) {
        return Err(Error::Invalid(
            "update the gallery recipe and README to match packaged anima styles".into(),
        ));
    }
    r.display(Size::new(960, 540)?, None)?;
    let mut total = 0;
    for (style, warmup) in styles {
        eprintln!("Recording {style}...");
        let accelerando = style == "accelerando";
        r.launch(
            "anima-demo",
            Command::new(&rio)
                .args(["--app-id", "anima-demo", "-e"])
                .arg(&anima)
                .arg(style)
                .args(if accelerando { &["--bold"][..] } else { &[] })
                .env(
                    "NRIO_CONFIG_HOME",
                    if accelerando {
                        &large_font
                    } else {
                        &small_font
                    },
                ),
        )?;
        r.sleep(Duration::from_secs(warmup))?;
        let recording = recordings.join(format!("{style}.mp4"));
        // Leave capture headroom: the final grab can contain a partial shutdown frame.
        r.record(&recording, |r| r.sleep(Duration::from_secs(3)))?;
        r.stop_app()?;
        let gif = gallery.join(format!("{style}.gif"));
        export_gif(r, &recording, &gif, 2, accelerando)?;
        let bytes = fs::metadata(&gif)?.len();
        eprintln!("{style}.gif: {bytes} bytes");
        total += bytes;
    }
    eprintln!("Gallery total: {total} bytes");
    let montage = demo.join(".work/anima.mp4");
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    for style in [
        "primordial",
        "mandelbrot",
        "matrix",
        "game_of_life_tumblers",
    ] {
        command
            .arg("-i")
            .arg(recordings.join(format!("{style}.mp4")));
    }
    command.args(["-filter_complex", "[0:v]trim=duration=2,setpts=PTS-STARTPTS[a];[1:v]trim=duration=2,setpts=PTS-STARTPTS[b];[2:v]trim=duration=2,setpts=PTS-STARTPTS[c];[3:v]trim=duration=2,setpts=PTS-STARTPTS[d];[a][b][c][d]concat=n=4:v=1:a=0[v]",
        "-map", "[v]", "-an", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-y"]).arg(&montage);
    r.exec(&mut command)?;
    export_gif(r, &montage, &assets.join("anima.gif"), 8, false)?;
    r.poster(
        &montage,
        Duration::from_millis(2500),
        &assets.join("anima-poster.png"),
    )?;
    r.exec(
        Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration,size:stream=width,height",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(assets.join("anima.gif")),
    )
}

fn export_gif(
    r: &mut Recorder,
    input: &Path,
    output: &Path,
    seconds: u32,
    high_quality: bool,
) -> Result<()> {
    let filter = if high_quality {
        "fps=20,split[a][b];[a]palettegen=max_colors=256[p];[b][p]paletteuse=dither=none"
    } else {
        // Keep terminal blocks readable without spending 256 colors on capture noise.
        "fps=10,scale=640:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64[p];[b][p]paletteuse=dither=bayer"
    };
    r.exec(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-abort_on",
                "empty_output",
            ])
            .arg("-i")
            .arg(input)
            .arg("-filter_complex")
            .arg(format!("trim=duration={seconds},{filter}"))
            .args(["-loop", "0", "-y"])
            .arg(output),
    )
}

fn main() -> ExitCode {
    if env::args_os().nth(1).is_some() {
        eprintln!("Usage: nix run .#record-demo (from the Anima repository root)");
        return ExitCode::from(2);
    }
    kinestra::run(record)
}
