package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.common.protocol.EventFrame;
import org.springframework.http.MediaType;
import org.springframework.web.client.RestClient;

/**
 * 基于 REST 的 P2P 转发客户端实现。
 */
public class RestP2PClient implements IP2PClient {

    private final RestClient restClient;

    public RestP2PClient(RestClient restClient) {
        this.restClient = restClient;
    }

    @Override
    public void forward(String targetNodeId, EventFrame frame) {
        // targetNodeId 格式为 ip:port
        String url = String.format("http://%s/internal/v1/p2p/push", targetNodeId);

        restClient.post()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON)
                .body(frame)
                .retrieve()
                .toBodilessEntity();
    }
}
