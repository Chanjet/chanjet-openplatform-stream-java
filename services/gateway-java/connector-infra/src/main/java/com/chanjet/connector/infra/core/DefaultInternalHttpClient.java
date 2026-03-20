package com.chanjet.connector.infra.core;

import com.chanjet.connector.api.client.IInternalHttpClient;
import org.springframework.http.MediaType;
import org.springframework.web.client.RestClient;
import java.util.Map;

/**
 * 基于 RestClient 的默认实现。
 */
public class DefaultInternalHttpClient implements IInternalHttpClient {

    private final RestClient restClient;

    public DefaultInternalHttpClient(RestClient restClient) {
        this.restClient = restClient;
    }

    @Override
    public <T> T post(String url, Object body, Class<T> responseType, Map<String, String> headers) {
        RestClient.RequestBodySpec spec = restClient.post()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON);
        if (headers != null) headers.forEach(spec::header);
        return spec.body(body).retrieve().body(responseType);
    }

    @Override
    public void patch(String url, Object body, Map<String, String> headers) {
        RestClient.RequestBodySpec spec = restClient.patch()
                .uri(url)
                .contentType(MediaType.APPLICATION_JSON);
        if (headers != null) headers.forEach(spec::header);
        spec.body(body).retrieve().toBodilessEntity();
    }
}
