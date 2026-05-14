package com.chanjet.connector.server.config;

import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.config.ConnectorProperties;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.infra.core.DefaultInternalHttpClient;
import com.chanjet.connector.infra.core.RemoteCjtCoreAdapter;
import com.chanjet.connector.infra.core.RestP2PClient;
import com.chanjet.connector.infra.redis.RedisFailStore;
import com.chanjet.connector.infra.redis.RedisNonceStore;
import com.chanjet.connector.infra.redis.RedisRouteStore;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.autoconfigure.condition.ConditionalOnMissingBean;
import org.springframework.cloud.client.loadbalancer.LoadBalanced;
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

    // 负载均衡 RestClient (用于调用 Auth / Sub 服务)
    @Bean
    @LoadBalanced
    public RestClient.Builder loadBalancedRestClientBuilder() {
        return RestClient.builder();
    }

    // 原生直连 RestClient (用于 P2P 节点间通信)
    @Bean
    @Primary
    public RestClient.Builder directRestClientBuilder() {
        return RestClient.builder();
    }

    @Bean
    @ConditionalOnMissingBean(IInternalHttpClient.class)
    public IInternalHttpClient internalHttpClient(@LoadBalanced RestClient.Builder loadBalancedBuilder) {
        return new DefaultInternalHttpClient(loadBalancedBuilder.build());
    }

    @Bean
    public RemoteCjtCoreAdapter remoteCjtCoreAdapter(
            IInternalHttpClient httpClient,
            @Value("${services.auth.id}") String authServiceId,
            @Value("${services.subscription.id}") String subServiceId) {
        return new RemoteCjtCoreAdapter(httpClient, authServiceId, subServiceId);
    }

    @Bean
    public IP2PClient p2pClient(ConnectorProperties properties) {
        // 使用直连的 RestClient 避免 Spring Cloud LoadBalancer 拦截 IP 地址
        return new RestP2PClient(new DefaultInternalHttpClient(RestClient.builder().build()), properties);
    }
}
