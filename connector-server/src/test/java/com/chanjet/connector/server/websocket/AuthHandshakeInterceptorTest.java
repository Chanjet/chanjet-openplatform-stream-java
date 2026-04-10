package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.store.INonceStore;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.server.ServerHttpRequest;
import org.springframework.http.server.ServerHttpResponse;
import org.springframework.web.socket.WebSocketHandler;

import java.net.URI;
import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

class AuthHandshakeInterceptorTest {

    private AuthHandshakeInterceptor interceptor;
    private INonceStore nonceStore;
    private IAuthService authService;

    @BeforeEach
    void setUp() {
        nonceStore = mock(INonceStore.class);
        authService = mock(IAuthService.class);
        interceptor = new AuthHandshakeInterceptor(nonceStore, authService);
    }

    @Test
    void shouldRejectHandshakeWhenAppKeyIsEmpty() {
        ServerHttpRequest request = mock(ServerHttpRequest.class);
        ServerHttpResponse response = mock(ServerHttpResponse.class);
        WebSocketHandler handler = mock(WebSocketHandler.class);
        Map<String, Object> attributes = new HashMap<>();

        // Case: app_key is empty string
        when(request.getURI()).thenReturn(URI.create("ws://localhost/connect?app_key=&nonce=n&sign=s"));

        boolean result = interceptor.beforeHandshake(request, response, handler, attributes);

        assertFalse(result, "Handshake should be rejected when app_key is empty");
    }

    @Test
    void shouldRejectHandshakeWhenNonceIsEmpty() {
        ServerHttpRequest request = mock(ServerHttpRequest.class);
        ServerHttpResponse response = mock(ServerHttpResponse.class);
        WebSocketHandler handler = mock(WebSocketHandler.class);
        Map<String, Object> attributes = new HashMap<>();

        when(request.getURI()).thenReturn(URI.create("ws://localhost/connect?app_key=ak&nonce=&sign=s"));

        boolean result = interceptor.beforeHandshake(request, response, handler, attributes);

        assertFalse(result, "Handshake should be rejected when nonce is empty");
    }

    @Test
    void shouldRejectHandshakeWhenSignIsEmpty() {
        ServerHttpRequest request = mock(ServerHttpRequest.class);
        ServerHttpResponse response = mock(ServerHttpResponse.class);
        WebSocketHandler handler = mock(WebSocketHandler.class);
        Map<String, Object> attributes = new HashMap<>();

        when(request.getURI()).thenReturn(URI.create("ws://localhost/connect?app_key=ak&nonce=n&sign="));

        boolean result = interceptor.beforeHandshake(request, response, handler, attributes);

        assertFalse(result, "Handshake should be rejected when sign is empty");
    }
}
