package cjtCli

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestWebhookCommandHierarchy(t *testing.T) {
	root := rootCmd
	
	// Check if 'webhook' is a root command
	webhookCmd, _, err := root.Find([]string{"webhook"})
	assert.NoError(t, err)
	assert.NotNil(t, webhookCmd)
	assert.Equal(t, "webhook", webhookCmd.Name())

	// Check if 'dlq' is a subcommand of 'webhook'
	dlqCmd, _, err := webhookCmd.Find([]string{"dlq"})
	assert.NoError(t, err)
	assert.NotNil(t, dlqCmd)
	assert.Equal(t, "dlq", dlqCmd.Name())

	// Check if 'list' is a subcommand of 'dlq'
	listCmd, _, err := dlqCmd.Find([]string{"list"})
	assert.NoError(t, err)
	assert.NotNil(t, listCmd)
}

func TestWebhookStartCommand(t *testing.T) {
	root := rootCmd
	webhookCmd, _, _ := root.Find([]string{"webhook"})
	startCmd, _, err := webhookCmd.Find([]string{"start"})
	assert.NoError(t, err)
	assert.NotNil(t, startCmd)
}
