package com.chanjet.connector.api.connection;

import com.chanjet.connector.common.protocol.EventFrame;
import java.util.List;

/**
 * 连接管理契约，抽象物理 Session 交互。
 */
public interface IConnectionManager {
    /**
     * 向指定客户端推送数据。
     */
    boolean push(String clientId, EventFrame frame);

    /**
     * 强制断开指定连接。
     */
    void close(String clientId, String reason);

    /**
     * 检索本地所有属于指定 AppKey 的活跃客户端 ID。
     */
    List<String> getClientsByAppKey(String appKey);
}
