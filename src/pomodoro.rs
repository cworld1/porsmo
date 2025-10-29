use crate::alert::Alerter;
use crate::input::{get_event, TIMEOUT};
use crate::stopwatch::Stopwatch;
use crate::terminal::running_color;
use crate::{format::format_duration, input::Command};
use crate::{prelude::*, CounterUI};
use crossterm::cursor::{MoveTo, MoveToNextLine};
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{queue, style::Color, style::Stylize};

use std::io::Write;
use std::time::{Duration, Instant};

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
    format!("[{}{}] {percent:>3}%", "█".repeat(filled), "-".repeat(empty))
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

#[derive(Clone, Copy, Debug, Default)]
pub enum Mode {
    #[default]
    Work,
    Break,
    LongBreak,
}

#[derive(Copy, Clone, Debug)]
pub struct PomodoroConfig {
    pub work_time: Duration,
    pub break_time: Duration,
    pub long_break: Duration,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self::short()
    }
}

impl PomodoroConfig {
    pub fn new(work_time: Duration, break_time: Duration, long_break: Duration) -> Self {
        Self {
            work_time,
            break_time,
            long_break,
        }
    }

    pub fn short() -> Self {
        Self {
            work_time: Duration::from_secs(25 * 60),
            break_time: Duration::from_secs(5 * 60),
            long_break: Duration::from_secs(10 * 60),
        }
    }

    pub fn long() -> Self {
        Self {
            work_time: Duration::from_secs(55 * 60),
            break_time: Duration::from_secs(10 * 60),
            long_break: Duration::from_secs(20 * 60),
        }
    }

    pub fn current_target(&self, mode: Mode) -> Duration {
        match mode {
            Mode::Work => self.work_time,
            Mode::Break => self.break_time,
            Mode::LongBreak => self.long_break,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Session {
    pub mode: Mode,
    pub round: u32,
    pub elapsed_time: [Duration; 2],
}

impl Default for Session {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            round: 1,
            elapsed_time: [Duration::ZERO; 2],
        }
    }
}

impl Session {
    pub fn advance(self, duration: Duration) -> Self {
        match self.mode {
            Mode::Work if self.round % 4 == 0 => Self {
                mode: Mode::LongBreak,
                elapsed_time: [self.elapsed_time[0] + duration, self.elapsed_time[1]],
                ..self
            },
            Mode::Work => Self {
                mode: Mode::Break,
                elapsed_time: [self.elapsed_time[0] + duration, self.elapsed_time[1]],
                ..self
            },
            Mode::Break | Mode::LongBreak => Self {
                mode: Mode::Work,
                round: self.round + 1,
                elapsed_time: [self.elapsed_time[0], self.elapsed_time[1] + duration],
            },
        }
    }

    pub fn next(&self) -> Self {
        self.advance(Duration::ZERO)
    }
}

const CONTROLS: &str = "[Q]: quit, [Shift S]: Skip, [Space]: pause/resume, [R]: reset";
const ENDING_CONTROLS: &str =
    "[Q]: quit, [Shift S]: Skip, [Space]: pause/resume, [Enter]: Next, [R]: reset";
const SKIP_CONTROLS: &str = "[Enter]: Yes, [Q/N]: No";

fn default_title(mode: Mode) -> &'static str {
    match mode {
        Mode::Work => "Pomodoro (Work)",
        Mode::Break => "Pomodoro (Break)",
        Mode::LongBreak => "Pomodoro (Long Break)",
    }
}

fn end_title(next_mode: Mode) -> &'static str {
    match next_mode {
        Mode::Work => "Break has ended! Start work?",
        Mode::Break => "Work has ended! Start break?",
        Mode::LongBreak => "Work has ended! Start a long break",
    }
}

fn alert_message(next_mode: Mode) -> (&'static str, &'static str) {
    match next_mode {
        Mode::Work => ("Your break ended!", "Time for some work"),
        Mode::Break => ("Pomodoro ended!", "Time for a short break"),
        Mode::LongBreak => ("Pomodoro 4 sessions complete!", "Time for a long break"),
    }
}

#[derive(Debug, Clone, Copy)]
enum UIMode {
    Skip(Duration),
    Running(Stopwatch),
}

impl Default for UIMode {
    fn default() -> Self {
        Self::Running(Stopwatch::default())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PomodoroUI {
    config: PomodoroConfig,
    session: Session,
    ui_mode: UIMode,
    alerter: Alerter,
}

impl PomodoroUI {
    pub fn new(config: PomodoroConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
}

impl CounterUI for PomodoroUI {
    fn show(&mut self, out: &mut impl Write) -> Result<()> {
        pomodoro_show(
            out,
            &self.config,
            &self.ui_mode,
            &self.session,
            &mut self.alerter,
        )
    }

    fn update(&mut self, command: Command) {
        pomodoro_update(
            command,
            &self.config,
            &mut self.alerter,
            &mut self.ui_mode,
            &mut self.session,
        );
    }

    fn run_ui(mut self, out: &mut impl Write) -> Result<String> {
        loop {
            self.show(out)?;
            if let Some(cmd) = get_event(TIMEOUT)?.map(Command::from) {
                match cmd {
                    Command::Quit => {
                        self.session = match self.ui_mode {
                            UIMode::Skip(elapsed) => self.session.advance(elapsed),
                            UIMode::Running(stopwatch) => self.session.advance(stopwatch.elapsed()),
                        };
                        break;
                    }
                    cmd => self.update(cmd),
                }
            }
        }
        Ok(format!(
            "You have spent {} working and {} on break. Well done!",
            format_duration(self.session.elapsed_time[0]),
            format_duration(self.session.elapsed_time[1]),
        ))
    }
}

fn pomodoro_update(
    command: Command,
    config: &PomodoroConfig,
    alerter: &mut Alerter,
    ui_mode: &mut UIMode,
    session: &mut Session,
) {
    match ui_mode {
        UIMode::Skip(elapsed) => match command {
            Command::Quit | Command::No => {
                *ui_mode = UIMode::Running(Stopwatch::new(Some(Instant::now()), *elapsed))
            }
            Command::Enter | Command::Yes => {
                alerter.reset();
                *session = session.advance(*elapsed);
                *ui_mode = UIMode::Running(Stopwatch::default());
            }
            _ => (),
        },
        UIMode::Running(ref mut stopwatch) => {
            let elapsed = stopwatch.elapsed();
            let target = config.current_target(session.mode);

            match command {
                Command::Enter if elapsed >= target => {
                    alerter.reset();
                    *session = session.advance(elapsed);
                    *ui_mode = UIMode::Running(Stopwatch::default());
                }
                Command::Pause => stopwatch.stop(),
                Command::Resume => stopwatch.start(),
                Command::Toggle => stopwatch.toggle(),
                Command::Skip => *ui_mode = UIMode::Skip(elapsed),
                Command::Reset => *stopwatch = Stopwatch::default(),
                _ => (),
            }
        }
    }
}

fn pomodoro_show(
    out: &mut impl Write,
    config: &PomodoroConfig,
    ui_mode: &UIMode,
    session: &Session,
    alerter: &mut Alerter,
) -> Result<()> {
    let target = config.current_target(session.mode);
    let round_number = format!("Session: {}", session.round);
    match ui_mode {
        UIMode::Skip(..) => {
            let (color, skip_to) = match session.next().mode {
                Mode::Work => (Color::Red, "skip to work?"),
                Mode::Break => (Color::Green, "skip to break?"),
                Mode::LongBreak => (Color::Green, "skip to long break?"),
            };
            queue!(
                out,
                MoveTo(0, 0),
                Print(frame_top()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // title
                Print("│"),
                Print(center_text(skip_to).with(color)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_sep()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line above where time would be
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // progress bar (centered)
                Print("│"),
                Print(" ".repeat((UI_WIDTH - (1 + 30 + 1 + 1 + 4)) / 2)),
                Print("["),
                Print("-".repeat(30).with(Color::DarkGrey)),
                Print("] "),
                Print("  0%".with(Color::White)),
                Print(" ".repeat((UI_WIDTH - (1 + 30 + 1 + 1 + 4) + 1) / 2)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line below progress for symmetry
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_sep()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // controls (uncolored borders, dimmed text)
                Print("│"),
                Print(SKIP_CONTROLS.with(Color::DarkGrey)),
                Print(" ".repeat(UI_WIDTH.saturating_sub(SKIP_CONTROLS.len()))),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(&round_number)),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_bottom()),
                Clear(ClearType::FromCursorDown),
            )?;
        }
        UIMode::Running(stopwatch) if stopwatch.elapsed() < target => {
            let time_left = target.saturating_sub(stopwatch.elapsed());
            let time_raw = format_duration(&time_left);
            let styled_time = time_raw.clone().with(running_color(stopwatch.started()));
            let bar_width = 30usize;
            let ratio = if target.is_zero() {
                1.0
            } else {
                (stopwatch.elapsed().as_secs_f64() / target.as_secs_f64()).clamp(0.0, 1.0)
            };
            let filled = (ratio * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            let percent = (ratio * 100.0).round() as usize;
            let percent_str = format!("{percent:>3}%");
            let content_len = 1 + bar_width + 1 + 1 + percent_str.len();
            let pad_left = (UI_WIDTH.saturating_sub(content_len)) / 2;
            let pad_right = UI_WIDTH.saturating_sub(content_len + pad_left);

            // split controls into two reasonable lines to avoid truncation
            let parts: Vec<&str> = CONTROLS.split(',').map(|s| s.trim()).collect();
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
                // title: print borders separately so frame isn't colored
                Print("│"),
                Print(center_text(default_title(session.mode)).with(Color::Cyan)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_sep()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line above time for symmetry
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // time line (centered)
                Print("│"),
                Print(" ".repeat((UI_WIDTH.saturating_sub(time_raw.len()))/2)),
                Print(styled_time),
                Print(" ".repeat(UI_WIDTH.saturating_sub(time_raw.len()) - (UI_WIDTH.saturating_sub(time_raw.len()))/2)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line below time for symmetry
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // centered colored progress bar
                Print("│"),
                Print(" ".repeat(pad_left)),
                Print("["),
                Print("█".repeat(filled).with(running_color(stopwatch.started()))),
                Print("-".repeat(empty).with(Color::DarkGrey)),
                Print("] "),
                Print(percent_str.with(Color::White)),
                Print(" ".repeat(pad_right)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // middle separator (blank line for spacing)
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_sep()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // controls split into two lines and dimmed (print borders separately)
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
                Print(frame_line(&round_number)),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_bottom()),
                Clear(ClearType::FromCursorDown),
            )?;
        }
        UIMode::Running(stopwatch) => {
            let excess_time = stopwatch.elapsed().saturating_sub(target);
            let (title, message) = alert_message(session.next().mode);
            alerter.alert_once(title, message);

            let plus_raw = format!("+{}", format_duration(&excess_time));
            let pad_plus = UI_WIDTH.saturating_sub(plus_raw.len());

            queue!(
                out,
                MoveTo(0, 0),
                Print(frame_top()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(&center_text(end_title(session.next().mode)))),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_sep()),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line above excess time for symmetry
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // excess time line with styling but correct padding
                Print("│"),
                Print(plus_raw.with(running_color(stopwatch.started()))),
                Print(" ".repeat(pad_plus)),
                Print("│"),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                // blank line below excess time for symmetry
                Print(frame_line("")),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(&progress_bar(target, target, 30))),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(ENDING_CONTROLS)),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(&round_number)),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_line(message)),
                Clear(ClearType::UntilNewLine),
                MoveToNextLine(1),
                Print(frame_bottom()),
                Clear(ClearType::FromCursorDown),
            )?;
        }
    }
    out.flush()?;
    Ok(())
}
