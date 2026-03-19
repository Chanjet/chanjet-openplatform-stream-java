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
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Collections;
import java.util.Optional;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class MessageDispatcherAddressingTest {

    private MessageDispatcher dispatcher;
    private static final String LOCAL_NODE = "127.0.0.1:8080";

    @Mock private IRouteStore routeStore;
    @Mock private IConnectionManager connectionManager;
    @Mock private IP2PClient p2pClient;
    @Mock private ILoadBalancer loadBalancer;
    @Mock private ToleranceManager toleranceManager;
    @Mock private IResilienceManager resilienceManager;

    @BeforeEach
    void setUp() {
        dispatcher = new MessageDispatcher(LOCAL_NODE, routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
    }

    @Test
    void shouldPopulateTargetClientIdWhenForwardingToRemoteNode() {
        // Arrange
        String appKey = "app-1";
        String remoteNode = "192.168.1.100:8080";
        String targetClientId = "client-B";
        String remoteRoute = remoteNode + ":" + targetClientId;
        EventFrame frame = new EventFrame("event", "m1", "t1", appKey, null, Collections.emptyMap(), "data", 1000L);

        when(resilienceManager.tryAcquire(appKey)).thenReturn(AcquisitionResult.ALLOWED);
        when(routeStore.getNodes(appKey)).thenReturn(Set.of(remoteRoute));
        when(loadBalancer.select(any())).thenReturn(Optional.of(remoteRoute));

        // Act
        dispatcher.dispatch(frame);

        // Assert
        ArgumentCaptor<EventFrame> frameCaptor = ArgumentCaptor.forClass(EventFrame.class);
        verify(p2pClient).forward(eq(remoteNode), frameCaptor.capture());
        
        // 关键断言：转发的帧必须包含精准的目标 ClientID
        assertThat(frameCaptor.getValue().targetClientId()).isEqualTo(targetClientId);
    }
}
