package com.chanjet.connector.sdk;

/**
 * 业务消息处理器接口。
 * @param <T> 消息类型
 */
@FunctionalInterface
public interface MessageHandler<T extends BaseMessage> {
    /**
     * 处理具体的业务消息。
     * @param message 转换后的业务 POJO
     * @return 是否成功处理
     */
    boolean handle(T message);
}
