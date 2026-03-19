package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
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

    public WebhookController(MessageDispatcher messageDispatcher, IConnectionManager connectionManager) {
        this.messageDispatcher = messageDispatcher;
        this.connectionManager = connectionManager;
    }

    @PostMapping("/internal/v1/webhook/dispatch")
    public void dispatch(
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
    }

    @PostMapping("/internal/v1/p2p/push")
    public void receiveP2P(@RequestBody EventFrame frame) {
        // 优先使用 P2P 报文中的 targetClientId 进行本地精准推送
        String targetId = (frame.targetClientId() != null) ? 
                frame.targetClientId() : 
                (frame.appKey() + "@local");
        
        connectionManager.push(targetId, frame);
    }
}
