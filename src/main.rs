use std::{
    io::{Write, stdin, stdout},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const DEFAULT_WORK_MINUTES: u64 = 25;
const DEFAULT_BREAK_MINUTES: u64 = 5;

// Function to play a simple beep sound (works on most systems)
fn play_beep() {
    print!("\x07"); // ASCII bell character
    stdout().flush().unwrap();
}

enum TimerState {
    Work,
    Break,
    Paused,
    Stopped,
}

enum TimerCommand {
    Pause,
    Resume,
    Skip,
    Quit,
}

fn main() {
    println!("--- Rust Pomodoro Timer ---");

    let mut work_minutes = DEFAULT_WORK_MINUTES;
    let mut break_minutes = DEFAULT_BREAK_MINUTES;

    println!("\nEnter work duration (minutes, default {}):", work_minutes);
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    if let Ok(value) = input.trim().parse() {
        if value > 0 {
            work_minutes = value;
        }
    }

    input.clear();
    println!("Enter break duration (minutes, default {}):", break_minutes);
    stdin().read_line(&mut input).unwrap();
    if let Ok(value) = input.trim().parse() {
        if value > 0 {
            break_minutes = value;
        }
    }

    let (sender, receiver) = mpsc::channel::<TimerCommand>();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Timer thread
    let timer_thread = thread::spawn(move || {
        let mut current_state = TimerState::Work;
        let mut session_count = 0;

        while running_clone.load(Ordering::SeqCst) {
            let (duration_minutes, session_type_name) = match current_state {
                TimerState::Work => (work_minutes, "Work"),
                TimerState::Break => (break_minutes, "Break"),
                TimerState::Paused | TimerState::Stopped => {
                    thread::sleep(Duration::from_millis(100)); // Sleep while paused/stopped
                    continue;
                }
            };

            let start_time = Instant::now();
            let session_duration = Duration::from_secs(duration_minutes * 60);
            let mut elapsed_time = Duration::new(0, 0);

            println!(
                "\n--- {} Session {} Started ---",
                session_type_name,
                session_count + 1
            );
            Command::new("paplay")
                .arg("/usr/share/sounds/freedesktop/stereo/complete.oga")
                .spawn()
                .unwrap();
            print!("\x07");
            println!("Press 'p' to pause, 's' to skip, 'q' to quit.");

            while elapsed_time < session_duration {
                let remaining = session_duration - elapsed_time;
                let minutes = remaining.as_secs() / 60;
                let seconds = remaining.as_secs() % 60;

                print!("\rTime remaining: {:02}:{:02}", minutes, seconds);
                stdout().flush().unwrap();

                match receiver.try_recv() {
                    Ok(TimerCommand::Pause) => {
                        println!("\nTimer Paused. Press 'r' to resume.");
                        current_state = TimerState::Paused;
                        break;
                    }
                    Ok(TimerCommand::Skip) => {
                        println!("\nSkipping current session.");
                        break;
                    }
                    Ok(TimerCommand::Quit) => {
                        running_clone.store(false, Ordering::SeqCst);
                        break;
                    }
                    Ok(TimerCommand::Resume) => {
                        // This should not happen if state is Paused, but good to handle
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        running_clone.store(false, Ordering::SeqCst);
                        break;
                    }
                }

                if let TimerState::Paused = current_state {
                    break;
                }

                thread::sleep(Duration::from_secs(1));
                elapsed_time = Instant::now().duration_since(start_time);
            }

            if let TimerState::Paused = current_state {
                continue; // Loop again and wait for resume command
            }

            if running_clone.load(Ordering::SeqCst) && elapsed_time >= session_duration {
                play_beep();
                println!("\n--- {} Session Finished! ---", session_type_name);
            }

            // Switch states or stop if quit
            if running_clone.load(Ordering::SeqCst) {
                match current_state {
                    TimerState::Work => {
                        current_state = TimerState::Break;
                        session_count += 1;
                    }
                    TimerState::Break => {
                        current_state = TimerState::Work;
                        session_count += 1;
                    }
                    _ => {} // Should not happen here due to continue
                }
            }
        }
        println!("Timer thread stopped.");
    });

    // Input handling thread
    for line in stdin().lines() {
        let input = line.unwrap().trim().to_lowercase();
        match input.as_str() {
            "p" => {
                sender.send(TimerCommand::Pause).unwrap();
            }
            "r" => {
                // If paused, try to resume
                if sender.send(TimerCommand::Resume).is_ok() {
                    // This command is primarily handled by the main timer loop's state
                    // transition logic, but sending it allows the timer to 'unblock'
                    // from a paused state.
                }
            }
            "s" => {
                sender.send(TimerCommand::Skip).unwrap();
            }
            "q" => {
                sender.send(TimerCommand::Quit).unwrap();
                running.store(false, Ordering::SeqCst);
                break;
            }
            _ => {
                println!(
                    "Unknown command. Use 'p' to pause, 'r' to resume, 's' to skip, 'q' to quit."
                );
            }
        }
    }

    timer_thread.join().unwrap();
    println!("Pomodoro timer finished. Goodbye!");
}
