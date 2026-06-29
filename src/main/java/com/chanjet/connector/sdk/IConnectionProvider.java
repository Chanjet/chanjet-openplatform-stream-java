package com.chanjet.connector.sdk;

import java.net.URI;
import java.net.http.WebSocket;
import java.util.concurrent.CompletableFuture;

/**
 * WebSocket 连接提供者契约，方便 TDD 测试 Mock。
 */
public interface IConnectionProvider {
    CompletableFuture<WebSocket> connect(URI uri, WebSocket.Listener listener);
}
