package com.chanjet.connector.api.config;

import java.util.List;

/**
 * 网关核心配置项，支持动态刷新。
 */
public class ConnectorProperties {
    /** 内部 P2P 通讯令牌列表，支持滚动更新 */
    private List<String> internalTokens;
    /** 当前节点物理标识 */
    private String nodeId;

    public ConnectorProperties() {}

    public ConnectorProperties(List<String> internalTokens, String nodeId) {
        this.internalTokens = internalTokens;
        this.nodeId = nodeId;
    }

    public List<String> getInternalTokens() {
        return internalTokens;
    }

    public void setInternalTokens(List<String> internalTokens) {
        this.internalTokens = internalTokens;
    }

    public String getNodeId() {
        return nodeId;
    }

    public void setNodeId(String nodeId) {
        this.nodeId = nodeId;
    }

    /** 获取发送端使用的主令牌（列表首位） */
    public String getPrimaryToken() {
        if (internalTokens == null || internalTokens.isEmpty()) {
            return "";
        }
        return internalTokens.get(0);
    }

    /** 校验请求令牌是否合法 */
    public boolean isValidToken(String token) {
        if (internalTokens == null || token == null) {
            return false;
        }
        return internalTokens.contains(token);
    }
}
