package stream

import (
	"cjtCli/internal/auth"
	"cjtCli/internal/core/config"
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/daemon/proxy"

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
}

func NewBridge(tel *telemetry.Telemetry, pool auth.TokenPool, forwarder proxy.Forwarder) Bridge {
	return &sdkBridge{
		tel:       tel,
		pool:      pool,
		forwarder: forwarder,
	}
}

func (b *sdkBridge) Start(profile string, cfg *config.Config) {
	options := sdk.ClientOptions{
		AppKey:     cfg.AppKey,
		AppSecret:  cfg.AppSecret,
		EncryptKey: cfg.Certificate,
		GatewayURL: cfg.AuthURL,
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
		b.client.Stop()
		b.tel.Sys().Info("Stream Bridge stopped")
	}
}
