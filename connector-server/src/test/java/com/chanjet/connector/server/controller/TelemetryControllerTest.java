package com.chanjet.connector.server.controller;

import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.servlet.MockMvc;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(TelemetryController.class)
@ActiveProfiles("dev")
@TestPropertySource(properties = {
    "spring.cloud.nacos.config.enabled=false",
    "spring.cloud.nacos.discovery.enabled=false",
    "spring.cloud.nacos.config.import-check.enabled=false",
    "spring.config.import=optional:nacos:",
    "spring.application.cid=SECURITY_FIX_TEST"
})
public class TelemetryControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Test
    public void testInjectedPayloadShouldBeRejected() throws Exception {
        // 发送非法 JSON（包含换行符尝试破坏格式）
        // Jackson 在反序列化 DTO 时会直接报错或将换行符作为字段内容处理
        String maliciousJson = "{\"event\":\"valid\"}\n{\"event\":\"injected\"}";

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(maliciousJson))
                .andExpect(status().isBadRequest()); // 预期 Jackson 会因为非法 JSON 结构返回 400
    }

    @Test
    public void testMissingMandatoryFieldsShouldBeRejected() throws Exception {
        // 缺少 event 字段
        String invalidJson = "{\"fingerprint\":\"test-fp\"}";

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(invalidJson))
                .andExpect(status().isBadRequest());
    }

    @Test
    public void testValidPayloadWithOptionalAppKey() throws Exception {
        // app_key 为空时依然应该成功
        String validJson = "{\"event\":\"cmd\",\"fingerprint\":\"fp-123\",\"app_key\":\"\"}";

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(validJson))
                .andExpect(status().isOk());
    }
}
