package com.chanjet.connector.common.protocol;

import java.util.Map;

/**
 * 核心数据推送帧 (Gateway -> Client)
 */
public record EventFrame(
    String msgId,
    String traceId,
    String appKey,
    Map<String, String> headers,
    String payload,
    long timestamp
) {}
