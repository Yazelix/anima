use crossterm::{
    SynchronizedUpdate,
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{
        Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io::{self, Write};

use crate::RgbColor;

fn command_string(write_commands: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
    crossterm::style::force_color_output(true);
    let mut output = Vec::new();
    write_commands(&mut output).expect("crossterm command writes to memory");
    String::from_utf8(output).expect("crossterm commands emit UTF-8")
}

pub(crate) fn styled(text: char, color: Color) -> String {
    command_string(|output| {
        queue!(
            output,
            SetForegroundColor(color),
            Print(text),
            SetForegroundColor(Color::Reset)
        )?;
        Ok(())
    })
}

pub(crate) fn styled_with_background(text: char, color: Color, background: Color) -> String {
    command_string(|output| {
        queue!(
            output,
            SetForegroundColor(color),
            SetBackgroundColor(background),
            Print(text),
            SetBackgroundColor(Color::Reset),
            SetForegroundColor(Color::Reset)
        )?;
        Ok(())
    })
}

pub(crate) fn write_truecolor(
    output: &mut impl Write,
    text: char,
    foreground: RgbColor,
    background: Option<RgbColor>,
) -> io::Result<()> {
    crossterm::style::force_color_output(true);
    queue!(
        output,
        SetForegroundColor(Color::Rgb {
            r: foreground.red,
            g: foreground.green,
            b: foreground.blue
        })
    )?;
    if let Some(color) = background {
        queue!(
            output,
            SetBackgroundColor(Color::Rgb {
                r: color.red,
                g: color.green,
                b: color.blue
            })
        )?;
    }
    queue!(
        output,
        Print(text),
        SetBackgroundColor(Color::Reset),
        SetForegroundColor(Color::Reset)
    )
}

pub(crate) fn screen_frame_output(frame: &[String]) -> String {
    if frame.is_empty() {
        return clear_screen_sequence();
    }
    command_string(|output| {
        queue!(output, ResetColor)?;
        for (row_index, line) in frame.iter().enumerate() {
            // Keep the old lower rows visible until their replacements arrive.
            // Only the last row erases any leftover rows from a taller frame.
            let clear = if row_index == frame.len() - 1 {
                ClearType::FromCursorDown
            } else {
                ClearType::CurrentLine
            };
            queue!(
                output,
                MoveTo(0, row_index.min(u16::MAX as usize) as u16),
                Clear(clear),
                Print(line),
                ResetColor
            )?;
        }
        Ok(())
    })
}

pub(crate) fn render_screen_frame(
    writer: &mut impl Write,
    frame: &[String],
    overlay: &str,
) -> io::Result<()> {
    let mut output = screen_frame_output(frame);
    output.push_str(overlay);
    writer.sync_update(|writer| writer.write_all(output.as_bytes()))?
}

pub(crate) fn clear_screen_sequence() -> String {
    command_string(|output| {
        queue!(output, ResetColor, MoveTo(0, 0), Clear(ClearType::All))?;
        Ok(())
    })
}

pub(crate) fn enter_screen_mode() -> io::Result<()> {
    crossterm::style::force_color_output(true);
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        DisableLineWrap,
        ResetColor,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )
    .inspect_err(|_| {
        let _ = leave_screen_mode();
    })
}

pub(crate) fn leave_screen_mode() -> io::Result<()> {
    crossterm::style::force_color_output(true);
    let mut stdout = io::stdout();
    execute!(
        stdout,
        ResetColor,
        EnableLineWrap,
        Show,
        LeaveAlternateScreen
    )
}
