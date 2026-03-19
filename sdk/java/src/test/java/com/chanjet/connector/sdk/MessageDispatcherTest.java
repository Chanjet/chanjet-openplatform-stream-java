package com.chanjet.connector.sdk;

import com.chanjet.connector.common.protocol.EventFrame;
import org.junit.jupiter.api.Test;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.*;

public class MessageDispatcherTest {

    public static class TestMessage extends BaseMessage {
        public String data;
    }

    @Test
    public void testDispatchWithoutEncryption() {
        MessageDispatcher dispatcher = new MessageDispatcher();
        AtomicBoolean handled = new AtomicBoolean(false);

        dispatcher.register("testMsg", TestMessage.class, msg -> {
            assertEquals("hello", ( (TestMessage) msg).data);
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
    public void testDispatchWithEncryptMsgWrapper() throws Exception {
        String appSecret = "12345678901234567890123456789012";
        MessageDispatcher dispatcher = new MessageDispatcher();
        AtomicBoolean handled = new AtomicBoolean(false);

        dispatcher.register("testWrapper", TestMessage.class, msg -> {
            assertEquals("wrapped-data", ((TestMessage) msg).data);
            handled.set(true);
            return true;
        });

        // 1. Prepare encrypted business JSON
        String businessJson = """
            {"msgType":"testWrapper","data":"wrapped-data"}""";
        
        // Use CryptoUtils logic (manually encrypt for test)
        byte[] encrypted = CryptoUtilsTest.encryptAes(businessJson, appSecret);
        String encryptedBase64 = java.util.Base64.getEncoder().encodeToString(encrypted);

        // 2. Create the wrapper payload
        String wrapperPayload = String.format("{\"encryptMsg\":\"%s\"}", encryptedBase64);

        EventFrame frame = new EventFrame(
                "event", "M-1", "T-1", "app-1", null,
                new HashMap<>(), wrapperPayload, System.currentTimeMillis()
        );

        // 3. Dispatch
        boolean result = dispatcher.dispatch(frame, appSecret);
        assertTrue(result);
        assertTrue(handled.get());
    }
}
