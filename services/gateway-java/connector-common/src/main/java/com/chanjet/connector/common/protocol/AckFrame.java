package com.chanjet.connector.common.protocol;

import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import com.fasterxml.jackson.databind.annotation.JsonNaming;

/**
 * 业务处理响应帧 (Client -> Gateway)
 */
@JsonNaming(PropertyNamingStrategies.SnakeCaseStrategy.class)
public record AckFrame(
    String msgId,
    int code,
    String message,
    long timestamp
) {}
