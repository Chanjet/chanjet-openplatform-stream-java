package com.chanjet.connector.core.loadbalance;

import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

class RandomLoadBalancerTest {

    private final RandomLoadBalancer loadBalancer = new RandomLoadBalancer();

    @Test
    void shouldReturnEmptyWhenNoRoutesProvided() {
        assertThat(loadBalancer.select(Collections.emptySet())).isEmpty();
    }

    @Test
    void shouldReturnTheOnlyRouteWhenSingleRouteProvided() {
        String route = "node1:8080:client1";
        assertThat(loadBalancer.select(Set.of(route))).contains(route);
    }

    @Test
    void shouldReturnOneOfRoutesWhenMultipleRoutesProvided() {
        Set<String> routes = Set.of("r1", "r2", "r3");
        Optional<String> selected = loadBalancer.select(routes);
        assertThat(selected).isPresent();
        assertThat(routes).contains(selected.get());
    }
}
