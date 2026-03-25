package main

import (
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"com.chanjet/connector-sdk-go/pkg/sdk"
)

func main() {
	// 实际应从环境变量或配置读取
	options := sdk.ClientOptions{
		AppKey:     "your_app_key",
		AppSecret:  "your_app_secret",
		EncryptKey: "1234567890123456",
		GatewayURL: "https://stream-open-chanapp.inte.chanjet.com",
	}

	// 1. 初始化分发器
	dispatcher := sdk.NewMessageDispatcher()

	// 注册处理器
	dispatcher.OnAppTicket(func(msg protocol.AppTicketMessage) bool {
		fmt.Printf("🎫 [Go Demo] 收到应用票据: %s\n", msg.BizContent.AppTicket)
		return true
	})

	dispatcher.OnEntAuthCode(func(msg protocol.EntAuthCodeMessage) bool {
		fmt.Printf("🔑 [Go Demo] 收到临时授权码: %s\n", msg.BizContent.TempAuthCode)
		return true
	})

	// 2. 创建并启动客户端
	client := sdk.NewGatewayClient(options)
	client.UseDispatcher(dispatcher)

	log.Println("🚀 [Go Demo] 正在启动 Go SDK Demo...")
	client.Start()

	// 3. 优雅退出
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit

	log.Println("Stopping...")
	client.Stop()
}
