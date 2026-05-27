package com.chanjet.connector.core.dispatcher;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import static org.junit.jupiter.api.Assertions.*;

class AckManagerTest {

    private AckManager ackManager;

    @BeforeEach
    void setUp() {
        ackManager = new AckManager();
    }

    @Test
    void testRegisterAndCompleteSuccessfully() throws ExecutionException, InterruptedException, TimeoutException {
        String msgId = "msg-123";
        CompletableFuture<Boolean> future = ackManager.registerAck(msgId, 5000);
        
        assertNotNull(future);
        assertFalse(future.isDone());
        
        ackManager.completeAck(msgId, true);
        
        assertTrue(future.isDone());
        assertTrue(future.get(1, TimeUnit.SECONDS));
    }

    @Test
    void testRegisterAndCompleteFailed() throws ExecutionException, InterruptedException, TimeoutException {
        String msgId = "msg-456";
        CompletableFuture<Boolean> future = ackManager.registerAck(msgId, 5000);
        
        assertFalse(future.isDone());
        
        ackManager.completeAck(msgId, false);
        
        assertTrue(future.isDone());
        assertFalse(future.get(1, TimeUnit.SECONDS));
    }

    @Test
    void testRegisterTimeout() throws ExecutionException, InterruptedException {
        String msgId = "msg-789";
        // Set a very short timeout
        CompletableFuture<Boolean> future = ackManager.registerAck(msgId, 100);
        
        assertFalse(future.isDone());
        
        // Wait for timeout to trigger
        Thread.sleep(200);
        
        assertTrue(future.isDone());
        assertFalse(future.get());
    }

    @Test
    void testCompleteUnknownMsgId() {
        // Should not throw any exception
        assertDoesNotThrow(() -> ackManager.completeAck("unknown-msg", true));
    }
}
