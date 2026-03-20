/**
 * 原始推送事件帧。
 */
export interface EventFrame {
    msg_type: 'event';
    msg_id: string;
    trace_id?: string;
    app_key: string;
    target_client_id: string;
    headers?: Record<string, string>;
    payload: string;
    timestamp: number;
}

/**
 * 消息确认帧。
 */
export interface AckFrame {
    msg_id: string;
    code: number;
    message: string;
    timestamp: number;
}

/**
 * 业务推送消息基类。
 */
export interface BaseMessage {
    msgId?: string;
    msgType?: string;
    appKey?: string;
    appId?: string;
    requestId?: string;
    uniqueId?: string;
    bookCode?: string;
    orgId?: string;
    timestamp?: number;
    headers?: Record<string, string>;
    [key: string]: any;
}

/**
 * 应用票据消息。
 */
export interface AppTicketMessage extends BaseMessage {
    appTicket: string;
}

/**
 * 企业临时授权码消息。
 */
export interface EntAuthCodeMessage extends BaseMessage {
    authCode: string;
}

/**
 * 好系列标准业务通知。
 */
export interface AppNoticeMessage extends BaseMessage {
    bizContent: {
        boName: string;
        transactionTypeEnum?: string;
        [key: string]: any;
    };
}
