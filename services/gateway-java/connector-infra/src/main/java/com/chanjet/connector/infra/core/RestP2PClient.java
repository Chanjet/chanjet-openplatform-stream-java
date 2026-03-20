package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.api.config.ConnectorProperties;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import java.util.Map;

/**
 * 基于 REST 的 P2P 转发客户端实现。
 */
public class RestP2PClient implements IP2PClient {

    private static final Logger log = LoggerFactory.getLogger(RestP2PClient.class);
    private final IInternalHttpClient httpClient;
    private final ConnectorProperties properties;

    public RestP2PClient(IInternalHttpClient httpClient, ConnectorProperties properties) {
        this.httpClient = httpClient;
        this.properties = properties;
    }

    @Override
    public boolean forward(String targetNodeId, EventFrame frame) {
        String host = targetNodeId;
        if (!host.startsWith("http")) {
            host = "http://" + host;
        }
        String url = host + "/internal/v1/p2p/push";
        String hopCountStr = frame.headers().getOrDefault("X-GW-Hop-Count", "0");
        int hopCount = Integer.parseInt(hopCountStr);

        log.info("Initiating P2P Forward: [{}] -> [{}] (Hop: {})", frame.msgId(), url, hopCount);

        try {
            httpClient.post(url, frame, Void.class, Map.of(
                    "X-Internal-Token", properties.getPrimaryToken(),
                    "X-GW-Hop-Count", String.valueOf(hopCount + 1)
            ));
            return true;
        } catch (Exception e) {
            log.warn("P2P Forward failed to {}: {}", targetNodeId, e.getMessage());
            return false;
        }
    }
}
