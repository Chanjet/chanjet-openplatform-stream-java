package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.exception.NoOnlineClientException;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.*;

import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

class MessageDispatcherEdgeCaseTest {

    private MessageDispatcher dispatcher;
    private IRouteStore routeStore;
    private IConnectionManager connectionManager;
    private IP2PClient p2pClient;
    private ILoadBalancer loadBalancer;
    private ToleranceManager toleranceManager;
    private IResilienceManager resilienceManager;

    @BeforeEach
    void setUp() {
        routeStore = mock(IRouteStore.class);
        connectionManager = mock(IConnectionManager.class);
        p2pClient = mock(IP2PClient.class);
        loadBalancer = mock(ILoadBalancer.class);
        toleranceManager = mock(ToleranceManager.class);
        resilienceManager = mock(IResilienceManager.class);

        when(resilienceManager.tryAcquire(anyString())).thenReturn(AcquisitionResult.ALLOWED);

        dispatcher = new MessageDispatcher(
                "node-1", routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager
        );
    }

    @Test
    void shouldThrowExceptionWhenNoRoutesInRedis() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());
        when(routeStore.getNodes(appKey)).thenReturn(Collections.emptySet());

        // 验证：没有任何路由时抛出 NoOnlineClientException
        assertThrows(NoOnlineClientException.class, () -> dispatcher.dispatch(frame));
    }

    @Test
    void shouldHandleMalformedRouteString() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());
        when(routeStore.getNodes(appKey)).thenReturn(Set.of("invalid-route-no-colon"));
        when(loadBalancer.select(anySet())).thenReturn(Optional.of("invalid-route-no-colon"));

        dispatcher.dispatch(frame);

        // 验证：格式错误时循环继续或退出，不抛异常
        verify(p2pClient, never()).forward(anyString(), any());
    }

    @Test
    void shouldStopForwardingIfMessageAlreadyHopped() {
        String appKey = "test-app";
        // 模拟已经跳过一次的帧
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of("X-GW-Hop-Count", "1"), "data", 1000L);

        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());

        dispatcher.dispatch(frame);

        // 验证：跳数为 1 且本地没连接时，直接放弃，不查 Redis，不继续转发
        verify(routeStore, never()).getNodes(anyString());
        verify(p2pClient, never()).forward(anyString(), any());
    }

    @Test
    void shouldHandleP2PRetryExhaustion() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());
        when(routeStore.getNodes(appKey)).thenReturn(Set.of("n1:c1", "n2:c2"));
        when(loadBalancer.select(anySet()))
                .thenReturn(Optional.of("n1:c1"))
                .thenReturn(Optional.of("n2:c2"));
        
        when(p2pClient.forward(anyString(), any())).thenReturn(false);

        dispatcher.dispatch(frame);

        verify(p2pClient, times(2)).forward(anyString(), any());
        verify(toleranceManager).handleFailure(eq(appKey), anyLong());
    }
}
