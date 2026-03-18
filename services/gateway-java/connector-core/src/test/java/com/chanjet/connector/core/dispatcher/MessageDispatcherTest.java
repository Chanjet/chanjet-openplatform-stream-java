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
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Map;
import java.util.Optional;
import java.util.Set;

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
    @Mock
    private IResilienceManager resilienceManager;

    @BeforeEach
    void setUp() {
        dispatcher = new MessageDispatcher(THIS_NODE, routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
    }

    @Test
    void shouldDenyDispatchWhenResilienceManagerReturnsNodeOverload() {
        // Arrange
        String appKey = "test-app";
        EventFrame frame = new EventFrame("msg-limit", "t-limit", appKey, Map.of(), "payload", 1000L);
        
        when(resilienceManager.tryAcquire(appKey)).thenReturn(AcquisitionResult.NODE_OVERLOAD);

        // Act
        dispatcher.dispatch(frame);

        // Assert: 预期不执行路由查询和后续推送
        verify(routeStore, never()).getNodes(anyString());
        verify(connectionManager, never()).push(any(), any());
    }

    @Test
    void shouldReleaseResiliencePermitAfterSuccessfulDispatch() {
        // Arrange
        String appKey = "test-app";
        String localRoute = THIS_NODE + ":client-1";
        EventFrame frame = new EventFrame("msg-ok", "t-ok", appKey, Map.of(), "payload", 1000L);

        when(resilienceManager.tryAcquire(appKey)).thenReturn(AcquisitionResult.ALLOWED);
        when(routeStore.getNodes(appKey)).thenReturn(Set.of(localRoute));
        when(loadBalancer.select(any())).thenReturn(Optional.of(localRoute));
        when(connectionManager.push(eq("client-1"), eq(frame))).thenReturn(true);

        // Act
        dispatcher.dispatch(frame);

        // Assert
        verify(resilienceManager).release(eq(appKey), eq(true));
    }
}
