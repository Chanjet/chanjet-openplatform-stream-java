package com.chanjet.connector.api.resilience;

import com.chanjet.connector.common.protocol.AcquisitionResult;

/**
 * 背压与熔断管理契约。
 */
public interface IResilienceManager {
    /**
     * 尝试获取执行许可。
     * @param appKey 应用标识
     * @return 获取结果
     */
    AcquisitionResult tryAcquire(String appKey);

    /**
     * 释放许可并反馈执行结果。
     * @param appKey 应用标识
     * @param success 是否执行成功（用于熔断计算）
     */
    void release(String appKey, boolean success);
}
