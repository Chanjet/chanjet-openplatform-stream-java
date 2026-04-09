package com.chanjet.connector.core.state;

import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * 30 分钟容忍期状态机实现。
 */
public class ToleranceManager {

    private static final Logger log = LoggerFactory.getLogger(ToleranceManager.class);
    private static final long THIRTY_MIN_MS = 30 * 60 * 1000L;
    private static final long FORCE_CLEAN_INTERVAL_MS = 10 * 1000L; // 每 10 秒允许一次兜底清理

    private final IFailStore failStore;
    private final IPushControl pushControl;
    
    // 本地脏标记：记录哪些 AppKey 处于失败观察期，需要被清理
    private final Map<String, Boolean> dirtyKeys = new ConcurrentHashMap<>();
    
    // 采样清理计时：用于分布式环境下的“随机熵减”，清理其他节点留下的幽灵键
    private final Map<String, AtomicLong> lastCleanTime = new ConcurrentHashMap<>();

    public ToleranceManager(IFailStore failStore, IPushControl pushControl) {
        this.failStore = failStore;
        this.pushControl = pushControl;
    }

    /**
     * 处理推送失败（无在线客户端）事件。
     */
    public PushStatus handleFailure(String appKey, long now) {
        long failStart = failStore.getOrSet(appKey, now);
        log.warn("Push failure for AppKey [{}]. Fail start timestamp: {}", appKey, failStart);
        
        dirtyKeys.put(appKey, true);
        
        if (now - failStart >= THIRTY_MIN_MS) {
            log.error("Tolerance period (30min) exceeded for AppKey [{}]. Disabling push.", appKey);
            pushControl.setPushEnabled(appKey, false);
            doClear(appKey);
            return PushStatus.SUSPENDED;
        }
        
        return PushStatus.WAITING;
    }

    /**
     * 业务投递成功时的清理。
     * 策略：本地脏检查优先 + 定时强制采样清理。
     */
    public void handleReconnect(String appKey) {
        boolean isDirty = dirtyKeys.containsKey(appKey);
        
        if (isDirty) {
            log.info("Optimized clearing fail timer for AppKey [{}] (Reason: Local Dirty).", appKey);
            doClear(appKey);
            return;
        }

        // 分布式兜底：即使本地不脏，每隔 10 秒也允许尝试清理一次 Redis，以消除其他节点产生的幽灵键
        long now = System.currentTimeMillis();
        AtomicLong last = lastCleanTime.computeIfAbsent(appKey, k -> new AtomicLong(0));
        long lastVal = last.get();
        
        if (now - lastVal > FORCE_CLEAN_INTERVAL_MS) {
            if (last.compareAndSet(lastVal, now)) {
                // 只有在确定 Redis 中可能存在残留时才真正 DEL (这里为了绝对健壮，直接 DEL 是最简单的)
                failStore.clear(appKey);
            }
        }
    }

    /**
     * 强力重置状态（不检查本地缓存，给 WebSocket 上线等低频事件使用）。
     */
    public void resetFailureState(String appKey) {
        log.info("Force resetting fail state for AppKey [{}].", appKey);
        doClear(appKey);
    }

    private void doClear(String appKey) {
        failStore.clear(appKey);
        pushControl.setPushEnabled(appKey, true);
        dirtyKeys.remove(appKey);
        lastCleanTime.put(appKey, new AtomicLong(System.currentTimeMillis()));
    }
}
