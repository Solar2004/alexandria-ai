//! Event bus simple (std mpsc broadcast).
//!
//! Fase 1: broadcast manual con `mpsc::Sender` clonados. En fases futuras
//! migrar a `tokio::broadcast` con prioridades Pre/Async/Post.

use crate::types::Event;
use std::sync::{Arc, Mutex};

/// Bus de eventos central. Los subscribers reciben una copia de cada evento.
#[derive(Clone, Default)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<std::sync::mpsc::Sender<Event>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Suscribe un receptor; recibe copias de todos los eventos publicados.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<Event> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.subscribers.lock().expect("bus lock").push(tx);
        rx
    }

    /// Publica un evento a todos los suscriptores (best-effort: si uno está
    /// caído, los demás siguen recibiendo).
    pub fn publish(&self, event: Event) {
        let subs = self.subscribers.lock().expect("bus lock");
        for s in subs.iter() {
            let _ = s.send(event.clone());
        }
    }

    /// Número de suscriptores activos.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().expect("bus lock").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_reaches_subscriber() {
        let bus = EventBus::new();
        let rx = bus.subscribe();
        bus.publish(Event::SessionStart);
        let got = rx.recv_timeout(std::time::Duration::from_millis(100));
        assert!(matches!(got, Ok(Event::SessionStart)));
    }

    #[test]
    fn publish_reaches_multiple_subscribers() {
        let bus = EventBus::new();
        let rx1 = bus.subscribe();
        let rx2 = bus.subscribe();
        bus.publish(Event::NightTick);
        assert!(matches!(rx1.recv_timeout(std::time::Duration::from_millis(100)), Ok(Event::NightTick)));
        assert!(matches!(rx2.recv_timeout(std::time::Duration::from_millis(100)), Ok(Event::NightTick)));
    }

    #[test]
    fn closed_subscriber_does_not_block_others() {
        let bus = EventBus::new();
        let rx1 = bus.subscribe();
        drop(rx1); // suscriptor caído
        let rx2 = bus.subscribe();
        bus.publish(Event::SessionStop);
        assert!(matches!(rx2.recv_timeout(std::time::Duration::from_millis(100)), Ok(Event::SessionStop)));
    }

    #[test]
    fn count_tracks_subscribers() {
        let bus = EventBus::new();
        let _rx = bus.subscribe();
        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }
}
