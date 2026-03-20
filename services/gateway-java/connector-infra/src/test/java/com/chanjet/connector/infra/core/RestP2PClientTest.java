package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.config.ConnectorProperties;
import com.chanjet.connector.common.protocol.EventFrame;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import java.util.Collections;
import java.util.List;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.contains;
import static org.mockito.Mockito.*;

class RestP2PClientTest {

    private IInternalHttpClient httpClient;
    private RestP2PClient p2pClient;

    @BeforeEach
    void setUp() {
        httpClient = mock(IInternalHttpClient.class);
        ConnectorProperties props = new ConnectorProperties(List.of("token-1"), "node-1");
        // 对齐生产代码的构造函数：httpClient, properties
        p2pClient = new RestP2PClient(httpClient, props);
    }

    @Test
    void shouldForwardEventFrameToRemoteNode() {
        String targetNode = "localhost:8080";
        EventFrame frame = new EventFrame("event", "msg-1", "trace-1", "app-1", "client-1", Collections.emptyMap(), "p2p-data", 1000L);

        p2pClient.forward(targetNode, frame);

        verify(httpClient).post(contains("/internal/v1/p2p/push"), eq(frame), any(), any());
    }
}
