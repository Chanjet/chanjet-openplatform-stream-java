package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.PushStatus;
import com.chanjet.connector.core.state.ToleranceManager;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Collections;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class MessageDispatcherTest {

    private static final String THIS_NODE = "127.0.0.1:8080";
    private MessageDispatcher dispatcher;

    @Mock
    private IRouteStore routeStore;

    @Mock
    private IConnectionManager connectionManager;

    @Mock
    private IP2PClient p2pClient;

    @Mock
    private ILoadBalancer loadBalancer;

    @Mock
    private ToleranceManager toleranceManager;

    @BeforeEach
    void setUp() {
        dispatcher = new MessageDispatcher(THIS_NODE, routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager);
    }

    @Test
    void shouldPushToLocalSessionWhenLoadBalancerSelectsLocalRoute() {
        String appKey = "test-app";
        String clientId = "client-local";
        String localRoute = THIS_NODE + ":" + clientId;
        EventFrame frame = new EventFrame("msg-1", "t-1", appKey, Map.of(), "payload", System.currentTimeMillis());

        when(routeStore.getNodes(appKey)).thenReturn(Set.of(localRoute));
        when(loadBalancer.select(any())).thenReturn(Optional.of(localRoute));

        dispatcher.dispatch(frame);

        verify(connectionManager).push(eq(clientId), eq(frame));
    }

    @Test
    void shouldInvokeToleranceManagerWhenNoRoutesFound() {
        // Arrange
        String appKey = "offline-app";
        EventFrame frame = new EventFrame("msg-3", "t-3", appKey, Map.of(), "payload", 1000L);

        when(routeStore.getNodes(appKey)).thenReturn(Collections.emptySet());
        // 模拟状态机返回等待
        when(toleranceManager.handleFailure(eq(appKey), anyLong())).thenReturn(PushStatus.WAITING);

        // Act & Assert
        // 预期分发器会调用状态机并根据状态决定后续行为
        dispatcher.dispatch(frame);

        verify(toleranceManager).handleFailure(eq(appKey), anyLong());
    }
}
