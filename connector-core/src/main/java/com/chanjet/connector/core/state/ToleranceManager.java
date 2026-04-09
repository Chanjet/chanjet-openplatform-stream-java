package com.chanjet.connector.core.state;

import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 30 分钟容忍期状态机实现。
 */
public class ToleranceManager {

    private static final Logger log = LoggerFactory.getLogger(ToleranceManager.class);
    private static final long THIRTY_MIN_MS = 30 * 60 * 1000L;
...
    public PushStatus handleFailure(String appKey, long now) {
        long failStart = failStore.getOrSet(appKey, now);
        log.warn("Push failure for AppKey [{}]. Fail start timestamp: {}", appKey, failStart);
        
        if (now - failStart >= THIRTY_MIN_MS) {
            log.error("Tolerance period (30min) exceeded for AppKey [{}]. Disabling push.", appKey);
            pushControl.setPushEnabled(appKey, false);
            failStore.clear(appKey);
            return PushStatus.SUSPENDED;
        }
        
        return PushStatus.WAITING;
    }

    /**
     * 处理客户端重连事件。
     */
    public void handleReconnect(String appKey) {
        log.info("Client reconnected for AppKey [{}]. Clearing fail timer and enabling push.", appKey);
        failStore.clear(appKey);
        pushControl.setPushEnabled(appKey, true);
    }
}
