package com.chanjet.connector.infra.redis;

import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.data.redis.connection.RedisConnectionFactory;
import org.springframework.data.redis.connection.lettuce.LettuceConnectionFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.testcontainers.containers.GenericContainer;
import org.testcontainers.utility.DockerImageName;

import java.time.Duration;

@SpringBootTest(classes = BaseRedisIntegrationTest.RedisIntegrationTestConfig.class)
public abstract class BaseRedisIntegrationTest {

    protected static final GenericContainer<?> REDIS;

    static {
        REDIS = new GenericContainer<>(DockerImageName.parse("redis:7.2-alpine"))
                .withExposedPorts(6379)
                .withStartupTimeout(Duration.ofSeconds(60));
        REDIS.start();
        
        // 确保容器在 JVM 退出时停止
        Runtime.getRuntime().addShutdownHook(new Thread(REDIS::stop));
    }

    @TestConfiguration
    public static class RedisIntegrationTestConfig {
        @Bean
        public RedisConnectionFactory redisConnectionFactory() {
            LettuceConnectionFactory factory = new LettuceConnectionFactory(REDIS.getHost(), REDIS.getMappedPort(6379));
            // 禁用重连尝试，避免测试结束后的异常干扰
            factory.setValidateConnection(false);
            return factory;
        }

        @Bean
        public StringRedisTemplate stringRedisTemplate(RedisConnectionFactory factory) {
            return new StringRedisTemplate(factory);
        }
    }
}
