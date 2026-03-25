package protocol

import "encoding/json"

// EventFrame 原始推送事件帧
type EventFrame struct {
	MsgType        string            `json:"msg_type"`
	MsgID          string            `json:"msg_id"`
	TraceID        string            `json:"trace_id,omitempty"`
	AppKey         string            `json:"app_key"`
	TargetClientID string            `json:"target_client_id"`
	Headers        map[string]string `json:"headers,omitempty"`
	Payload        string            `json:"payload"`
	Timestamp      int64             `json:"timestamp"`
}

// AckFrame 消息确认帧
type AckFrame struct {
	MsgID     string `json:"msg_id"`
	Code      int    `json:"code"`
	Message   string `json:"message"`
	Timestamp int64  `json:"timestamp"`
}

// BaseMessage 业务推送消息基类
type BaseMessage struct {
	ID        string            `json:"id,omitempty"`
	MsgID     string            `json:"msgId,omitempty"`
	MsgType   string            `json:"msgType"`
	AppKey    string            `json:"appKey"`
	AppID     string            `json:"appId,omitempty"`
	Timestamp string            `json:"time"`
	Headers   map[string]string `json:"headers,omitempty"`
}

// AppTicketMessage 应用票据消息
type AppTicketMessage struct {
	BaseMessage
	BizContent struct {
		AppTicket string `json:"appTicket"`
	} `json:"bizContent"`
}

// EntAuthCodeMessage 企业临时授权码消息 (TEMP_AUTH_CODE)
type EntAuthCodeMessage struct {
	BaseMessage
	BizContent struct {
		TempAuthCode string `json:"tempAuthCode"`
		State        string `json:"state"`
	} `json:"bizContent"`
}

// OrderStatusMessage 订单支付成功消息 (PAY_ORDER_SUCCESS)
type OrderStatusMessage struct {
	BaseMessage
	BizContent struct {
		OrderNo string `json:"orderNo"`
		OrgID   string `json:"orgId"`
		Detail  struct {
			PayTotal   float64 `json:"payTotal"`
			OrderItems []struct {
				ProductID interface{} `json:"productId"`
			} `json:"orderItems"`
		} `json:"detail"`
	} `json:"bizContent"`
}

// AppNoticeMessage 好系列标准业务通知
type AppNoticeMessage struct {
	BaseMessage
	BizContent struct {
		BoName              string `json:"boName"`
		TransactionTypeEnum string `json:"transactionTypeEnum"`
	} `json:"bizContent"`
}

// RawPayload 用于探测是否加密
type RawPayload struct {
	EncryptMsg string `json:"encryptMsg"`
	MsgType    string `json:"msgType"`
}

// GetPayloadJSON 将 Payload 字符串解析为 map 以探测 msgType
func (f *EventFrame) GetPayloadJSON() (map[string]interface{}, error) {
	var m map[string]interface{}
	err := json.Unmarshal([]byte(f.Payload), &m)
	return m, err
}
