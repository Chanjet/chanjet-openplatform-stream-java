package com.chanjet.connector.api.push;

/**
 * 推送状态控制契约。
 */
public interface IPushControl {
    /**
     * 动态开启或挂起特定应用的 Webhook 推送。
     * @param appKey 应用标识
     * @param enabled true 为开启，false 为挂起（进入离线积压池）
     */
    void setPushEnabled(String appKey, boolean enabled);
}
