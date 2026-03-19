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

    /**
     * 外部 Webhook 入口 (来自 Core)。
     */
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
                Map.of(),
                body,
                System.currentTimeMillis()
        );

        messageDispatcher.dispatch(frame);
    }

    /**
     * 内部 P2P 转发入口 (来自其他网关节点)。
     * 此时已经完成寻址，直接推送到本地 Session。
     */
    @PostMapping("/internal/v1/p2p/push")
    public void receiveP2P(@RequestBody EventFrame frame) {
        // P2P 转发直接下发，不经过逻辑分发器再次寻址，以防循环转发
        // 注意：此处 clientId 已在 frame 的业务上下文或 routing 逻辑中确定
        // 目前简化实现：在 frame 的 headers 中携带或通过 routing 逻辑重取
        // 实际上 MessageDispatcher 在转发前应确保 frame 包含目标 clientId
        connectionManager.push(frame.appKey() + "@local", frame); // 此处 clientId 逻辑待精化
    }
}
