package com.chanjet.connector.server.controller;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.ResponseEntity;
import org.springframework.util.StringUtils;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/v1/telemetry")
public class TelemetryController {

    private static final Logger telemetryLogger = LoggerFactory.getLogger("telemetry");

    @PostMapping("/events")
    public ResponseEntity<Void> receiveEvents(@RequestBody String rawJson) {
        // 1. 基本判空与长度限制 (防止超大报文攻击)
        if (!StringUtils.hasText(rawJson) || rawJson.length() > 2048) {
            return ResponseEntity.badRequest().build();
        }

        // 2. 极致性能：清洗换行符以防止日志注入
        String sanitized = rawJson.replace('\n', ' ').replace('\r', ' ').trim();

        // 3. 增强版轻量级合法性检查
        // 必须以 { 开始 } 结束，且必须包含 event 和 fingerprint 关键字（以 JSON Key 形式）
        if (!sanitized.startsWith("{") || !sanitized.endsWith("}") 
                || !sanitized.contains("\"event\"") 
                || !sanitized.contains("\"fingerprint\"")) {
            return ResponseEntity.badRequest().build();
        }

        // 4. 异步记录
        telemetryLogger.info(sanitized);

        return ResponseEntity.ok().build();
    }
}
