package com.chanjet.connector.server.config;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import com.chanjet.connector.core.loadbalance.RandomLoadBalancer;
import com.chanjet.connector.core.resilience.InMemResilienceManager;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.infra.redis.RedisRouteStore;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.data.redis.core.StringRedisTemplate;

/**
 * 核心 Bean 装配配置。
 */
@Configuration
public class AppConfig {

    @Bean
    public ILoadBalancer loadBalancer() {
        return new RandomLoadBalancer();
    }

    @Bean
    public IResilienceManager resilienceManager() {
        // 默认节点限流 5000，租户限流 100
        return new InMemResilienceManager(5000, 100);
    }

    @Bean
    public MessageDispatcher messageDispatcher(
            @Value("${connector.node-id}") String nodeId,
            IRouteStore routeStore,
            IConnectionManager connectionManager,
            IP2PClient p2pClient,
            ILoadBalancer loadBalancer,
            ToleranceManager toleranceManager,
            IResilienceManager resilienceManager) {
        return new MessageDispatcher(nodeId, routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
    }

    @Bean
    public ToleranceManager toleranceManager(
            com.chanjet.connector.api.store.IFailStore failStore,
            com.chanjet.connector.api.push.IPushControl pushControl) {
        return new ToleranceManager(failStore, pushControl);
    }
}
