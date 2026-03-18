package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.push.IPushControl;
import org.springframework.http.MediaType;
import org.springframework.web.client.RestClient;

import java.util.Map;

/**
 * 畅捷通 Core 微服务适配器实现。
 */
public class RemoteCjtCoreAdapter implements IAuthService, IPushControl {

    private final RestClient restClient;
    private final String authServiceId;
    private final String subServiceId;

    public RemoteCjtCoreAdapter(RestClient restClient, String authServiceId, String subServiceId) {
        this.restClient = restClient;
        this.authServiceId = authServiceId;
        this.subServiceId = subServiceId;
    }

    @Override
    public boolean verifySign(String appKey, String nonce, String sign) {
        String host = (authServiceId == null || authServiceId.isEmpty()) ? "" : "http://" + authServiceId;
        String url = host + "/internal/v1/auth/verify-sign";
        
        AuthResponse response = restClient.post()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON)
                .body(Map.of(
                        "app_key", appKey,
                        "nonce", nonce,
                        "sign", sign
                ))
                .retrieve()
                .body(AuthResponse.class);

        return response != null && response.valid();
    }

    @Override
    public boolean verifyPreAuth(String appKey, String prefix) {
        return true;
    }

    @Override
    public void setPushEnabled(String appKey, boolean enabled) {
        String host = (subServiceId == null || subServiceId.isEmpty()) ? "" : "http://" + subServiceId;
        String url = host + "/internal/v1/subscriptions/" + appKey + "/push-status";
        
        restClient.patch()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON)
                .body(Map.of("enabled", enabled))
                .retrieve()
                .toBodilessEntity();
    }

    public record AuthResponse(boolean valid) {}
}
