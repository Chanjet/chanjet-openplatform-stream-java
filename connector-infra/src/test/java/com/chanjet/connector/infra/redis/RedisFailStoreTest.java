package com.chanjet.connector.infra.redis;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.Set;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;

@Timeout(value = 2, unit = TimeUnit.MINUTES)
class RedisFailStoreTest extends BaseRedisIntegrationTest {

    @Autowired
    private StringRedisTemplate redisTemplate;

    private RedisFailStore failStore;

    @BeforeEach
    void setUp() {
        failStore = new RedisFailStore(redisTemplate);
        Set<String> keys = redisTemplate.keys("cjt:gw:fail_start:*");
        if (keys != null) redisTemplate.delete(keys);
    }

    @Test
    void shouldGetOrSetFailStartTime() {
        String appKey = "test-app";
        long firstNow = 1000L;
        long secondNow = 2000L;

        // 第一次设置，应返回传入的 firstNow
        assertThat(failStore.getOrSet(appKey, firstNow)).isEqualTo(firstNow);
        
        // 第二次获取，虽然传入 secondNow，但应返回已存储的 firstNow
        assertThat(failStore.getOrSet(appKey, secondNow)).isEqualTo(firstNow);
    }

    @Test
    void shouldClearFailStartTime() {
        String appKey = "test-app";
        failStore.getOrSet(appKey, 1000L);
        failStore.clear(appKey);

        assertThat(failStore.get(appKey)).isEmpty();
    }
}
