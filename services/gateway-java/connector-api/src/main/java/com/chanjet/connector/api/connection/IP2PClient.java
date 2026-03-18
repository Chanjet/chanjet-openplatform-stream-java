package com.chanjet.connector.api.connection;

import com.chanjet.connector.common.protocol.EventFrame;

/**
 * 内部 P2P 转发契约，负责将消息转发至集群内其他节点。
 */
public interface IP2PClient {
    /**
     * 转发消息至指定节点。
     * @param targetNodeId 目标节点 ID (ip:port)
     * @param frame 数据帧
     */
    void forward(String targetNodeId, EventFrame frame);
}
