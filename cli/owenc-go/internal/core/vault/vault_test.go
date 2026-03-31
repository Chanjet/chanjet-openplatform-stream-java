package vault

import (
	"crypto/sha256"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestAESGCM(t *testing.T) {
	key := sha256.Sum256([]byte("test-key"))
	plaintext := []byte("secret-message")

	ciphertext, err := EncryptAESGCM(plaintext, key[:])
	assert.NoError(t, err)
	assert.NotEqual(t, plaintext, ciphertext)

	decrypted, err := DecryptAESGCM(ciphertext, key[:])
	assert.NoError(t, err)
	assert.Equal(t, plaintext, decrypted)

	// Test with wrong key
	wrongKey := sha256.Sum256([]byte("wrong-key"))
	_, err = DecryptAESGCM(ciphertext, wrongKey[:])
	assert.Error(t, err)
}

func TestVaultFallback(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtc-vault-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempDir)

	sealPath := filepath.Join(tempDir, ".seal")
	// Use a dummy service name to force Keyring failure if possible, 
	// or just let it succeed if Keyring is available. 
	// The point is to test that if it fails, it uses the seal file.
	v, err := NewVault("cjtc-test-service", sealPath, "test-master-key")
	assert.NoError(t, err)

	profile := "shop-A"
	key := "app_secret"
	secret := "top-secret-123"

	// Test Set
	err = v.Set(profile, key, secret)
	assert.NoError(t, err)

	// Test Get
	got, err := v.Get(profile, key)
	assert.NoError(t, err)
	assert.Equal(t, secret, got)

	// Verify seal file exists if Keyring failed (or just check functionality)
	// In most CI environments, Keyring fails and it falls back to seal file.
	// Let's force check the seal file content if it exists.
	if _, err := os.Stat(sealPath); err == nil {
		t.Log("Verified: Fallback seal file was created.")
		
		// Create a new vault instance with the same seal path and key to test persistence
		v2, err := NewVault("cjtc-test-service", sealPath, "test-master-key")
		assert.NoError(t, err)
		got2, err := v2.Get(profile, key)
		assert.NoError(t, err)
		assert.Equal(t, secret, got2)
	}

	// Test Delete
	err = v.Delete(profile, key)
	assert.NoError(t, err)
	_, err = v.Get(profile, key)
	assert.Error(t, err)
}
