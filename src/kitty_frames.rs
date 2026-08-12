use crossterm::event::{self, Event};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyFrameLayout {
    pub columns: usize,
    pub rows: usize,
    pub top_padding: usize,
    pub left_padding: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyFrameSequence {
    pub frame_paths: Vec<PathBuf>,
    pub frame_delay: Duration,
    pub image_id: u32,
    pub attribution: Option<String>,
    pub edge_inset_columns: usize,
    pub edge_inset_rows: usize,
}

pub fn kitty_frame_layout(
    width: usize,
    height: usize,
    edge_inset_columns: usize,
    edge_inset_rows: usize,
) -> KittyFrameLayout {
    let available_rows = height
        .saturating_sub(edge_inset_rows.saturating_mul(2).saturating_add(1))
        .max(1);
    let available_columns = width
        .saturating_sub(edge_inset_columns.saturating_mul(2))
        .max(1);
    let rows = available_rows.min(available_columns.max(2) / 2).max(1);
    let columns = rows.saturating_mul(2).min(available_columns);
    let top_padding = height.saturating_sub(rows + 1) / 2;
    let left_padding = width.saturating_sub(columns) / 2;

    KittyFrameLayout {
        columns,
        rows,
        top_padding,
        left_padding,
    }
}

pub fn kitty_png_file_command(image_id: u32, columns: usize, rows: usize, path: &Path) -> String {
    let payload = base64_encode_bytes(path.as_os_str().as_encoded_bytes());
    format!(
        "\u{1b}_Ga=T,f=100,t=f,i={image_id},p=1,c={columns},r={rows},C=1,z=-1,q=2;{payload}\u{1b}\\"
    )
}

pub fn kitty_delete_image_command(image_id: u32) -> String {
    format!(
        "\u{1b}_Ga=d,d=i,i={image_id},p=1,q=2;\u{1b}\\\
         \u{1b}_Ga=d,d=I,i={image_id},q=2;\u{1b}\\"
    )
}

pub fn cleanup_kitty_image(image_id: u32) -> io::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    write!(
        stdout,
        "{}{}",
        kitty_delete_image_command(image_id),
        crate::terminal_control::clear_screen_sequence()
    )?;
    stdout.flush()
}

pub fn draw_kitty_png_frame(
    sequence: &KittyFrameSequence,
    width: usize,
    height: usize,
    frame_index: usize,
) -> io::Result<()> {
    if sequence.frame_paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Kitty frame sequence requires at least one frame path",
        ));
    }

    let layout = kitty_frame_layout(
        width,
        height,
        sequence.edge_inset_columns,
        sequence.edge_inset_rows,
    );
    let frame_path = &sequence.frame_paths[frame_index % sequence.frame_paths.len()];

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    write!(
        stdout,
        "{}",
        crate::terminal_control::clear_screen_sequence()
    )?;
    for _ in 0..layout.top_padding {
        stdout.write_all(b"\r\n")?;
    }
    write!(stdout, "{}", " ".repeat(layout.left_padding))?;
    write!(
        stdout,
        "{}",
        kitty_png_file_command(sequence.image_id, layout.columns, layout.rows, frame_path)
    )?;
    for _ in 0..layout.rows {
        stdout.write_all(b"\r\n")?;
    }
    if let Some(attribution) = &sequence.attribution {
        write!(stdout, "{}\r\n", crate::center_text(attribution, width))?;
    }
    stdout.flush()
}

pub fn play_kitty_png_frame_sequence(
    sequence: &KittyFrameSequence,
    duration: Option<Duration>,
    terminal_width: impl Fn() -> usize,
    terminal_height: impl Fn() -> usize,
) -> io::Result<()> {
    let started_at = Instant::now();
    if sequence.frame_paths.is_empty() {
        return draw_kitty_png_frame(sequence, terminal_width(), terminal_height(), 0);
    }
    let mut frame_index = 0usize;

    let play_result = loop {
        if let Err(error) =
            draw_kitty_png_frame(sequence, terminal_width(), terminal_height(), frame_index)
        {
            break Err(error);
        }
        let Some(delay) = kitty_frame_wait(sequence.frame_delay, duration, started_at.elapsed())
        else {
            break Ok(());
        };
        match poll_for_keypress(delay) {
            Ok(true) => break Ok(()),
            Ok(false) => {}
            Err(error) => break Err(error),
        }
        frame_index += 1;

        if let Some(duration) = duration
            && started_at.elapsed() >= duration
        {
            break Ok(());
        }
    };

    let cleanup_result = cleanup_kitty_image(sequence.image_id);
    play_result.and(cleanup_result)
}

fn kitty_frame_wait(
    frame_delay: Duration,
    duration: Option<Duration>,
    elapsed: Duration,
) -> Option<Duration> {
    match duration {
        Some(duration) if elapsed >= duration => None,
        Some(duration) => Some(frame_delay.min(duration - elapsed)),
        None => Some(frame_delay),
    }
}

pub fn poll_for_keypress(timeout: Duration) -> io::Result<bool> {
    if !event::poll(timeout)? {
        return Ok(false);
    }

    loop {
        match event::read()? {
            Event::Key(_) => return Ok(true),
            _ => {
                if !event::poll(Duration::from_millis(0))? {
                    return Ok(false);
                }
            }
        }
    }
}

fn base64_encode_bytes(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let packed = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        encoded.push(TABLE[((packed >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((packed >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[((packed >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(packed & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test lane: default

    // Defends: Kitty frame placement stays inset and underneath terminal text so multiplexer pane frames remain visible.
    #[test]
    fn kitty_frame_layout_preserves_edge_inset() {
        assert_eq!(
            kitty_frame_layout(120, 40, 8, 8),
            KittyFrameLayout {
                columns: 46,
                rows: 23,
                top_padding: 8,
                left_padding: 37,
            }
        );
        assert_eq!(
            kitty_frame_layout(1, 1, usize::MAX, usize::MAX),
            KittyFrameLayout {
                columns: 1,
                rows: 1,
                top_padding: 0,
                left_padding: 0,
            }
        );
    }

    // Defends: Kitty graphics commands use file-backed PNG placement plus deletion commands that clear both placement and image id.
    #[test]
    fn kitty_commands_use_file_payload_z_index_and_full_cleanup() {
        let command = kitty_png_file_command(123, 80, 40, Path::new("/tmp/frame.png"));
        assert!(command.starts_with("\u{1b}_Ga=T,f=100,t=f,i=123,p=1,c=80,r=40,C=1,z=-1,q=2;"));
        assert!(command.contains("L3RtcC9mcmFtZS5wbmc="));
        assert!(command.ends_with("\u{1b}\\"));

        let delete_command = kitty_delete_image_command(123);
        assert!(delete_command.contains("\u{1b}_Ga=d,d=i,i=123,p=1,q=2;\u{1b}\\"));
        assert!(delete_command.contains("\u{1b}_Ga=d,d=I,i=123,q=2;\u{1b}\\"));
    }

    #[test]
    fn kitty_frame_wait_honors_duration() {
        let frame_delay = Duration::from_secs(1);

        assert_eq!(
            kitty_frame_wait(frame_delay, None, Duration::ZERO),
            Some(frame_delay)
        );
        assert_eq!(
            kitty_frame_wait(frame_delay, Some(Duration::ZERO), Duration::ZERO),
            None
        );
        assert_eq!(
            kitty_frame_wait(
                frame_delay,
                Some(Duration::from_millis(80)),
                Duration::from_millis(30),
            ),
            Some(Duration::from_millis(50))
        );
    }

    #[cfg(unix)]
    #[test]
    fn kitty_file_command_preserves_non_utf8_path_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(b"/tmp/\xff.png".to_vec()));
        let command = kitty_png_file_command(123, 80, 40, &path);

        assert!(command.contains("L3RtcC//LnBuZw=="));
    }
}
