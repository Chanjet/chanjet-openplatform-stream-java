package auth

import (
	"cjtc/internal/core/vault"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestTokenPoolPersistence(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtc-pool-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempDir)

	sealPath := filepath.Join(tempDir, ".seal")
	v, _ := vault.NewVault("test", sealPath, "key")
	pool := NewTokenPool(v)

	profile := "test-profile"
	
	// Test Ticket Persistence
	err = pool.SetAppTicket(profile, "ticket-123")
	assert.NoError(t, err)

	ticket, err := pool.GetAppTicket(profile)
	assert.NoError(t, err)
	assert.Equal(t, "ticket-123", ticket.Value)

	// Test AccessToken Persistence
	tok := &Token{
		Value:     "access-123",
		ExpiresAt: time.Now().Add(1 * time.Hour),
	}
	err = pool.SetAccessToken(profile, tok)
	assert.NoError(t, err)

	gotTok, err := pool.GetAccessToken(profile)
	assert.NoError(t, err)
	assert.Equal(t, "access-123", gotTok.Value)
	assert.False(t, gotTok.IsExpired())

	// Create a new pool with the same vault to verify it loads from vault
	pool2 := NewTokenPool(v)
	ticket2, err := pool2.GetAppTicket(profile)
	assert.NoError(t, err)
	assert.Equal(t, "ticket-123", ticket2.Value)
}
