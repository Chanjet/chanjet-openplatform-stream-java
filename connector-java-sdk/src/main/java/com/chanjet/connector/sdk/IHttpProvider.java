package com.chanjet.connector.sdk;

import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.io.IOException;

/**
 * 内部 HTTP 请求提供者，用于单元测试 Mock。
 */
public interface IHttpProvider {
    HttpResponse<String> send(HttpRequest request) throws IOException, InterruptedException;
}
