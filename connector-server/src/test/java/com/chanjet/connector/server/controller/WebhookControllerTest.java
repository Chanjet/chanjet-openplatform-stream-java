package com.chanjet.connector.server.controller;

import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.core.dispatcher.MessageDispatcher;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.verify;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(WebhookController.class)
class WebhookControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private MessageDispatcher messageDispatcher;

    @MockBean
    private com.chanjet.connector.api.connection.IConnectionManager connectionManager;

    @MockBean
    private IInternalHttpClient httpClient;

    @MockBean
    private com.chanjet.connector.api.config.ConnectorProperties connectorProperties;

    @Test
    void shouldReturn200WhenDispatchSucceeds() throws Exception {
        mockMvc.perform(post("/internal/v1/webhook/dispatch")
                        .header("X-C-APP_KEY", "test-app")
                        .header("X-MSG-ID", "msg-123")
                        .content("{\"data\":\"hello\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(status().isOk());

        verify(messageDispatcher).dispatch(any());
    }

    @Test
    void shouldReturn400WhenAppKeyIsMissing() throws Exception {
        mockMvc.perform(post("/internal/v1/webhook/dispatch")
                        .header("X-MSG-ID", "msg-123")
                        .content("data"))
                .andExpect(status().isBadRequest());
    }

    @Test
    void shouldReturn503WhenDomainErrorOccurs() throws Exception {
        // 模拟由于无在线客户端导致的异常（待后续定义具体的领域异常类）
        doThrow(new RuntimeException("No client online"))
                .when(messageDispatcher).dispatch(any());

        mockMvc.perform(post("/internal/v1/webhook/dispatch")
                        .header("X-C-APP_KEY", "offline-app")
                        .header("X-MSG-ID", "msg-1")
                        .content("data"))
                .andExpect(status().isServiceUnavailable());
    }
}
