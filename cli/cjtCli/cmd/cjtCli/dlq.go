package cjtCli

import (
	"cjtCli/internal/daemon/dlq"
	"cjtCli/internal/daemon/proxy"
	"cjtCli/internal/core/telemetry"
	"fmt"
	"strconv"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"github.com/spf13/cobra"
)

var dlqCmd = &cobra.Command{
	Use:   "dlq",
	Short: "Manage the Dead-Letter Queue (DLQ)",
}

var dlqListCmd = &cobra.Command{
	Use:   "list",
	Short: "List all entries in the DLQ",
	Run: func(cmd *cobra.Command, args []string) {
		s, err := dlq.NewStore("")
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}
		defer s.Close()

		entries, err := s.List()
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		telemetry.FormatOutput(entries, nil, telemetry.OutputFormat(format))
	},
}

var dlqRetryCmd = &cobra.Command{
	Use:   "retry <id>",
	Short: "Retry a specific event from the DLQ",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		id, err := strconv.ParseInt(args[0], 10, 64)
		if err != nil {
			telemetry.FormatOutput(nil, fmt.Errorf("invalid DLQ ID: %s", args[0]), telemetry.OutputFormat(format))
			return
		}

		s, err := dlq.NewStore("")
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}
		defer s.Close()

		entries, err := s.List()
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		var entry *dlq.DLQEntry
		for _, e := range entries {
			if e.ID == id {
				entry = &e
				break
			}
		}

		if entry == nil {
			telemetry.FormatOutput(nil, fmt.Errorf("DLQ entry %d not found", id), telemetry.OutputFormat(format))
			return
		}

		// Perform Retry
		conf := cfgMgr.Get()
		if conf.WebhookTarget == "" {
			telemetry.FormatOutput(nil, fmt.Errorf("webhook_target is not configured"), telemetry.OutputFormat(format))
			return
		}

		forwarder := proxy.NewForwarder(tel, s)
		event := protocol.EventFrame{
			MsgID:   entry.MsgID,
			MsgType: entry.MsgType,
			Payload: entry.Payload,
		}
		// Note: headers are simplified here, in production we would unmarshal entry.Headers
		
		err = forwarder.Forward(event, conf.WebhookTarget)
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// Delete from DLQ
		s.Delete(id)

		res := map[string]interface{}{
			"id":      id,
			"status":  "retrying",
			"message": "Retry triggered. Check audit log for results.",
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
	},
}

func init() {
	dlqCmd.AddCommand(dlqListCmd)
	dlqCmd.AddCommand(dlqRetryCmd)
	rootCmd.AddCommand(dlqCmd)
}
