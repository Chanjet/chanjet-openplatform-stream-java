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
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.atomic.AtomicReference;

import static com.github.tomakehurst.wiremock.client.WireMock.*;
import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.mock;

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
        
        WebSocketStub stubWs = new WebSocketStub(sentAck);
        IConnectionProvider stubProvider = (uri, listener) -> {
            capturedListener.set(listener);
            return CompletableFuture.completedFuture(stubWs);
        };

        AtomicReference<String> receivedPayload = new AtomicReference<>();
        GatewayClient client = GatewayClient.builder()
                .appKey("app1").appSecret("s1").gatewayUrl(baseUrl)
                .connectionProvider(stubProvider)
                .build();

        client.onEvent(frame -> {
            receivedPayload.set(frame.payload());
            return true; 
        });

        client.start();

        EventFrame frame = new EventFrame("event", "msg-1", "t1", "app1", Collections.emptyMap(), "hello-tdd", 1000L);
        String json = objectMapper.writeValueAsString(frame);
        
        capturedListener.get().onText(stubWs, json, true);

        assertThat(receivedPayload.get()).isEqualTo("hello-tdd");
        assertThat(sentAck.get()).contains("\"msg_id\":\"msg-1\"").contains("\"code\":200");
    }

    private static class WebSocketStub implements WebSocket {
        private final AtomicReference<String> sentData;
        public WebSocketStub(AtomicReference<String> sentData) { this.sentData = sentData; }
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
