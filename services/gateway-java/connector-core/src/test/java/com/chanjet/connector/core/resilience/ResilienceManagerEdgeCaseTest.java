package com.chanjet.connector.core.resilience;

import com.chanjet.connector.common.protocol.AcquisitionResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class ResilienceManagerEdgeCaseTest {

    @Test
    void shouldDenyAcquisitionWhenConcurrentLimitReached() {
        // 设置极低的并发限流：1
        InMemResilienceManager manager = new InMemResilienceManager(1000, 1);
        String appKey = "limited-app";

        // 第一次：成功
        assertEquals(AcquisitionResult.ALLOWED, manager.tryAcquire(appKey));
        
        // 第二次：拒绝
        assertEquals(AcquisitionResult.TENANT_LIMITED, manager.tryAcquire(appKey));

        // 释放后再次尝试：成功
        manager.release(appKey, true);
        assertEquals(AcquisitionResult.ALLOWED, manager.tryAcquire(appKey));
    }
}
