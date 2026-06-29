package com.chanjet.connector.sdk;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import java.net.http.HttpResponse;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

@Timeout(value = 30, unit = TimeUnit.SECONDS)
class GatewayClientResilienceTest {

    @Test
    @SuppressWarnings("unchecked")
    void shouldEnterStandbyModeWhenServerIsFull() throws Exception {
        IHttpProvider httpProvider = mock(IHttpProvider.class);
        HttpResponse<String> response = mock(HttpResponse.class);
        
        CountDownLatch latch = new CountDownLatch(1);
        when(response.statusCode()).thenReturn(503);
        when(httpProvider.send(any())).thenAnswer(inv -> {
            latch.countDown();
            return response;
        });

        IConnectionProvider wsProvider = mock(IConnectionProvider.class);

        GatewayClient client = GatewayClient.builder()
                .appKey("test-app")
                .appSecret("test-secret")
                .gatewayUrl("http://localhost:8080")
                .httpProvider(httpProvider)
                .connectionProvider(wsProvider)
                .build();

        client.start();
        
        // 等待请求发生
        boolean called = latch.await(5, TimeUnit.SECONDS);
        assertThat(called).isTrue();
        
        assertThat(client.isConnected()).isFalse();
        
        client.stop();
    }

    @Test
    @SuppressWarnings("unchecked")
    void shouldStopReconnectingOnAuthError() throws Exception {
        IHttpProvider httpProvider = mock(IHttpProvider.class);
        HttpResponse<String> response = mock(HttpResponse.class);
        
        CountDownLatch latch = new CountDownLatch(1);
        when(response.statusCode()).thenReturn(401);
        when(httpProvider.send(any())).thenAnswer(inv -> {
            latch.countDown();
            return response;
        });

        GatewayClient client = GatewayClient.builder()
                .appKey("test-app")
                .appSecret("test-secret")
                .gatewayUrl("http://localhost:8080")
                .httpProvider(httpProvider)
                .build();

        client.start();
        
        boolean called = latch.await(5, TimeUnit.SECONDS);
        assertThat(called).isTrue();

        // 稍微等一下状态更新
        Thread.sleep(100);

        // 通过反射检查 running 状态
        java.lang.reflect.Field runningField = GatewayClient.class.getDeclaredField("running");
        runningField.setAccessible(true);
        assertThat((Boolean) runningField.get(client)).isFalse();
        
        client.stop();
    }
}
