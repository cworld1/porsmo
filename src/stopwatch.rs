use std::time::Instant;
use std::{io::Write, time::Duration};

use crate::{prelude::*, CounterUI};
use crate::terminal::running_color;
use crate::{format::format_duration, input::Command};
use crossterm::{
    cursor::{MoveTo, MoveToNextLine},
    queue,
    style::{Print, Stylize, Color},
    terminal::{Clear, ClearType},
};

const UI_WIDTH: usize = 50;

fn frame_top() -> String {
    format!("╭{}╮", "─".repeat(UI_WIDTH))
}

fn frame_bottom() -> String {
    format!("╰{}╯", "─".repeat(UI_WIDTH))
}

fn frame_sep() -> String {
    format!("│{}│", "─".repeat(UI_WIDTH))
}

fn frame_line(s: &str) -> String {
    let content = if s.len() >= UI_WIDTH {
        s[..UI_WIDTH].to_string()
    } else {
        format!("{}{}", s, " ".repeat(UI_WIDTH - s.len()))
    };
    format!("│{}│", content)
}

fn center_text(s: &str) -> String {
    if s.len() >= UI_WIDTH {
        s[..UI_WIDTH].to_string()
    } else {
        let pad = (UI_WIDTH - s.len()) / 2;
        format!("{}{}{}", " ".repeat(pad), s, " ".repeat(UI_WIDTH - pad - s.len()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stopwatch {
    start_time: Option<Instant>,
    elapsed_before: Duration,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self {
            start_time: Some(Instant::now()),
            elapsed_before: Duration::ZERO,
        }
    }
}

impl Stopwatch {
    pub fn new(start_time: Option<Instant>, elapsed_before: Duration) -> Self {
        Self {
            start_time,
            elapsed_before,
        }
    }

    pub fn elapsed(&self) -> Duration {
        match self.start_time {
            Some(start_time) => self.elapsed_before + start_time.elapsed(),
            None => self.elapsed_before,
        }
    }

    pub fn started(&self) -> bool {
        if matches!(self.start_time, None) {
            false
        } else {
            true
        }
    }

    pub fn start(&mut self) {
        if matches!(self.start_time, None) {
            self.start_time = Some(Instant::now());
        }
    }

    pub fn stop(&mut self) {
        if let Some(start_time) = self.start_time {
            self.elapsed_before += start_time.elapsed();
            self.start_time = None;
        }
    }

    pub fn toggle(&mut self) {
        match self.start_time {
            Some(start_time) => {
                self.elapsed_before += start_time.elapsed();
                self.start_time = None;
            }
            None => {
                self.start_time = Some(Instant::now());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StopwatchUI {
    stopwatch: Stopwatch,
}

impl CounterUI for StopwatchUI {
    fn show(&mut self, out: &mut impl Write) -> Result<()> {
        let elapsed = self.stopwatch.elapsed();
        let is_running = self.stopwatch.started();
        // prepare controls split
    let controls = "[Q]: quit, [Space]: pause/resume";
    let parts: Vec<&str> = controls.split(',').map(|s| s.trim()).collect();
    let mid = (parts.len() + 1) / 2;
    let controls1 = parts[..mid].join(", ");
    let controls2 = parts[mid..].join(", ");
    let len1 = controls1.len();
    let len2 = controls2.len();
    let styled_controls1 = controls1.clone().with(Color::DarkGrey);
    let styled_controls2 = controls2.clone().with(Color::DarkGrey);

        let time_raw = format_duration(elapsed);
        let styled_time = time_raw.clone().with(running_color(is_running));

        queue!(
            out,
            MoveTo(0, 0),
            Print(frame_top()),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            // title
            Print("│"),
            Print(center_text("Stopwatch").with(Color::Cyan)),
            Print("│"),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            Print(frame_sep()),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            // blank framed line above time for symmetry
            Print(frame_line("")),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            // centered time
            Print("│"),
            Print(" ".repeat((UI_WIDTH.saturating_sub(time_raw.len()))/2)),
            Print(styled_time),
            Print(" ".repeat(UI_WIDTH.saturating_sub(time_raw.len()) - (UI_WIDTH.saturating_sub(time_raw.len()))/2)),
            Print("│"),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            // blank separator
            Print(frame_line("")),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            // controls with uncolored borders
            Print("│"),
            Print(styled_controls1),
            Print(" ".repeat(UI_WIDTH.saturating_sub(len1))),
            Print("│"),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            Print("│"),
            Print(styled_controls2),
            Print(" ".repeat(UI_WIDTH.saturating_sub(len2))),
            Print("│"),
            Clear(ClearType::UntilNewLine),
            MoveToNextLine(1),
            Print(frame_bottom()),
            Clear(ClearType::FromCursorDown),
        )?;
        out.flush()?;
        Ok(())
    }

    fn update(&mut self, command: Command) {
        match command {
            Command::Pause => self.stopwatch.stop(),
            Command::Resume => self.stopwatch.start(),
            Command::Toggle | Command::Enter => self.stopwatch.toggle(),
            _ => (),
        }
    }
}

