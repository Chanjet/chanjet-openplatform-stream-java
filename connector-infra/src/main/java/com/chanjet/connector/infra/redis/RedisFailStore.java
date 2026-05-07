package com.chanjet.connector.infra.redis;

import com.chanjet.connector.api.store.IFailStore;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.Optional;
import java.util.concurrent.TimeUnit;

/**
 * 基于 Redis 的失败计时存储实现。
 */
public class RedisFailStore implements IFailStore {

    private static final String KEY_PREFIX = "cjt:gw:fail_start:";
    private final StringRedisTemplate redisTemplate;

    public RedisFailStore(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
    }

    @Override
    public long getOrSet(String appKey, long now) {
        String key = KEY_PREFIX + appKey;
        // 使用 SET IF NOT EXISTS (NX) 指令
        Boolean setOk = redisTemplate.opsForValue().setIfAbsent(key, String.valueOf(now), 1, TimeUnit.HOURS);
        if (setOk != null && setOk) {
            return now;
        }
        // 已经存在，获取原有的时间戳
        String storedValue = redisTemplate.opsForValue().get(key);
        return storedValue != null ? Long.parseLong(storedValue) : now;
    }

    @Override
    public boolean clear(String appKey) {
        String key = KEY_PREFIX + appKey;
        Boolean deleted = redisTemplate.delete(key);
        return deleted != null && deleted;
    }

    @Override
    public Optional<Long> get(String appKey) {
        String key = KEY_PREFIX + appKey;
        String storedValue = redisTemplate.opsForValue().get(key);
        return Optional.ofNullable(storedValue).map(Long::parseLong);
    }
}
