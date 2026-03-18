package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Collections;
import java.util.Map;
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

    @BeforeEach
    void setUp() {
        dispatcher = new MessageDispatcher(THIS_NODE, routeStore, connectionManager, p2pClient);
    }

    @Test
    void shouldPushToLocalSessionWhenRouteMatchesCurrentNode() {
        // Arrange
        String appKey = "test-app";
        String clientId = "client-123";
        EventFrame frame = new EventFrame("msg-1", "t-1", appKey, Map.of(), "payload", System.currentTimeMillis());

        when(routeStore.getNodes(appKey)).thenReturn(Set.of(THIS_NODE + ":" + clientId));

        // Act
        dispatcher.dispatch(frame);

        // Assert
        verify(connectionManager).push(eq(clientId), eq(frame));
        verify(p2pClient, never()).forward(any(), any());
    }

    @Test
    void shouldForwardToRemoteNodeWhenRouteIsOtherNode() {
        // Arrange
        String appKey = "test-app";
        String remoteNode = "192.168.1.100:8080";
        String clientId = "client-456";
        EventFrame frame = new EventFrame("msg-2", "t-2", appKey, Map.of(), "payload", System.currentTimeMillis());

        when(routeStore.getNodes(appKey)).thenReturn(Set.of(remoteNode + ":" + clientId));

        // Act
        dispatcher.dispatch(frame);

        // Assert
        verify(p2pClient).forward(eq(remoteNode), eq(frame));
        verify(connectionManager, never()).push(any(), any());
    }

    @Test
    void shouldDoNothingWhenNoRoutesFound() {
        // Arrange
        String appKey = "test-app";
        EventFrame frame = new EventFrame("msg-3", "t-3", appKey, Map.of(), "payload", System.currentTimeMillis());

        when(routeStore.getNodes(appKey)).thenReturn(Collections.emptySet());

        // Act
        dispatcher.dispatch(frame);

        // Assert
        verify(connectionManager, never()).push(any(), any());
        verify(p2pClient, never()).forward(any(), any());
    }
}
