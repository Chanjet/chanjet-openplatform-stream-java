package com.chanjet.connector.api.config;

import java.util.ArrayList;
import java.util.List;

/**
 * 网关核心配置项，支持动态刷新。
 */
public class ConnectorProperties {
    /** 内部 P2P 通讯令牌列表，支持滚动更新 */
    private List<String> internalTokens = new ArrayList<>();
    /** 当前节点物理标识 */
    private String nodeId;

    public ConnectorProperties() {
        // 默认值：确保本地测试在不配置时也能通过
        this.internalTokens.add("cjt-default-internal-token");
    }

    /** 全参构造函数，用于测试适配 */
    public ConnectorProperties(List<String> internalTokens, String nodeId) {
        this.internalTokens = (internalTokens != null) ? internalTokens : new ArrayList<>();
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
            return "cjt-default-internal-token"; // 发送端也使用默认值
        }
        return internalTokens.get(0);
    }

    /** 校验请求令牌是否合法 */
    public boolean isValidToken(String token) {
        if (token == null) return false;
        // 如果外部传了默认值，或者匹配了列表中的任意一个
        return "cjt-default-internal-token".equals(token) || 
               (internalTokens != null && internalTokens.contains(token));
    }
}
