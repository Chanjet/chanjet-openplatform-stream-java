package com.chanjet.connector.api.connection;

import com.chanjet.connector.common.protocol.EventFrame;

/**
 * 连接管理契约，抽象物理 Session 交互。
 */
public interface IConnectionManager {
    /**
     * 向指定客户端推送数据。
     * @param clientId 客户端唯一 ID
     * @param frame 推送帧
     * @return 发送是否成功
     */
    boolean push(String clientId, EventFrame frame);

    /**
     * 强制断开指定连接。
     * @param clientId 客户端 ID
     * @param reason 断连原因
     */
    void close(String clientId, String reason);
}
