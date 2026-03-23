package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.client.IInternalHttpClient;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

class RemoteCjtCoreAdapterTest {

    private IInternalHttpClient httpClient;
    private RemoteCjtCoreAdapter adapter;

    @BeforeEach
    void setUp() {
        httpClient = mock(IInternalHttpClient.class);
        // 对齐生产代码的构造函数：httpClient, authServiceId, subServiceId
        adapter = new RemoteCjtCoreAdapter(httpClient, "auth-service", "subs-service");
    }

    @Test
    void shouldReturnTrueWhenVerifySignSucceeds() {
        RemoteCjtCoreAdapter.AuthResponse mockResponse = new RemoteCjtCoreAdapter.AuthResponse(true);
        when(httpClient.post(anyString(), any(), eq(RemoteCjtCoreAdapter.AuthResponse.class), any()))
                .thenReturn(mockResponse);

        boolean result = adapter.verifySign("app1", "nonce", "sign");

        assertThat(result).isTrue();
        verify(httpClient).post(contains("/internal/v1/auth/verify-sign"), any(), any(), any());
    }

    @Test
    void shouldReturnTrueWhenVerifyPreAuthSucceeds() {
        RemoteCjtCoreAdapter.AuthResponse mockResponse = new RemoteCjtCoreAdapter.AuthResponse(true);
        when(httpClient.post(anyString(), any(), eq(RemoteCjtCoreAdapter.AuthResponse.class), any()))
                .thenReturn(mockResponse);

        boolean result = adapter.verifyPreAuth("app1", "prefix");

        assertThat(result).isTrue();
        verify(httpClient).post(contains("/internal/v1/auth/verify-preauth"), any(), any(), any());
    }

    @Test
    void shouldHandlePushStatusUpdate() {
        adapter.setPushEnabled("app1", true);
        verify(httpClient).patch(contains("/push-status"), any(), any());
    }
}
