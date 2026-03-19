package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.core.state.PushStatus;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Collections;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class MessageDispatcherTest {

    private static final String THIS_NODE = "127.0.0.1:8080";
    private MessageDispatcher dispatcher;

    @Mock private IRouteStore routeStore;
    @Mock private IConnectionManager connectionManager;
    @Mock private IP2PClient p2pClient;
    @Mock private ILoadBalancer loadBalancer;
    @Mock private ToleranceManager toleranceManager;
    @Mock private IResilienceManager resilienceManager;

    @BeforeEach
    void setUp() {
        dispatcher = new MessageDispatcher(THIS_NODE, routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
    }

    @Test
    void shouldPushToLocalSessionWhenLoadBalancerSelectsLocalRoute() {
        String appKey = "test-app";
        String clientId = "client-local";
        String localRoute = THIS_NODE + ":" + clientId;
        EventFrame frame = createFrame(appKey);

        when(resilienceManager.tryAcquire(appKey)).thenReturn(AcquisitionResult.ALLOWED);
        when(routeStore.getNodes(appKey)).thenReturn(Set.of(localRoute));
        when(loadBalancer.select(any())).thenReturn(Optional.of(localRoute));
        when(connectionManager.push(eq(clientId), any())).thenReturn(true);

        dispatcher.dispatch(frame);

        verify(connectionManager).push(eq(clientId), any());
    }

    @Test
    void shouldInvokeToleranceManagerWhenNoRoutesFound() {
        String appKey = "offline-app";
        EventFrame frame = createFrame(appKey);

        when(resilienceManager.tryAcquire(appKey)).thenReturn(AcquisitionResult.ALLOWED);
        when(routeStore.getNodes(appKey)).thenReturn(Collections.emptySet());
        when(toleranceManager.handleFailure(eq(appKey), anyLong())).thenReturn(PushStatus.WAITING);

        try { dispatcher.dispatch(frame); } catch (Exception ignored) {}

        verify(toleranceManager).handleFailure(eq(appKey), anyLong());
    }

    private EventFrame createFrame(String appKey) {
        return new EventFrame("event", "msg-1", "t-1", appKey, Collections.emptyMap(), "payload", 1000L);
    }
}
