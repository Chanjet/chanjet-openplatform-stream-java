use std::sync::OnceLock;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum GlobalEvent {
    ProfileRenamed { old: String, new: String },
    ProfileDeleted { name: String },
    ConfigChanged { profile: String, key: String },
}

pub static EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

pub fn event_bus() -> &'static EventBus {
    EVENT_BUS.get_or_init(EventBus::new)
}

pub struct EventBus {
    tx: broadcast::Sender<GlobalEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GlobalEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, event: GlobalEvent) {
        let _ = self.tx.send(event);
    }
}
