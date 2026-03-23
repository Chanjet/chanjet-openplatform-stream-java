package com.chanjet.connector.server.websocket;

import org.springframework.context.annotation.Configuration;
import org.springframework.web.socket.config.annotation.EnableWebSocket;
import org.springframework.web.socket.config.annotation.WebSocketConfigurer;
import org.springframework.web.socket.config.annotation.WebSocketHandlerRegistry;

@Configuration
@EnableWebSocket
public class WebSocketConfig implements WebSocketConfigurer {

    private final DefaultWsHandler wsHandler;
    private final AuthHandshakeInterceptor authInterceptor;

    public WebSocketConfig(DefaultWsHandler wsHandler, AuthHandshakeInterceptor authInterceptor) {
        this.wsHandler = wsHandler;
        this.authInterceptor = authInterceptor;
    }

    @Override
    public void registerWebSocketHandlers(WebSocketHandlerRegistry registry) {
        registry.addHandler(wsHandler, "/connect")
                .addInterceptors(authInterceptor)
                .setAllowedOrigins("*");
    }
}
