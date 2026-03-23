package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.push.IPushControl;
import java.util.Map;

/**
 * 远程畅捷通 Core 服务适配器。
 */
public class RemoteCjtCoreAdapter implements IAuthService, IPushControl {

    private final IInternalHttpClient httpClient;
    private final String authServiceId;
    private final String subServiceId;

    public RemoteCjtCoreAdapter(IInternalHttpClient httpClient, String authServiceId, String subServiceId) {
        this.httpClient = httpClient;
        this.authServiceId = authServiceId;
        this.subServiceId = subServiceId;
    }

    @Override
    public boolean verifySign(String appKey, String nonce, String sign) {
        if (authServiceId == null || authServiceId.isEmpty()) return true;
        String url = "http://" + authServiceId + "/internal/v1/auth/verify-sign";
        AuthResponse response = httpClient.post(url, Map.of("app_key", appKey, "nonce", nonce, "sign", sign), 
                AuthResponse.class, Map.of());
        return response != null && response.valid();
    }

    @Override
    public boolean verifyPreAuth(String appKey, String prefix) {
        if (authServiceId == null || authServiceId.isEmpty()) return true;
        String url = "http://" + authServiceId + "/internal/v1/auth/verify-preauth";
        AuthResponse response = httpClient.post(url, Map.of("app_key", appKey, "pre_auth_prefix", prefix), 
                AuthResponse.class, Map.of());
        return response != null && response.valid();
    }

    @Override
    public void setPushEnabled(String appKey, boolean enabled) {
        if (subServiceId == null || subServiceId.isEmpty()) return;
        String url = "http://" + subServiceId + "/internal/v1/subscriptions/" + appKey + "/push-status";
        httpClient.patch(url, Map.of("enabled", enabled), Map.of());
    }

    public record AuthResponse(boolean valid) {}
}
