package com.chanjet.connector.infra.redis;

import com.chanjet.connector.api.store.IRouteStore;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.Set;
import java.util.concurrent.TimeUnit;

/**
 * 基于 Redis 的路由存储实现。
 */
public class RedisRouteStore implements IRouteStore {

    private static final String KEY_PREFIX = "cjt:gw:route:";
    private final StringRedisTemplate redisTemplate;

    public RedisRouteStore(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
    }

    @Override
    public void add(String appKey, String nodeId, String clientId) {
        String key = KEY_PREFIX + appKey;
        String value = nodeId + ":" + clientId;
        redisTemplate.opsForSet().add(key, value);
        redisTemplate.expire(key, 60, TimeUnit.SECONDS);
    }

    @Override
    public Set<String> getNodes(String appKey) {
        String key = KEY_PREFIX + appKey;
        return redisTemplate.opsForSet().members(key);
    }

    @Override
    public void remove(String appKey, String nodeId, String clientId) {
        String key = KEY_PREFIX + appKey;
        String value = nodeId + ":" + clientId;
        redisTemplate.opsForSet().remove(key, value);
    }
}
