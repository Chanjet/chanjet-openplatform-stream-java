package com.chanjet.connector.api.store;

/**
 * 挑战码（Nonce）存储契约，用于 WebSocket 握手鉴权。
 */
public interface INonceStore {
    /**
     * 为应用生成并存储一个 Nonce。
     * @param appKey 应用标识
     * @return 生成的 Nonce 字符串
     */
    String createNonce(String appKey);

    /**
     * 验证并销毁 Nonce（确保单次有效）。
     * @param nonce 待验证的 Nonce
     * @param appKey 应用标识
     * @return 验证是否通过
     */
    boolean verifyAndConsume(String nonce, String appKey);
}
