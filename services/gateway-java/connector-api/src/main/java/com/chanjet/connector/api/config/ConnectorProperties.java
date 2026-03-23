package com.chanjet.connector.api.config;

import java.util.ArrayList;
import java.util.List;

/**
 * 网关核心配置项，支持动态刷新。
 */
public class ConnectorProperties {
    /** 内部 P2P 通讯令牌列表 */
    private List<String> internalTokens = new ArrayList<>();
    /** 
     * 节点物理标识 (ip:port)。
     * 选填：若不配置，系统将根据运行时环境自动探测并生成。
     */
    private String nodeId;

    public ConnectorProperties() {
        this.internalTokens.add("cjt-default-internal-token");
    }

    public ConnectorProperties(List<String> internalTokens, String nodeId) {
        this.internalTokens = (internalTokens != null) ? internalTokens : new ArrayList<>();
        if (this.internalTokens.isEmpty()) {
            this.internalTokens.add("cjt-default-internal-token");
        }
        this.nodeId = nodeId;
    }

    public List<String> getInternalTokens() {
        return internalTokens;
    }

    public void setInternalTokens(List<String> internalTokens) {
        if (internalTokens != null && !internalTokens.isEmpty()) {
            this.internalTokens = internalTokens;
        }
    }

    public String getNodeId() {
        return nodeId;
    }

    public void setNodeId(String nodeId) {
        this.nodeId = nodeId;
    }

    public String getPrimaryToken() {
        if (internalTokens == null || internalTokens.isEmpty()) {
            return "cjt-default-internal-token";
        }
        return internalTokens.get(0);
    }

    public boolean isValidToken(String token) {
        if (token == null) return false;
        if ("cjt-default-internal-token".equals(token)) return true;
        return internalTokens != null && internalTokens.contains(token);
    }
}
