//! End-to-end smoke tests through a real pseudo-terminal.
//!
//! Every other key test in this repo feeds `KeyEvent`s to the app directly.
//! These spawn the actual binary under a pty, write the bytes a terminal
//! really sends, and assert on what the terminal would display. That covers
//! the one layer transcription can drift at: bytes → crossterm decode →
//! dispatch → render. The `Ctrl+\` bug (0x1C decodes to `Ctrl+4`) and the
//! unhydrated-scan bug would both have been caught here on day one.
//!
//! Unix only: the harness uses `openpty`. The CI integration matrix is
//! Linux + macOS; Windows is covered by the build job.
#![cfg(unix)]

mod helpers;

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A spawned binary attached to a pseudo-terminal, with everything it has
/// drawn accumulated in `output`.
struct PtySession {
    master: File,
    child: Child,
    output: Arc<Mutex<String>>,
}

impl PtySession {
    fn spawn(args: &[&str]) -> PtySession {
        let mut master: RawFd = 0;
        let mut slave: RawFd = 0;
        let mut winsize = libc::winsize {
            ws_row: 30,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let rc = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut winsize,
            )
        };
        assert_eq!(rc, 0, "openpty failed");

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_reddish-tui"));
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(move || {
                // New session with the pty slave as the controlling
                // terminal, then point stdio at it. crossterm's raw mode
                // needs a real tty.
                libc::setsid();
                if libc::ioctl(slave, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for fd in [0, 1, 2] {
                    if libc::dup2(slave, fd) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if slave > 2 {
                    libc::close(slave);
                }
                Ok(())
            });
        }
        let child = cmd.spawn().expect("the binary must spawn");
        unsafe {
            libc::close(slave);
        }

        let master = unsafe { File::from_raw_fd(master) };
        let reader = master.try_clone().expect("dup the pty master");
        let output = Arc::new(Mutex::new(String::new()));
        let sink = output.clone();
        std::thread::spawn(move || {
            let mut r = reader;
            let mut buf = [0u8; 8192];
            loop {
                match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink
                        .lock()
                        .unwrap()
                        .push_str(&String::from_utf8_lossy(&buf[..n])),
                }
            }
        });

        PtySession {
            master,
            child,
            output,
        }
    }

    /// Write raw bytes, exactly as a terminal would.
    fn send(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).expect("write to the pty");
        self.master.flush().ok();
    }

    /// Send bytes and pause. A bare Esc followed immediately by another key
    /// is ambiguous at the terminal: crossterm can decode `ESC q` as
    /// Alt+q, which the palette treats as a character. Real keystrokes are
    /// spaced; mirror that.
    fn send_slow(&mut self, bytes: &[u8]) {
        self.send(bytes);
        std::thread::sleep(Duration::from_millis(150));
    }

    fn screen(&self) -> String {
        strip_ansi(&self.output.lock().unwrap())
    }

    /// Wait until the rendered output contains `needle`.
    fn wait_for(&self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if self.screen().contains(needle) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Quit via the app's own binding and require a clean exit.
    fn quit(mut self) {
        if self.is_running() {
            self.send(b"q");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    assert!(status.success(), "the app must exit cleanly, got {status}");
                    return;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    panic!(
                        "the app did not exit within 5s; screen was:\n{}",
                        self.screen()
                    );
                }
            }
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Remove ANSI escape sequences so assertions can match the text a user
/// would read. ratatui positions each span with a cursor-move escape, so a
/// raw buffer contains "Keyboard" + ESC[8;26H + "shortcuts"; stripping the
/// escapes stitches them back together.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ ... final byte in 0x40..=0x7E.
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... terminated by BEL or ESC \.
            Some(']') => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // Other two-byte escapes (character set selection etc.).
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// Ctrl+\ is byte 0x1C (crossterm decodes it as `Ctrl+4`); the help overlay
/// must be modal; Ctrl+P is byte 0x10. All driven as raw bytes.
#[test]
fn terminal_bytes_reach_the_expected_screens() {
    let mut app = PtySession::spawn(&["--theme", "nord"]);

    assert!(
        app.wait_for("Keys (", Duration::from_secs(10)),
        "the app must start on the Keys tab; screen:\n{}",
        app.screen()
    );

    // The original bug: the binding matched Char('\\'), but terminals send
    // 0x1C. If the decode path regresses, this fails.
    app.send(&[0x1C]);
    assert!(
        app.wait_for("Connections (", Duration::from_secs(5)),
        "byte 0x1C (Ctrl+\\) must open the connection screen; screen:\n{}",
        app.screen()
    );
    app.send_slow(&[0x1B]); // Esc
    assert!(
        app.wait_for("Keys (", Duration::from_secs(5)),
        "Esc must return to the Keys tab; screen:\n{}",
        app.screen()
    );

    // Help, and the modal guarantee: `q` behind the overlay must not quit.
    app.send(b"?");
    assert!(
        app.wait_for("Keyboard", Duration::from_secs(5)),
        "? must open the help overlay; screen:\n{}",
        app.screen()
    );
    app.send(b"q");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        app.is_running(),
        "q must not quit through the help overlay; screen:\n{}",
        app.screen()
    );
    app.send_slow(&[0x1B]); // close help
    assert!(
        app.wait_for("Keys (", Duration::from_secs(5)),
        "Esc must close the help overlay"
    );

    // Command palette, opened with the byte a terminal sends for Ctrl+P.
    app.send(&[0x10]);
    assert!(
        app.wait_for("Command Palette (Ctrl+P)", Duration::from_secs(5)),
        "byte 0x10 (Ctrl+P) must open the palette; screen:\n{}",
        app.screen()
    );
    app.send_slow(&[0x1B]);

    app.quit();
}

/// The full pipeline against a live server: scan → typed tree → Enter →
/// value pane. This is the flow whose first manual run found inspection
/// broken end to end.
#[tokio::test]
async fn enter_on_a_scanned_key_shows_its_value() {
    let port = 16393;
    let Some(mut redis) = helpers::spawn_redis_server(port).await else {
        eprintln!("skipping: redis-server is not on PATH");
        return;
    };
    let seed = redis::Client::open(format!("redis://127.0.0.1:{port}/0"))
        .expect("seed client")
        .get_connection()
        .expect("seed connection");
    let mut seed = seed;
    let _: () = redis::cmd("FLUSHDB").query(&mut seed).expect("flush");
    let _: () = redis::cmd("SET")
        .arg("ptykey")
        .arg("hello-pty")
        .query(&mut seed)
        .expect("seed key");

    let url = format!("redis://127.0.0.1:{port}/0");
    let mut app = PtySession::spawn(&["--url", &url]);

    assert!(
        app.wait_for("ptykey", Duration::from_secs(10)),
        "the scanned key must appear in the tree; screen:\n{}",
        app.screen()
    );
    // The type badge only renders when the scan hydrated each entry.
    assert!(
        app.wait_for("[S]", Duration::from_secs(5)),
        "the string type badge must render (scan hydration); screen:\n{}",
        app.screen()
    );

    app.send(b"\r"); // Enter on the selected key
    assert!(
        app.wait_for("hello-pty", Duration::from_secs(5)),
        "Enter must load the key's value into the inspector; screen:\n{}",
        app.screen()
    );

    app.quit();
    let _ = redis.kill();
    let _ = redis.wait();
}
