/**
 * 死信队列 (DLQ) 提供者接口。
 * 当消息分发处理失败时，SDK 会尝试将消息持久化到 DLQ 中。
 * 并在处理成功后，从 DLQ 中移除。
 */
export interface DlqProvider {
    /**
     * 暂存收到的消息（落盘死信队列）。
     * @param msgId 消息唯一标识
     * @param payload 消息内容 (JSON 字符串)
     */
    store(msgId: string, payload: string): Promise<void>;

    /**
     * 消息成功处理后，从死信队列中移除。
     * @param msgId 消息唯一标识
     */
    remove(msgId: string): Promise<void>;
}
