package stream

import (
	"cjtc/internal/auth"
	"cjtc/internal/core/telemetry"
	"cjtc/internal/core/vault"
	"cjtc/internal/daemon/dlq"
	"cjtc/internal/daemon/proxy"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewBridge(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtc-bridge-test-*")
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
