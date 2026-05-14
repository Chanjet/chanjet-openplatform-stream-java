package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.server.config.NodeIdResolver;
import com.chanjet.connector.common.protocol.AckFrame;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
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
    private final PushAuditLogger auditLogger;
    private final ObjectMapper objectMapper;
    private final EvictionArbitrator evictionArbitrator;

    public DefaultWsHandler(NodeIdResolver nodeIdResolver,
            WsSessionRegistry sessionRegistry,
            IRouteStore routeStore,
            ToleranceManager toleranceManager,
            IP2PClient p2pClient,
            PushAuditLogger auditLogger,
            ObjectMapper objectMapper,
            EvictionArbitrator evictionArbitrator) {
        this.nodeId = nodeIdResolver.getResolvedNodeId();
        this.sessionRegistry = sessionRegistry;
        this.routeStore = routeStore;
        this.toleranceManager = toleranceManager;
        this.p2pClient = p2pClient;
        this.auditLogger = auditLogger;
        this.objectMapper = objectMapper;
        this.evictionArbitrator = evictionArbitrator;
    }

    @Override
    public void afterConnectionEstablished(WebSocketSession session) {
        String clientId = (String) session.getAttributes().get("clientId");
        MDC.put("traceId", "CONN-" + clientId);
        try {
            doAfterConnectionEstablished(session);
        } finally {
            MDC.remove("traceId");
        }
    }

    private void doAfterConnectionEstablished(WebSocketSession session) {
        log.info("Handshake Attributes: {}", session.getAttributes());
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");

        if (clientId != null) {
            // 0. 执行原子仲裁驱逐 (分布式锁保护)
            if (appKey != null) {
                boolean exclusive = Boolean.TRUE.equals(session.getAttributes().get("exclusive"));
                evictionArbitrator.arbitrate(appKey, clientId, exclusive);
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
        MDC.put("traceId", "DISC-" + clientId);
        try {
            String appKey = (String) session.getAttributes().get("appKey");
            if (clientId != null) {
                // 使用带有 session 引用的安全注销方法
                if (sessionRegistry.unregister(clientId, session)) {
                    if (appKey != null) {
                        routeStore.remove(appKey, nodeId, clientId);
                    }
                }
                log.info("Client disconnected: {} (App: {})", clientId, appKey != null ? appKey : "NONE");
            }
        } finally {
            MDC.remove("traceId");
        }
    }

    @Override
    protected void handleTextMessage(WebSocketSession session, org.springframework.web.socket.TextMessage message) {
        String clientId = (String) session.getAttributes().get("clientId");
        String appKey = (String) session.getAttributes().get("appKey");
        if (clientId == null) return;

        MDC.put("traceId", "WS-" + clientId);
        try {
            sessionRegistry.updateActiveTime(clientId);
            if (appKey != null) {
                routeStore.add(appKey, nodeId, clientId);
                toleranceManager.handleReconnect(appKey);
            }

            // 🚀 Parse Application ACK
            String payload = message.getPayload();
            try {
                JsonNode root = objectMapper.readTree(payload);
                if (root.has("code") && (root.has("msg_id") || root.has("msgId"))) {
                    AckFrame ack = objectMapper.treeToValue(root, AckFrame.class);
                    // Override traceId for actual ACK messages to match the original message
                    MDC.put("traceId", ack.msgId());
                    auditLogger.logAck(clientId, ack);
                }
            } catch (Exception e) {
                // Ignore non-ACK messages or malformed JSON
                log.trace("Received non-ACK or malformed message from client {}: {}", clientId, payload);
            }
        } finally {
            MDC.remove("traceId");
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
