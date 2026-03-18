package com.chanjet.connector.core.loadbalance;

import com.chanjet.connector.api.store.ILoadBalancer;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.Random;

/**
 * 随机负载均衡策略实现。
 */
public class RandomLoadBalancer implements ILoadBalancer {

    private final Random random = new Random();

    @Override
    public Optional<String> select(Collection<String> routes) {
        if (routes == null || routes.isEmpty()) {
            return Optional.empty();
        }
        List<String> list = new ArrayList<>(routes);
        return Optional.of(list.get(random.nextInt(list.size())));
    }
}
