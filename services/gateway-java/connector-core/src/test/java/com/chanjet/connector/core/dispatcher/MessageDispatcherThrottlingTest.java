package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

class MessageDispatcherThrottlingTest {

    @Test
    void shouldAbortDispatchWhenThrottled() {
        // 这一用例专门用于覆盖 dispatch 方法中的限流分支
        var routeStore = mock(com.chanjet.connector.api.store.IRouteStore.class);
        var connectionManager = mock(com.chanjet.connector.api.connection.IConnectionManager.class);
        var p2pClient = mock(com.chanjet.connector.api.connection.IP2PClient.class);
        var loadBalancer = mock(com.chanjet.connector.api.store.ILoadBalancer.class);
        var toleranceManager = mock(com.chanjet.connector.core.state.ToleranceManager.class);
        var resilienceManager = mock(com.chanjet.connector.api.resilience.IResilienceManager.class);

        var dispatcher = new MessageDispatcher("n1", routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
        
        EventFrame frame = new EventFrame("event", "m1", "t1", "app", null, Map.of(), "data", 1000L);

        // 模拟限流拒绝
        when(resilienceManager.tryAcquire(anyString())).thenReturn(AcquisitionResult.TENANT_LIMITED);

        dispatcher.dispatch(frame);

        // 验证：直接退出，不执行后续逻辑
        verify(connectionManager, never()).getClientsByAppKey(anyString());
        verify(resilienceManager, never()).release(anyString(), anyBoolean());
    }
}
