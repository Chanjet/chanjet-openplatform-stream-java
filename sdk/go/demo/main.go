package main

import (
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"com.chanjet/connector-sdk-go/pkg/sdk"
	"github.com/joho/godotenv"
)

func main() {
	// 加载 .env 文件
	if err := godotenv.Load(); err != nil {
		log.Println("No .env file found, using system environment variables")
	}

	appKey := os.Getenv("APP_KEY")
	appSecret := os.Getenv("APP_SECRET")
	encryptKey := os.Getenv("ENCRYPT_KEY")
	gatewayURL := os.Getenv("GATEWAY_URL")

	if appKey == "" || appSecret == "" {
		log.Fatal("❌ [Error] 缺少必要配置：请确保 .env 文件已正确配置 APP_KEY, APP_SECRET")
	}

	options := sdk.ClientOptions{
		AppKey:     appKey,
		AppSecret:  appSecret,
		EncryptKey: encryptKey,
		GatewayURL: gatewayURL,
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
