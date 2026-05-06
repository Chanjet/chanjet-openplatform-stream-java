package com.chanjet.connector.server.websocket;

import io.micrometer.core.instrument.MeterRegistry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;

import java.io.IOException;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 本地 WebSocket 会话注册表，包含心跳探测与自愈清理逻辑。
 */
@Component
public class WsSessionRegistry {

    private static final Logger log = LoggerFactory.getLogger(WsSessionRegistry.class);
    
    // 会话存储：ClientId -> Session
    private final Map<String, WebSocketSession> sessions = new ConcurrentHashMap<>();
    
    // 最后活跃时间：ClientId -> Timestamp
    private final Map<String, Long> lastActiveTimes = new ConcurrentHashMap<>();

    public WsSessionRegistry(MeterRegistry meterRegistry) {
        // 注册监控指标：连接总数
        meterRegistry.gauge("connector.websocket.sessions", sessions, Map::size);
    }

    public void register(String clientId, WebSocketSession session) {
        WebSocketSession oldSession = sessions.put(clientId, session);
        updateActiveTime(clientId);
        
        // 如果存在旧连接，立即将其关闭，防止 Socket 泄漏
        if (oldSession != null && oldSession.isOpen()) {
            log.info("Client [{}] reconnected to same node. Closing previous ghost session.", clientId);
            try {
                oldSession.close();
            } catch (java.io.IOException e) {
                log.warn("Failed to close ghost session for client [{}]: {}", clientId, e.getMessage());
            }

        }
    }

    public void unregister(String clientId, WebSocketSession sessionToClose) {
        // 仅当当前存储的 Session 和要关闭的 Session 是同一个实例时才移除。
        // 这防止了“旧连接关闭事件”意外注销掉刚刚建立的“新连接”。
        boolean removed = sessions.remove(clientId, sessionToClose);
        if (removed) {
            lastActiveTimes.remove(clientId);
        }
    }

    // 兼容遗留的强制清理调用（如心跳超时）
    public void unregister(String clientId) {
        sessions.remove(clientId);
        lastActiveTimes.remove(clientId);
    }

    public void updateActiveTime(String clientId) {
        lastActiveTimes.put(clientId, System.currentTimeMillis());
    }

    public Optional<WebSocketSession> getSession(String clientId) {
        return Optional.ofNullable(sessions.get(clientId));
    }

    public Map<String, WebSocketSession> getAllSessions() {
        return java.util.Collections.unmodifiableMap(sessions);
    }

    /**
     * 每 10 秒发送一次应用级 Ping。
     */
    @Scheduled(fixedRate = 10000)
    public void sendPings() {
        sessions.forEach((clientId, session) -> {
            if (session.isOpen()) {
                try {
                    session.sendMessage(new TextMessage("{\"msg_type\":\"ping\"}"));
                } catch (IOException e) {
                    log.error("Failed to send ping to {}: {}", clientId, e.getMessage());
                }
            }
        });
    }

    /**
     * 每 5 秒检查一次僵死连接。
     * 若 20 秒未收到任何消息（Ping 或业务数据），则强制关闭。
     */
    @Scheduled(fixedRate = 5000)
    public void cleanupStaleSessions() {
        long now = System.currentTimeMillis();
        lastActiveTimes.forEach((clientId, lastTime) -> {
            if (now - lastTime > 20000) {
                log.warn("Session {} timeout, forcing close.", clientId);
                WebSocketSession session = sessions.get(clientId);
                if (session != null) {
                    try {
                        session.close();
                    } catch (java.io.IOException e) {
                        log.warn("Failed to close stale session [{}]: {}", clientId, e.getMessage());
                    }

                }
                unregister(clientId);
            }
        });
    }
}
