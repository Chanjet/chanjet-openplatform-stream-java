package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.client.standard.StandardWebSocketClient;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.Map;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.atLeastOnce;
import static org.mockito.Mockito.verify;

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
class WebSocketIntegrationTest {

    @LocalServerPort
    private int port;

    @Autowired
    private IConnectionManager connectionManager;

    @MockBean private IRouteStore routeStore;
    @MockBean private IFailStore failStore;
    @MockBean private IPushControl pushControl;
    @MockBean private IAuthService authService;
    @MockBean private IResilienceManager resilienceManager;
    @MockBean private IP2PClient p2pClient;
    @MockBean private ILoadBalancer loadBalancer;
    @MockBean private ToleranceManager toleranceManager;

    @Test
    void shouldInvokeDomainServicesOnLifecycleEvents() throws Exception {
        String clientId = "lifecycle-client";
        String appKey = "test-app";
        String wsUrl = "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=" + appKey;
        
        StandardWebSocketClient client = new StandardWebSocketClient();
        
        // 1. 建立连接
        WebSocketSession session = client.execute(new TextWebSocketHandler(), wsUrl).get(5, TimeUnit.SECONDS);
        Thread.sleep(200);

        // 验证：触发了状态机的重连处理
        verify(toleranceManager).handleReconnect(eq(appKey));
        // 验证：注册了物理路由
        verify(routeStore).add(eq(appKey), anyString(), eq(clientId));

        // 2. 主动断开连接
        session.close();
        Thread.sleep(200);

        // 验证：清理了物理路由
        verify(routeStore).remove(eq(appKey), anyString(), eq(clientId));
    }

    @Test
    void shouldPushMessageToConnectedClient() throws Exception {
        String clientId = "push-client";
        String wsUrl = "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=test-app";
        
        BlockingQueue<String> receivedMessages = new LinkedBlockingQueue<>();
        StandardWebSocketClient client = new StandardWebSocketClient();
        
        client.execute(new TextWebSocketHandler() {
            @Override
            protected void handleTextMessage(WebSocketSession session, TextMessage message) {
                receivedMessages.add(message.getPayload());
            }
        }, wsUrl).get(5, TimeUnit.SECONDS);

        Thread.sleep(200);
        
        EventFrame frame = new EventFrame("msg-1", "trace-1", "test-app", Map.of(), "hello", System.currentTimeMillis());
        boolean pushed = connectionManager.push(clientId, frame);

        assertThat(pushed).isTrue();
        String received = receivedMessages.poll(5, TimeUnit.SECONDS);
        assertThat(received).contains("hello");
    }
}
