package com.chanjet.connector.common.protocol;

import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import com.fasterxml.jackson.databind.annotation.JsonNaming;
import java.util.Map;

/**
 * 核心数据推送帧 (Gateway -> Client)
 */
@JsonNaming(PropertyNamingStrategies.SnakeCaseStrategy.class)
public record EventFrame(
    String msgType, // "event"
    String msgId,
    String traceId,
    String appKey,
    Map<String, String> headers,
    String payload,
    long timestamp
) {}
