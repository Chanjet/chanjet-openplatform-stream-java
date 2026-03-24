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
    id?: string;
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
    bizContent: {
        appTicket: string;
    };
}

/**
 * 企业临时授权码消息 (TEMP_AUTH_CODE)。
 */
export interface EntAuthCodeMessage extends BaseMessage {
    bizContent: {
        tempAuthCode: string;
        state?: string;
    };
}

/**
 * 解除授权消息 (APP_CANCEL_AUTHORIZATION)。
 */
export interface EntUnauthMessage extends BaseMessage {
    bizContent: {
        appKey: string;
        appId: string;
        orgId: string;
        userId: string;
        completedTime: string | number;
    };
}

/**
 * 应用取消开通消息 (APP_CANCEL_OPEN)。
 */
export interface AppCancelOpenMessage extends BaseMessage {
    bizContent: {
        appKey: string;
        appId: string;
        orgId: string;
        userId: string;
        completedTime: string | number;
    };
}

/**
 * 订单支付成功消息 (PAY_ORDER_SUCCESS)。
 */
export interface OrderStatusMessage extends BaseMessage {
    bizContent: {
        orderNo: string;
        orgId: string;
        detail: {
            orderNo: string;
            orderTotal: number;
            orderType: number;
            payTotal: number;
            paidTime: string;
            createdTime: string;
            userId: number | string;
            orgId: number | string;
            orderItems: Array<{
                payPrice: number;
                productId: number | string;
                startDate: string;
                endDate: string;
                amountInfo: string;
            }>;
        };
    };
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
