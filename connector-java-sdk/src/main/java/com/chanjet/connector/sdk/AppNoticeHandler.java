package com.chanjet.connector.sdk;

/**
 * 针对好系列 APP_NOTICE 的语义化处理器。
 * 同时提供完整消息对象和业务内容，确保信息无损。
 */
@FunctionalInterface
public interface AppNoticeHandler {
    /**
     * 处理通知。
     * @param message 完整的消息对象 (包含 orgId, bookCode, headers 等)
     * @param content 解密后的业务内容 (bizContent)
     * @return 是否成功处理
     */
    boolean handle(AppNoticeMessage message, AppNoticeMessage.NoticeContent content);
}
