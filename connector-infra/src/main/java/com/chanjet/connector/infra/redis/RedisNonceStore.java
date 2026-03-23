package com.chanjet.connector.infra.redis;

import com.chanjet.connector.api.store.INonceStore;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * 基于 Redis 的挑战码存储实现。
 */
public class RedisNonceStore implements INonceStore {

    private static final String KEY_PREFIX = "cjt:gw:nonce:";
    private final StringRedisTemplate redisTemplate;

    public RedisNonceStore(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
    }

    @Override
    public String createNonce(String appKey) {
        String nonce = UUID.randomUUID().toString();
        String key = KEY_PREFIX + nonce;
        redisTemplate.opsForValue().set(key, appKey, 30, TimeUnit.SECONDS);
        return nonce;
    }

    @Override
    public boolean verifyAndConsume(String nonce, String appKey) {
        String key = KEY_PREFIX + nonce;
        String storedAppKey = redisTemplate.opsForValue().get(key);
        
        if (storedAppKey != null && storedAppKey.equals(appKey)) {
            Boolean deleted = redisTemplate.delete(key);
            return deleted != null && deleted;
        }
        return false;
    }
}
