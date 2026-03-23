package com.chanjet.connector.server;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.ApplicationContext;
import org.springframework.data.redis.core.StringRedisTemplate;

import static org.assertj.core.api.Assertions.assertThat;

@SpringBootTest(properties = {
    "spring.autoconfigure.exclude=org.springframework.cloud.client.loadbalancer.LoadBalancerAutoConfiguration,org.springframework.boot.autoconfigure.web.client.RestClientAutoConfiguration"
})
class SystemIntegrationTest {

    @Autowired
    private ApplicationContext context;

    @MockBean private StringRedisTemplate redisTemplate;

    @Test
    void shouldLoadAllRequiredBeans() {
        // 验证 SPI 实现类是否已成功装配
        assertThat(context.getBean(IRouteStore.class)).isNotNull();
        assertThat(context.getBean(IAuthService.class)).isNotNull();
        assertThat(context.getBean(IConnectionManager.class)).isNotNull();
        
        // 验证核心逻辑类
        assertThat(context.getBean(MessageDispatcher.class)).isNotNull();
    }
}
