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
    "spring.application.cid=OPTIMIZE_TEST"
})
public class TelemetryControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Test
    public void testInjectedPayloadShouldBeSanitizedToSingleLine() throws Exception {
        // 包含换行符的输入
        String rawJson = "{\"event\":\"test\"}\n{\"info\":\"injected\"}";
        Path logPath = Paths.get("target/logs/OPTIMIZE_TEST/telemetry.log");
        if (Files.exists(logPath)) Files.delete(logPath);

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(rawJson))
                .andExpect(status().isOk());

        // 等待异步写入
        Thread.sleep(500);

        List<String> lines = Files.readAllLines(logPath);
        
        // 验证：虽然输入有换行，但日志中必须只有 1 行
        assertThat(lines.size()).isEqualTo(1);
        assertThat(lines.get(0)).doesNotContain("\n").doesNotContain("\r");
    }

    @Test
    public void testInvalidFormatShouldBeRejected() throws Exception {
        // 显然不是 JSON 的垃圾数据
        String garbage = "hello world";

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(garbage))
                .andExpect(status().isBadRequest());
    }
}
