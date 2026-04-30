package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.EventFrame;
import org.junit.jupiter.api.Test;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.*;

public class MessageDispatcherTest {

    static class TestMessage extends BaseMessage {
        public String data;
    }

    @Test
    public void testDispatchWithoutEncryption() {
        MessageDispatcher dispatcher = new MessageDispatcher();
        AtomicBoolean handled = new AtomicBoolean(false);

        dispatcher.register("testMsg", TestMessage.class, msg -> {
            assertEquals("hello", msg.data);
            assertEquals("ID-123", msg.getMsgId());
            handled.set(true);
            return true;
        });

        String payload = """
            {"msgId":"ID-123","msgType":"testMsg","data":"hello"}""";

        EventFrame frame = new EventFrame(
                "event", "ID-123", "trace-1", "app-1", null,
                new HashMap<>(), payload,
                System.currentTimeMillis()
        );

        boolean result = dispatcher.dispatch(frame, "secret");
        assertTrue(result);
        assertTrue(handled.get());
    }

    @Test
    public void testDispatchUnknownType() {
        MessageDispatcher dispatcher = new MessageDispatcher();
        EventFrame frame = new EventFrame(
                "event", "ID-123", "trace-1", "app-1", null,
                new HashMap<>(), "{}", System.currentTimeMillis()
        );
        boolean result = dispatcher.dispatch(frame, "secret");
        assertTrue(result, "Should return true (silent ignore) for unknown message type");
    }

    @Test
    public void testHaoYeCaiLosslessDispatch() throws Exception {
        String appSecret = "<DUMMY_SECRET_32>";
        MessageDispatcher dispatcher = new MessageDispatcher();
        AtomicBoolean handled = new AtomicBoolean(false);

        // 1. 注册好系列监听器 (语义化 + 无损)
        dispatcher.onAppNotice("GoodsIssue", (msg, content) -> {
            assertEquals("ORG-100", msg.getOrgId());
            assertEquals("BOOK-200", msg.getBookCode());
            assertEquals("SA-001", content.getCode());
            assertEquals("GoodsIssue", content.getBoName());
            handled.set(true);
            return true;
        });

        // 2. 模拟业务明文
        String businessJson = """
            {
                "msgType": "APP_NOTICE",
                "orgId": "ORG-100",
                "bookCode": "BOOK-200",
                "bizContent": {
                    "boName": "GoodsIssue",
                    "code": "SA-001",
                    "userName": "Tester"
                }
            }""";
        
        // 3. 模拟畅捷通推送包装结构
        byte[] encrypted = CryptoUtilsTest.encryptAes(businessJson, appSecret);
        String encryptedBase64 = java.util.Base64.getEncoder().encodeToString(encrypted);
        String wrapperPayload = String.format("{\"encryptMsg\":\"%s\"}", encryptedBase64);

        EventFrame frame = new EventFrame(
                "event", "M-1", "T-1", "app-1", null,
                new HashMap<>(), wrapperPayload, System.currentTimeMillis()
        );

        // 4. 执行分发
        boolean result = dispatcher.dispatch(frame, appSecret);
        
        assertTrue(result);
        assertTrue(handled.get());
    }

    @Test
    public void testAppTicketShortcut() throws Exception {
        String appSecret = "<DUMMY_SECRET_32>";
        MessageDispatcher dispatcher = new MessageDispatcher();
        AtomicBoolean handled = new AtomicBoolean(false);

        dispatcher.onAppTicket(msg -> {
            assertEquals("TICKET-999", msg.getBizContent().getAppTicket());
            handled.set(true);
            return true;
        });

        String payload = """
            {"msgType":"APP_TICKET","bizContent":{"appTicket":"TICKET-999"}}""";
        
        EventFrame frame = new EventFrame("event", "m", "t", "a", null, new HashMap<>(), payload, 1000L);
        
        boolean result = dispatcher.dispatch(frame, appSecret);
        assertTrue(result);
        assertTrue(handled.get());
    }
}
