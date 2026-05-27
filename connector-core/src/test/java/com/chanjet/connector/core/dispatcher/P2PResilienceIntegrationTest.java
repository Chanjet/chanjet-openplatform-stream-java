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
import org.mockito.Mockito;

import java.util.*;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

class P2PResilienceIntegrationTest {

    private MessageDispatcher dispatcher;
    private IRouteStore routeStore;
    private IConnectionManager connectionManager;
    private IP2PClient p2pClient;
    private ILoadBalancer loadBalancer;
    private ToleranceManager toleranceManager;
    private IResilienceManager resilienceManager;
    private AckManager ackManager;

    @BeforeEach
    void setUp() {
        routeStore = mock(IRouteStore.class);
        connectionManager = mock(IConnectionManager.class);
        p2pClient = mock(IP2PClient.class);
        loadBalancer = mock(ILoadBalancer.class);
        toleranceManager = mock(ToleranceManager.class);
        resilienceManager = mock(IResilienceManager.class);
        ackManager = mock(AckManager.class);

        when(resilienceManager.tryAcquire(anyString())).thenReturn(AcquisitionResult.ALLOWED);
        when(ackManager.registerAck(anyString(), anyLong())).thenReturn(java.util.concurrent.CompletableFuture.completedFuture(true));

        dispatcher = new MessageDispatcher(
                "self-node:8080",
                routeStore,
                connectionManager,
                p2pClient,
                loadBalancer,
                toleranceManager,
                resilienceManager,
                ackManager
        );
    }

    @Test
    void shouldRetryNextNodeWhenFirstP2PAttemptFails() {
        String appKey = "test-app";
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of(), "data", 1000L);

        // 模拟 Redis 中有两个路由
        Set<String> routes = new HashSet<>(Arrays.asList("node-failed:8080:c1", "node-success:8080:c2"));
        when(routeStore.getNodes(appKey)).thenReturn(routes);
        
        // 模拟负载均衡：第一选到 node-failed，第二次选到 node-success
        when(loadBalancer.select(anySet()))
                .thenReturn(Optional.of("node-failed:8080:c1"))
                .thenReturn(Optional.of("node-success:8080:c2"));

        // 模拟 P2P 转发结果
        when(p2pClient.forward(eq("node-failed:8080"), any())).thenReturn(false); // 第一次失败
        when(p2pClient.forward(eq("node-success:8080"), any())).thenReturn(true);  // 第二次成功

        dispatcher.dispatch(frame).join();

        // 验证：最终成功转发到了 node-success
        verify(p2pClient, times(2)).forward(anyString(), any());
        verify(p2pClient).forward(eq("node-success:8080"), any());
    }

    @Test
    void shouldPreventInfiniteLoopIfMessageAlreadyHopped() {
        String appKey = "test-app";
        // 模拟一个带有 Hop-Count=1 的帧（已经过转发）
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Map.of("X-GW-Hop-Count", "1"), "data", 1000L);

        when(connectionManager.getClientsByAppKey(appKey)).thenReturn(Collections.emptyList());

        dispatcher.dispatch(frame).join();

        // 验证：禁止再次发起 P2P 转发
        verify(p2pClient, never()).forward(anyString(), any());
        verify(routeStore, never()).getNodes(anyString());
    }
}
