package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.infra.core.RemoteCjtCoreAdapter;
import com.chanjet.connector.server.config.NodeIdResolver;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.client.standard.StandardWebSocketClient;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.Set;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

/**
 * 验证驱逐策略的集成测试。
 * 覆盖：本地驱逐、跨节点远程驱逐、排他连接、非排他连接。
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT, properties = {
    "spring.data.redis.host=127.0.0.1",
    "spring.data.redis.port=6379",
    "spring.cloud.bootstrap.enabled=true",
    "spring.autoconfigure.exclude=org.springframework.cloud.client.loadbalancer.LoadBalancerAutoConfiguration",
    "spring.cloud.nacos.discovery.enabled=false",
    "spring.cloud.discovery.enabled=false"
})
class EvictionStrategyIntegrationTest {

    @LocalServerPort
    private int port;

    @Autowired
    private StringRedisTemplate redisTemplate;

    @Autowired
    private IRouteStore routeStore;

    @Autowired
    private NodeIdResolver nodeIdResolver;

    @MockBean
    private INonceStore nonceStore;

    @MockBean
    private IAuthService authService;

    @MockBean
    private IInternalHttpClient httpClient;

    @MockBean
    private IP2PClient p2pClient;

    @MockBean
    private IPushControl pushControl;

    @BeforeEach
    void setUp() {
        redisTemplate.getConnectionFactory().getConnection().flushAll();
        when(nonceStore.verifyAndConsume(anyString(), anyString())).thenReturn(true);
        when(httpClient.post(anyString(), any(), any(), any())).thenReturn(new RemoteCjtCoreAdapter.AuthResponse(true));
        when(authService.verifySign(anyString(), anyString(), anyString())).thenReturn(true);
    }

    private String getWsUrl(String clientId, String appKey, boolean exclusive) {
        return "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=" + appKey + "&nonce=n1&sign=s1&exclusive=" + exclusive;
    }

    @Test
    void testNonExclusiveMode_ShouldAllowMultipleClients() throws Exception {
        String appKey = "non-exclusive-app";
        String clientId1 = "client-1";
        String clientId2 = "client-2";

        // 1. Client 1 连接 (非排他)
        StandardWebSocketClient wsClient1 = new StandardWebSocketClient();
        WebSocketSession session1 = wsClient1.execute(new TextWebSocketHandler(), getWsUrl(clientId1, appKey, false)).get(5, TimeUnit.SECONDS);

        // 2. Client 2 连接 (非排他)
        StandardWebSocketClient wsClient2 = new StandardWebSocketClient();
        WebSocketSession session2 = wsClient2.execute(new TextWebSocketHandler(), getWsUrl(clientId2, appKey, false)).get(5, TimeUnit.SECONDS);

        // 等待 Redis 状态稳定
        Thread.sleep(500);

        // 验证：两个 Client 都可以存活
        assertThat(session1.isOpen()).isTrue();
        assertThat(session2.isOpen()).isTrue();

        Set<String> routes = routeStore.getNodes(appKey);
        assertThat(routes).hasSize(2);
        
        session1.close();
        session2.close();
    }

    @Test
    void testProactiveEviction_ShouldEvictSameClientOnDifferentNode() throws Exception {
        String appKey = "proactive-app";
        String clientId = "ghost-client";
        String remoteNode = "10.0.0.99:8080";

        // 1. 在 Redis 中人工伪造一个远端的僵尸路由
        routeStore.add(appKey, remoteNode, clientId);
        
        Set<String> initialRoutes = routeStore.getNodes(appKey);
        assertThat(initialRoutes).contains(remoteNode + ":" + clientId);

        // 2. 该 Client 在本地发起重连 (模拟网络漂移)
        StandardWebSocketClient wsClient = new StandardWebSocketClient();
        WebSocketSession session = wsClient.execute(new TextWebSocketHandler(), getWsUrl(clientId, appKey, false)).get(5, TimeUnit.SECONDS);

        // 等待驱逐线程执行
        Thread.sleep(500);

        // 验证 1: 本地连接建立成功
        assertThat(session.isOpen()).isTrue();
        
        // 验证 2: 远端僵尸路由被从 Redis 中清理
        Set<String> newRoutes = routeStore.getNodes(appKey);
        assertThat(newRoutes).contains(nodeIdResolver.getResolvedNodeId() + ":" + clientId);
        assertThat(newRoutes).doesNotContain(remoteNode + ":" + clientId);

        // 验证 3: 向远端节点发送了 P2P 驱逐指令
        verify(p2pClient, times(1)).evict(remoteNode, clientId);

        session.close();
    }

    @Test
    void testExclusiveMode_ShouldEvictAllOtherClients() throws Exception {
        String appKey = "exclusive-app";
        String clientOld1 = "old-local-client";
        String clientOld2 = "old-remote-client";
        String clientNew = "new-exclusive-client";
        String remoteNode = "10.0.0.88:8080";

        // 1. 本地建立一个旧连接 (模拟正在运行的端点)
        StandardWebSocketClient oldWsClient = new StandardWebSocketClient();
        WebSocketSession oldSession = oldWsClient.execute(new TextWebSocketHandler(), getWsUrl(clientOld1, appKey, false)).get(5, TimeUnit.SECONDS);
        
        // 2. 在 Redis 伪造一个远端的旧连接
        routeStore.add(appKey, remoteNode, clientOld2);

        Thread.sleep(500);
        assertThat(routeStore.getNodes(appKey)).hasSize(2);

        // 3. 新客户端以 排他模式 (exclusive=true) 连入
        StandardWebSocketClient newWsClient = new StandardWebSocketClient();
        WebSocketSession newSession = newWsClient.execute(new TextWebSocketHandler(), getWsUrl(clientNew, appKey, true)).get(5, TimeUnit.SECONDS);

        // 等待驱逐完成
        Thread.sleep(1000);

        // 验证 1: 新连接存活
        assertThat(newSession.isOpen()).isTrue();

        // 验证 2: 本地旧连接被强制关闭
        assertThat(oldSession.isOpen()).isFalse();

        // 验证 3: 远端旧连接被发送了 P2P 驱逐指令
        verify(p2pClient, times(1)).evict(remoteNode, clientOld2);

        // 验证 4: Redis 中只剩下新连接的路由
        Set<String> finalRoutes = routeStore.getNodes(appKey);
        assertThat(finalRoutes).hasSize(1);
        assertThat(finalRoutes).contains(nodeIdResolver.getResolvedNodeId() + ":" + clientNew);

        newSession.close();
    }
}
