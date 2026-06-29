package com.chanjet.connector.sdk;

import com.chanjet.connector.sdk.protocol.AckFrame;
import com.chanjet.connector.sdk.protocol.EventFrame;
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
import java.util.Random;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * 畅捷通 Stream Gateway 客户端 SDK 主入口。
 * 支持智能重连策略：区分故障退避与排队待命。
 */
public class GatewayClient {

    private static final Logger log = LoggerFactory.getLogger(GatewayClient.class);

    private final String appKey;
    private final String appSecret;
    private final String encryptKey;
    private final String gatewayUrl;
    private final String clientId;
    private final IHttpProvider httpProvider;
    private final IConnectionProvider connectionProvider;
    private final ObjectMapper objectMapper = new ObjectMapper()
            .setPropertyNamingStrategy(com.fasterxml.jackson.databind.PropertyNamingStrategies.SNAKE_CASE)
            .configure(com.fasterxml.jackson.databind.DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);
    
    private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread t = new Thread(r, "gateway-client-scheduler");
        t.setDaemon(true);
        return t;
    });

    private final Random random = new Random();
    private WebSocket webSocket;
    private volatile boolean connected = false;
    private volatile boolean running = false;
    private EventHandler eventHandler;
    private MessageDispatcher messageDispatcher;
    private int attempt = 0;

    private GatewayClient(Builder builder) {
        this.appKey = builder.appKey;
        this.appSecret = builder.appSecret;
        this.encryptKey = (builder.encryptKey != null) ? builder.encryptKey : builder.appSecret;
        if (builder.gatewayUrl == null || builder.gatewayUrl.trim().isEmpty()) {
            this.gatewayUrl = "https://stream-open.chanapp.chanjet.com";
        } else {
            this.gatewayUrl = builder.gatewayUrl;
        }
        // 自动生成唯一 ClientId: appKey@hostname_pid_uuid
        String hostname = getHostname();
        long pid = ProcessHandle.current().pid();
        String uniqueId = java.util.UUID.randomUUID().toString().substring(0, 8);
        this.clientId = String.format("%s@%s_%d_%s", appKey, hostname, pid, uniqueId);
        
        HttpClient client = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(5))
                .build();
        
        this.httpProvider = (builder.httpProvider != null) ? builder.httpProvider : 
            (req) -> client.send(req, HttpResponse.BodyHandlers.ofString());

        this.connectionProvider = (builder.connectionProvider != null) ? builder.connectionProvider : 
            (uri, listener) -> client.newWebSocketBuilder()
                    .connectTimeout(Duration.ofSeconds(10))
                    .buildAsync(uri, listener);
    }

    public static Builder builder() {
        return new Builder();
    }

    public void onEvent(EventHandler handler) {
        this.eventHandler = handler;
    }

    /**
     * 配置业务消息分发器。
     * 配置后，SDK 将自动执行解密、验签并分发到具体的业务 Handler。
     */
    public void useDispatcher(MessageDispatcher dispatcher) {
        this.messageDispatcher = dispatcher;
    }

    public synchronized void start() {
        if (running) return;
        this.running = true;
        connectAsync();
    }

    public synchronized void stop() {
        this.running = false;
        if (webSocket != null) {
            webSocket.sendClose(WebSocket.NORMAL_CLOSURE, "SDK Stop");
        }
        this.connected = false;
        scheduler.shutdown();
    }

    public boolean isConnected() {
        return connected;
    }

    private void connectAsync() {
        if (!running) return;

        scheduler.execute(() -> {
            try {
                log.info("Attempting to connect to Gateway (Attempt: {})...", attempt + 1);
                
                String nonce = fetchNonce();
                if (nonce == null) return;

                String sign = SignUtils.hmacSha256(appKey + "&" + nonce, appSecret);
                
                String connectUrl = gatewayUrl.replace("http://", "ws://").replace("https://", "wss://")
                        + "/connect?app_key=" + appKey 
                        + "&nonce=" + nonce 
                        + "&sign=" + sign 
                        + "&client_id=" + clientId;

                this.webSocket = connectionProvider.connect(URI.create(connectUrl), new InternalWebSocketListener()).join();
                this.connected = true;
                this.attempt = 0; 
                log.info("Successfully connected to Gateway.");
            } catch (Exception e) {
                log.error("Failed to connect: {}", e.getMessage());
                handleReconnect(503); 
            }
        });
    }

    private String fetchNonce() {
        try {
            String url = gatewayUrl.replace("ws://", "http://").replace("wss://", "https://") 
                    + "/v1/ws/challenge?app_key=" + appKey;
            
            String signPrefix = SignUtils.hmacSha256(appKey, appSecret).substring(0, 16);
            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create(url))
                    .header("X-CJT-PreAuth", signPrefix)
                    .GET()
                    .build();
            
            HttpResponse<String> response = httpProvider.send(request);
            
            if (response.statusCode() == 200) {
                JsonNode root = objectMapper.readTree(response.body());
                return root.path("data").path("nonce").asText();
            } else {
                log.warn("Fetch nonce failed with status: {}", response.statusCode());
                handleReconnect(response.statusCode());
                return null;
            }
        } catch (Exception e) {
            log.error("Error fetching nonce: {}", e.getMessage());
            handleReconnect(503);
            return null;
        }
    }

    private void handleReconnect(int statusCode) {
        if (!running) return;

        long delay;
        if (statusCode == 503 || statusCode == 429) {
            delay = 5000 + random.nextInt(10000);
            log.info("Gateway is busy (HTTP {}), entering STANDBY mode. Next attempt in {}ms", statusCode, delay);
        } else if (statusCode == 401 || statusCode == 403) {
            log.error("Authentication failed (HTTP {}). Permanent failure, stopping.", statusCode);
            this.running = false;
            return;
        } else {
            delay = Math.min(60000, 1000L * (long) Math.pow(2, attempt++));
            log.info("Connection failed (HTTP {}), entering BACKOFF mode. Next attempt in {}ms", statusCode, delay);
        }

        scheduler.schedule(this::connectAsync, delay, TimeUnit.MILLISECONDS);
    }

    private void sendAck(String msgId, boolean success) {
        if (webSocket == null || !connected) return;
        try {
            AckFrame ack = new AckFrame(msgId, success ? 200 : 500, success ? "success" : "failed", System.currentTimeMillis());
            String json = objectMapper.writeValueAsString(ack);
            webSocket.sendText(json, true);
        } catch (Exception e) {
            log.error("Failed to send ACK: {}", e.getMessage());
        }
    }

    private String getHostname() {
        // 优先从环境变量读取，避免 InetAddress.getLocalHost() 可能存在的阻塞
        String host = System.getenv("HOSTNAME");
        if (host == null) host = System.getenv("COMPUTERNAME");
        if (host != null) return host;

        try {
            return java.net.InetAddress.getLocalHost().getHostName();
        } catch (Exception e) {
            return "unknown";
        }
    }

    private class InternalWebSocketListener implements WebSocket.Listener {
        @Override
        public void onOpen(WebSocket webSocket) {
            log.info("WebSocket Session opened.");
            WebSocket.Listener.super.onOpen(webSocket);
        }

        @Override
        public CompletionStage<?> onText(WebSocket webSocket, CharSequence data, boolean last) {
            String text = data.toString();
            try {
                JsonNode root = objectMapper.readTree(text);
                String msgType = root.path("msg_type").asText();

                if ("event".equals(msgType)) {
                    EventFrame frame = objectMapper.treeToValue(root, EventFrame.class);
                    boolean success = false;
                    
                    if (messageDispatcher != null) {
                        success = messageDispatcher.dispatch(frame, encryptKey);
                    } else if (eventHandler != null) {
                        success = eventHandler.handle(frame);
                    }
                    
                    sendAck(frame.msgId(), success);
                } else if ("ping".equals(msgType)) {
                    webSocket.sendText("{\"msg_type\":\"pong\"}", true);
                }
            } catch (Exception e) {
                log.error("Error processing text frame: {}", e.getMessage());
            }
            return WebSocket.Listener.super.onText(webSocket, data, last);
        }

        @Override
        public CompletionStage<?> onClose(WebSocket webSocket, int statusCode, String reason) {
            log.warn("WebSocket closed: {} - {}", statusCode, reason);
            connected = false;
            if (running) {
                handleReconnect(statusCode == 1008 ? 403 : 503); 
            }
            return WebSocket.Listener.super.onClose(webSocket, statusCode, reason);
        }

        @Override
        public void onError(WebSocket webSocket, Throwable error) {
            log.error("WebSocket error: {}", error.getMessage());
            connected = false;
            if (running) {
                handleReconnect(503);
            }
        }
    }

    public static class Builder {
        private String appKey;
        private String appSecret;
        private String encryptKey;
        private String gatewayUrl;
        private IHttpProvider httpProvider;
        private IConnectionProvider connectionProvider;

        public Builder appKey(String appKey) { this.appKey = appKey; return this; }
        public Builder appSecret(String appSecret) { this.appSecret = appSecret; return this; }
        public Builder encryptKey(String encryptKey) { this.encryptKey = encryptKey; return this; }
        public Builder gatewayUrl(String gatewayUrl) { this.gatewayUrl = gatewayUrl; return this; }
        public Builder httpProvider(IHttpProvider provider) { this.httpProvider = provider; return this; }
        public Builder connectionProvider(IConnectionProvider provider) {
            this.connectionProvider = provider;
            return this;
        }
        public GatewayClient build() { return new GatewayClient(this); }
    }
}
