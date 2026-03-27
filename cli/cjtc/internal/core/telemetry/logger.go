package telemetry

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/natefinch/lumberjack"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

type Domain string

const (
	DomainSys    Domain = "sys"
	DomainAudit  Domain = "audit"
	DomainStream Domain = "stream"
	DomainDLQ    Domain = "dlq"
)

type Telemetry struct {
	Loggers map[Domain]*zap.Logger
}

func NewTelemetry(logDir string, logLevel string) (*Telemetry, error) {
	if logDir == "" {
		home, _ := os.UserHomeDir()
		logDir = filepath.Join(home, ".cjtc", "log")
	}

	if err := os.MkdirAll(logDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create log directory: %w", err)
	}

	var zapLevel zapcore.Level
	if err := zapLevel.UnmarshalText([]byte(logLevel)); err != nil {
		zapLevel = zap.InfoLevel
	}

	loggers := make(map[Domain]*zap.Logger)
	domains := []Domain{DomainSys, DomainAudit, DomainStream, DomainDLQ}

	for _, d := range domains {
		l, err := createLogger(logDir, d, zapLevel)
		if err != nil {
			return nil, err
		}
		loggers[d] = l
	}

	return &Telemetry{Loggers: loggers}, nil
}

func createLogger(logDir string, domain Domain, level zapcore.Level) (*zap.Logger, error) {
	fileName := string(domain) + ".log"
	filePath := filepath.Join(logDir, fileName)

	w := zapcore.AddSync(&lumberjack.Logger{
		Filename:   filePath,
		MaxSize:    500, // megabytes
		MaxBackups: 3,
		MaxAge:     28, // days
	})

	encoderConfig := zap.NewProductionEncoderConfig()
	encoderConfig.EncodeTime = zapcore.ISO8601TimeEncoder
	
	core := zapcore.NewCore(
		zapcore.NewJSONEncoder(encoderConfig),
		w,
		level,
	)

	return zap.New(core), nil
}

func (t *Telemetry) Sys() *zap.Logger    { return t.Loggers[DomainSys] }
func (t *Telemetry) Audit() *zap.Logger  { return t.Loggers[DomainAudit] }
func (t *Telemetry) Stream() *zap.Logger { return t.Loggers[DomainStream] }
func (t *Telemetry) DLQ() *zap.Logger    { return t.Loggers[DomainDLQ] }

func (t *Telemetry) Sync() {
	for _, l := range t.Loggers {
		l.Sync()
	}
}

func Err(err error) zap.Field {
	return zap.Error(err)
}

func ZapError(err error) zap.Field {
	return zap.Error(err)
}

func ZapString(key, val string) zap.Field {
	return zap.String(key, val)
}

func ZapInt(key string, val int) zap.Field {
	return zap.Int(key, val)
}
