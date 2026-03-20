package com.chanjet.connector.api.client;

import java.util.Map;

/**
 * 内部 HTTP 客户端抽象，用于隔离特定的 HTTP 库实现并解决 Bean 冲突。
 */
public interface IInternalHttpClient {
    <T> T post(String url, Object body, Class<T> responseType, Map<String, String> headers);
    void patch(String url, Object body, Map<String, String> headers);
}
