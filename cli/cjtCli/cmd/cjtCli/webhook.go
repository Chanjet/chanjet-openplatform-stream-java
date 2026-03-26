package cjtCli

import (
	"github.com/spf13/cobra"
)

var webhookCmd = &cobra.Command{
	Use:   "webhook",
	Short: "管理 Stream Connector Webhook 生命周期",
	Long:  `管理 Stream Connector Webhook 的生命周期，包括启动本地监听器以及死信队列 (DLQ) 的管理。`,
}

var webhookStartCmd = &cobra.Command{
	Use:   "start",
	Short: "启动 Webhook 本地监听器",
	Run: func(cmd *cobra.Command, args []string) {
		// Implementation will be moved from daemon.go
	},
}

func init() {
	webhookCmd.AddCommand(webhookStartCmd)
	rootCmd.AddCommand(webhookCmd)
}
