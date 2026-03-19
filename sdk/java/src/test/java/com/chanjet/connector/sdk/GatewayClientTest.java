package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.EventFrame;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.github.tomakehurst.wiremock.junit5.WireMockRuntimeInfo;
import com.github.tomakehurst.wiremock.junit5.WireMockTest;
import org.junit.jupiter.api.Test;

import java.net.URI;
import java.net.http.WebSocket;
import java.nio.ByteBuffer;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.atomic.AtomicReference;

import static com.github.tomakehurst.wiremock.client.WireMock.*;
import static org.assertj.core.api.Assertions.assertThat;

@WireMockTest
class GatewayClientTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

    @Test
    void shouldHandleMessageAndSendAck(WireMockRuntimeInfo wmRuntimeInfo) throws Exception {
        String baseUrl = wmRuntimeInfo.getHttpBaseUrl();
        stubFor(get(urlMatching("/v1/ws/challenge.*"))
                .willReturn(okJson("{\"code\":\"GW-0000\",\"data\":{\"nonce\":\"n1\"}}")));

        AtomicReference<String> sentAck = new AtomicReference<>();
        AtomicReference<WebSocket.Listener> capturedListener = new AtomicReference<>();
        
        // 1. 手动实现 WebSocket 桩
        WebSocket stubWs = new WebSocketStub(sentAck);

        // 2. 手动实现 ConnectionProvider 桩
        IConnectionProvider stubProvider = (uri, listener) -> {
            capturedListener.set(listener);
            return CompletableFuture.completedFuture(stubWs);
        };

        AtomicReference<String> receivedPayload = new AtomicReference<>();
        GatewayClient client = GatewayClient.builder()
                .appKey("app1").appSecret("s1").gatewayUrl(baseUrl)
                .connectionProvider(stubProvider)
                .build();

        // 3. 注册业务回调
        client.onEvent(frame -> {
            receivedPayload.set(frame.payload());
            return true; 
        });

        client.start();

        // 4. 模拟网关下发消息
        EventFrame frame = new EventFrame("msg-1", "t1", "app1", Map.of(), "hello-tdd", 1000L);
        String json = objectMapper.writeValueAsString(frame);
        
        capturedListener.get().onText(stubWs, json, true);

        // 5. 验证结果
        assertThat(receivedPayload.get()).isEqualTo("hello-tdd");
        assertThat(sentAck.get()).contains("\"msg_id\":\"msg-1\"").contains("\"code\":200");
    }

    /**
     * 手动实现的 WebSocket 桩，用于捕获发送的数据。
     */
    private static class WebSocketStub implements WebSocket {
        private final AtomicReference<String> sentData;

        public WebSocketStub(AtomicReference<String> sentData) {
            this.sentData = sentData;
        }

        @Override
        public CompletableFuture<WebSocket> sendText(CharSequence data, boolean last) {
            sentData.set(data.toString());
            return CompletableFuture.completedFuture(this);
        }

        @Override public CompletableFuture<WebSocket> sendBinary(ByteBuffer data, boolean last) { return null; }
        @Override public CompletableFuture<WebSocket> sendPing(ByteBuffer message) { return null; }
        @Override public CompletableFuture<WebSocket> sendPong(ByteBuffer message) { return null; }
        @Override public CompletableFuture<WebSocket> sendClose(int statusCode, String reason) { return null; }
        @Override public void request(long n) {}
        @Override public String getSubprotocol() { return ""; }
        @Override public boolean isOutputClosed() { return false; }
        @Override public boolean isInputClosed() { return false; }
        @Override public void abort() {}
    }
}
