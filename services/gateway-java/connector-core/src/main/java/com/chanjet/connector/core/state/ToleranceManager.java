package com.chanjet.connector.core.state;

import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;

/**
 * 30 分钟容忍期状态机实现。
 */
public class ToleranceManager {

    private static final long THIRTY_MIN_MS = 30 * 60 * 1000L;

    private final IFailStore failStore;
    private final IPushControl pushControl;

    public ToleranceManager(IFailStore failStore, IPushControl pushControl) {
        this.failStore = failStore;
        this.pushControl = pushControl;
    }

    /**
     * 处理推送失败（无在线客户端）事件。
     * @return 当前应用应处于的状态
     */
    public PushStatus handleFailure(String appKey, long now) {
        long failStart = failStore.getOrSet(appKey, now);
        
        if (now - failStart >= THIRTY_MIN_MS) {
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
        failStore.clear(appKey);
        pushControl.setPushEnabled(appKey, true);
    }
}
