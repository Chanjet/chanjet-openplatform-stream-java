package com.chanjet.connector.server.config;

import com.chanjet.connector.api.config.ConnectorProperties;
import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.infra.core.RemoteCjtCoreAdapter;
import com.chanjet.connector.infra.core.RestP2PClient;
import com.chanjet.connector.infra.redis.RedisFailStore;
import com.chanjet.connector.infra.redis.RedisNonceStore;
import com.chanjet.connector.infra.redis.RedisRouteStore;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.context.annotation.Primary;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.web.client.RestClient;

/**
 * 基础设施 Bean 装配配置。
 */
@Configuration
public class InfraConfig {

    @Bean
    public IRouteStore routeStore(StringRedisTemplate redisTemplate) {
        return new RedisRouteStore(redisTemplate);
    }

    @Bean
    public INonceStore nonceStore(StringRedisTemplate redisTemplate) {
        return new RedisNonceStore(redisTemplate);
    }

    @Bean
    public IFailStore failStore(StringRedisTemplate redisTemplate) {
        return new RedisFailStore(redisTemplate);
    }

    /**
     * 用于微服务调用的负载均衡 Builder。
     */
    @Bean
    @org.springframework.cloud.client.loadbalancer.LoadBalanced
    @Qualifier("loadBalancedRestClientBuilder")
    public RestClient.Builder loadBalancedRestClientBuilder() {
        return RestClient.builder();
    }

    /**
     * 用于 P2P 直连调用的普通 Builder。
     */
    @Bean
    @Qualifier("directRestClientBuilder")
    public RestClient.Builder directRestClientBuilder() {
        return RestClient.builder();
    }

    @Bean
    @Primary
    public RemoteCjtCoreAdapter remoteCjtCoreAdapter(
            @Qualifier("loadBalancedRestClientBuilder") RestClient.Builder restClientBuilder,
            @Value("${services.auth.id}") String authServiceId,
            @Value("${services.subscription.id}") String subServiceId) {
        return new RemoteCjtCoreAdapter(restClientBuilder.build(), authServiceId, subServiceId);
    }

    @Bean
    public IP2PClient p2pClient(ConnectorProperties properties) {
        // 彻底隔离：手动创建 Builder 以避开 Spring Cloud 的全局拦截器
        RestClient directClient = RestClient.builder().build();
        return new RestP2PClient(directClient, properties);
    }
}
