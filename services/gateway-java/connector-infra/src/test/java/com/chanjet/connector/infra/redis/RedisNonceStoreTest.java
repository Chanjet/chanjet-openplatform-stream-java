package com.chanjet.connector.infra.redis;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

class RedisNonceStoreTest extends BaseRedisIntegrationTest {

    @Autowired
    private StringRedisTemplate redisTemplate;

    private RedisNonceStore nonceStore;

    @BeforeEach
    void setUp() {
        nonceStore = new RedisNonceStore(redisTemplate);
        Set<String> keys = redisTemplate.keys("cjt:gw:nonce:*");
        if (keys != null) redisTemplate.delete(keys);
    }

    @Test
    void shouldCreateAndVerifyNonce() {
        String appKey = "test-app";
        String nonce = nonceStore.createNonce(appKey);

        assertThat(nonce).isNotBlank();
        
        // 第一次校验应通过
        assertThat(nonceStore.verifyAndConsume(nonce, appKey)).isTrue();
        
        // 第二次校验（已销毁）应失败
        assertThat(nonceStore.verifyAndConsume(nonce, appKey)).isFalse();
    }

    @Test
    void shouldFailWhenAppKeyMismatches() {
        String appKey = "app1";
        String nonce = nonceStore.createNonce(appKey);

        assertThat(nonceStore.verifyAndConsume(nonce, "wrong-app")).isFalse();
    }
}
