package com.chanjet.connector.api.store;

import java.util.Collection;
import java.util.Optional;

/**
 * 负载均衡策略契约。
 */
public interface ILoadBalancer {
    /**
     * 从一组路由候选项中选择一个目标。
     * @param routes 路由字符串集合 (format: node_id:client_id)
     * @return 选中的路由记录
     */
    Optional<String> select(Collection<String> routes);
}
