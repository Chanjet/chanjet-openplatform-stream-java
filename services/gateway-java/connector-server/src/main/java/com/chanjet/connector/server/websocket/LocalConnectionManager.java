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

    public LocalConnectionManager(WsSessionRegistry sessionRegistry, ObjectMapper objectMapper) {
        this.sessionRegistry = sessionRegistry;
        this.objectMapper = objectMapper;
    }

    @Override
    public boolean push(String clientId, EventFrame frame) {
        return sessionRegistry.getSession(clientId)
                .map(session -> {
                    try {
                        String json = objectMapper.writeValueAsString(frame);
                        session.sendMessage(new TextMessage(json));
                        return true;
                    } catch (IOException e) {
                        log.error("Failed to push message to client {}: {}", clientId, e.getMessage());
                        return false;
                    }
                }).orElse(false);
    }

    @Override
    public void close(String clientId, String reason) {
        sessionRegistry.getSession(clientId).ifPresent(session -> {
            try {
                session.close();
            } catch (IOException ignored) {}
        });
    }
}
