package com.chanjet.connector.server.config;

import com.chanjet.connector.api.config.ConnectorProperties;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.net.InetAddress;
import java.net.UnknownHostException;

/**
 * 运行时节点 ID 解析器。
 */
@Component
public class NodeIdResolver {
    private static final Logger log = LoggerFactory.getLogger(NodeIdResolver.class);

    private final String resolvedNodeId;

    public NodeIdResolver(ConnectorProperties properties, @Value("${server.port:8080}") int port) {
        this.resolvedNodeId = resolve(properties.getNodeId(), port);
        log.info("Node ID auto-resolved to: [{}]", resolvedNodeId);
    }

    public String getResolvedNodeId() {
        return resolvedNodeId;
    }

    private String resolve(String configuredNodeId, int port) {
        if (configuredNodeId != null && !configuredNodeId.isEmpty()) {
            return configuredNodeId;
        }
        String ip = System.getenv("POD_IP");
        if (ip == null || ip.isEmpty()) {
            try {
                ip = InetAddress.getLocalHost().getHostAddress();
            } catch (UnknownHostException e) {
                ip = "127.0.0.1";
            }
        }
        return ip + ":" + port;
    }
}
