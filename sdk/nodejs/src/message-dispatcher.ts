import { EventFrame, BaseMessage, AppNoticeMessage } from './protocol.js';
import { CryptoUtils } from './crypto-utils.js';

export type MessageHandler<T extends BaseMessage> = (message: T) => Promise<boolean> | boolean;

/**
 * 业务消息分发器。
 * 支持自动解密与多级 Handler 路由。
 */
export class MessageDispatcher {
    private handlerRegistry = new Map<string, MessageHandler<any>>();
    private fallbackHandler: MessageHandler<any> | null = null;

    /**
     * 设置兜底的 fallback 处理器，当找不到对应 msgType 时调用。
     */
    setFallbackHandler(handler: MessageHandler<any>) {
        this.fallbackHandler = handler;
    }

    /**
     * 注册消息处理器。
     */
    register<T extends BaseMessage>(msgType: string, handler: MessageHandler<T>) {
        this.handlerRegistry.set(msgType, handler);
    }

    /**
     * 快捷注册：应用票据。
     */
    onAppTicket(handler: MessageHandler<any>) {
        this.register('APP_TICKET', handler);
    }

    /**
     * 快捷注册：企业临时授权码。
     */
    onEntAuthCode(handler: MessageHandler<any>) {
        this.register('TEMP_AUTH_CODE', handler);
    }

    /**
     * 快捷注册：订单支付成功。
     */
    onOrderStatus(handler: MessageHandler<any>) {
        this.register('PAY_ORDER_SUCCESS', handler);
    }

    /**
     * 快捷注册：应用取消开通。
     */
    onAppCancelOpen(handler: MessageHandler<any>) {
        this.register('APP_CANCEL_OPEN', handler);
    }

    /**
     * 快捷注册：企业解除授权。
     */
    onEntUnauth(handler: MessageHandler<any>) {
        this.register('APP_CANCEL_AUTHORIZATION', handler);
    }

    /**
     * 快捷注册：好系列业务通知。
     */
    onAppNotice(boName: string, handler: MessageHandler<AppNoticeMessage>, transactionType?: string) {
        const key = transactionType ? `APP_NOTICE:${boName}:${transactionType}` : `APP_NOTICE:${boName}`;
        this.register(key, handler);
    }

    /**
     * 执行分发逻辑。
     * @param frame 事件帧
     * @param decryptKey 独立的解密密钥
     */
    async dispatch(frame: EventFrame, decryptKey: string): Promise<boolean> {
        try {
            let payload = frame.payload;
            let root = JSON.parse(payload);

            // 1. 自动解密
            if (root.encryptMsg) {
                const decrypted = CryptoUtils.aesDecrypt(root.encryptMsg, decryptKey);
                root = JSON.parse(decrypted);
            }

            return await this.dispatchValue(root, frame.headers);
        } catch (e) {
            console.error(`[MessageDispatcher] Dispatch failed for msg ${frame.msg_id}:`, e);
            return false;
        }
    }

    /**
     * 执行分发逻辑，直接根据 root 对象进行路由
     * @param root 解析后或解密后的 JSON 对象
     * @param headers 可选的请求头信息
     */
    async dispatchValue(root: any, headers?: Record<string, string>): Promise<boolean> {
        try {
            // 2. 路由计算
            let msgType = root.msgType || root.msg_type || 'UNKNOWN';

            if (msgType === 'APP_NOTICE') {
                const biz = root.bizContent || {};
                const boName = biz.boName;
                const transType = biz.transactionTypeEnum;

                const fullKey = `APP_NOTICE:${boName}:${transType}`;
                const boKey = `APP_NOTICE:${boName}`;

                if (this.handlerRegistry.has(fullKey)) {
                    msgType = fullKey;
                } else if (this.handlerRegistry.has(boKey)) {
                    msgType = boKey;
                }
            }

            const handler = this.handlerRegistry.get(msgType);
            const message: BaseMessage = {
                ...root,
                msgId: root.id || root.msgId || root.msg_id, // 兼容性自动填充
                headers: headers
            };

            if (handler) {
                return await handler(message);
            } else if (this.fallbackHandler) {
                return await this.fallbackHandler(message);
            } else {
                console.warn(`[MessageDispatcher] No handler for msgType: ${msgType}. Skipping.`);
                return true;
            }
        } catch (e) {
            console.error(`[MessageDispatcher] dispatchValue failed:`, e);
            return false;
        }
    }
}
