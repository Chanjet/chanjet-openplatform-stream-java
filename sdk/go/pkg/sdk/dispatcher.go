package sdk

import (
	"encoding/json"
	"fmt"
	"log"
	"strings"
	"sync"

	"com.chanjet/connector-sdk-go/pkg/crypto"
	"com.chanjet/connector-sdk-go/pkg/protocol"
)

// MessageHandler 业务消息处理器
type MessageHandler func(message interface{}) (bool, error)

// MessageDispatcher 业务消息分发器
type MessageDispatcher struct {
	handlers map[string]MessageHandler
	mu       sync.RWMutex
}

func NewMessageDispatcher() *MessageDispatcher {
	return &MessageDispatcher{
		handlers: make(map[string]MessageHandler),
	}
}

// Register 注册消息处理器
func (d *MessageDispatcher) Register(msgType string, handler MessageHandler) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.handlers[msgType] = handler
}

// OnAppTicket 注册应用票据处理器
func (d *MessageDispatcher) OnAppTicket(handler func(msg protocol.AppTicketMessage) bool) {
	d.Register("APP_TICKET", func(message interface{}) (bool, error) {
		return handler(message.(protocol.AppTicketMessage)), nil
	})
}

// OnEntAuthCode 注册企业授权码处理器
func (d *MessageDispatcher) OnEntAuthCode(handler func(msg protocol.EntAuthCodeMessage) bool) {
	d.Register("TEMP_AUTH_CODE", func(message interface{}) (bool, error) {
		return handler(message.(protocol.EntAuthCodeMessage)), nil
	})
}

// Dispatch 执行分发
func (d *MessageDispatcher) Dispatch(frame protocol.EventFrame, decryptKey string) (bool, error) {
	var root map[string]interface{}
	if err := json.Unmarshal([]byte(frame.Payload), &root); err != nil {
		return false, err
	}

	payloadJSON := frame.Payload

	// 1. 自动解密
	if encryptMsg, ok := root["encryptMsg"].(string); ok {
		decrypted, err := crypto.AesDecrypt(encryptMsg, decryptKey)
		if err != nil {
			log.Printf("[MessageDispatcher] Decrypt failed: %v", err)
			return false, err
		}
		payloadJSON = decrypted
		if err := json.Unmarshal([]byte(payloadJSON), &root); err != nil {
			return false, err
		}
	}

	// 2. 路由计算
	msgType, _ := root["msgType"].(string)
	
	// 处理 APP_NOTICE 复合键
	if msgType == "APP_NOTICE" {
		if biz, ok := root["bizContent"].(map[string]interface{}); ok {
			boName, _ := biz["boName"].(string)
			transType, _ := biz["transactionTypeEnum"].(string)
			
			fullKey := fmt.Sprintf("APP_NOTICE:%s:%s", boName, transType)
			boKey := fmt.Sprintf("APP_NOTICE:%s", boName)
			
			d.mu.RLock()
			if _, ok := d.handlers[fullKey]; ok {
				msgType = fullKey
			} else if _, ok := d.handlers[boKey]; ok {
				msgType = boKey
			}
			d.mu.RUnlock()
		}
	}

	d.mu.RLock()
	handler, ok := d.handlers[msgType]
	d.mu.RUnlock()

	if !ok {
		log.Printf("[MessageDispatcher] No handler for msgType: %s. Skipping.", msgType)
		return true, nil
	}

	// 3. 映射到具体结构体并调用处理器
	msgObj, err := d.unmarshalToType(msgType, payloadJSON, frame.Headers)
	if err != nil {
		return false, err
	}

	return handler(msgObj)
}

func (d *MessageDispatcher) unmarshalToType(msgType, payload string, headers map[string]string) (interface{}, error) {
	// 根据 msgType 选择目标类型，这里简化处理，实际可根据注册时的类型反射
	var target interface{}
	
	switch {
	case msgType == "APP_TICKET":
		var m protocol.AppTicketMessage
		json.Unmarshal([]byte(payload), &m)
		m.Headers = headers
		target = m
	case msgType == "TEMP_AUTH_CODE":
		var m protocol.EntAuthCodeMessage
		json.Unmarshal([]byte(payload), &m)
		m.Headers = headers
		target = m
	case msgType == "PAY_ORDER_SUCCESS":
		var m protocol.OrderStatusMessage
		json.Unmarshal([]byte(payload), &m)
		m.Headers = headers
		target = m
	case strings.HasPrefix(msgType, "APP_NOTICE"):
		var m protocol.AppNoticeMessage
		json.Unmarshal([]byte(payload), &m)
		m.Headers = headers
		target = m
	default:
		var m protocol.BaseMessage
		json.Unmarshal([]byte(payload), &m)
		m.Headers = headers
		target = m
	}
	
	return target, nil
}
