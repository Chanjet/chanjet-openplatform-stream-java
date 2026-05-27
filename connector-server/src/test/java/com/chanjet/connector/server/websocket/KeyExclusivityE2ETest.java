package com.chanjet.connector.server.websocket;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.client.IInternalHttpClient;
import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.store.IFailStore;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.core.state.ToleranceManager;
import com.chanjet.connector.infra.core.RemoteCjtCoreAdapter;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.client.standard.StandardWebSocketClient;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.Map;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;

/**
 * 核心 E2E 测试：验证在 Redis 真实环境下，route 和 fail_start 键的互斥性。
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT, properties = {
    "spring.cloud.bootstrap.enabled=true",
    "spring.autoconfigure.exclude=org.springframework.cloud.client.loadbalancer.LoadBalancerAutoConfiguration",
    "spring.cloud.nacos.discovery.enabled=false",
    "spring.cloud.discovery.enabled=false",
    "services.auth.id=mock-auth",
    "services.subscription.id=mock-sub"
})
class KeyExclusivityE2ETest {

    @LocalServerPort
    private int port;

    @Autowired
    private StringRedisTemplate redisTemplate;

    @Autowired
    private IRouteStore routeStore;

    @Autowired
    private IFailStore failStore;

    @Autowired
    private ToleranceManager toleranceManager;

    @Autowired
    private INonceStore nonceStore;

    @MockBean
    private IAuthService authService;

    @MockBean
    private com.chanjet.connector.api.push.IPushControl pushControl;

    @MockBean
    private IInternalHttpClient httpClient;

    @BeforeEach
    void setUp() {
        redisTemplate.getConnectionFactory().getConnection().flushAll();
        // 模拟认证通过
        when(httpClient.post(anyString(), any(), any(), any()))
                .thenReturn(new RemoteCjtCoreAdapter.AuthResponse(true));
        // 模拟签名验证通过
        when(authService.verifySign(anyString(), anyString(), anyString())).thenReturn(true);
    }

    private String getWsUrl(String clientId, String appKey) {
        return "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=" + appKey + "&nonce=n1&sign=s1";
    }

    @Test
    void shouldMaintainKeyExclusivityInComplexScenario() throws Exception {
        String appKey = "exclusive-app";
        String clientId = "c1";
        String routeKey = "cjt:gw:route:" + appKey;
        String failKey = "cjt:gw:fail_start:" + appKey;

        System.out.println(">>> Starting Complex E2E Scenario for Key Exclusivity <<<");

        // 1. 模拟一个残留的故障计时器 (Landmine)
        failStore.getOrSet(appKey, System.currentTimeMillis() - 10000);
        assertThat(redisTemplate.hasKey(failKey)).isTrue();
        System.out.println("Step 1: Ghost fail timer created in Redis.");

        // 2. 准备握手参数 (Nonce)
        String nonce = "n123";
        redisTemplate.opsForValue().set("cjt:gw:nonce:" + nonce, appKey);
        
        // 3. 建立新连接
        System.out.println("Step 2: Connecting client " + clientId + " to Gateway...");
        StandardWebSocketClient client = new StandardWebSocketClient();
        String wsUrl = "ws://localhost:" + port + "/connect?client_id=" + clientId + "&app_key=" + appKey + "&nonce=" + nonce + "&sign=s1";
        WebSocketSession session = client.execute(new TextWebSocketHandler(), wsUrl).get(5, TimeUnit.SECONDS);
        
        // 验证：连接建立后，故障计时器应被立即重置 (resetFailureState)
        assertThat(session.isOpen()).isTrue();
        System.out.println("Step 2: WebSocket session opened.");

        // 循环等待 Redis 更新 (可能存在极短的延迟)
        boolean routeExists = false;
        boolean failGone = false;
        for (int i = 0; i < 20; i++) {
            if (redisTemplate.hasKey(routeKey)) routeExists = true;
            if (!redisTemplate.hasKey(failKey)) failGone = true;
            if (routeExists && failGone) break;
            Thread.sleep(100);
        }

        assertThat(routeExists).as("Route key should exist in Redis").isTrue();
        assertThat(failGone).as("Fail timer should be cleared on connection").isTrue();
        System.out.println("Step 2 SUCCESS: Fail timer cleared automatically.");

        // 3. 模拟在连接期间，另一个节点触发了故障逻辑 (例如 P2P 失败)
        // 强制写入一个故障计时器
        System.out.println("Step 3: Simulating ghost failure timer arrival while client is connected...");
        failStore.getOrSet(appKey, System.currentTimeMillis());
        assertThat(redisTemplate.hasKey(failKey)).isTrue();
        assertThat(redisTemplate.hasKey(routeKey)).isTrue();
        System.out.println("Step 3: Simultaneous keys verified (Simulation of cluster inconsistency).");

        // 4. 发送心跳 (Pong)
        System.out.println("Step 4: Waiting for 10s throttle to expire...");
        Thread.sleep(11000); // 必须超过 FORCE_CLEAN_INTERVAL_MS (10s)
        
        System.out.println("Step 4: Sending heartbeat (pong) from client to trigger self-healing...");
        session.sendMessage(new TextMessage("{\"msg_type\":\"pong\"}"));
        
        // 循环等待清理
        boolean cleared = false;
        for (int i = 0; i < 50; i++) {
            if (!redisTemplate.hasKey(failKey)) {
                cleared = true;
                break;
            }
            Thread.sleep(100);
            if (i % 5 == 0) {
                 // 持续发送心跳直到清理 (模拟真实世界的周期性心跳)
                 session.sendMessage(new TextMessage("{\"msg_type\":\"pong\"}"));
            }
        }

        assertThat(cleared).as("Self-healing should clear the ghost fail timer on heartbeat").isTrue();
        assertThat(redisTemplate.hasKey(routeKey)).as("Route key must remain after self-healing").isTrue();
        System.out.println("Step 4 SUCCESS: Ghost fail timer cleared via self-healing heartbeat.");

        session.close();
        System.out.println(">>> E2E Scenario Completed Successfully <<<");
    }
}
