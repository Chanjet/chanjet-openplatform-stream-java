package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.EventFrame;

/**
 * 业务消息处理器契约。
 */
@FunctionalInterface
public interface EventHandler {
    /**
     * 处理推送事件。
     * @param frame 事件帧
     * @return true 表示成功（发送 200 ACK），false 表示失败（发送 500 ACK）
     */
    boolean handle(EventFrame frame);
}
