package sdk

// DlqProvider 死信队列 (DLQ) 提供者接口。
// 当消息分发处理失败时，SDK 会尝试将消息持久化到 DLQ 中。
// 并在处理成功后，从 DLQ 中移除。
type DlqProvider interface {
	// Store 暂存收到的消息（落盘死信队列）
	Store(msgID string, payload string) error
	
	// Remove 消息成功处理后，从死信队列中移除
	Remove(msgID string) error
}
