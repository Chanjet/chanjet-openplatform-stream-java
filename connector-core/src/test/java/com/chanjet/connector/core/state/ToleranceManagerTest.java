package com.chanjet.connector.core.state;

import com.chanjet.connector.api.push.IPushControl;
import com.chanjet.connector.api.store.IFailStore;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class ToleranceManagerTest {

    private static final String APP_KEY = "test-app";
    private static final long THIRTY_MIN_MS = 30 * 60 * 1000L;

    private ToleranceManager toleranceManager;

    @Mock
    private IFailStore failStore;

    @Mock
    private IPushControl pushControl;

    @BeforeEach
    void setUp() {
        toleranceManager = new ToleranceManager(failStore, pushControl);
    }

    @Test
    void shouldReturnWaitingStatusWithinTolerancePeriod() {
        long now = 1000000L;
        // 第一次失败，设置计时器
        when(failStore.getOrSet(APP_KEY, now)).thenReturn(now);

        PushStatus status = toleranceManager.handleFailure(APP_KEY, now);

        assertThat(status).isEqualTo(PushStatus.WAITING);
        verify(pushControl, never()).setPushEnabled(anyString(), anyBoolean());
    }

    @Test
    void shouldReturnSuspendedStatusAndDisablePushWhenToleranceExpires() {
        long failStart = 1000000L;
        long now = failStart + THIRTY_MIN_MS + 1; // 超过 30 分钟
        
        when(failStore.getOrSet(APP_KEY, now)).thenReturn(failStart);

        PushStatus status = toleranceManager.handleFailure(APP_KEY, now);

        assertThat(status).isEqualTo(PushStatus.SUSPENDED);
        verify(pushControl).setPushEnabled(APP_KEY, false);
        verify(failStore).clear(APP_KEY);
    }

    @Test
    void shouldEnablePushAndClearTimerOnReconnect() {
        // 先触发一次失败，让本地产生 Dirty 标记
        when(failStore.getOrSet(APP_KEY, 1000L)).thenReturn(1000L);
        toleranceManager.handleFailure(APP_KEY, 1000L);
        
        toleranceManager.handleReconnect(APP_KEY);

        verify(pushControl).setPushEnabled(APP_KEY, true);
        verify(failStore).clear(APP_KEY);
    }

    @Test
    void shouldForceClearTimerOnReset() {
        // 直接重置，不需要前置失败条件
        toleranceManager.resetFailureState(APP_KEY);

        verify(pushControl).setPushEnabled(APP_KEY, true);
        verify(failStore).clear(APP_KEY);
    }
}
