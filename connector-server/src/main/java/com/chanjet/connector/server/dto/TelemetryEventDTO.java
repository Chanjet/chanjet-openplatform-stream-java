package com.chanjet.connector.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;

import java.util.Map;

@Data
public class TelemetryEventDTO {
    @JsonProperty("event")
    private String event;

    @JsonProperty("fingerprint")
    private String fingerprint;

    @JsonProperty("app_key")
    private String appKey;

    @JsonProperty("version")
    private String version;

    @JsonProperty("os")
    private String os;

    @JsonProperty("arch")
    private String arch;

    @JsonProperty("timestamp")
    private String timestamp;

    @JsonProperty("payload")
    private Map<String, Object> payload;
}
