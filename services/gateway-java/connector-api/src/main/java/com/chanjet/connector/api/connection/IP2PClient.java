package com.chanjet.connector.api.connection;

import com.chanjet.connector.common.protocol.EventFrame;

/**
 * 内部 P2P 转发契约，负责将消息转发至集群内其他节点。
 */
public interface IP2PClient {
    /**
     * 将数据帧转发至指定网关节点。
     * @param targetNodeId 目标节点 ID (ip:port)
     * @param frame 待转发的数据帧
     * @return 转发是否成功（目标节点成功接收并响应 200）
     */
    boolean forward(String targetNodeId, EventFrame frame);
    }

