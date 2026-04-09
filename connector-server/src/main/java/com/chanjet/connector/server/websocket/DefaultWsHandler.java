package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.server.config.NodeIdResolver;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;
import org.springframework.web.socket.CloseStatus;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.handler.TextWebSocketHandler;

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

    public DefaultWsHandler(NodeIdResolver nodeIdResolver,
                            WsSessionRegistry sessionRegistry,
                            IRouteStore routeStore,
                            ToleranceManager toleranceManager) {
        this.nodeId = nodeIdResolver.getResolvedNodeId();
        this.sessionRegistry = sessionRegistry;
        this.routeStore = routeStore;
        this.toleranceManager = toleranceManager;
    }

    @Override
    public void afterConnectionEstablished(WebSocketSession session) {
        log.info("Handshake Attributes: {}", session.getAttributes());
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");

        if (clientId != null && appKey != null) {
            // 1. 注册本地会话
            sessionRegistry.register(clientId, session);
            
            // 2. 注册物理路由 (Redis)
            routeStore.add(appKey, nodeId, clientId);
            
            // 3. 强力重置领域层失败计时 (确保分布式环境下的绝对一致性)
            toleranceManager.resetFailureState(appKey);
            
            log.info("Client connected and registered: {} (App: {})", clientId, appKey);
        } else {
            log.warn("Missing connection parameters, closing session.");
            closeSilently(session);
        }
    }

    @Override
    public void afterConnectionClosed(WebSocketSession session, CloseStatus status) {
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");

        if (clientId != null && appKey != null) {
            // 1. 注销本地会话
            sessionRegistry.unregister(clientId);
            
            // 2. 清理物理路由
            routeStore.remove(appKey, nodeId, clientId);
            
            log.info("Client disconnected and cleaned: {}", clientId);
        }
    }

    @Override
    protected void handleTextMessage(WebSocketSession session, org.springframework.web.socket.TextMessage message) {
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");
        if (clientId != null) {
            // 1. 更新本地活跃时间
            sessionRegistry.updateActiveTime(clientId);
            
            // 2. 刷新分布式路由 TTL (Redis)
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
