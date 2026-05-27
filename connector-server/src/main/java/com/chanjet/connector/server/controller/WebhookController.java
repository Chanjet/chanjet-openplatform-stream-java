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
import org.springframework.web.context.request.async.DeferredResult;
import org.springframework.http.ResponseEntity;
import org.springframework.http.HttpStatus;
import com.chanjet.connector.core.dispatcher.AckManager;

import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * Webhook 接收器，支持外部推送与内部 P2P 转发。
 */
@RestController
public class WebhookController {

    private final MessageDispatcher messageDispatcher;
    private final IConnectionManager connectionManager;
    private final ConnectorProperties properties;
    private final AckManager ackManager;

    public WebhookController(MessageDispatcher messageDispatcher, 
                             IConnectionManager connectionManager,
                             ConnectorProperties properties,
                             AckManager ackManager) {
        this.messageDispatcher = messageDispatcher;
        this.connectionManager = connectionManager;
        this.properties = properties;
        this.ackManager = ackManager;
    }

    @PostMapping("/internal/v1/webhook/dispatch")
    public DeferredResult<ResponseEntity<Map<String, String>>> dispatch(
            @RequestHeader("X-C-APP_KEY") String appKey,
            @RequestHeader("X-MSG-ID") String msgId,
            @RequestHeader(value = "X-Trace-Id", required = false) String traceId,
            @RequestBody String body) {

        DeferredResult<ResponseEntity<Map<String, String>>> deferredResult = new DeferredResult<>(10000L);
        deferredResult.onTimeout(() -> {
            deferredResult.setErrorResult(ResponseEntity.status(HttpStatus.GATEWAY_TIMEOUT).body(Map.of("error", "Dispatch timeout")));
        });

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

        CompletableFuture<Boolean> dispatchFuture = messageDispatcher.dispatch(frame);
        dispatchFuture.whenComplete((success, ex) -> {
            if (success != null && success) {
                deferredResult.setResult(ResponseEntity.ok(Map.of("result", "success")));
            } else {
                deferredResult.setErrorResult(ResponseEntity.status(HttpStatus.SERVICE_UNAVAILABLE).body(Map.of("error", "Dispatch failed")));
            }
        });

        return deferredResult;
    }

    @PostMapping("/internal/v1/p2p/push")
    public DeferredResult<ResponseEntity<Map<String, String>>> receiveP2P(
            @RequestHeader(value = "X-Internal-Token", required = false) String token,
            @RequestBody EventFrame frame) {
        
        // 校验令牌
        if (!properties.isValidToken(token)) {
            org.slf4j.LoggerFactory.getLogger(WebhookController.class)
                .error("P2P Auth Failed. Received: [{}], Expected one of: {}", token, properties.getInternalTokens());
            throw new InvalidInternalTokenException();
        }

        DeferredResult<ResponseEntity<Map<String, String>>> deferredResult = new DeferredResult<>(10000L);
        deferredResult.onTimeout(() -> {
            deferredResult.setErrorResult(ResponseEntity.status(HttpStatus.GATEWAY_TIMEOUT).body(Map.of("error", "P2P Push timeout")));
        });

        CompletableFuture<Boolean> ackFuture = ackManager.registerAck(frame.msgId(), 10000);
        
        boolean pushed = false;
        // 如果指定了具体 ClientId 则精确推送，否则对该 AppKey 进行本地广播
        if (frame.targetClientId() != null) {
            pushed = connectionManager.push(frame.targetClientId(), frame);
        } else {
            // 查找本地所有匹配 AppKey 的连接并推送
            long successCount = connectionManager.getClientsByAppKey(frame.appKey())
                .stream()
                .filter(clientId -> connectionManager.push(clientId, frame))
                .count();
            pushed = successCount > 0;
        }

        if (!pushed) {
            ackManager.completeAck(frame.msgId(), false);
        }

        ackFuture.whenComplete((success, ex) -> {
            if (success != null && success) {
                deferredResult.setResult(ResponseEntity.ok(Map.of("result", "success")));
            } else {
                deferredResult.setErrorResult(ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR).body(Map.of("error", "P2P Client ACK failed")));
            }
        });

        return deferredResult;
    }

    @PostMapping("/internal/v1/p2p/evict/{clientId}")
    public Map<String, String> evictP2P(
            @RequestHeader(value = "X-Internal-Token", required = false) String token,
            @PathVariable("clientId") String clientId) {
        
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
