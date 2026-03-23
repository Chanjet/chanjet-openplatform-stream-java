package com.chanjet.connector.core.resilience;

import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.common.protocol.AcquisitionResult;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * 内存级防护管理器实现。
 */
public class InMemResilienceManager implements IResilienceManager {

    private final int nodeLimit;
    private final int tenantLimit;

    private final AtomicInteger nodeCounter = new AtomicInteger(0);
    private final Map<String, AtomicInteger> tenantCounters = new ConcurrentHashMap<>();

    public InMemResilienceManager(int nodeLimit, int tenantLimit) {
        this.nodeLimit = nodeLimit;
        this.tenantLimit = tenantLimit;
    }

    @Override
    public AcquisitionResult tryAcquire(String appKey) {
        // 1. 节点级背压检查
        if (nodeCounter.get() >= nodeLimit) {
            return AcquisitionResult.NODE_OVERLOAD;
        }

        // 2. 租户级限流检查
        AtomicInteger tenantCounter = tenantCounters.computeIfAbsent(appKey, k -> new AtomicInteger(0));
        if (tenantCounter.get() >= tenantLimit) {
            return AcquisitionResult.TENANT_LIMITED;
        }

        // 3. 执行原子递增 (乐观锁定思路，简化实现)
        nodeCounter.incrementAndGet();
        tenantCounter.incrementAndGet();

        return AcquisitionResult.ALLOWED;
    }

    @Override
    public void release(String appKey, boolean success) {
        nodeCounter.decrementAndGet();
        AtomicInteger tenantCounter = tenantCounters.get(appKey);
        if (tenantCounter != null) {
            tenantCounter.decrementAndGet();
        }
        // TODO: 之后在 Task 3.3 增强熔断逻辑时利用 success 参数
    }
}
