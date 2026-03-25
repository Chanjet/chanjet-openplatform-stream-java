package sdk

import (
	"context"
	"encoding/json"
	"fmt"
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
	AppKey     string
	AppSecret  string
	EncryptKey string
	GatewayURL string
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
			wsURL := strings.Replace(c.options.GatewayURL, "http", "ws", 1) + 
				fmt.Sprintf("/connect?app_key=%s&nonce=%s&sign=%s&client_id=%s", 
					c.options.AppKey, nonce, sign, c.clientID)

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
	url := strings.Replace(c.options.GatewayURL, "ws", "http", 1) + 
		"/v1/ws/challenge?app_key=" + c.options.AppKey
	
	signPrefix := crypto.HmacSha256(c.options.AppKey, c.options.AppSecret)[:16]
	
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-CJT-PreAuth", signPrefix)

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("http status %d", resp.StatusCode)
	}

	var result struct {
		Data struct {
			Nonce string `json:"nonce"`
		} `json:"data"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", err
	}
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
		sec := math.Min(60, math.Pow(2, float64(c.attempt)))
		delay = time.Duration(sec) * time.Second
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

		msgType := root["msg_type"].(string)
		if msgType == "event" {
			var frame protocol.EventFrame
			json.Unmarshal(message, &frame)
			
			success := false
			if c.dispatcher != nil {
				success, _ = c.dispatcher.Dispatch(frame, c.encryptKey)
			} else if c.eventHandler != nil {
				success, _ = c.eventHandler(frame)
			}
			c.sendAck(frame.MsgID, success)
		} else if msgType == "ping" {
			conn.WriteJSON(map[string]string{"msg_type": "pong"})
		}
	}
}

func (c *GatewayClient) sendAck(msgID string, success bool) {
	code := 200
	msg := "success"
	if !success {
		code = 500
		msg = "failed"
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
