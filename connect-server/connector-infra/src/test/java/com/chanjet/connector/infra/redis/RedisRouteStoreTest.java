package com.chanjet.connector.infra.redis;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

class RedisRouteStoreTest extends BaseRedisIntegrationTest {

    @Autowired
    private StringRedisTemplate redisTemplate;

    private RedisRouteStore routeStore;

    @BeforeEach
    void setUp() {
        routeStore = new RedisRouteStore(redisTemplate);
        // 清理测试数据
        Set<String> keys = redisTemplate.keys("cjt:gw:route:*");
        if (keys != null) redisTemplate.delete(keys);
    }

    @Test
    void shouldAddAndGetRoutes() {
        String appKey = "app1";
        String route1 = "127.0.0.1:8080:client1";
        String route2 = "127.0.0.1:8080:client2";

        routeStore.add(appKey, "127.0.0.1:8080", "client1");
        routeStore.add(appKey, "127.0.0.1:8080", "client2");

        Set<String> nodes = routeStore.getNodes(appKey);
        
        assertThat(nodes).hasSize(2).contains(route1, route2);
    }

    @Test
    void shouldRemoveRoute() {
        String appKey = "app1";
        routeStore.add(appKey, "127.0.0.1:8080", "client1");
        routeStore.remove(appKey, "127.0.0.1:8080", "client1");

        assertThat(routeStore.getNodes(appKey)).isEmpty();
    }
}
