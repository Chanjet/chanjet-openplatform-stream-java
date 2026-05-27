package com.chanjet.connector.core.dispatcher;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.concurrent.*;

/**
 * 管理端到端消息投递 ACK 状态机
 */
public class AckManager {
    
    private static final Logger log = LoggerFactory.getLogger(AckManager.class);
    
    private final ConcurrentHashMap<String, CompletableFuture<Boolean>> pendingAcks = new ConcurrentHashMap<>();
    private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread t = new Thread(r, "AckTimeoutScheduler");
        t.setDaemon(true);
        return t;
    });

    /**
     * 注册一条等待 ACK 的消息
     * @param msgId 消息ID
     * @param timeoutMs 超时时间(毫秒)
     * @return CompletableFuture，当收到ACK或超时完成
     */
    public CompletableFuture<Boolean> registerAck(String msgId, long timeoutMs) {
        CompletableFuture<Boolean> future = new CompletableFuture<>();
        pendingAcks.put(msgId, future);
        
        // 调度超时任务
        scheduler.schedule(() -> {
            CompletableFuture<Boolean> removed = pendingAcks.remove(msgId);
            if (removed != null && !removed.isDone()) {
                log.warn("ACK timeout for MsgId: {}", msgId);
                removed.complete(false);
            }
        }, timeoutMs, TimeUnit.MILLISECONDS);
        
        return future;
    }

    /**
     * 收到 ACK 时完成对应的 Future
     * @param msgId 消息ID
     * @param success 客户端处理是否成功
     */
    public void completeAck(String msgId, boolean success) {
        CompletableFuture<Boolean> future = pendingAcks.remove(msgId);
        if (future != null && !future.isDone()) {
            future.complete(success);
        } else {
            log.debug("Received ACK for unknown or already completed MsgId: {}", msgId);
        }
    }
}
