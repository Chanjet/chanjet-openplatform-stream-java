package com.chanjet.connector.core.resilience;

import com.chanjet.connector.common.protocol.AcquisitionResult;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class InMemResilienceManagerTest {

    @Test
    void shouldDenyRequestWhenNodeLimitExceeded() {
        // Arrange: 设置节点最大并发为 2
        InMemResilienceManager manager = new InMemResilienceManager(2, 100);

        // Act & Assert
        assertThat(manager.tryAcquire("app1")).isEqualTo(AcquisitionResult.ALLOWED);
        assertThat(manager.tryAcquire("app2")).isEqualTo(AcquisitionResult.ALLOWED);
        assertThat(manager.tryAcquire("app3")).isEqualTo(AcquisitionResult.NODE_OVERLOAD);

        // Release and retry
        manager.release("app1", true);
        assertThat(manager.tryAcquire("app3")).isEqualTo(AcquisitionResult.ALLOWED);
    }

    @Test
    void shouldDenyRequestWhenTenantLimitExceeded() {
        // Arrange: 设置租户最大并发为 2
        InMemResilienceManager manager = new InMemResilienceManager(5000, 2);

        // Act & Assert
        assertThat(manager.tryAcquire("app1")).isEqualTo(AcquisitionResult.ALLOWED);
        assertThat(manager.tryAcquire("app1")).isEqualTo(AcquisitionResult.ALLOWED);
        assertThat(manager.tryAcquire("app1")).isEqualTo(AcquisitionResult.TENANT_LIMITED);

        // Release and retry
        manager.release("app1", true);
        assertThat(manager.tryAcquire("app1")).isEqualTo(AcquisitionResult.ALLOWED);
    }
}
