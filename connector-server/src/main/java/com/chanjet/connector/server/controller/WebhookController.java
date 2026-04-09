package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.exception.InvalidInternalTokenException;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import com.chanjet.connector.api.config.ConnectorProperties;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

/**
 * Webhook 接收器，支持外部推送与内部 P2P 转发。
 */
@RestController
public class WebhookController {

    private final MessageDispatcher messageDispatcher;
    private final IConnectionManager connectionManager;
    private final ConnectorProperties properties;

    public WebhookController(MessageDispatcher messageDispatcher, 
                             IConnectionManager connectionManager,
                             ConnectorProperties properties) {
        this.messageDispatcher = messageDispatcher;
        this.connectionManager = connectionManager;
        this.properties = properties;
    }

    @PostMapping("/internal/v1/webhook/dispatch")
    public Map<String, String> dispatch(
            @RequestHeader("X-C-APP_KEY") String appKey,
            @RequestHeader("X-MSG-ID") String msgId,
            @RequestHeader(value = "X-Trace-Id", required = false) String traceId,
            @RequestBody String body) {

        EventFrame frame = new EventFrame(
                "event",
                msgId,
                traceId != null ? traceId : msgId,
                appKey,
                null,
                Map.of(),
                body,
                System.currentTimeMillis()
        );

        messageDispatcher.dispatch(frame);
        return Map.of("result", "success");
    }

    @PostMapping("/internal/v1/p2p/push")
    public void receiveP2P(
            @RequestHeader(value = "X-Internal-Token", required = false) String token,
            @RequestBody EventFrame frame) {
        
        // 校验令牌
        if (!properties.isValidToken(token)) {
            org.slf4j.LoggerFactory.getLogger(WebhookController.class)
                .error("P2P Auth Failed. Received: [{}], Expected one of: {}", token, properties.getInternalTokens());
            throw new InvalidInternalTokenException();
        }

        // 如果指定了具体 ClientId 则精确推送，否则对该 AppKey 进行本地广播
        if (frame.targetClientId() != null) {
            connectionManager.push(frame.targetClientId(), frame);
        } else {
            // 查找本地所有匹配 AppKey 的连接并推送
            connectionManager.getClientsByAppKey(frame.appKey())
                .forEach(clientId -> connectionManager.push(clientId, frame));
        }
    }

    @PostMapping("/internal/v1/p2p/evict/{clientId}")
    public Map<String, String> evictP2P(
            @RequestHeader(value = "X-Internal-Token", required = false) String token,
            @PathVariable String clientId) {
        
        // 校验令牌
        if (!properties.isValidToken(token)) {
            org.slf4j.LoggerFactory.getLogger(WebhookController.class)
                .error("P2P Auth Failed for Eviction. Expected one of: {}", properties.getInternalTokens());
            throw new InvalidInternalTokenException();
        }

        org.slf4j.LoggerFactory.getLogger(WebhookController.class)
            .info("Received P2P eviction request for client: {}", clientId);
            
        connectionManager.close(clientId, "Conflict: Reconnected to another node");
        return Map.of("result", "success");
    }
}
