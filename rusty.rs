use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::color;
use termion::clear;

const DEFAULT_BREAK_INTERVAL: u64 = 50 * 60;
const MIN_BREAK_INTERVAL: u64 = 5 * 60;
const INTERVAL_CHANGE: u64 = 5 * 60;

fn get_current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn main() -> io::Result<()> {
    let break_interval = Arc::new(AtomicU64::new(DEFAULT_BREAK_INTERVAL));
    let next_break_time = Arc::new(AtomicU64::new(get_current_time() + DEFAULT_BREAK_INTERVAL));
    let break_interval_clone = Arc::clone(&break_interval);
    let next_break_time_clone = Arc::clone(&next_break_time);

    print!("{}", color::Fg(color::Green));
    println!("Rusty timer started. Commands:");
    println!("  ↑: Check remaining time");
    println!("  +: Increase break interval by 5 minutes");
    println!("  -: Decrease break interval by 5 minutes");
    println!("  q: Quit");
    io::stdout().flush()?;

    // Timer thread
    thread::spawn(move || loop {
        let now = get_current_time();
        let next_break = next_break_time_clone.load(Ordering::Relaxed);
        if now >= next_break {
            println!("\n\rTime to take a break!");
            io::stdout().flush().unwrap();
            let interval = break_interval_clone.load(Ordering::Relaxed);
            next_break_time_clone.store(now + interval, Ordering::Relaxed);
        }
        thread::sleep(Duration::from_secs(1));
    });

    let stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode()?;
    for key in stdin.keys().flatten() {
        let message = match key {
            Key::Up => {
                let now = get_current_time();
                let next_break = next_break_time.load(Ordering::Relaxed);
                if now < next_break {
                    format!("Time until next break: {}", format_time(next_break - now))
                } else {
                    "Break time! Take a break now.".to_string()
                }
            },
            Key::Char('+') | Key::Char('-') => {
                let current_interval = break_interval.load(Ordering::Relaxed);
                let (new_interval, action) = if key == Key::Char('+') {
                    (current_interval + INTERVAL_CHANGE, "increased")
                } else {
                    let new_interval = (current_interval - INTERVAL_CHANGE).max(MIN_BREAK_INTERVAL);
                    (new_interval, if new_interval < current_interval { "decreased" } else { "already at minimum" })
                };

                break_interval.store(new_interval, Ordering::Relaxed);

                let now = get_current_time();
                let current_next_break = next_break_time.load(Ordering::Relaxed);
                let new_next_break = if current_next_break > now {
                    if action == "increased" {
                        current_next_break + INTERVAL_CHANGE
                    } else {
                        (current_next_break - INTERVAL_CHANGE).max(now + new_interval)
                    }
                } else {
                    now + new_interval
                };
                next_break_time.store(new_next_break, Ordering::Relaxed);

                format!("Break interval {} to {}", action, format_time(new_interval))
            },
            Key::Char('q') => break,
            _ => continue,
        };

        write!(stdout, "\r{}{}", clear::CurrentLine, message)?;
        stdout.flush()?;
    }

    Ok(())
}
