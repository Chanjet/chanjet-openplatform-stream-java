package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/fsnotify/fsnotify"
	"github.com/spf13/viper"
)

// AppMode defines the application operation mode
type AppMode string

const (
	AppModeSelfBuilt AppMode = "self-built"
)

var (
	// DefaultOpenApiURL 可以在编译时通过 -ldflags "-X ..." 注入
	DefaultOpenApiURL = "https://openapi.chanjet.com"
	// DefaultStreamURL 可以在编译时通过 -ldflags "-X ..." 注入
	DefaultStreamURL = "https://stream-open.chanapp.chanjet.com"
)

// Config represents the core configuration model
type Config struct {
	AppKey        string  `mapstructure:"app_key" json:"app_key" yaml:"app_key"`
	AppSecret     string  `mapstructure:"-" json:"-" yaml:"-"` // Not stored in yaml/json config
	Certificate   string  `mapstructure:"-" json:"-" yaml:"-"` // Now stored in Vault
	EncryptKey    string  `mapstructure:"-" json:"-" yaml:"-"` // Now stored in Vault
	AppMode       AppMode `mapstructure:"app_mode" json:"app_mode" yaml:"app_mode"`
	LogLevel      string  `mapstructure:"log_level" json:"log_level" yaml:"log_level"`
	OpenApiURL    string  `mapstructure:"openapi_url" json:"openapi_url" yaml:"openapi_url"`
	StreamURL     string  `mapstructure:"stream_url" json:"stream_url" yaml:"stream_url"`
	WebhookTarget string  `mapstructure:"webhook_target" json:"webhook_target" yaml:"webhook_target"`
}


// Manager defines the interface for configuration management
type Manager interface {
	Get() *Config
	Load(profile string) error
	Save(profile string) error
	Delete(profile string) error
	CreateEmpty(profile string) error
	Watch(onUpdate func(*Config))
}

type viperManager struct {
	v      *viper.Viper
	config *Config
}

// NewManager creates a new configuration manager
func NewManager() Manager {
	return &viperManager{
		v:      viper.New(),
		config: &Config{},
	}
}

func (m *viperManager) Get() *Config {
	return m.config
}

func (m *viperManager) Delete(profile string) error {
	home, _ := os.UserHomeDir()
	path := filepath.Join(home, ".cjtCli", profile+".yaml")
	if _, err := os.Stat(path); err == nil {
		return os.Remove(path)
	}
	return nil
}

func (m *viperManager) Save(profile string) error {
	m.v.Set("app_key", m.config.AppKey)
	m.v.Set("app_mode", m.config.AppMode)
	m.v.Set("log_level", m.config.LogLevel)
	m.v.Set("openapi_url", m.config.OpenApiURL)
	m.v.Set("stream_url", m.config.StreamURL)
	m.v.Set("webhook_target", m.config.WebhookTarget)

	// Ensure config file path is set
	if m.v.ConfigFileUsed() == "" {
		home, _ := os.UserHomeDir()
		m.v.SetConfigFile(filepath.Join(home, ".cjtCli", profile+".yaml"))
	}

	return m.v.WriteConfig()
}

func (m *viperManager) CreateEmpty(profile string) error {
	home, _ := os.UserHomeDir()
	configDir := filepath.Join(home, ".cjtCli")
	os.MkdirAll(configDir, 0755)

	path := filepath.Join(configDir, profile+".yaml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return os.WriteFile(path, []byte("{}"), 0644)
	}
	return nil
}

func (m *viperManager) Load(profile string) error {
	if profile == "" {
		profile = "default"
	}

	home, err := os.UserHomeDir()
	if err != nil {
		return fmt.Errorf("failed to get home directory: %w", err)
	}

	configDir := filepath.Join(home, ".cjtCli")
	if _, err := os.Stat(configDir); os.IsNotExist(err) {
		if err := os.MkdirAll(configDir, 0755); err != nil {
			return fmt.Errorf("failed to create config directory: %w", err)
		}
	}

	m.v.SetConfigName(profile)
	m.v.SetConfigType("yaml")
	m.v.AddConfigPath(configDir)

	// Set defaults
	m.v.SetDefault("app_mode", AppModeSelfBuilt)
	m.v.SetDefault("log_level", "info")
	m.v.SetDefault("openapi_url", DefaultOpenApiURL)
	m.v.SetDefault("stream_url", DefaultStreamURL)

	// Read config file
	if err := m.v.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); ok {
			// Config file not found; ignore error if you want to use defaults
		} else {
			return fmt.Errorf("failed to read config file: %w", err)
		}
	} else {
		m.v.SetConfigFile(m.v.ConfigFileUsed())
	}

	// Automatic env vars
	m.v.SetEnvPrefix("CJT")
	m.v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	m.v.AutomaticEnv()

	// Explicitly bind to struct fields to ensure env vars work without config file
	m.v.BindEnv("app_key")
	m.v.BindEnv("certificate")
	m.v.BindEnv("app_mode")
	m.v.BindEnv("log_level")
	m.v.BindEnv("openapi_url")
	m.v.BindEnv("stream_url")

	if err := m.v.Unmarshal(m.config); err != nil {
		return fmt.Errorf("failed to unmarshal config: %w", err)
	}

	return nil
}

func (m *viperManager) Watch(onUpdate func(*Config)) {
	m.v.OnConfigChange(func(e fsnotify.Event) {
		newConfig := &Config{}
		if err := m.v.Unmarshal(newConfig); err == nil {
			m.config = newConfig
			if onUpdate != nil {
				onUpdate(newConfig)
			}
		}
	})
	m.v.WatchConfig()
}
