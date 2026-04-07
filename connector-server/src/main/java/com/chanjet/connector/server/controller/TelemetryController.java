package com.chanjet.connector.server.controller;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

@RestController
@RequestMapping("/v1/telemetry")
public class TelemetryController {

    private static final Logger telemetryLogger = LoggerFactory.getLogger("telemetry");

    @PostMapping("/events")
    public ResponseEntity<Void> receiveEvents(@RequestBody String eventJson) {
        // 直接将接收到的 JSON 字符串原样记录到 telemetry 专用日志中
        telemetryLogger.info(eventJson);
        return ResponseEntity.ok().build();
    }
}
