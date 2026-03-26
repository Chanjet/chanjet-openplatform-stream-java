package stream

import (
	"cjtCli/internal/auth"
	"cjtCli/internal/core/config"
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/daemon/proxy"
	"sync"
	"time"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"com.chanjet/connector-sdk-go/pkg/sdk"
	"go.uber.org/zap"
)

// Bridge wraps the SDK GatewayClient and integrates it with CLI core
type Bridge interface {
	Start(profile string, cfg *config.Config)
	Stop()
}

type sdkBridge struct {
	client    *sdk.GatewayClient
	tel       *telemetry.Telemetry
	pool      auth.TokenPool
	forwarder proxy.Forwarder
	
	wg        sync.WaitGroup
	stopping  bool
	mu        sync.Mutex
}

func NewBridge(tel *telemetry.Telemetry, pool auth.TokenPool, forwarder proxy.Forwarder) Bridge {
	return &sdkBridge{
		tel:       tel,
		pool:      pool,
		forwarder: forwarder,
	}
}

func (b *sdkBridge) Start(profile string, cfg *config.Config) {
	encryptKey := cfg.EncryptKey
	if encryptKey == "" {
		encryptKey = cfg.Certificate
	}

	options := sdk.ClientOptions{
		AppKey:     cfg.AppKey,
		AppSecret:  cfg.AppSecret,
		EncryptKey: encryptKey,
		GatewayURL: cfg.StreamURL,
	}

	b.client = sdk.NewGatewayClient(options)
	
	dispatcher := sdk.NewMessageDispatcher()
	
	// Handle AppTicket updates automatically
	dispatcher.OnAppTicket(func(msg protocol.AppTicketMessage) bool {
		b.tel.Sys().Info("Received AppTicket update via Stream", 
			zap.String("profile", profile))
		
		err := b.pool.SetAppTicket(profile, msg.BizContent.AppTicket)
		if err != nil {
			b.tel.Sys().Error("Failed to save AppTicket to pool", zap.Error(err))
			return false
		}
		return true
	})

	// Handle generic events (Webhooks)
	b.client.OnEvent(func(event protocol.EventFrame) (bool, error) {
		b.mu.Lock()
		if b.stopping {
			b.mu.Unlock()
			b.tel.Sys().Warn("Dropping event because bridge is stopping", 
				zap.String("msg_id", event.MsgID))
			return false, nil
		}
		b.wg.Add(1)
		b.mu.Unlock()
		defer b.wg.Done()

		b.tel.Stream().Info("Received EventFrame", 
			zap.String("msg_id", event.MsgID),
			zap.String("msg_type", event.MsgType))
		
		if cfg.WebhookTarget != "" {
			b.forwarder.Forward(event, cfg.WebhookTarget)
		} else {
			b.tel.Sys().Warn("Received event but no webhook_target is configured", zap.String("msg_id", event.MsgID))
		}
		return true, nil
	})

	b.client.UseDispatcher(dispatcher)
	
	b.tel.Sys().Info("Starting Stream Bridge", zap.String("profile", profile))
	b.client.Start()
}

func (b *sdkBridge) Stop() {
	if b.client != nil {
		b.mu.Lock()
		b.stopping = true
		b.mu.Unlock()

		b.tel.Sys().Info("Waiting for pending Webhook transactions to complete (Timeout 5s)...")
		
		// Graceful wait channel
		done := make(chan struct{})
		go func() {
			b.wg.Wait()
			close(done)
		} ()

		select {
		case <-done:
			b.tel.Sys().Info("All pending transactions completed.")
		case <-time.After(5 * time.Second):
			b.tel.Sys().Warn("Graceful shutdown timeout. Some transactions may be lost.")
		}

		b.client.Stop()
		b.tel.Sys().Info("Stream Bridge stopped")
	}
}
