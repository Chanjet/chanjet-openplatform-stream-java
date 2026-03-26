package config

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestLoadConfig(t *testing.T) {
	// Set up a temporary home directory for testing
	tempHome, err := os.MkdirTemp("", "cjtCli-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempHome)

	// Mock UserHomeDir for testing
	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tempHome)
	defer os.Setenv("HOME", oldHome)

	configDir := filepath.Join(tempHome, ".cjtCli")
	err = os.MkdirAll(configDir, 0755)
	assert.NoError(t, err)

	// Create a test profile
	configContent := `
app_key: "test-app-key"
certificate: "test-certificate"
log_level: "debug"
`
	profileName := "test-profile"
	err = os.WriteFile(filepath.Join(configDir, profileName+".yaml"), []byte(configContent), 0644)
	assert.NoError(t, err)

	m := NewManager()
	err = m.Load(profileName)
	assert.NoError(t, err)

	conf := m.Get()
	assert.Equal(t, "test-app-key", conf.AppKey)
	assert.Equal(t, "test-certificate", conf.Certificate)
	assert.Equal(t, "debug", conf.LogLevel)
	assert.Equal(t, AppModeSelfBuilt, conf.AppMode) // Default value
}

func TestEnvOverride(t *testing.T) {
	os.Setenv("CJT_APP_KEY", "env-app-key")
	os.Setenv("CJT_CERTIFICATE", "env-certificate")
	defer os.Unsetenv("CJT_APP_KEY")
	defer os.Unsetenv("CJT_CERTIFICATE")

	m := NewManager()
	// Load with non-existent profile to test env only
	err := m.Load("non-existent")
	assert.NoError(t, err)

	conf := m.Get()
	assert.Equal(t, "env-app-key", conf.AppKey)
	assert.Equal(t, "env-certificate", conf.Certificate)
}

func TestWatchConfig(t *testing.T) {
	tempHome, err := os.MkdirTemp("", "cjtCli-watch-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempHome)

	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tempHome)
	defer os.Setenv("HOME", oldHome)

	configDir := filepath.Join(tempHome, ".cjtCli")
	os.MkdirAll(configDir, 0755)

	profileName := "watch-profile"
	configPath := filepath.Join(configDir, profileName+".yaml")
	
	err = os.WriteFile(configPath, []byte("log_level: info"), 0644)
	assert.NoError(t, err)

	m := NewManager()
	err = m.Load(profileName)
	assert.NoError(t, err)
	assert.Equal(t, "info", m.Get().LogLevel)

	updated := make(chan bool, 1)
	m.Watch(func(c *Config) {
		if c.LogLevel == "debug" {
			updated <- true
		}
	})

	// Update the config file
	time.Sleep(100 * time.Millisecond) // Give fsnotify a moment
	err = os.WriteFile(configPath, []byte("log_level: debug"), 0644)
	assert.NoError(t, err)

	select {
	case <-updated:
		// Success
	case <-time.After(2 * time.Second):
		t.Fatal("Config watch timed out")
	}

	assert.Equal(t, "debug", m.Get().LogLevel)
}
