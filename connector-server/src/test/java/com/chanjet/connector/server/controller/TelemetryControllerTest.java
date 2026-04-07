package com.chanjet.connector.server.controller;

import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.servlet.MockMvc;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(TelemetryController.class)
@ActiveProfiles("dev")
@TestPropertySource(properties = {
    "spring.cloud.nacos.config.enabled=false",
    "spring.cloud.nacos.discovery.enabled=false",
    "spring.cloud.nacos.config.import-check.enabled=false",
    "spring.config.import=optional:nacos:",
    "spring.application.cid=VALIDATION_FIX_TEST"
})
public class TelemetryControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Test
    public void testValidPayloadWithMandatoryFields() throws Exception {
        String validJson = "{\"event\":\"test\",\"fingerprint\":\"fp-123\"}";
        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(validJson))
                .andExpect(status().isOk());
    }

    @Test
    public void testMissingFingerprintShouldBeRejected() throws Exception {
        String invalidJson = "{\"event\":\"test\"}"; // 缺少 fingerprint
        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(invalidJson))
                .andExpect(status().isBadRequest());
    }

    @Test
    public void testOverlyLargePayloadShouldBeRejected() throws Exception {
        // 创建超过 2KB 的报文
        StringBuilder sb = new StringBuilder("{\"event\":\"test\",\"fingerprint\":\"fp\", \"data\":\"");
        for (int i = 0; i < 2100; i++) sb.append("x");
        sb.append("\"}");

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(sb.toString()))
                .andExpect(status().isBadRequest());
    }

    @Test
    public void testSanitizationLogic() throws Exception {
        // 包含换行但字段完整的合法 JSON
        String rawJson = "{\"event\":\"test\",\n\"fingerprint\":\"fp-123\"}";
        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(rawJson))
                .andExpect(status().isOk());
    }
}
