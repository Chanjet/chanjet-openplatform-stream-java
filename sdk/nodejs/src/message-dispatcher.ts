import { EventFrame, BaseMessage, AppNoticeMessage } from './protocol.js';
import { CryptoUtils } from './crypto-utils.js';

export type MessageHandler<T extends BaseMessage> = (message: T) => Promise<boolean> | boolean;

/**
 * 业务消息分发器。
 * 支持自动解密与多级 Handler 路由。
 */
export class MessageDispatcher {
    private handlerRegistry = new Map<string, MessageHandler<any>>();

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
     * 快捷注册：好系列业务通知。
     */
    onAppNotice(boName: string, handler: MessageHandler<AppNoticeMessage>, transactionType?: string) {
        const key = transactionType ? `APP_NOTICE:${boName}:${transactionType}` : `APP_NOTICE:${boName}`;
        this.register(key, handler);
    }

    /**
     * 执行分发逻辑。
     */
    async dispatch(frame: EventFrame, appSecret: string): Promise<boolean> {
        try {
            let payload = frame.payload;
            let root = JSON.parse(payload);

            // 1. 自动解密
            if (root.encryptMsg) {
                const decrypted = CryptoUtils.aesDecrypt(root.encryptMsg, appSecret);
                root = JSON.parse(decrypted);
            }

            // 2. 路由计算
            let msgType = root.msgType;

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
            if (!handler) {
                console.warn(`[MessageDispatcher] No handler for msgType: ${msgType}. Skipping.`);
                return true;
            }

            const message: BaseMessage = {
                ...root,
                headers: frame.headers
            };

            return await handler(message);
        } catch (e) {
            console.error(`[MessageDispatcher] Dispatch failed for msg ${frame.msg_id}:`, e);
            return false;
        }
    }
}
