package com.chanjet.connector.infra.core;

import com.chanjet.connector.common.protocol.EventFrame;
import com.github.tomakehurst.wiremock.junit5.WireMockRuntimeInfo;
import com.github.tomakehurst.wiremock.junit5.WireMockTest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.client.HttpComponentsClientHttpRequestFactory;
import org.springframework.web.client.RestClient;

import java.util.Collections;

import static com.github.tomakehurst.wiremock.client.WireMock.*;

@WireMockTest
class RestP2PClientTest {

    private RestP2PClient p2pClient;

    @BeforeEach
    void setUp(WireMockRuntimeInfo wmRuntimeInfo) {
        RestClient restClient = RestClient.builder()
                .requestFactory(new HttpComponentsClientHttpRequestFactory())
                .build();
        p2pClient = new RestP2PClient(restClient);
    }

    @Test
    void shouldForwardEventFrameToRemoteNode(WireMockRuntimeInfo wmRuntimeInfo) {
        String targetNode = "localhost:" + wmRuntimeInfo.getHttpPort();
        EventFrame frame = new EventFrame("event", "msg-1", "trace-1", "app-1", "client-1", Collections.emptyMap(), "p2p-data", 1000L);

        stubFor(post(urlEqualTo("/internal/v1/p2p/push"))
                .willReturn(ok()));

        p2pClient.forward(targetNode, frame);

        verify(postRequestedFor(urlEqualTo("/internal/v1/p2p/push"))
                .withRequestBody(containing("\"msg_id\":\"msg-1\""))
                .withRequestBody(containing("\"target_client_id\":\"client-1\"")));
    }
}
