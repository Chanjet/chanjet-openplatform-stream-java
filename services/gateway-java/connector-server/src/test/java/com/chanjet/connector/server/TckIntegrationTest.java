package com.chanjet.connector.server;

import com.chanjet.connector.api.auth.IAuthService;
import com.chanjet.connector.api.store.INonceStore;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.sdk.GatewayClient;
import com.chanjet.connector.server.websocket.WsSessionRegistry;
import org.junit.jupiter.api.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import java.util.Map;
import java.util.Set;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT, properties = {
    "connector.node-id=TCK-NODE:8080"
})
@AutoConfigureMockMvc
@Import(TckTestConfig.class)
class TckIntegrationTest {

    private static final Logger log = LoggerFactory.getLogger(TckIntegrationTest.class);

    @LocalServerPort
    private int port;

    @Value("${connector.node-id}")
    private String nodeId;

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private WsSessionRegistry realSessionRegistry;

    @MockBean private IRouteStore routeStore;
    @MockBean private INonceStore nonceStore;
    @MockBean private IAuthService authService;
    @MockBean private IResilienceManager resilienceManager;
    @MockBean private ILoadBalancer loadBalancer;
    @MockBean private com.chanjet.connector.api.push.IPushControl pushControl;
    @MockBean private com.chanjet.connector.api.store.IFailStore failStore;
    @MockBean private com.chanjet.connector.api.connection.IP2PClient p2pClient;

    @Test
    void tck01_shouldCompleteEndToEndMessageLoop() throws Exception {
        String appKey = "tck-app";
        String clientId = appKey + "@local";
        String nonce = "tck-nonce";
        
        when(nonceStore.createNonce(appKey)).thenReturn(nonce);
        when(nonceStore.verifyAndConsume(nonce, appKey)).thenReturn(true);
        when(authService.verifySign(anyString(), anyString(), anyString())).thenReturn(true);
        when(resilienceManager.tryAcquire(anyString())).thenReturn(AcquisitionResult.ALLOWED);
        
        String routeValue = nodeId + ":" + clientId;
        when(routeStore.getNodes(appKey)).thenReturn(Set.of(routeValue));
        when(loadBalancer.select(any())).thenReturn(Optional.of(routeValue));

        BlockingQueue<EventFrame> receivedFrames = new LinkedBlockingQueue<>();
        GatewayClient sdkClient = GatewayClient.builder()
                .appKey(appKey)
                .appSecret("tck-secret")
                .gatewayUrl("http://localhost:" + port)
                .build();
        
        sdkClient.onEvent(frame -> {
            receivedFrames.add(frame);
            return true;
        });

        sdkClient.start();
        
        long start = System.currentTimeMillis();
        while (realSessionRegistry.getSession(clientId).isEmpty()) {
            if (System.currentTimeMillis() - start > 5000) {
                throw new RuntimeException("Timeout waiting for SDK connection in Registry");
            }
            Thread.sleep(100);
        }

        mockMvc.perform(post("/internal/v1/webhook/dispatch")
                        .header("X-C-APP_KEY", appKey)
                        .header("X-MSG-ID", "msg-tck-01")
                        .content("{\"data\":\"tck-payload\"}")
                        .contentType(MediaType.APPLICATION_JSON))
                .andExpect(status().isOk());

        EventFrame received = receivedFrames.poll(10, TimeUnit.SECONDS);
        assertThat(received).isNotNull();
        assertThat(received.payload()).isEqualTo("{\"data\":\"tck-payload\"}");

        sdkClient.stop();
    }

    @Test
    void tck03_shouldForwardToExactClientInMultiClientScenario() throws Exception {
        String appKey = "multi-app";
        String clientIdA = "client-A";
        String clientIdB = "client-B";
        
        when(nonceStore.createNonce(appKey)).thenReturn("n-any");
        when(nonceStore.verifyAndConsume(anyString(), anyString())).thenReturn(true);
        when(authService.verifySign(anyString(), anyString(), anyString())).thenReturn(true);
        when(resilienceManager.tryAcquire(anyString())).thenReturn(AcquisitionResult.ALLOWED);
        
        String routeB = nodeId + ":" + clientIdB;
        when(routeStore.getNodes(appKey)).thenReturn(Set.of(nodeId + ":" + clientIdA, routeB));
        when(loadBalancer.select(any())).thenReturn(Optional.of(routeB));

        BlockingQueue<EventFrame> queueB = new LinkedBlockingQueue<>();
        GatewayClient sdkB = GatewayClient.builder()
                .appKey(appKey).appSecret("s").gatewayUrl("http://localhost:" + port).build();
        sdkB.onEvent(frame -> { queueB.add(frame); return true; });
        sdkB.start();

        while (realSessionRegistry.getSession(clientIdB).isEmpty()) { Thread.sleep(50); }

        mockMvc.perform(post("/internal/v1/webhook/dispatch")
                        .header("X-C-APP_KEY", appKey)
                        .header("X-MSG-ID", "p2p-exact-1")
                        .content("data")).andExpect(status().isOk());

        EventFrame received = queueB.poll(5, TimeUnit.SECONDS);
        assertThat(received).isNotNull();
        assertThat(received.targetClientId()).isEqualTo(clientIdB);

        sdkB.stop();
    }
}
