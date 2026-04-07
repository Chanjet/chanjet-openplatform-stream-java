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
    "spring.application.cid=TELEMETRY_TEST"
})
public class TelemetryControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Test
    public void testPostTelemetryEventAndVerifyLog() throws Exception {
        String eventJson = "{\"event\":\"command_run\",\"fingerprint\":\"test-fp\",\"app_key\":\"test-key\"}";

        mockMvc.perform(post("/v1/telemetry/events")
                .contentType(MediaType.APPLICATION_JSON)
                .content(eventJson))
                .andExpect(status().isOk());

        // 验证日志文件写入
        // 在 mvn 子模块运行时，target 通常在子模块目录下
        Path logPath = Paths.get("target/logs/TELEMETRY_TEST/telemetry.log");
        
        // 尝试多次寻找
        int retries = 10;
        while (retries > 0 && !Files.exists(logPath)) {
            Thread.sleep(500);
            retries--;
        }

        assertThat(Files.exists(logPath)).as("Telemetry log file should exist at: " + logPath.toAbsolutePath()).isTrue();
        List<String> lines = Files.readAllLines(logPath);
        assertThat(lines).anyMatch(line -> line.contains("test-fp") && line.contains("test-key"));
    }
}
