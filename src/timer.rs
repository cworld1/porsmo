use crate::alert::Alerter;
use crate::stopwatch::Stopwatch;
use crate::terminal::running_color;
use crate::{format::format_duration, input::Command};
use crate::{prelude::*, CounterUI};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{
    cursor::{MoveTo, MoveToNextLine},
    queue,
    style::{Print, Stylize, Color},
};
use std::io::Write;
use std::time::Duration;

#[allow(dead_code)]
fn progress_bar(elapsed: Duration, target: Duration, width: usize) -> String {
    let ratio = if target.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f64() / target.as_secs_f64()).clamp(0.0, 1.0)
    };
    let filled = (ratio * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let percent = (ratio * 100.0).round() as usize;
    let bar = format!(
        "[{}{}] {percent:>3}%",
        "█".repeat(filled),
        "-".repeat(empty),
    );
    bar
}

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

fn timer_show(
    out: &mut impl Write,
    elapsed: Duration,
    target: Duration,
    is_running: bool,
    alerter: &mut Alerter,
) -> Result<()> {
    let (title, timer_raw, controls) = if elapsed < target {
        let time_left = target.saturating_sub(elapsed);
        (
            "Timer",
            format_duration(time_left),
            "[Q]: quit, [Space]: pause/resume",
        )
    } else {
        alerter.alert_once(
            "The timer has ended!",
            format!(
                "Your Timer of {initial} has ended",
                initial = format_duration(target)
            ),
        );
        let excess_time = format_duration(elapsed.saturating_sub(target));
        (
            "Timer has ended",
            format!("+{excess_time}"),
            "[Q]: quit, [Space]: pause/resume",
        )
    };
    // prepare styled time and padding before moving values into the queue
    let styled_timer = timer_raw.clone().with(running_color(is_running));
    let _timer_pad = UI_WIDTH.saturating_sub(timer_raw.len());
    // compute progress geometry
    let bar_width = 30usize;
    let ratio = if target.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f64() / target.as_secs_f64()).clamp(0.0, 1.0)
    };
    let filled = (ratio * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let percent = (ratio * 100.0).round() as usize;
    let percent_str = format!("{percent:>3}%");
    // content length = 1('[')+bar_width+1(']')+1(space)+percent_len
    let content_len = 1 + bar_width + 1 + 1 + percent_str.len();
    let pad_left = (UI_WIDTH.saturating_sub(content_len)) / 2;
    let pad_right = UI_WIDTH.saturating_sub(content_len + pad_left);

    // split controls defensively (timer controls short, but keep consistency)
    let parts: Vec<&str> = controls.split(',').map(|s| s.trim()).collect();
    let mid = (parts.len() + 1) / 2;
    let controls1 = parts[..mid].join(", ");
    let controls2 = parts[mid..].join(", ");
    let controls1_len = controls1.len();
    let controls2_len = controls2.len();

    queue!(
        out,
        MoveTo(0, 0),
        Print(frame_top()),
        Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    // title: print frame borders separately so only content colored
    Print("│"),
    Print(center_text(title).with(Color::Cyan)),
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
    // timer centered
    Print("│"),
    Print(" ".repeat((UI_WIDTH.saturating_sub(timer_raw.len()))/2)),
    Print(styled_timer),
    Print(" ".repeat(UI_WIDTH.saturating_sub(timer_raw.len()) - (UI_WIDTH.saturating_sub(timer_raw.len()))/2)),
    Print("│"),
        Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    // blank framed separator for symmetry
    Print(frame_line("")),
    Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    // progress bar
    Print("│"),
    Print(" ".repeat(pad_left)),
    Print("["),
    Print("█".repeat(filled).with(running_color(is_running))),
    Print("-".repeat(empty).with(Color::DarkGrey)),
    Print("] "),
    Print(percent_str.with(Color::White)),
    Print(" ".repeat(pad_right)),
    Print("│"),
    Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    // blank framed separator for spacing
    Print(frame_line("")),
    Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    Print(frame_sep()),
    Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    // controls split and printed with uncolored borders
    Print("│"),
    Print(controls1.clone().with(Color::DarkGrey)),
    Print(" ".repeat(UI_WIDTH.saturating_sub(controls1_len))),
    Print("│"),
    Clear(ClearType::UntilNewLine),
    MoveToNextLine(1),
    Print("│"),
    Print(controls2.clone().with(Color::DarkGrey)),
    Print(" ".repeat(UI_WIDTH.saturating_sub(controls2_len))),
    Print("│"),
        Clear(ClearType::UntilNewLine),
        MoveToNextLine(1),
        Print(frame_bottom()),
        Clear(ClearType::FromCursorDown),
    )?;
    out.flush()?;
    Ok(())
}

fn timer_update(command: Command, stopwatch: &mut Stopwatch) {
    match command {
        Command::Pause => stopwatch.stop(),
        Command::Resume => stopwatch.start(),
        Command::Toggle | Command::Enter => stopwatch.toggle(),
        Command::Reset => *stopwatch = Stopwatch::default(),
        _ => (),
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TimerUI {
    stopwatch: Stopwatch,
    target: Duration,
    alerter: Alerter,
}

impl TimerUI {
    pub fn new(target: Duration) -> Self {
        Self {
            target,
            ..Default::default()
        }
    }
}

impl CounterUI for TimerUI {
    fn show(&mut self, out: &mut impl Write) -> Result<()> {
        let elapsed = self.stopwatch.elapsed();
        let is_running = self.stopwatch.started();
        timer_show(out, elapsed, self.target, is_running, &mut self.alerter)
    }

    fn update(&mut self, command: Command) {
        timer_update(command, &mut self.stopwatch)
    }
}
