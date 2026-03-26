package proxy

import (
	"bytes"
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/daemon/dlq"
	"fmt"
	"math"
	"net/http"
	"time"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"go.uber.org/zap"
)

// Forwarder handles forwarding events to local webhook targets
type Forwarder interface {
	Forward(event protocol.EventFrame, targetURL string) error
}

type simpleForwarder struct {
	tel    *telemetry.Telemetry
	dlq    dlq.Store
	client *http.Client
}

func NewForwarder(tel *telemetry.Telemetry, dlq dlq.Store) Forwarder {
	return &simpleForwarder{
		tel:    tel,
		dlq:    dlq,
		client: &http.Client{Timeout: 5 * time.Second},
	}
}

func (f *simpleForwarder) Forward(event protocol.EventFrame, targetURL string) error {
	if targetURL == "" {
		return fmt.Errorf("target URL is empty")
	}

	go f.forwardWithRetry(event, targetURL)
	return nil
}

func (f *simpleForwarder) forwardWithRetry(event protocol.EventFrame, targetURL string) {
	maxRetries := 5
	var lastErr error

	for i := 0; i <= maxRetries; i++ {
		if i > 0 {
			delay := time.Duration(math.Pow(2, float64(i-1))) * time.Second
			f.tel.Sys().Warn("Retrying event forward", 
				zap.String("msg_id", event.MsgID), 
				zap.Int("attempt", i), 
				zap.Duration("delay", delay))
			time.Sleep(delay)
		}

		err := f.doForward(event, targetURL)
		if err == nil {
			f.tel.Audit().Info("Event delivered successfully", zap.String("msg_id", event.MsgID))
			return
		}

		lastErr = err
		f.tel.Sys().Warn("Forward attempt failed", zap.String("msg_id", event.MsgID), zap.Error(err))
	}

	// All retries failed, sink to DLQ
	f.tel.DLQ().Error("Event delivery failed, sinking to DLQ", zap.String("msg_id", event.MsgID), zap.Error(lastErr))
	if err := f.dlq.Save(event, lastErr.Error()); err != nil {
		f.tel.Sys().Error("CRITICAL: Failed to save to DLQ", zap.Error(err))
	}
}

func (f *simpleForwarder) doForward(event protocol.EventFrame, targetURL string) error {
	req, err := http.NewRequest("POST", targetURL, bytes.NewBuffer([]byte(event.Payload)))
	if err != nil {
		return err
	}

	// Pass headers
	for k, v := range event.Headers {
		req.Header.Set(k, v)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-MSG-ID", event.MsgID)

	resp, err := f.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("target returned status %d", resp.StatusCode)
	}

	return nil
}
