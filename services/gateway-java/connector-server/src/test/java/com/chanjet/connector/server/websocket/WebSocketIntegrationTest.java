package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.infra.core.RemoteCjtCoreAdapter;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.client.standard.StandardWebSocketClient;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.Collections;
import java.util.Map;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
class WebSocketIntegrationTest {

    @LocalServerPort
    private int port;

    @MockBean private INonceStore nonceStore;
    @MockBean private IRouteStore routeStore;
    @MockBean private IP2PClient p2pClient;
    @MockBean private IResilienceManager resilienceManager;
    @MockBean private IFailStore failStore;
    @MockBean private ILoadBalancer loadBalancer;
    @MockBean private ToleranceManager toleranceManager;
    
    @MockBean private IInternalHttpClient httpClient;

    @Autowired
    private IConnectionManager connectionManager;

    @Autowired
    private IPushControl pushControl;

    @BeforeEach
    void setUp() {
        when(nonceStore.verifyAndConsume(anyString(), anyString())).thenReturn(true);
        // 允许所有签名校验通过
        when(httpClient.post(anyString(), any(), any(), any()))
                .thenReturn(new RemoteCjtCoreAdapter.AuthResponse(true));
    }

    private String getWsUrl(String clientId, String appKey) {
        return "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=" + appKey + "&nonce=n1&sign=s1";
    }

    @Test
    void shouldInvokeDomainServicesOnLifecycleEvents() throws Exception {
        String clientId = "lifecycle-client";
        String appKey = "test-app";
        String wsUrl = getWsUrl(clientId, appKey);

        StandardWebSocketClient client = new StandardWebSocketClient();
        WebSocketSession session = client.execute(new TextWebSocketHandler(), wsUrl).get(5, TimeUnit.SECONDS);

        assertThat(session.isOpen()).isTrue();
        assertThat(connectionManager.getClientsByAppKey(appKey)).contains(clientId);

        session.close();
        
        // 等待异步清理完成
        long start = System.currentTimeMillis();
        while (connectionManager.getClientsByAppKey(appKey).contains(clientId) && System.currentTimeMillis() - start < 2000) {
            Thread.sleep(100);
        }
        assertThat(connectionManager.getClientsByAppKey(appKey)).doesNotContain(clientId);
    }

    @Test
    void shouldPushMessageToConnectedClient() throws Exception {
        String clientId = "push-client";
        String appKey = "test-app";
        String wsUrl = getWsUrl(clientId, appKey);

        BlockingQueue<String> receivedMessages = new LinkedBlockingQueue<>();
        StandardWebSocketClient client = new StandardWebSocketClient();
        WebSocketSession session = client.execute(new TextWebSocketHandler() {
            @Override
            protected void handleTextMessage(WebSocketSession session, TextMessage message) {
                receivedMessages.add(message.getPayload());
            }
        }, wsUrl).get(5, TimeUnit.SECONDS);

        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, clientId, Collections.emptyMap(), "payload-data", 1000L);
        boolean pushed = connectionManager.push(clientId, frame);
        assertThat(pushed).isTrue();

        String payload = receivedMessages.poll(5, TimeUnit.SECONDS);
        assertThat(payload).contains("payload-data").contains("m1");

        session.close();
    }
}
