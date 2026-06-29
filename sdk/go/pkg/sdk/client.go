package sdk

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"math"
	"math/rand"
	"net/http"
	"os"
	"strings"
	"sync"
	"time"

	"com.chanjet/connector-sdk-go/pkg/crypto"
	"com.chanjet/connector-sdk-go/pkg/protocol"
	"github.com/gorilla/websocket"
)

// ClientOptions 客户端配置选项
type ClientOptions struct {
	AppKey            string
	AppSecret         string
	EncryptKey        string
	GatewayURL        string
	ReconnectInterval time.Duration
	MaxBackoff        time.Duration
	Exclusive         bool
	DlqProvider       DlqProvider
}

// EventHandler 原始事件处理器
type EventHandler func(event protocol.EventFrame) (bool, error)

// GatewayClient 畅捷通 Stream Gateway 客户端
type GatewayClient struct {
	options    ClientOptions
	encryptKey string
	clientID   string
	
	conn       *websocket.Conn
	running    bool
	mu         sync.Mutex
	cancel     context.CancelFunc
	
	eventHandler EventHandler
	dispatcher   *MessageDispatcher
	attempt      int
}

// NewGatewayClient 创建客户端实例
func NewGatewayClient(options ClientOptions) *GatewayClient {
	hostname, _ := os.Hostname()
	pid := os.Getpid()
	random := rand.Intn(1000000)
	clientID := fmt.Sprintf("%s@%s_%d_%d", options.AppKey, hostname, pid, random)

	encryptKey := options.EncryptKey
	if encryptKey == "" {
		encryptKey = options.AppSecret
	}

	if options.ReconnectInterval == 0 {
		options.ReconnectInterval = time.Second
	}
	if options.MaxBackoff == 0 {
		options.MaxBackoff = 60 * time.Second
	}

	return &GatewayClient{
		options:    options,
		encryptKey: encryptKey,
		clientID:   clientID,
	}
}

func (c *GatewayClient) OnEvent(handler EventHandler) {
	c.eventHandler = handler
}

func (c *GatewayClient) UseDispatcher(d *MessageDispatcher) {
	c.dispatcher = d
}

func (c *GatewayClient) Start() {
	c.mu.Lock()
	if c.running {
		c.mu.Unlock()
		return
	}
	c.running = true
	ctx, cancel := context.WithCancel(context.Background())
	c.cancel = cancel
	c.mu.Unlock()

	go c.connectLoop(ctx)
}

func (c *GatewayClient) Stop() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.running = false
	if c.cancel != nil {
		c.cancel()
	}
	if c.conn != nil {
		c.conn.Close()
	}
}

func (c *GatewayClient) connectLoop(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		default:
			log.Printf("[GatewayClient] Attempting to connect (Attempt: %d)...", c.attempt+1)
			
			nonce, err := c.fetchNonce()
			if err != nil {
				log.Printf("[GatewayClient] Fetch nonce failed: %v", err)
				c.handleReconnect(503)
				continue
			}

			sign := crypto.HmacSha256(c.options.AppKey+"&"+nonce, c.options.AppSecret)
			
			wsURL := c.options.GatewayURL
			if strings.HasPrefix(wsURL, "http://") {
				wsURL = strings.Replace(wsURL, "http://", "ws://", 1)
			} else if strings.HasPrefix(wsURL, "https://") {
				wsURL = strings.Replace(wsURL, "https://", "wss://", 1)
			}
			
			if !strings.HasSuffix(wsURL, "/connect") {
				wsURL = strings.TrimRight(wsURL, "/") + "/connect"
			}
			
			wsURL += fmt.Sprintf("?app_key=%s&nonce=%s&sign=%s&client_id=%s", 
					c.options.AppKey, nonce, sign, c.clientID)
			
			if c.options.Exclusive {
				wsURL += "&exclusive=true"
			}

			log.Printf("[GatewayClient] Dialing WebSocket: %s", wsURL)
			conn, resp, err := websocket.DefaultDialer.Dial(wsURL, nil)
			if err != nil {
				statusCode := 503
				if resp != nil {
					statusCode = resp.StatusCode
				}
				log.Printf("[GatewayClient] Connection failed (%d): %v", statusCode, err)
				c.handleReconnect(statusCode)
				continue
			}

			c.mu.Lock()
			c.conn = conn
			c.mu.Unlock()
			c.attempt = 0
			log.Printf("[GatewayClient] WebSocket connected.")

			c.readLoop(ctx, conn)
		}
	}
}

func (c *GatewayClient) fetchNonce() (string, error) {
	baseURL := c.options.GatewayURL
	if strings.HasPrefix(baseURL, "ws://") {
		baseURL = strings.Replace(baseURL, "ws://", "http://", 1)
	} else if strings.HasPrefix(baseURL, "wss://") {
		baseURL = strings.Replace(baseURL, "wss://", "https://", 1)
	}

	url := baseURL + "/v1/ws/challenge?app_key=" + c.options.AppKey
	
	signPrefix := crypto.HmacSha256(c.options.AppKey, c.options.AppSecret)[:16]
	
	log.Printf("[GatewayClient] Fetching nonce from: %s", url)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-CJT-PreAuth", signPrefix)
	req.Header.Set("User-Agent", "cjtCli-Go-SDK/0.1.0")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		log.Printf("[GatewayClient] Nonce request error (Transport level): %v", err)
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		respData, _ := io.ReadAll(resp.Body)
		log.Printf("[GatewayClient] Nonce request failed (HTTP %d): %s", resp.StatusCode, string(respData))
		return "", fmt.Errorf("http status %d", resp.StatusCode)
	}

	var result struct {
		Data struct {
			Nonce string `json:"nonce"`
		} `json:"data"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		log.Printf("[GatewayClient] Nonce decode error: %v", err)
		return "", err
	}
	log.Printf("[GatewayClient] Nonce received: %s", result.Data.Nonce)
	return result.Data.Nonce, nil
}

func (c *GatewayClient) handleReconnect(statusCode int) {
	if statusCode == 401 || statusCode == 403 {
		log.Printf("[GatewayClient] Auth failed (%d). Permanent failure, stopping.", statusCode)
		c.Stop()
		return
	}

	var delay time.Duration
	if statusCode == 503 || statusCode == 429 {
		delay = time.Duration(5+rand.Intn(10)) * time.Second
		log.Printf("[GatewayClient] Gateway busy (%d), standby mode. Reconnect in %v", statusCode, delay)
	} else {
		sec := math.Pow(2, math.Min(float64(c.attempt), 6))
		delay = time.Duration(sec) * time.Second
		if delay < c.options.ReconnectInterval {
			delay = c.options.ReconnectInterval
		}
		if delay > c.options.MaxBackoff {
			delay = c.options.MaxBackoff
		}
		c.attempt++
		log.Printf("[GatewayClient] Connection failed (%d), backoff mode. Reconnect in %v", statusCode, delay)
	}

	time.Sleep(delay)
}

func (c *GatewayClient) readLoop(ctx context.Context, conn *websocket.Conn) {
	defer conn.Close()
	for {
		_, message, err := conn.ReadMessage()
		if err != nil {
			log.Printf("[GatewayClient] Read error: %v", err)
			return
		}

		var root map[string]interface{}
		if err := json.Unmarshal(message, &root); err != nil {
			continue
		}

		msgType, _ := root["msg_type"].(string)
		if msgType == "event" {
			var frame protocol.EventFrame
			json.Unmarshal(message, &frame)
			
			dlqStored := false
			if c.options.DlqProvider != nil {
				if err := c.options.DlqProvider.Store(frame.MsgID, string(message)); err != nil {
					log.Printf("[GatewayClient] DLQ store failed: %v", err)
					c.sendAck(frame.MsgID, false, "DLQ store failed: "+err.Error())
					continue
				}
				dlqStored = true
			}
			
			success := false
			if c.dispatcher != nil {
				success, _ = c.dispatcher.Dispatch(frame, c.encryptKey)
			} else if c.eventHandler != nil {
				success, _ = c.eventHandler(frame)
			}
			
			if success && dlqStored {
				if err := c.options.DlqProvider.Remove(frame.MsgID); err != nil {
					log.Printf("[GatewayClient] DLQ remove failed: %v", err)
				}
			}
			
			c.sendAck(frame.MsgID, success, "")
		} else if msgType == "ping" {
			c.mu.Lock()
			if c.conn != nil {
				c.conn.WriteJSON(map[string]string{"msg_type": "pong"})
			}
			c.mu.Unlock()
		} else if msgType != "" {
			// Handle top-level system messages (e.g. APP_TICKET)
			msgID, _ := root["msg_id"].(string)
			if msgID == "" {
				msgID, _ = root["msgId"].(string)
			}
			
			if c.dispatcher != nil {
				success, _ := c.dispatcher.DispatchValue(root, string(message), nil)
				if msgID != "" {
					c.sendAck(msgID, success, "")
				}
			}
		}
	}
}

func (c *GatewayClient) sendAck(msgID string, success bool, errorMessage string) {
	code := 200
	msg := "success"
	if !success {
		code = 500
		msg = "failed"
		if errorMessage != "" {
			msg = errorMessage
		}
	}
	ack := protocol.AckFrame{
		MsgID:     msgID,
		Code:      code,
		Message:   msg,
		Timestamp: time.Now().UnixMilli(),
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.conn != nil {
		c.conn.WriteJSON(ack)
	}
}
