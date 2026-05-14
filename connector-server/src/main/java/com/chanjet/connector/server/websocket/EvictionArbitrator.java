package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Component;

import java.time.Duration;
import java.util.Set;
import java.util.UUID;

/**
 * 驱逐决策器。
 * 采用分布式锁确保连接建立时的“互斥/抢占”逻辑是原子的，防止集群内出现踢人环路。
 */
@Component
public class EvictionArbitrator {

    private static final Logger log = LoggerFactory.getLogger(EvictionArbitrator.class);
    private static final String LOCK_PREFIX = "cjt:gw:lock:conn:";

    private final IRouteStore routeStore;
    private final WsSessionRegistry sessionRegistry;
    private final IP2PClient p2pClient;
    private final StringRedisTemplate redisTemplate;
    private final String currentNodeId;

    public EvictionArbitrator(IRouteStore routeStore, 
                              WsSessionRegistry sessionRegistry, 
                              IP2PClient p2pClient,
                              StringRedisTemplate redisTemplate,
                              com.chanjet.connector.server.config.NodeIdResolver nodeIdResolver) {
        this.routeStore = routeStore;
        this.sessionRegistry = sessionRegistry;
        this.p2pClient = p2pClient;
        this.redisTemplate = redisTemplate;
        this.currentNodeId = nodeIdResolver.getResolvedNodeId();
    }

    /**
     * 执行驱逐仲裁。
     * @param appKey 应用 Key
     * @param clientId 当前准备连入的 ClientId
     * @param exclusive 是否开启互斥模式
     */
    public void arbitrate(String appKey, String clientId, boolean exclusive) {
        String lockKey = LOCK_PREFIX + appKey;
        String requestId = UUID.randomUUID().toString();

        // 1. 尝试获取分布式锁 (3s 超时)
        Boolean acquired = redisTemplate.opsForValue().setIfAbsent(lockKey, requestId, Duration.ofSeconds(3));
        if (Boolean.FALSE.equals(acquired)) {
            log.warn("Failed to acquire connection lock for AppKey [{}]. Eviction might be delayed.", appKey);
            return; 
        }

        try {
            // 2. 执行驱逐逻辑
            Set<String> existingRoutes = routeStore.getNodes(appKey);
            if (existingRoutes == null || existingRoutes.isEmpty()) return;

            for (String route : existingRoutes) {
                int lastColonIndex = route.lastIndexOf(":");
                if (lastColonIndex == -1) continue;

                String oldNodeId = route.substring(0, lastColonIndex);
                String oldClientId = route.substring(lastColonIndex + 1);

                // 情况 A：同一 ClientId 在不同节点（抢占式下线探测）
                if (oldClientId.equals(clientId)) {
                    if (!oldNodeId.equals(currentNodeId)) {
                        evictRemote(appKey, oldNodeId, oldClientId, "Ghost/Reconnected");
                    }
                }
                // 情况 B：开启互斥模式，下线该 AppKey 的所有【其它】本地或远程连接
                else if (exclusive) {
                    if (oldNodeId.equals(currentNodeId)) {
                        evictLocal(oldClientId, "Exclusive Mode (Local)");
                    } else {
                        evictRemote(appKey, oldNodeId, oldClientId, "Exclusive Mode (Remote)");
                    }
                }
            }
        } finally {
            // 3. 释放锁 (仅释放自己的锁)
            String currentVal = redisTemplate.opsForValue().get(lockKey);
            if (requestId.equals(currentVal)) {
                redisTemplate.delete(lockKey);
            }
        }
    }

    private void evictLocal(String clientId, String reason) {
        log.info("[EVICT_LOCAL] Client: {}, Reason: {}", clientId, reason);
        sessionRegistry.getSession(clientId).ifPresent(s -> {
            try {
                s.close();
            } catch (Exception e) {
                log.warn("Failed to close local session [{}]: {}", clientId, e.getMessage());
            }
        });
        // 注册表注销由 afterConnectionClosed 触发
    }

    private void evictRemote(String appKey, String nodeId, String clientId, String reason) {
        log.info("[EVICT_REMOTE] Node: {}, Client: {}, Reason: {}", nodeId, clientId, reason);
        // 先清理路由，防止分发器继续往那边送
        routeStore.remove(appKey, nodeId, clientId);
        // 发送 P2P 驱逐指令
        new Thread(() -> p2pClient.evict(nodeId, clientId)).start();
    }
}
