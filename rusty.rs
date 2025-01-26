use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{clear, color, cursor};

#[derive(Debug, Clone)]
struct TimerConfig {
    default_break_interval: u64,
    min_break_interval: u64,
    interval_change: u64,
    max_reminders: usize,
    reminder_interval: u64,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            default_break_interval: 50 * 60,
            min_break_interval: 5 * 60,
            interval_change: 5 * 60,
            max_reminders: 8,
            reminder_interval: 5 * 60,
        }
    }
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// A configurable timer that manages break intervals and reminders
///
/// Handles thread-safe state management and user interactions
struct Timer {
    break_interval: Arc<AtomicU64>,
    next_break_time: Arc<AtomicU64>,
    reminder_count: Arc<AtomicUsize>,
    should_exit: Arc<AtomicBool>,
    is_break_time: Arc<AtomicBool>,
    config: TimerConfig,
}

impl Timer {
    /// Handle interval change requests from user input
    fn handle_interval_change(
        &self,
        key: Key,
        stdout: &mut termion::raw::RawTerminal<io::Stdout>,
    ) -> io::Result<()> {
        if !self.is_break_time.load(Ordering::SeqCst) {
            let current_interval = self.break_interval.load(Ordering::SeqCst);
            let (new_interval, action) = if key == Key::Char('+') {
                (current_interval + self.config.interval_change, "increased")
            } else {
                let new_interval = (current_interval - self.config.interval_change)
                    .max(self.config.min_break_interval);
                (
                    new_interval,
                    if new_interval < current_interval {
                        "decreased"
                    } else {
                        "already at minimum"
                    },
                )
            };

            self.break_interval.store(new_interval, Ordering::SeqCst);
            self.next_break_time.store(new_interval, Ordering::SeqCst);

            let message = format!("Break interval {} to {}", action, format_time(new_interval));
            write!(stdout, "\r{}{}", clear::CurrentLine, message)?;
            stdout.flush()?;
        }
        Ok(())
    }

    /// Create a new Timer with the specified configuration
    fn new(config: TimerConfig) -> Self {
        Self {
            break_interval: Arc::new(AtomicU64::new(config.default_break_interval)),
            next_break_time: Arc::new(AtomicU64::new(config.default_break_interval)),
            reminder_count: Arc::new(AtomicUsize::new(0)),
            should_exit: Arc::new(AtomicBool::new(false)),
            is_break_time: Arc::new(AtomicBool::new(false)),
            config,
        }
    }

    /// Starts the timer thread that manages break reminders and countdown display
    ///
    /// Returns a thread handle that should be joined during cleanup
    fn start_timer_thread(&self) -> thread::JoinHandle<()> {
        let next_break_time = Arc::clone(&self.next_break_time);
        let reminder_count = Arc::clone(&self.reminder_count);
        let should_exit = Arc::clone(&self.should_exit);
        let is_break_time = Arc::clone(&self.is_break_time);
        let config = self.config.clone();

        thread::spawn(move || {
            let start_time = Instant::now();
            while !should_exit.load(Ordering::SeqCst) {
                let elapsed = start_time.elapsed().as_secs();
                let next_break = next_break_time.load(Ordering::SeqCst);
                let reminders = reminder_count.load(Ordering::SeqCst);

                if elapsed >= next_break {
                    is_break_time.store(true, Ordering::SeqCst);
                    if reminders < config.max_reminders {
                        println!("\r{}Time to take a break!", clear::CurrentLine);
                        io::stdout().flush().unwrap();
                        reminder_count.fetch_add(1, Ordering::SeqCst);
                        next_break_time
                            .store(next_break + config.reminder_interval, Ordering::SeqCst);
                    } else {
                        break;
                    }
                } else if !is_break_time.load(Ordering::SeqCst) {
                    let remaining = next_break - elapsed;
                    print!(
                        "\r{}Time until next break: {}",
                        clear::CurrentLine,
                        format_time(remaining)
                    );
                    io::stdout().flush().unwrap();
                }

                let sleep_duration = if elapsed >= next_break {
                    config.reminder_interval
                } else {
                    next_break - elapsed
                };
                thread::sleep(Duration::from_secs(sleep_duration.min(1)));
            }
        })
    }
}

fn main() -> io::Result<()> {
    let config = TimerConfig::default();
    let timer = Timer::new(config);

    print!("{}{}", cursor::Hide, color::Fg(color::Green));
    println!("Rusty timer started. Commands:");
    println!("  +: Increase break interval by 5 minutes");
    println!("  -: Decrease break interval by 5 minutes");
    println!("  q: Quit");
    io::stdout().flush()?;

    // Start timer thread using Timer's method
    let timer_handle = timer.start_timer_thread();

    let stdin = io::stdin();
    // RAII guard for terminal state
    struct TerminalGuard {
        stdout: termion::raw::RawTerminal<io::Stdout>,
    }

    impl TerminalGuard {
        fn new() -> io::Result<Self> {
            let mut stdout = io::stdout().into_raw_mode()?;
            write!(stdout, "{}{}", cursor::Hide, color::Fg(color::Green))?;
            Ok(Self { stdout })
        }
    }

    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            write!(
                self.stdout,
                "{}{}{}",
                clear::CurrentLine,
                color::Fg(color::Reset),
                cursor::Show
            )
            .unwrap();
            self.stdout.flush().unwrap();
        }
    }

    let mut terminal = TerminalGuard::new()?;

    for key in stdin.keys().flatten() {
        match key {
            Key::Char('+') | Key::Char('-') => {
                timer.handle_interval_change(key, &mut terminal.stdout)?
            }
            Key::Char('?') => {
                write!(terminal.stdout, "\r{}Commands: + - q", clear::CurrentLine)?;
                terminal.stdout.flush()?;
            }
            Key::Char('q') => {
                timer.should_exit.store(true, Ordering::SeqCst);
                break;
            }
            _ => {
                write!(
                    terminal.stdout,
                    "\r{}Unknown command (press ? for help)",
                    clear::CurrentLine
                )?;
                terminal.stdout.flush()?;
            }
        }
    }

    // Wait for the timer thread to finish
    timer.should_exit.store(true, Ordering::SeqCst);
    timer_handle.join().unwrap();

    // Clean exit
    write!(
        terminal.stdout,
        "\r{}{}{}",
        clear::CurrentLine,
        color::Fg(color::Reset),
        cursor::Show
    )?;
    terminal.stdout.flush()?;

    Ok(())
}
