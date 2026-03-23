package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.store.INonceStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.server.ServerHttpRequest;
import org.springframework.http.server.ServerHttpResponse;
import org.springframework.stereotype.Component;
import org.springframework.web.socket.WebSocketHandler;
import org.springframework.web.socket.server.HandshakeInterceptor;
import org.springframework.web.util.UriComponentsBuilder;

import java.util.Map;

/**
 * WebSocket 握手安全拦截器，执行基于 Nonce 的签名验证。
 */
@Component
public class AuthHandshakeInterceptor implements HandshakeInterceptor {

    private static final Logger log = LoggerFactory.getLogger(AuthHandshakeInterceptor.class);
    private final INonceStore nonceStore;
    private final IAuthService authService;

    public AuthHandshakeInterceptor(INonceStore nonceStore, IAuthService authService) {
        this.nonceStore = nonceStore;
        this.authService = authService;
    }

    @Override
    public boolean beforeHandshake(ServerHttpRequest request, ServerHttpResponse response,
                                   WebSocketHandler wsHandler, Map<String, Object> attributes) {
        
        Map<String, String> params = UriComponentsBuilder.fromUri(request.getURI())
                .build().getQueryParams().toSingleValueMap();

        String appKey = params.get("app_key");
        String nonce = params.get("nonce");
        String sign = params.get("sign");

        if (appKey == null || nonce == null || sign == null) {
            log.warn("Missing handshake parameters for AppKey: {}", appKey);
            return false;
        }

        // 1. 验证并核销 Nonce (单次有效)
        if (!nonceStore.verifyAndConsume(nonce, appKey)) {
            log.warn("Invalid or expired nonce: {} for AppKey: {}", nonce, appKey);
            return false;
        }

        // 2. 在线验证签名 (代理给 Core 服务)
        if (!authService.verifySign(appKey, nonce, sign)) {
            log.warn("Signature verification failed for AppKey: {}", appKey);
            return false;
        }

        // 3. 存储关键信息至 Session Attributes
        attributes.put("appKey", appKey);
        attributes.put("clientId", params.getOrDefault("client_id", appKey + "@local"));

        log.info("Handshake pre-check success for AppKey: {}. Proceeding to upgrade.", appKey);
        return true;
    }

    @Override
    public void afterHandshake(ServerHttpRequest request, ServerHttpResponse response,
                               WebSocketHandler wsHandler, Exception exception) {
    }
}
