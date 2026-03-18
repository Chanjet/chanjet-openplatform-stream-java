package com.chanjet.connector.api.auth;

/**
 * 在线鉴权代理契约。
 */
public interface IAuthService {
    /**
     * 代理 Core 校验 WebSocket 签名。
     */
    boolean verifySign(String appKey, String nonce, String sign);

    /**
     * 校验 PreAuth HMAC 前缀。
     */
    boolean verifyPreAuth(String appKey, String prefix);
}
