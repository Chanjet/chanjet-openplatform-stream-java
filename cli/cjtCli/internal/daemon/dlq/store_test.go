package dlq

import (
	"os"
	"path/filepath"
	"testing"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	"github.com/stretchr/testify/assert"
)

func TestDLQStore(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "cjtCli-dlq-test-*")
	assert.NoError(t, err)
	defer os.RemoveAll(tempDir)

	dbPath := filepath.Join(tempDir, "dlq.db")
	store, err := NewStore(dbPath)
	assert.NoError(t, err)
	defer store.Close()

	event := protocol.EventFrame{
		MsgID:   "msg-1",
		MsgType: "test-event",
		Payload: "{\"hello\": \"world\"}",
		Headers: map[string]string{"X-Test": "True"},
	}

	err = store.Save(event, "connection refused")
	assert.NoError(t, err)

	entries, err := store.List()
	assert.NoError(t, err)
	assert.Len(t, entries, 1)
	assert.Equal(t, "msg-1", entries[0].MsgID)
	assert.Equal(t, "connection refused", entries[0].Error)

	err = store.Delete(entries[0].ID)
	assert.NoError(t, err)

	entries, _ = store.List()
	assert.Len(t, entries, 0)
}
