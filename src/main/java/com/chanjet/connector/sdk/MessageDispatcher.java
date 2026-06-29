package com.chanjet.connector.sdk;

import com.chanjet.connector.sdk.protocol.EventFrame;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 业务消息分发器。
 * 支持自动解密、验签及 POJO 转换。
 */
public class MessageDispatcher {

    private static final Logger log = LoggerFactory.getLogger(MessageDispatcher.class);

    private final ObjectMapper objectMapper = new ObjectMapper()
            .configure(com.fasterxml.jackson.databind.DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);

    private final Map<String, Class<? extends BaseMessage>> typeRegistry = new ConcurrentHashMap<>();
    private final Map<String, MessageHandler<? extends BaseMessage>> handlerRegistry = new ConcurrentHashMap<>();

    /**
     * 注册消息处理器。
     * @param msgType 消息类型标识 (如 manufactureOrderMsg)
     * @param clazz 对应的 POJO 类型
     * @param handler 处理器实现
     * @param <T> 消息具体类型
     */
    public <T extends BaseMessage> void register(String msgType, Class<T> clazz, MessageHandler<T> handler) {
        typeRegistry.put(msgType, clazz);
        handlerRegistry.put(msgType, handler);
    }

    /**
     * 快捷注册：应用票据消息 (APP_TICKET)。
     */
    public void onAppTicket(MessageHandler<AppTicketMessage> handler) {
        register("APP_TICKET", AppTicketMessage.class, handler);
    }

    /**
     * 快捷注册：企业临时授权码消息 (TEMP_AUTH_CODE)。
     */
    public void onEntAuthCode(MessageHandler<EntAuthCodeMessage> handler) {
        register("TEMP_AUTH_CODE", EntAuthCodeMessage.class, handler);
    }

    /**
     * 快捷注册：解除授权消息 (APP_CANCEL_AUTHORIZATION)。
     */
    public void onEntUnauth(MessageHandler<EntUnauthMessage> handler) {
        register("APP_CANCEL_AUTHORIZATION", EntUnauthMessage.class, handler);
    }

    /**
     * 快捷注册：应用取消开通消息 (APP_CANCEL_OPEN)。
     */
    public void onAppCancelOpen(MessageHandler<AppCancelOpenMessage> handler) {
        register("APP_CANCEL_OPEN", AppCancelOpenMessage.class, handler);
    }

    /**
     * 快捷注册：订单支付成功消息 (PAY_ORDER_SUCCESS)。
     */
    public void onOrderStatus(MessageHandler<OrderStatusMessage> handler) {
        register("PAY_ORDER_SUCCESS", OrderStatusMessage.class, handler);
    }

    /**
     * 快捷注册：好系列标准业务通知 (APP_NOTICE)。
     * 处理器将同时接收完整消息对象和业务内容负载，确保上下文无损。
     * @param boName 业务对象名称 (如 GoodsIssue)
     * @param handler 语义化处理器
     */
    public void onAppNotice(String boName, AppNoticeHandler handler) {
        register("APP_NOTICE:" + boName, AppNoticeMessage.class, msg -> {
            AppNoticeMessage am = (AppNoticeMessage) msg;
            return handler.handle(am, am.getBizContent());
        });
    }

    /**
     * 快捷注册：好系列标准业务通知 (APP_NOTICE) - 带交易类型。
     * 处理器将同时接收完整消息对象和业务内容负载，确保上下文无损。
     * @param boName 业务对象名称
     * @param transactionType 交易类型 (transactionTypeEnum)
     * @param handler 语义化处理器
     */
    public void onAppNotice(String boName, String transactionType, AppNoticeHandler handler) {
        register("APP_NOTICE:" + boName + ":" + transactionType, AppNoticeMessage.class, msg -> {
            AppNoticeMessage am = (AppNoticeMessage) msg;
            return handler.handle(am, am.getBizContent());
        });
    }

    /**
     * 执行分发逻辑。
     * @param frame 原始推送帧
     * @param decryptKey 独立的解密密钥
     * @return 处理结果
     */
    public boolean dispatch(EventFrame frame, String decryptKey) {
        try {
            String payload = frame.payload();

            // 1. 解析包装层 (格式: {"encryptMsg": "..."})
            JsonNode root = objectMapper.readTree(payload);
            
            // 2. 提取并解密包装层 (encryptMsg)
            if (root.has("encryptMsg")) {
                String encryptedData = root.get("encryptMsg").asText();
                payload = CryptoUtils.aesDecrypt(encryptedData, decryptKey);
                root = objectMapper.readTree(payload); // 重新解析解密后的业务明文
            }

            // 3. 提取消息类型并分发
            String finalMsgType = root.path("msgType").asText();

            // 特殊处理好系列的 APP_NOTICE
            if ("APP_NOTICE".equals(finalMsgType)) {
                JsonNode bizContent = root.path("bizContent");
                String boName = bizContent.path("boName").asText("");
                String transType = bizContent.path("transactionTypeEnum").asText("");
                
                // 优先级 1: APP_NOTICE:boName:transType
                String fullKey = "APP_NOTICE:" + boName + (transType.isEmpty() ? "" : ":" + transType);
                if (typeRegistry.containsKey(fullKey)) {
                    finalMsgType = fullKey;
                } else {
                    // 优先级 2: APP_NOTICE:boName
                    String boKey = "APP_NOTICE:" + boName;
                    if (typeRegistry.containsKey(boKey)) {
                        finalMsgType = boKey;
                    }
                }
            }

            if (!typeRegistry.containsKey(finalMsgType)) {
                log.warn("No handler registered for message type: {}. Message will be acknowledged as success (silent ignore).", finalMsgType);
                return true;
            }

            Class<? extends BaseMessage> clazz = typeRegistry.get(finalMsgType);
            MessageHandler handler = handlerRegistry.get(finalMsgType);

            BaseMessage message = objectMapper.treeToValue(root, clazz);
            message.setHeaders(frame.headers());

            return handler.handle(message);
        } catch (Exception e) {
            log.error("Failed to dispatch message [{}]: {}", frame.msgId(), e.getMessage());
            return false;
        }
    }
}
