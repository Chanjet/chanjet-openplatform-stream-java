package com.chanjet.connector.server.controller;

import com.chanjet.connector.server.dto.TelemetryEventDTO;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
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
    private final ObjectMapper objectMapper = new ObjectMapper();

    @PostMapping("/events")
    public ResponseEntity<Void> receiveEvents(@RequestBody TelemetryEventDTO eventDTO) {
        // 核心字段基本校验
        if (eventDTO == null || !StringUtils.hasText(eventDTO.getEvent()) || !StringUtils.hasText(eventDTO.getFingerprint())) {
            return ResponseEntity.badRequest().build();
        }

        try {
            // 通过重新序列化 DTO，彻底解决 Payload 中包含换行符导致的日志注入问题
            // Jackson 会自动处理并转义非法字符
            String safeJson = objectMapper.writeValueAsString(eventDTO);
            telemetryLogger.info(safeJson);
        } catch (JsonProcessingException e) {
            // 记录原始错误，但不影响接口返回
            return ResponseEntity.badRequest().build();
        }

        return ResponseEntity.ok().build();
    }
}
