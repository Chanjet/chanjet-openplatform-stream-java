package com.chanjet.connector.common.protocol;

/**
 * 业务处理响应帧 (Client -> Gateway)
 */
public record AckFrame(
    String msgId,
    int code,
    String message,
    long timestamp
) {}
