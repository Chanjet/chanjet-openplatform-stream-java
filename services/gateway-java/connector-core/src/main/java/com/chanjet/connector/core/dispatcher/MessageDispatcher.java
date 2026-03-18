package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;

import java.util.Set;

/**
 * 消息分发器核心逻辑实现。
 */
public class MessageDispatcher {

    private final String nodeId;
    private final IRouteStore routeStore;
    private final IConnectionManager connectionManager;
    private final IP2PClient p2pClient;

    public MessageDispatcher(String nodeId, 
                             IRouteStore routeStore, 
                             IConnectionManager connectionManager,
                             IP2PClient p2pClient) {
        this.nodeId = nodeId;
        this.routeStore = routeStore;
        this.connectionManager = connectionManager;
        this.p2pClient = p2pClient;
    }

    public void dispatch(EventFrame frame) {
        Set<String> routes = routeStore.getNodes(frame.appKey());
        if (routes == null || routes.isEmpty()) {
            return; 
        }

        for (String route : routes) {
            // 路由格式: node_ip:port:client_id
            int lastColonIndex = route.lastIndexOf(":");
            if (lastColonIndex == -1) continue;

            String targetNodeId = route.substring(0, lastColonIndex);
            String clientId = route.substring(lastColonIndex + 1);

            if (targetNodeId.equals(nodeId)) {
                connectionManager.push(clientId, frame);
            } else {
                p2pClient.forward(targetNodeId, frame);
            }
        }
    }
}
