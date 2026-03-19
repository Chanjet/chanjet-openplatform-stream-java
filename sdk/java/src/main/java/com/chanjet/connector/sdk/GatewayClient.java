package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.AckFrame;
import com.chanjet.connector.common.protocol.EventFrame;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.net.http.WebSocket;
import java.time.Duration;
import java.util.concurrent.CompletionStage;

/**
 * 畅捷通 Stream Gateway 客户端 SDK 主入口。
 */
public class GatewayClient {

    private static final Logger log = LoggerFactory.getLogger(GatewayClient.class);

    private final String appKey;
    private final String appSecret;
    private final String gatewayUrl;
    private final HttpClient httpClient;
    private final IConnectionProvider connectionProvider;
    private final ObjectMapper objectMapper = new ObjectMapper();
    
    private WebSocket webSocket;
    private volatile boolean connected = false;
    private EventHandler eventHandler;

    private GatewayClient(Builder builder) {
        this.appKey = builder.appKey;
        this.appSecret = builder.appSecret;
        this.gatewayUrl = builder.gatewayUrl;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(5))
                .build();
        
        this.connectionProvider = (builder.connectionProvider != null) ? builder.connectionProvider : 
            (uri, listener) -> httpClient.newWebSocketBuilder()
                    .connectTimeout(Duration.ofSeconds(10))
                    .buildAsync(uri, listener);
    }

    public static Builder builder() {
        return new Builder();
    }

    public void onEvent(EventHandler handler) {
        this.eventHandler = handler;
    }

    public void start() {
        try {
            String nonce = fetchNonce();
            String sign = SignUtils.hmacSha256(appKey + "&" + nonce, appSecret);
            String connectUrl = gatewayUrl.replace("http://", "ws://").replace("https://", "wss://")
                    + "/connect?app_key=" + appKey + "&nonce=" + nonce + "&sign=" + sign + "&client_id=" + appKey + "@local";

            this.webSocket = connectionProvider.connect(URI.create(connectUrl), new InternalWebSocketListener()).join();
            this.connected = true;
            log.info("Successfully connected to Gateway.");
        } catch (Exception e) {
            throw new RuntimeException("Failed to start GatewayClient: " + e.getMessage(), e);
        }
    }

    public void stop() {
        if (webSocket != null) {
            webSocket.sendClose(WebSocket.NORMAL_CLOSURE, "SDK Stop");
        }
        this.connected = false;
    }

    public boolean isConnected() {
        return connected;
    }

    public String fetchNonce() {
        try {
            String url = gatewayUrl.replace("ws://", "http://").replace("wss://", "https://") 
                    + "/v1/ws/challenge?app_key=" + appKey;
            HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
            HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
            
            if (response.statusCode() != 200) {
                throw new RuntimeException("Failed to fetch nonce: HTTP " + response.statusCode());
            }

            JsonNode root = objectMapper.readTree(response.body());
            return root.path("data").path("nonce").asText();
        } catch (Exception e) {
            throw new RuntimeException("Error fetching nonce: " + e.getMessage(), e);
        }
    }

    private void sendAck(String msgId, boolean success) {
        try {
            AckFrame ack = new AckFrame(msgId, success ? 200 : 500, success ? "success" : "failed", System.currentTimeMillis());
            String json = objectMapper.writeValueAsString(ack);
            webSocket.sendText(json, true);
        } catch (Exception e) {
            log.error("Failed to send ACK: {}", e.getMessage());
        }
    }

    private class InternalWebSocketListener implements WebSocket.Listener {
        @Override
        public void onOpen(WebSocket webSocket) {
            WebSocket.Listener.super.onOpen(webSocket);
        }

        @Override
        public CompletionStage<?> onText(WebSocket webSocket, CharSequence data, boolean last) {
            String text = data.toString();
            try {
                // 1. 判断消息类型
                JsonNode root = objectMapper.readTree(text);
                
                // 2. 只有 event 类型才触发回调
                if (eventHandler != null) {
                    EventFrame frame = objectMapper.treeToValue(root, EventFrame.class);
                    if (frame.msgId() != null) {
                        // 3. 执行回调并自动 ACK
                        boolean success = eventHandler.handle(frame);
                        sendAck(frame.msgId(), success);
                    }
                }
            } catch (Exception e) {
                log.error("Error processing text frame: {}", e.getMessage());
            }
            return WebSocket.Listener.super.onText(webSocket, data, last);
        }

        @Override
        public CompletionStage<?> onClose(WebSocket webSocket, int statusCode, String reason) {
            connected = false;
            return WebSocket.Listener.super.onClose(webSocket, statusCode, reason);
        }

        @Override
        public void onError(WebSocket webSocket, Throwable error) {
            connected = false;
        }
    }

    public static class Builder {
        private String appKey;
        private String appSecret;
        private String gatewayUrl;
        private IConnectionProvider connectionProvider;

        public Builder appKey(String appKey) { this.appKey = appKey; return this; }
        public Builder appSecret(String appSecret) { this.appSecret = appSecret; return this; }
        public Builder gatewayUrl(String gatewayUrl) { this.gatewayUrl = gatewayUrl; return this; }
        public Builder connectionProvider(IConnectionProvider provider) {
            this.connectionProvider = provider;
            return this;
        }
        public GatewayClient build() { return new GatewayClient(this); }
    }
}
