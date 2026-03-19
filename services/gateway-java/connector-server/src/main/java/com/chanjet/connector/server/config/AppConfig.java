package com.chanjet.connector.server.config;

import com.chanjet.connector.api.config.ConnectorProperties;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import com.chanjet.connector.core.loadbalance.RandomLoadBalancer;
import com.chanjet.connector.core.resilience.InMemResilienceManager;
import com.chanjet.connector.core.state.ToleranceManager;
import org.springframework.cloud.context.config.annotation.RefreshScope;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * 核心 Bean 装配配置。
 */
@Configuration
public class AppConfig {

    @Bean
    @RefreshScope
    public ConnectorProperties connectorProperties() {
        return new ConnectorProperties();
    }

    @Bean
    public ILoadBalancer loadBalancer() {
        return new RandomLoadBalancer();
    }

    @Bean
    public IResilienceManager resilienceManager() {
        return new InMemResilienceManager(5000, 100);
    }

    @Bean
    public MessageDispatcher messageDispatcher(
            NodeIdResolver nodeIdResolver,
            IRouteStore routeStore,
            IConnectionManager connectionManager,
            IP2PClient p2pClient,
            ILoadBalancer loadBalancer,
            ToleranceManager toleranceManager,
            IResilienceManager resilienceManager) {
        
        return new MessageDispatcher(nodeIdResolver.getResolvedNodeId(), routeStore, connectionManager, p2pClient, loadBalancer, toleranceManager, resilienceManager);
    }

    @Bean
    public ToleranceManager toleranceManager(
            com.chanjet.connector.api.store.IFailStore failStore,
            com.chanjet.connector.api.push.IPushControl pushControl) {
        return new ToleranceManager(failStore, pushControl);
    }
}
