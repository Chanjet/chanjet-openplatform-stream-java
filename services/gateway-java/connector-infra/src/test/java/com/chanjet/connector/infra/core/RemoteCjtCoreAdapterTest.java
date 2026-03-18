package com.chanjet.connector.infra.core;

import com.github.tomakehurst.wiremock.junit5.WireMockRuntimeInfo;
import com.github.tomakehurst.wiremock.junit5.WireMockTest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.client.HttpComponentsClientHttpRequestFactory;
import org.springframework.web.client.RestClient;

import static com.github.tomakehurst.wiremock.client.WireMock.*;
import static org.assertj.core.api.Assertions.assertThat;

@WireMockTest
class RemoteCjtCoreAdapterTest {

    private RemoteCjtCoreAdapter adapter;

    @BeforeEach
    void setUp(WireMockRuntimeInfo wmRuntimeInfo) {
        String wmBaseUrl = wmRuntimeInfo.getHttpBaseUrl();
        
        // 使用 Apache HttpClient 5 提高测试稳定性
        RestClient restClient = RestClient.builder()
                .baseUrl(wmBaseUrl)
                .requestFactory(new HttpComponentsClientHttpRequestFactory())
                .build();
        
        adapter = new RemoteCjtCoreAdapter(restClient, "", "");
    }

    @Test
    void shouldReturnTrueWhenVerifySignSucceeds() {
        stubFor(post(urlEqualTo("/internal/v1/auth/verify-sign"))
                .willReturn(okJson("{\"valid\": true}")));

        boolean result = adapter.verifySign("app1", "n1", "s1");

        assertThat(result).isTrue();
    }

    @Test
    void shouldReturnFalseWhenVerifySignFails() {
        stubFor(post(urlEqualTo("/internal/v1/auth/verify-sign"))
                .willReturn(okJson("{\"valid\": false}")));

        boolean result = adapter.verifySign("app1", "n1", "s1");

        assertThat(result).isFalse();
    }

    @Test
    void shouldInvokePushStatusApi() {
        stubFor(patch(urlMatching("/internal/v1/subscriptions/app1/push-status"))
                .willReturn(noContent()));

        adapter.setPushEnabled("app1", false);

        verify(patchRequestedFor(urlMatching("/internal/v1/subscriptions/app1/push-status")));
    }
}
