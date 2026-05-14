package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.server.config.NodeIdResolver;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;

import java.util.HashMap;
import java.util.Map;

import static org.mockito.Mockito.*;

class DefaultWsHandlerUnitTest {

    private DefaultWsHandler handler;
    private IRouteStore routeStore;
    private WsSessionRegistry sessionRegistry;
    private ToleranceManager toleranceManager;
    private NodeIdResolver nodeIdResolver;
    private IP2PClient p2pClient;
    private PushAuditLogger auditLogger;
    private ObjectMapper objectMapper;
    private EvictionArbitrator evictionArbitrator;

    @BeforeEach
    void setUp() {
        routeStore = mock(IRouteStore.class);
        sessionRegistry = mock(WsSessionRegistry.class);
        toleranceManager = mock(ToleranceManager.class);
        nodeIdResolver = mock(NodeIdResolver.class);
        p2pClient = mock(IP2PClient.class);
        auditLogger = mock(PushAuditLogger.class);
        objectMapper = mock(ObjectMapper.class);
        evictionArbitrator = mock(EvictionArbitrator.class);

        when(nodeIdResolver.getResolvedNodeId()).thenReturn("node-1");

        handler = new DefaultWsHandler(
            nodeIdResolver, 
            sessionRegistry, 
            routeStore, 
            toleranceManager, 
            p2pClient, 
            auditLogger, 
            objectMapper, 
            evictionArbitrator
        );
    }

    @Test
    void shouldRefreshRouteStoreOnIncomingMessage() throws Exception {
        WebSocketSession session = mock(WebSocketSession.class);
        Map<String, Object> attrs = new HashMap<>();
        attrs.put("clientId", "client-1");
        attrs.put("appKey", "app-1");
        when(session.getAttributes()).thenReturn(attrs);

        // 1. 建立连接
        handler.afterConnectionEstablished(session);
        verify(routeStore, times(1)).add("app-1", "node-1", "client-1");

        // 2. 收到消息
        handler.handleTextMessage(session, new TextMessage("{\"msg_type\":\"pong\"}"));

        // 验证：应该再次调用 routeStore.add 以刷新 TTL
        verify(routeStore, times(2)).add("app-1", "node-1", "client-1");
    }

    @Test
    void shouldInvokeEvictionArbitratorOnConnectionEstablished() throws Exception {
        WebSocketSession session = mock(WebSocketSession.class);
        Map<String, Object> attrs = new HashMap<>();
        attrs.put("clientId", "client-1");
        attrs.put("appKey", "app-1");
        attrs.put("exclusive", true);
        when(session.getAttributes()).thenReturn(attrs);

        handler.afterConnectionEstablished(session);

        // 验证：建立连接时应调用驱逐仲裁器
        verify(evictionArbitrator, times(1)).arbitrate("app-1", "client-1", true);
    }

    @Test
    void shouldResetFailureStateOnConnectionEstablished() throws Exception {
        WebSocketSession session = mock(WebSocketSession.class);
        Map<String, Object> attrs = new HashMap<>();
        attrs.put("clientId", "client-1");
        attrs.put("appKey", "app-1");
        when(session.getAttributes()).thenReturn(attrs);

        handler.afterConnectionEstablished(session);

        // 验证：建立连接后应立即重置失败状态以恢复推送 (Stream Start)
        verify(toleranceManager).resetFailureState("app-1");
    }

    @Test
    void shouldClearFailureStateOnHeartbeat() throws Exception {
        WebSocketSession session = mock(WebSocketSession.class);
        Map<String, Object> attrs = new HashMap<>();
        attrs.put("clientId", "client-1");
        attrs.put("appKey", "app-1");
        when(session.getAttributes()).thenReturn(attrs);

        // 收到心跳消息
        handler.handleTextMessage(session, new TextMessage("{\"msg_type\":\"pong\"}"));

        // 验证：心跳也应触发故障状态清理，避免 stale timer 导致误判
        verify(toleranceManager).handleReconnect("app-1");
    }
}
