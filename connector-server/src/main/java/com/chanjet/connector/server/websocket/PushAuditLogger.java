package com.chanjet.connector.server.websocket;

import com.chanjet.connector.common.protocol.AckFrame;
import com.chanjet.connector.common.protocol.EventFrame;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

/**
 * 消息推送审计日志器。
 * 专门记录每条消息从分发、物理下发到接收 ACK 的完整生命周期。
 */
@Component
public class PushAuditLogger {

    private static final Logger log = LoggerFactory.getLogger("push-audit");

    /**
     * 记录消息分发开始
     */
    public void logDispatch(EventFrame frame, String nodeId) {
        log.info("[DISPATCH] Node: {}, MsgId: {}, AppKey: {}, TraceId: {}", 
            nodeId, frame.msgId(), frame.appKey(), frame.traceId());
    }

    /**
     * 记录物理推送结果 (本地)
     */
    public void logPushResult(String msgId, String clientId, boolean success, String reason) {
        if (success) {
            log.info("[PUSH_SUCCESS] MsgId: {}, Target: {}", msgId, clientId);
        } else {
            log.error("[PUSH_FAILED] MsgId: {}, Target: {}, Reason: {}", msgId, clientId, reason);
        }
    }

    /**
     * 记录远程转发结果 (P2P)
     */
    public void logForwardResult(String msgId, String targetNodeId, boolean success, String reason) {
        if (success) {
            log.info("[FORWARD_SUCCESS] MsgId: {}, TargetNode: {}", msgId, targetNodeId);
        } else {
            log.error("[FORWARD_FAILED] MsgId: {}, TargetNode: {}, Reason: {}", msgId, targetNodeId, reason);
        }
    }

    /**
     * 记录客户端 ACK 响应
     */
    public void logAck(String clientId, AckFrame ack) {
        if (ack.code() == 200) {
            log.info("[ACK_RECEIVED] MsgId: {}, From: {}, Code: {}, Message: {}", 
                ack.msgId(), clientId, ack.code(), ack.message());
        } else {
            log.warn("[ACK_ERROR] MsgId: {}, From: {}, Code: {}, Message: {}", 
                ack.msgId(), clientId, ack.code(), ack.message());
        }
    }

    /**
     * 记录分发阶段的致命错误 (例如找不到任何在线客户端)
     */
    public void logDispatchError(String msgId, String appKey, String reason) {
        log.error("[DISPATCH_ERROR] MsgId: {}, AppKey: {}, Reason: {}", msgId, appKey, reason);
    }

    /**
     * 记录限流丢弃
     */
    public void logThrottled(String msgId, String appKey) {
        log.warn("[THROTTLED] MsgId: {}, AppKey: {}", msgId, appKey);
    }
}
