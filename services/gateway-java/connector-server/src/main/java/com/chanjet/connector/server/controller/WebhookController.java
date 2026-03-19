package com.chanjet.connector.server.controller;

import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

/**
 * Webhook 接收器，将外部推送路由至领域分发器。
 */
@RestController
public class WebhookController {

    private final MessageDispatcher messageDispatcher;

    public WebhookController(MessageDispatcher messageDispatcher) {
        this.messageDispatcher = messageDispatcher;
    }

    @PostMapping("/internal/v1/webhook/dispatch")
    public void dispatch(
            @RequestHeader("X-C-APP_KEY") String appKey,
            @RequestHeader("X-MSG-ID") String msgId,
            @RequestHeader(value = "X-Trace-Id", required = false) String traceId,
            @RequestBody String body) {

        EventFrame frame = new EventFrame(
                msgId,
                traceId != null ? traceId : msgId,
                appKey,
                Map.of(), // 可根据需要提取更多 Headers 存入 map
                body,
                System.currentTimeMillis()
        );

        messageDispatcher.dispatch(frame);
    }
}
