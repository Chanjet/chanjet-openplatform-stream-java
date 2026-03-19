package com.chanjet.connector.sdk;

import org.junit.jupiter.api.Test;
import java.net.http.HttpResponse;
import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

class GatewayClientResilienceTest {

    @Test
    @SuppressWarnings("unchecked")
    void shouldEnterStandbyModeWhenServerIsFull() throws Exception {
        IHttpProvider httpProvider = mock(IHttpProvider.class);
        HttpResponse<String> response = mock(HttpResponse.class);
        
        when(response.statusCode()).thenReturn(503);
        when(httpProvider.send(any())).thenReturn(response);

        IConnectionProvider wsProvider = mock(IConnectionProvider.class);

        GatewayClient client = GatewayClient.builder()
                .appKey("test-app")
                .appSecret("test-secret")
                .gatewayUrl("http://localhost:8080")
                .httpProvider(httpProvider)
                .connectionProvider(wsProvider)
                .build();

        client.start();
        Thread.sleep(500);

        // 验证：httpProvider 被调用了
        verify(httpProvider).send(any());
        assertThat(client.isConnected()).isFalse();
        
        client.stop();
    }

    @Test
    @SuppressWarnings("unchecked")
    void shouldStopReconnectingOnAuthError() throws Exception {
        IHttpProvider httpProvider = mock(IHttpProvider.class);
        HttpResponse<String> response = mock(HttpResponse.class);
        
        when(response.statusCode()).thenReturn(401);
        when(httpProvider.send(any())).thenReturn(response);

        GatewayClient client = GatewayClient.builder()
                .appKey("test-app")
                .appSecret("test-secret")
                .gatewayUrl("http://localhost:8080")
                .httpProvider(httpProvider)
                .build();

        client.start();
        Thread.sleep(500);

        // 通过反射检查 running 状态
        java.lang.reflect.Field runningField = GatewayClient.class.getDeclaredField("running");
        runningField.setAccessible(true);
        assertThat((Boolean) runningField.get(client)).isFalse();
        
        client.stop();
    }
}
