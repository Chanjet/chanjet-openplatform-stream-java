package com.chanjet.connector.api.store;

import java.util.Optional;

/**
 * 失败计时存储契约，记录客户端全量离线后的首条消息到达时间。
 */
public interface IFailStore {
    /**
     * 获取失败开始时间，如果不存在则设置为当前时间。
     * @param appKey 应用标识
     * @param now 当前时间戳
     * @return 最终存储的失败开始时间
     */
    long getOrSet(String appKey, long now);

    /**
     * 清理计时器。
     */
    void clear(String appKey);

    /**
     * 获取失败开始时间。
     */
    Optional<Long> get(String appKey);
}
