package com.chanjet.connector.core.state;

import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 30 分钟容忍期状态机实现。
 */
public class ToleranceManager {

    private static final Logger log = LoggerFactory.getLogger(ToleranceManager.class);
    private static final long THIRTY_MIN_MS = 30 * 60 * 1000L;

    private final IFailStore failStore;
    private final IPushControl pushControl;
    
    // 本地脏标记：记录哪些 AppKey 处于失败观察期，需要被清理
    private final Map<String, Boolean> dirtyKeys = new ConcurrentHashMap<>();

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
        
        // 标记为“已失败”，意味着未来连接恢复时需要清理 Redis
        dirtyKeys.put(appKey, true);
        
        if (now - failStart >= THIRTY_MIN_MS) {
            log.error("Tolerance period (30min) exceeded for AppKey [{}]. Disabling push.", appKey);
            pushControl.setPushEnabled(appKey, false);
            failStore.clear(appKey);
            dirtyKeys.remove(appKey);
            return PushStatus.SUSPENDED;
        }
        
        return PushStatus.WAITING;
    }

    /**
     * 业务投递成功时的清理（带脏检查，高性能）。
     */
    public void handleReconnect(String appKey) {
        if (dirtyKeys.containsKey(appKey)) {
            log.info("Optimized clearing fail timer for AppKey [{}] (State: Dirty).", appKey);
            doClear(appKey);
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
    }
}
