package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

class MessageDispatcherTest {

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
                "node-1",
                routeStore,
                connectionManager,
                p2pClient,
                loadBalancer,
                toleranceManager,
                resilienceManager
        );
    }

    @Test
    void shouldPushLocallyWhenClientIsPresentOnCurrentNode() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        // 模拟本地存在连接
        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(List.of("client-local"));

        dispatcher.dispatch(frame);

        // 验证：直接进行本地推送，不查询 Redis
        verify(connectionManager).push(eq("client-local"), any());
        verify(routeStore, never()).getNodes(anyString());
    }

    @Test
    void shouldForwardToRemoteNodeWhenLocalIsMissing() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        // 1. 本地无连接
        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());
        
        // 2. Redis 中有远程路由
        when(routeStore.getNodes(appKey)).thenReturn(Set.of("node-2:c2"));
        when(loadBalancer.select(anySet())).thenReturn(Optional.of("node-2:c2"));
        when(p2pClient.forward(anyString(), any())).thenReturn(true);

        dispatcher.dispatch(frame);

        // 验证：发起了 P2P 转发
        verify(p2pClient).forward(eq("node-2"), any());
    }
}
