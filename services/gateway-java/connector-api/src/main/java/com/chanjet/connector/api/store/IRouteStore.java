package com.chanjet.connector.api.store;

import java.util.Set;

/**
 * 路由存储契约，负责集群内连接的物理寻址。
 */
public interface IRouteStore {
    /**
     * 注册路由信息。
     * @param appKey 应用标识
     * @param nodeId 节点标识 (ip:port)
     * @param clientId 客户端实例 ID
     */
    void add(String appKey, String nodeId, String clientId);

    /**
     * 获取应用的所有活跃路由。
     * @param appKey 应用标识
     * @return 路由记录集合
     */
    Set<String> getNodes(String appKey);

    /**
     * 移除路由。
     */
    void remove(String appKey, String nodeId, String clientId);
}
