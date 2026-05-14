package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.common.protocol.EventFrame;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;

import java.io.IOException;

/**
 * 本地连接管理实现，负责将消息物理下发至本地维护的 WebSocket 会话。
 */
@Service
public class LocalConnectionManager implements IConnectionManager {

    private static final Logger log = LoggerFactory.getLogger(LocalConnectionManager.class);
    private final WsSessionRegistry sessionRegistry;
    private final ObjectMapper objectMapper;
    private final PushAuditLogger auditLogger;

    public LocalConnectionManager(WsSessionRegistry sessionRegistry, ObjectMapper objectMapper, PushAuditLogger auditLogger) {
        this.sessionRegistry = sessionRegistry;
        this.objectMapper = objectMapper;
        this.auditLogger = auditLogger;
    }

    @Override
    public boolean push(String clientId, EventFrame frame) {
        return sessionRegistry.getSession(clientId)
                .map(session -> {
                    try {
                        // 确保下发的消息带有 msg_type 标识
                        EventFrame fullFrame = new EventFrame(
                                frame.msgType() != null ? frame.msgType() : "event",
                                frame.msgId(),
                                frame.traceId(),
                                frame.appKey(),
                                clientId, // 设置 targetClientId
                                frame.headers(),
                                frame.payload(),
                                frame.timestamp()
                        );
                        String json = objectMapper.writeValueAsString(fullFrame);
                        session.sendMessage(new TextMessage(json));
                        auditLogger.logPushResult(frame.msgId(), clientId, true, null);
                        return true;
                    } catch (IOException e) {
                        log.error("Failed to push message to client {}: {}", clientId, e.getMessage());
                        auditLogger.logPushResult(frame.msgId(), clientId, false, e.getMessage());
                        return false;
                    }
                }).orElseGet(() -> {
                    auditLogger.logPushResult(frame.msgId(), clientId, false, "Client not found in session registry");
                    return false;
                });
    }

    @Override
    public void close(String clientId, String reason) {
        sessionRegistry.getSession(clientId).ifPresent(session -> {
            try {
                session.close();
            } catch (java.io.IOException e) {
                log.warn("Failed to close session [{}]: {}", clientId, e.getMessage());
            }
        });
    }

    @Override
    public java.util.List<String> getClientsByAppKey(String appKey) {
        return sessionRegistry.getAllSessions().entrySet().stream()
                .filter(entry -> appKey.equals(entry.getValue().getAttributes().get("appKey")))
                .map(java.util.Map.Entry::getKey)
                .toList();
    }
}
