package stream

import (
	"cjtCli/internal/auth"
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/core/vault"
	"cjtCli/internal/daemon/dlq"
	"cjtCli/internal/daemon/proxy"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewBridge(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtCli-bridge-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempDir)

	tel, _ := telemetry.NewTelemetry(filepath.Join(tempDir, "log"), "info")
	v, _ := vault.NewVault("test", filepath.Join(tempDir, ".seal"), "key")
	pool := auth.NewTokenPool(v)
	dlqStore, _ := dlq.NewStore(filepath.Join(tempDir, "dlq.db"))
	forwarder := proxy.NewForwarder(tel, dlqStore)

	bridge := NewBridge(tel, pool, forwarder)
	assert.NotNil(t, bridge)
}
