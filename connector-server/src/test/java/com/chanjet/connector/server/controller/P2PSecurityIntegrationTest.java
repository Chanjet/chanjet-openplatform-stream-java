package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.client.IInternalHttpClient;
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
    @MockBean private IInternalHttpClient httpClient;
    
    @MockBean private ConnectorProperties properties;
    @MockBean private com.chanjet.connector.core.dispatcher.AckManager ackManager;

    @BeforeEach
    void setUp() {
        when(properties.isValidToken("token-new")).thenReturn(true);
        when(properties.isValidToken("token-old")).thenReturn(true);
        when(properties.isValidToken("wrong-token")).thenReturn(false);
        org.mockito.Mockito.when(messageDispatcher.dispatch(org.mockito.ArgumentMatchers.any())).thenReturn(new java.util.concurrent.CompletableFuture<>());
        org.mockito.Mockito.when(ackManager.registerAck(org.mockito.ArgumentMatchers.anyString(), org.mockito.ArgumentMatchers.anyLong())).thenReturn(new java.util.concurrent.CompletableFuture<>());
    }

    @Test
    void shouldAllowRequestWithPrimaryToken() throws Exception {
        java.util.concurrent.CompletableFuture<Boolean> future = new java.util.concurrent.CompletableFuture<>();
        org.mockito.Mockito.when(ackManager.registerAck(org.mockito.ArgumentMatchers.anyString(), org.mockito.ArgumentMatchers.anyLong())).thenReturn(future);

        org.springframework.test.web.servlet.MvcResult result = mockMvc.perform(post("/internal/v1/p2p/push")
                        .header("X-Internal-Token", "token-new")
                        .content("{\"msg_type\":\"event\",\"app_key\":\"test\",\"msg_id\":\"test-msg-1\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(org.springframework.test.web.servlet.result.MockMvcResultMatchers.request().asyncStarted())
                .andReturn();
                
        future.complete(true);
        mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders.asyncDispatch(result))
                .andExpect(status().isOk());
    }

    @Test
    void shouldAllowRequestWithSecondaryToken() throws Exception {
        java.util.concurrent.CompletableFuture<Boolean> future = new java.util.concurrent.CompletableFuture<>();
        org.mockito.Mockito.when(ackManager.registerAck(org.mockito.ArgumentMatchers.anyString(), org.mockito.ArgumentMatchers.anyLong())).thenReturn(future);

        org.springframework.test.web.servlet.MvcResult result = mockMvc.perform(post("/internal/v1/p2p/push")
                        .header("X-Internal-Token", "token-old")
                        .content("{\"msg_type\":\"event\",\"app_key\":\"test\",\"msg_id\":\"test-msg-2\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(org.springframework.test.web.servlet.result.MockMvcResultMatchers.request().asyncStarted())
                .andReturn();
                
        future.complete(true);
        mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders.asyncDispatch(result))
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
