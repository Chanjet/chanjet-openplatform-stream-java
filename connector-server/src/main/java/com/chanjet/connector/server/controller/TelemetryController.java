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
        // 1. 基本判空
        if (!StringUtils.hasText(rawJson)) {
            return ResponseEntity.badRequest().build();
        }

        // 2. 极致性能：清洗换行符以防止日志注入 (Log Injection)
        // 直接操作原始字符串，不涉及昂贵的 Jackson 反序列化
        String sanitized = rawJson.replace('\n', ' ').replace('\r', ' ').trim();

        // 3. 轻量级合法性检查 (极速模式)
        // 确保它像个 JSON 对象且包含核心关键字
        if (!sanitized.startsWith("{") || !sanitized.endsWith("}") || !sanitized.contains("\"event\"")) {
            return ResponseEntity.badRequest().build();
        }

        // 4. 异步记录
        telemetryLogger.info(sanitized);

        return ResponseEntity.ok().build();
    }
}
