package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.server.config.NodeIdResolver;
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

    @BeforeEach
    void setUp() {
        routeStore = mock(IRouteStore.class);
        sessionRegistry = mock(WsSessionRegistry.class);
        toleranceManager = mock(ToleranceManager.class);
        nodeIdResolver = mock(NodeIdResolver.class);
        when(nodeIdResolver.getResolvedNodeId()).thenReturn("node-1");

        handler = new DefaultWsHandler(nodeIdResolver, sessionRegistry, routeStore, toleranceManager);
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
        // 目前的代码只会验证失败，因为 handleTextMessage 里没写这个逻辑
        verify(routeStore, times(2)).add("app-1", "node-1", "client-1");
    }
}
