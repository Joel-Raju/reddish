use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Tick,
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Input task
        let tx_input = tx.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                match event::read() {
                    Ok(CrosstermEvent::Key(key)) => {
                        if tx_input.blocking_send(Event::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(CrosstermEvent::Resize(w, h)) => {
                        if tx_input.blocking_send(Event::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Ok(CrosstermEvent::Mouse(mouse)) => {
                        if tx_input.blocking_send(Event::Mouse(mouse)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        Self::spawn_tick(tx, tick_rate);
        Self { rx }
    }

    /// Test-only constructor: only produces Tick events.
    pub fn new_test(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self::spawn_tick(tx, tick_rate);
        Self { rx }
    }

    fn spawn_tick(tx: mpsc::Sender<Event>, tick_rate: Duration) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                if tx.send(Event::Tick).await.is_err() {
                    break;
                }
            }
        });
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
