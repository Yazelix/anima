use kinestra::{Error, Recorder, Result, Size};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Duration,
};

fn record(r: &mut Recorder) -> Result<()> {
    let root = env::current_dir()?;
    let demo = root.join("demo");
    if !demo.join("mars/config.toml").is_file() {
        return Err(Error::Invalid(
            "run record-demo from the Anima repository root".into(),
        ));
    }
    let assets = root.join("assets");
    let gallery = assets.join("animations");
    let mars = PathBuf::from(env::var_os("ANIMA_MARS").expect("Nix supplies ANIMA_MARS"));
    let anima = env::var_os("ANIMA_BIN").expect("Nix supplies ANIMA_BIN");
    let work = r.work().to_path_buf();
    let recordings = demo.join(".work");
    fs::create_dir_all(&gallery)?;
    fs::create_dir_all(&recordings)?;
    // Warm-up seconds: trail networks need longer to develop than the other styles.
    let styles = [
        ("logo", 4),
        ("aquarium", 4),
        ("boids_predator", 4),
        ("boids_schools", 4),
        ("friends_and_enemies", 4),
        ("primordial", 4),
        ("physarum", 15),
        ("chladni", 4),
        ("plasma", 4),
        ("mandelbrot", 4),
        ("matrix", 4),
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
        r.launch(
            "anima-demo",
            Command::new(mars.join("bin/mars"))
                .args(["-e"])
                .arg(&anima)
                .arg(style)
                .env("MARS_CONFIG_HOME", demo.join("mars"))
                .env("MARS_APP_ID", "anima-demo")
                .env("MARS_BASE_CONFIG_HOME", mars.join("share/mars"))
                .env("YAZELIX_CONFIG_HOME", work.join("config"))
                .env_remove("YAZELIX_CURSOR_CONFIG")
                .env_remove("MARS_PROFILE"),
        )?;
        r.sleep(Duration::from_secs(warmup))?;
        let recording = recordings.join(format!("{style}.mp4"));
        // Leave capture headroom: the final grab can contain a partial shutdown frame.
        r.record(&recording, |r| r.sleep(Duration::from_secs(3)))?;
        r.stop_app()?;
        let gif = gallery.join(format!("{style}.gif"));
        export_gif(r, &recording, &gif, 2)?;
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
    export_gif(r, &montage, &assets.join("anima.gif"), 8)?;
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

fn export_gif(r: &mut Recorder, input: &Path, output: &Path, seconds: u32) -> Result<()> {
    // Keep terminal blocks readable without spending 256 colors on capture noise.
    r.exec(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-abort_on", "empty_output"])
            .arg("-i")
            .arg(input)
            .arg("-filter_complex")
            .arg(format!("trim=duration={seconds},fps=10,scale=640:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64[p];[b][p]paletteuse=dither=bayer"))
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
