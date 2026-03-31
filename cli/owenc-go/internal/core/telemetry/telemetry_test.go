package telemetry

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewTelemetry(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtc-log-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempDir)

	tel, err := NewTelemetry(tempDir, "info")
	assert.NoError(t, err)
	assert.NotNil(t, tel)

	tel.Sys().Info("system start")
	tel.Audit().Info("audit event")
	tel.Stream().Info("stream event")
	tel.DLQ().Info("dlq event")
	tel.Sync()

	// Verify files exist
	assert.FileExists(t, filepath.Join(tempDir, "sys.log"))
	assert.FileExists(t, filepath.Join(tempDir, "audit.log"))
	assert.FileExists(t, filepath.Join(tempDir, "stream.log"))
	assert.FileExists(t, filepath.Join(tempDir, "dlq.log"))
}

func TestFormatOutputJSON(t *testing.T) {
	// Redirect stdout to capture output
	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	data := map[string]string{"foo": "bar"}
	FormatOutput(data, nil, FormatJSON)

	w.Close()
	out, _ := io.ReadAll(r)
	os.Stdout = oldStdout

	var resp Response
	err := json.Unmarshal(out, &resp)
	assert.NoError(t, err)
	assert.True(t, resp.Success)
	
	// Map to concrete type for assertion
	receivedData := resp.Data.(map[string]interface{})
	assert.Equal(t, "bar", receivedData["foo"])
}

func TestFormatOutputError(t *testing.T) {
	// Redirect stdout
	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	execErr := errors.New("missing app_key")
	FormatOutput(nil, execErr, FormatJSON)

	w.Close()
	out, _ := io.ReadAll(r)
	os.Stdout = oldStdout

	var resp Response
	err := json.Unmarshal(out, &resp)
	assert.NoError(t, err)
	assert.False(t, resp.Success)
	assert.Contains(t, resp.Error.Message, "missing app_key")
	assert.Contains(t, resp.Suggestion, "app_key")
}
