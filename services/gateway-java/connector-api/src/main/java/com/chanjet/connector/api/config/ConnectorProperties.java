package com.chanjet.connector.api.config;

import org.springframework.boot.context.properties.ConfigurationProperties;
import java.util.List;

/**
 * 网关核心配置项。
 */
@ConfigurationProperties(prefix = "connector")
public record ConnectorProperties(
    /** 内部 P2P 通讯令牌列表，支持滚动更新 */
    List<String> internalTokens,
    /** 当前节点物理标识 */
    String nodeId
) {
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
