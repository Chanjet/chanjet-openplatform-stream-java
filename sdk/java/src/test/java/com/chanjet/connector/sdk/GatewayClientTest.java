package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.EventFrame;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import java.net.URI;
import java.net.http.WebSocket;
import java.util.Collections;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

class GatewayClientTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

    @Test
    void shouldProcessEventAndSendAck() throws Exception {
        WebSocket mockWebSocket = mock(WebSocket.class);
        IConnectionProvider mockProvider = (uri, listener) -> CompletableFuture.completedFuture(mockWebSocket);

        GatewayClient client = GatewayClient.builder()
                .appKey("test-app")
                .appSecret("test-secret")
                .gatewayUrl("ws://localhost:8080")
                .connectionProvider(mockProvider)
                .build();

        AtomicBoolean handled = new AtomicBoolean(false);
        client.onEvent(frame -> {
            handled.set(true);
            return true;
        });

        // 注入状态
        java.lang.reflect.Field connectedField = GatewayClient.class.getDeclaredField("connected");
        connectedField.setAccessible(true);
        connectedField.set(client, true);
        
        java.lang.reflect.Field wsField = GatewayClient.class.getDeclaredField("webSocket");
        wsField.setAccessible(true);
        wsField.set(client, mockWebSocket);

        EventFrame frame = new EventFrame(
                "event", "m1", "t1", "test-app", "p2p-client",
                Collections.emptyMap(), "{\"data\":1}", System.currentTimeMillis()
        );
        String json = objectMapper.writeValueAsString(frame);

        // 精确寻找 InternalWebSocketListener 类
        Class<?> listenerClass = null;
        for (Class<?> clazz : GatewayClient.class.getDeclaredClasses()) {
            if (clazz.getName().endsWith("InternalWebSocketListener")) {
                listenerClass = clazz;
                break;
            }
        }
        
        assertThat(listenerClass).isNotNull();
        java.lang.reflect.Constructor<?> constructor = listenerClass.getDeclaredConstructor(GatewayClient.class);
        constructor.setAccessible(true);
        Object listenerInstance = constructor.newInstance(client);

        java.lang.reflect.Method onTextMethod = listenerClass.getDeclaredMethod("onText", WebSocket.class, CharSequence.class, boolean.class);
        onTextMethod.setAccessible(true);
        
        onTextMethod.invoke(listenerInstance, mockWebSocket, json, true);

        assertThat(handled.get()).isTrue();
        verify(mockWebSocket).sendText(contains("\"msg_id\":\"m1\""), anyBoolean());
    }
}
