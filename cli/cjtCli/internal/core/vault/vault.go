package vault

import (
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/zalando/go-keyring"
)

// Vault defines the interface for managing secrets
type Vault interface {
	Set(profile, key, secret string) error
	Get(profile, key string) (string, error)
	Delete(profile, key string) error
}

type multiVault struct {
	serviceName string
	sealPath    string
	masterKey   []byte
}

// NewVault creates a new vault instance with Keyring and Fallback support
func NewVault(serviceName, sealPath, masterKeyStr string) (Vault, error) {
	if masterKeyStr == "" {
		// Fallback to a simple derivation if not provided
		masterKeyStr = os.Getenv("CJT_MASTER_KEY")
		if masterKeyStr == "" {
			// For TDD and initial implementation, we require it or use a fixed string (NOT SECURE for production)
			// In production, this should be derived from machine ID.
			masterKeyStr = "fallback-default-insecure-key"
		}
	}

	key := sha256.Sum256([]byte(masterKeyStr))

	return &multiVault{
		serviceName: serviceName,
		sealPath:    sealPath,
		masterKey:   key[:],
	}, nil
}

func (v *multiVault) getFullKey(profile, key string) string {
	return fmt.Sprintf("%s:%s", profile, key)
}

func (v *multiVault) Set(profile, key, secret string) error {
	fullKey := v.getFullKey(profile, key)
	// 1. Try Keyring
	err := keyring.Set(v.serviceName, fullKey, secret)
	if err == nil {
		return nil
	}

	// 2. Fallback to Seal File
	return v.setSeal(fullKey, secret)
}

func (v *multiVault) Get(profile, key string) (string, error) {
	fullKey := v.getFullKey(profile, key)
	// 1. Try Keyring
	secret, err := keyring.Get(v.serviceName, fullKey)
	if err == nil {
		return secret, nil
	}

	// 2. Fallback to Seal File
	return v.getSeal(fullKey)
}

func (v *multiVault) Delete(profile, key string) error {
	fullKey := v.getFullKey(profile, key)
	// Try Keyring
	keyring.Delete(v.serviceName, fullKey)

	// Try Seal File (remove from map)
	return v.deleteSeal(fullKey)
}

type sealData map[string]string

func (v *multiVault) readSeal() (sealData, error) {
	if _, err := os.Stat(v.sealPath); os.IsNotExist(err) {
		return make(sealData), nil
	}

	data, err := os.ReadFile(v.sealPath)
	if err != nil {
		return nil, err
	}

	decrypted, err := DecryptAESGCM(data, v.masterKey)
	if err != nil {
		return nil, fmt.Errorf("failed to decrypt seal file: %w", err)
	}

	var res sealData
	if err := json.Unmarshal(decrypted, &res); err != nil {
		return nil, err
	}

	return res, nil
}

func (v *multiVault) writeSeal(data sealData) error {
	raw, err := json.Marshal(data)
	if err != nil {
		return err
	}

	encrypted, err := EncryptAESGCM(raw, v.masterKey)
	if err != nil {
		return err
	}

	// Ensure directory exists
	if err := os.MkdirAll(filepath.Dir(v.sealPath), 0700); err != nil {
		return err
	}

	return os.WriteFile(v.sealPath, encrypted, 0600)
}

func (v *multiVault) setSeal(key, secret string) error {
	data, err := v.readSeal()
	if err != nil {
		return err
	}
	data[key] = secret
	return v.writeSeal(data)
}

func (v *multiVault) getSeal(key string) (string, error) {
	data, err := v.readSeal()
	if err != nil {
		return "", err
	}
	secret, ok := data[key]
	if !ok {
		return "", errors.New("secret not found in seal file")
	}
	return secret, nil
}

func (v *multiVault) deleteSeal(key string) error {
	data, err := v.readSeal()
	if err != nil {
		return err
	}
	delete(data, key)
	return v.writeSeal(data)
}
