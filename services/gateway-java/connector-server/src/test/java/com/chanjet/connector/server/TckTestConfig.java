package com.chanjet.connector.server;

import com.chanjet.connector.api.store.INonceStore;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

@TestConfiguration
public class TckTestConfig {

    @RestController
    public static class TckChallengeController {
        @Autowired
        private INonceStore nonceStore;

        @GetMapping("/v1/ws/challenge")
        public Map<String, Object> challenge(@RequestParam("app_key") String appKey) {
            String nonce = nonceStore.createNonce(appKey);
            return Map.of("code", "GW-0000", "data", Map.of("nonce", nonce != null ? nonce : "mock-nonce"));
        }
    }
}
