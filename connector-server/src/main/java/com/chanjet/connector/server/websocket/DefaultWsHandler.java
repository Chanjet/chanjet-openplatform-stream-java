package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.server.config.NodeIdResolver;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;
import org.springframework.web.socket.CloseStatus;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.Set;

/**
 * WebSocket 连接处理器，集成领域层逻辑。
 */
@Component
public class DefaultWsHandler extends TextWebSocketHandler {

    private static final Logger log = LoggerFactory.getLogger(DefaultWsHandler.class);
    
    private final String nodeId;
    private final WsSessionRegistry sessionRegistry;
    private final IRouteStore routeStore;
    private final ToleranceManager toleranceManager;
    private final IP2PClient p2pClient;

    public DefaultWsHandler(NodeIdResolver nodeIdResolver,
                            WsSessionRegistry sessionRegistry,
                            IRouteStore routeStore,
                            ToleranceManager toleranceManager,
                            IP2PClient p2pClient) {
        this.nodeId = nodeIdResolver.getResolvedNodeId();
        this.sessionRegistry = sessionRegistry;
        this.routeStore = routeStore;
        this.toleranceManager = toleranceManager;
        this.p2pClient = p2pClient;
    }

    @Override
    public void afterConnectionEstablished(WebSocketSession session) {
        log.info("Handshake Attributes: {}", session.getAttributes());
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");

        if (clientId != null) {
            // 0. 抢占式下线探测 (Proactive Eviction)
            // 无论 appKey 是否存在，clientId 必须唯一
            // 遍历 Redis 中的该 clientId（如果存在某种全局映射）
            // 目前 routeStore 是基于 appKey 组织的，为了解决没有 appKey 也能连接的问题，
            // 我们需要确保 clientId 的唯一性。
            
            if (appKey != null) {
                Set<String> existingRoutes = routeStore.getNodes(appKey);
                if (existingRoutes != null) {
                    for (String route : existingRoutes) {
                        if (route.endsWith(":" + clientId)) {
                            String oldNodeId = route.substring(0, route.lastIndexOf(":"));
                            if (!oldNodeId.equals(this.nodeId)) {
                                log.info("Proactive Eviction: Notifying remote node [{}] to close conflicting session for [{}]", oldNodeId, clientId);
                                String finalClientId = clientId;
                                new Thread(() -> p2pClient.evict(oldNodeId, finalClientId)).start();
                            }
                        }
                    }
                }
            }

            // 1. 注册本地会话
            sessionRegistry.register(clientId, session);
            
            // 2. 如果提供了 appKey，则注册物理路由并重置失败计时
            if (appKey != null) {
                routeStore.add(appKey, nodeId, clientId);
                toleranceManager.resetFailureState(appKey);
            }
            
            log.info("Client connected and registered: {} (App: {})", clientId, appKey != null ? appKey : "NONE");
        } else {
            log.warn("Missing clientId, closing session.");
            closeSilently(session);
        }
    }

    @Override
    public void afterConnectionClosed(WebSocketSession session, CloseStatus status) {
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");
        if (clientId != null) {
            sessionRegistry.unregister(clientId);
            if (appKey != null) {
                routeStore.remove(appKey, nodeId, clientId);
            }
            log.info("Client disconnected: {} (App: {})", clientId, appKey != null ? appKey : "NONE");
        }
    }

    @Override
    protected void handleTextMessage(WebSocketSession session, org.springframework.web.socket.TextMessage message) {
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");
        if (clientId != null) {
            sessionRegistry.updateActiveTime(clientId);
            if (appKey != null) {
                routeStore.add(appKey, nodeId, clientId);
            }
        }
    }

    private void closeSilently(WebSocketSession session) {
        try {
            session.close();
        } catch (Exception e) {
            log.debug("Error during silent session close: {}", e.getMessage());
        }
    }
}
