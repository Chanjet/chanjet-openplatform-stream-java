package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.api.config.ConnectorProperties;
import org.springframework.http.MediaType;
import org.springframework.web.client.RestClient;

/**
 * 基于 REST 的 P2P 转发客户端实现。
 */
public class RestP2PClient implements IP2PClient {

    private final RestClient restClient;
    private final ConnectorProperties properties;

    public RestP2PClient(RestClient restClient, ConnectorProperties properties) {
        this.restClient = restClient;
        this.properties = properties;
    }

    @Override
    public void forward(String targetNodeId, EventFrame frame) {
        String url = String.format("http://%s/internal/v1/p2p/push", targetNodeId);

        restClient.post()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON)
                .header("X-Internal-Token", properties.getPrimaryToken()) // 使用主令牌
                .body(frame)
                .retrieve()
                .toBodilessEntity();
    }
}
