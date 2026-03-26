package cjtCli

import (
	"github.com/spf13/cobra"
)

var webhookCmd = &cobra.Command{
	Use:   "webhook",
	Short: "Manage Stream Connector Webhook lifecycle",
	Long:  `Manage the lifecycle of the Stream Connector Webhook, including starting the listener and managing the DLQ.`,
}

var webhookStartCmd = &cobra.Command{
	Use:   "start",
	Short: "Start the Webhook listener",
	Run: func(cmd *cobra.Command, args []string) {
		// Implementation will be moved from daemon.go
	},
}

func init() {
	webhookCmd.AddCommand(webhookStartCmd)
	rootCmd.AddCommand(webhookCmd)
}
