use kinestra::{Error, Recorder, Result, Size};
use std::{
    env, fs,
    path::PathBuf,
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
    let mars = PathBuf::from(env::var_os("ANIMA_MARS").expect("Nix supplies ANIMA_MARS"));
    let anima = env::var_os("ANIMA_BIN").expect("Nix supplies ANIMA_BIN");
    let work = r.work().to_path_buf();
    fs::create_dir_all(&assets)?;
    fs::create_dir_all(demo.join(".work"))?;
    r.display(Size::new(960, 540)?, None)?;
    let styles = [
        "primordial",
        "mandelbrot",
        "matrix",
        "game_of_life_tumblers",
    ];
    for style in styles {
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
        r.sleep(Duration::from_secs(4))?;
        r.record(&work.join(format!("{style}.mp4")), |r| {
            r.sleep(Duration::from_secs(2))
        })?;
    }
    r.stop_app()?;
    let montage = demo.join(".work/anima.mp4");
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    for style in styles {
        command.arg("-i").arg(work.join(format!("{style}.mp4")));
    }
    command.args(["-filter_complex", "[0:v]trim=duration=2,setpts=PTS-STARTPTS[a];[1:v]trim=duration=2,setpts=PTS-STARTPTS[b];[2:v]trim=duration=2,setpts=PTS-STARTPTS[c];[3:v]trim=duration=2,setpts=PTS-STARTPTS[d];[a][b][c][d]concat=n=4:v=1:a=0[v]",
        "-map", "[v]", "-an", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-y"]).arg(&montage);
    r.exec(&mut command)?;
    r.gif(&montage, &assets.join("anima.gif"), 640, 10)?;
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

fn main() -> ExitCode {
    if env::args_os().nth(1).is_some() {
        eprintln!("Usage: nix run .#record-demo (from the Anima repository root)");
        return ExitCode::from(2);
    }
    kinestra::run(record)
}
