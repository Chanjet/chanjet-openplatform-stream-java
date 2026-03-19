package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.api.config.ConnectorProperties;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(WebhookController.class)
class P2PSecurityIntegrationTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean private MessageDispatcher messageDispatcher;
    @MockBean private IConnectionManager connectionManager;
    @MockBean private IRouteStore routeStore;
    @MockBean private INonceStore nonceStore;
    @MockBean private IAuthService authService;
    @MockBean private IResilienceManager resilienceManager;
    @MockBean private IP2PClient p2pClient;
    @MockBean private IFailStore failStore;
    @MockBean private ILoadBalancer loadBalancer;
    
    // 使用 MockBean 替代真实的 Properties Bean
    @MockBean private ConnectorProperties properties;

    @BeforeEach
    void setUp() {
        // 配置 Mock 行为
        when(properties.isValidToken("token-new")).thenReturn(true);
        when(properties.isValidToken("token-old")).thenReturn(true);
        when(properties.isValidToken("wrong-token")).thenReturn(false);
    }

    @Test
    void shouldAllowRequestWithPrimaryToken() throws Exception {
        mockMvc.perform(post("/internal/v1/p2p/push")
                        .header("X-Internal-Token", "token-new")
                        .content("{\"msg_type\":\"event\",\"app_key\":\"test\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(status().isOk());
    }

    @Test
    void shouldAllowRequestWithSecondaryToken() throws Exception {
        mockMvc.perform(post("/internal/v1/p2p/push")
                        .header("X-Internal-Token", "token-old")
                        .content("{\"msg_type\":\"event\",\"app_key\":\"test\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(status().isOk());
    }

    @Test
    void shouldRejectRequestWithWrongToken() throws Exception {
        mockMvc.perform(post("/internal/v1/p2p/push")
                        .header("X-Internal-Token", "wrong-token")
                        .content("{}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(status().isUnauthorized());
    }
}
