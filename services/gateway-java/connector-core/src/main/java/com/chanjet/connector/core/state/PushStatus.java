package com.chanjet.connector.core.state;

/**
 * 应用推送状态。
 */
public enum PushStatus {
    /** 在线，正常推送 */
    ACTIVE,
    /** 零在线客户端，容忍期内等待重连 */
    WAITING,
    /** 已通知 Core 挂起推送 */
    SUSPENDED
}
