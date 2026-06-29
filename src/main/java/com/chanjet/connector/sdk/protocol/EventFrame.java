package com.chanjet.connector.sdk.protocol;

import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import com.fasterxml.jackson.databind.annotation.JsonNaming;
import java.util.Map;

/**
 * 核心数据推送帧 (Gateway -> Client)
 */
@JsonNaming(PropertyNamingStrategies.SnakeCaseStrategy.class)
public record EventFrame(
    String msgType,      // "event"
    String msgId,
    String traceId,
    String appKey,
    String targetClientId, // 新增：用于 P2P 精确寻址
    Map<String, String> headers,
    String payload,
    long timestamp
) {}
