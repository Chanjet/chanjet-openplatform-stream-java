package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.store.INonceStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

/**
 * 挑战码（Nonce）颁发入口。
 */
@RestController
public class ChallengeController {

    private static final Logger log = LoggerFactory.getLogger(ChallengeController.class);
    private final INonceStore nonceStore;
    private final IAuthService authService;

    public ChallengeController(INonceStore nonceStore, IAuthService authService) {
        this.nonceStore = nonceStore;
        this.authService = authService;
    }

    /**
     * 获取具有时效性的 Nonce。
     * 需携带 X-CJT-PreAuth 以进行轻量级身份预校验。
     */
    @GetMapping("/v1/ws/challenge")
    public ResponseEntity<Map<String, Object>> getChallenge(
            @RequestParam("app_key") String appKey,
            @RequestHeader(value = "X-CJT-PreAuth", required = false) String preAuth) {

        if (preAuth == null) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).build();
        }

        // 1. 代理 Core 进行预校验 (本地开发且未配置 auth.id 时跳过)
        if (preAuth != null && !"none".equals(preAuth)) {
            if (!authService.verifyPreAuth(appKey, preAuth)) {
                log.warn("PreAuth failed for AppKey: {}", appKey);
                return ResponseEntity.status(HttpStatus.FORBIDDEN).build();
            }
        }

        // 2. 生成并存储 Nonce
        String nonce = nonceStore.createNonce(appKey);

        return ResponseEntity.ok(Map.of(
                "code", "GW-0000",
                "data", Map.of(
                        "nonce", nonce,
                        "expires_in", 30
                )
        ));
    }
}
