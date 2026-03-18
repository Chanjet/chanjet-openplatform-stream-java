package com.chanjet.connector.common.protocol;

/**
 * 资源获取结果（用于背压与限流）。
 */
public enum AcquisitionResult {
    /** 允许执行 */
    ALLOWED,
    /** 节点负载过高（503） */
    NODE_OVERLOAD,
    /** 租户限流（429） */
    TENANT_LIMITED,
    /** 熔断开启（503） */
    CIRCUIT_OPEN
}
