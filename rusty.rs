use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::thread;
use std::sync::atomic::{AtomicU64, AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{color, clear, cursor};

const DEFAULT_BREAK_INTERVAL: u64 = 50 * 60;
const MIN_BREAK_INTERVAL: u64 = 5 * 60;
const INTERVAL_CHANGE: u64 = 5 * 60;
const MAX_REMINDERS: usize = 8;
const REMINDER_INTERVAL: u64 = 5 * 60;

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn main() -> io::Result<()> {
    let break_interval = Arc::new(AtomicU64::new(DEFAULT_BREAK_INTERVAL));
    let next_break_time = Arc::new(AtomicU64::new(DEFAULT_BREAK_INTERVAL));
    let reminder_count = Arc::new(AtomicUsize::new(0));
    let should_exit = Arc::new(AtomicBool::new(false));
    let is_break_time = Arc::new(AtomicBool::new(false));

    let next_break_time_clone = Arc::clone(&next_break_time);
    let reminder_count_clone = Arc::clone(&reminder_count);
    let should_exit_clone = Arc::clone(&should_exit);
    let is_break_time_clone = Arc::clone(&is_break_time);

    print!("{}{}", cursor::Hide, color::Fg(color::Green));
    println!("Rusty timer started. Commands:");
    println!("  +: Increase break interval by 5 minutes");
    println!("  -: Decrease break interval by 5 minutes");
    println!("  q: Quit");
    io::stdout().flush()?;

    // Timer thread
    let timer_handle = thread::spawn(move || {
        let start_time = Instant::now();
        while !should_exit_clone.load(Ordering::Relaxed) {
            let elapsed = start_time.elapsed().as_secs();
            let next_break = next_break_time_clone.load(Ordering::Relaxed);
            let reminders = reminder_count_clone.load(Ordering::Relaxed);

            if elapsed >= next_break {
                is_break_time_clone.store(true, Ordering::Relaxed);
                if reminders < MAX_REMINDERS {
                    println!("\r{}Time to take a break!", clear::CurrentLine);
                    io::stdout().flush().unwrap();
                    reminder_count_clone.fetch_add(1, Ordering::Relaxed);
                    next_break_time_clone.store(next_break + REMINDER_INTERVAL, Ordering::Relaxed);
                } else {
                    break;
                }
            } else if !is_break_time_clone.load(Ordering::Relaxed) {
                let remaining = next_break - elapsed;
                print!("\r{}Time until next break: {}", clear::CurrentLine, format_time(remaining));
                io::stdout().flush().unwrap();
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    let stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode()?;
    for key in stdin.keys().flatten() {
        match key {
            Key::Char('+') | Key::Char('-') => {
                if !is_break_time.load(Ordering::Relaxed) {
                    let current_interval = break_interval.load(Ordering::Relaxed);
                    let (new_interval, action) = if key == Key::Char('+') {
                        (current_interval + INTERVAL_CHANGE, "increased")
                    } else {
                        let new_interval = (current_interval - INTERVAL_CHANGE).max(MIN_BREAK_INTERVAL);
                        (new_interval, if new_interval < current_interval { "decreased" } else { "already at minimum" })
                    };

                    break_interval.store(new_interval, Ordering::Relaxed);
                    next_break_time.store(new_interval, Ordering::Relaxed);

                    let message = format!("Break interval {} to {}", action, format_time(new_interval));
                    write!(stdout, "\r{}{}", clear::CurrentLine, message)?;
                    stdout.flush()?;
                }
            },
            Key::Char('q') => {
                should_exit.store(true, Ordering::Relaxed);
                break;
            },
            _ => continue,
        };
        if should_exit.load(Ordering::Relaxed) {
            break;
        }
    }

    // Wait for the timer thread to finish
    should_exit.store(true, Ordering::Relaxed);
    timer_handle.join().unwrap();

    // Clean exit
    write!(stdout, "\r{}{}{}", clear::CurrentLine, color::Fg(color::Reset), cursor::Show)?;
    stdout.flush()?;

    Ok(())
}
