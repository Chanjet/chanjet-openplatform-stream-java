use std::sync::OnceLock;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GlobalEvent {
    /// 当 Profile 被重命名时触发
    ProfileRenamed { old: String, new: String },
    /// 当 Profile 被删除时触发
    ProfileDeleted { name: String },
    /// 当配置发生变更时触发 (用于热加载)
    ConfigChanged { profile: String, key: String },
}

/// 全局事件总线 (进程内广播)
pub static EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

pub fn event_bus() -> &'static EventBus {
    EVENT_BUS.get_or_init(EventBus::new)
}

pub struct EventBus {
    tx: broadcast::Sender<GlobalEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<GlobalEvent> {
        self.tx.subscribe()
    }

    /// 发布事件
    pub fn publish(&self, event: GlobalEvent) {
        // 如果没有订阅者，发送可能会失败，但在我们的场景下可以忽略
        let _ = self.tx.send(event);
    }
}
