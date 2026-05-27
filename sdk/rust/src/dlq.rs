use anyhow::Result;

#[async_trait::async_trait]
pub trait DlqProvider: Send + Sync {
    /// 暂存收到的消息（落盘死信队列）
    async fn store(&self, msg_id: &str, payload: &str) -> Result<()>;
    
    /// 消息成功处理后，从死信队列中移除
    async fn remove(&self, msg_id: &str) -> Result<()>;
}
