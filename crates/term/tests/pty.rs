use std::time::{Duration, Instant};

use alacritty_terminal::event::Event;
use term::{Session, SpawnOptions};

fn sh(script: &str) -> SpawnOptions {
    SpawnOptions {
        shell: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: None,
        cols: 80,
        rows: 24,
        env: Vec::new(),
    }
}

fn wait_for_text(session: &Session, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if session.snapshot().text().contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn command_output_appears_in_snapshot() {
    let session = Session::spawn(&sh("echo hello")).expect("spawn");
    assert!(
        wait_for_text(&session, "hello"),
        "output was: {:?}",
        session.snapshot().text()
    );
}

#[test]
fn child_exit_code_is_reported() {
    let session = Session::spawn(&sh("exit 3")).expect("spawn");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = session
            .events()
            .recv_timeout(remaining)
            .expect("ChildExit event");
        if let Event::ChildExit(status) = event {
            assert_eq!(status.code(), Some(3));
            break;
        }
    }
}

#[test]
fn input_is_delivered_to_shell() {
    let options = SpawnOptions {
        shell: Some(("/bin/cat".into(), vec![])),
        cwd: None,
        cols: 80,
        rows: 24,
        env: Vec::new(),
    };
    let session = Session::spawn(&options).expect("spawn");
    session.write(b"ping\n").expect("write");
    assert!(
        wait_for_text(&session, "ping"),
        "output was: {:?}",
        session.snapshot().text()
    );
}
