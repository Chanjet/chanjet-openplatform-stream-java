package dlq

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"com.chanjet/connector-sdk-go/pkg/protocol"
	_ "modernc.org/sqlite"
)

// DLQEntry represents a failed event in the Dead-Letter Queue
type DLQEntry struct {
	ID        int64              `json:"id"`
	MsgID     string             `json:"msg_id"`
	MsgType   string             `json:"msg_type"`
	Payload   string             `json:"payload"`
	Headers   string             `json:"headers"`
	Error     string             `json:"error"`
	CreatedAt time.Time          `json:"created_at"`
	Attempts  int                `json:"attempts"`
}

type Store interface {
	Save(event protocol.EventFrame, errStr string) error
	List() ([]DLQEntry, error)
	Delete(id int64) error
	Close() error
}

type sqliteStore struct {
	db *sql.DB
}

func NewStore(dbPath string) (Store, error) {
	if dbPath == "" {
		home, _ := os.UserHomeDir()
		dbPath = filepath.Join(home, ".cjtCli", "dlq.db")
	}

	if err := os.MkdirAll(filepath.Dir(dbPath), 0755); err != nil {
		return nil, err
	}

	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open sqlite: %w", err)
	}

	s := &sqliteStore{db: db}
	if err := s.init(); err != nil {
		return nil, err
	}

	return s, nil
}

func (s *sqliteStore) init() error {
	query := `
	CREATE TABLE IF NOT EXISTS dlq (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		msg_id TEXT,
		msg_type TEXT,
		payload TEXT,
		headers TEXT,
		error TEXT,
		created_at DATETIME,
		attempts INTEGER
	);`
	_, err := s.db.Exec(query)
	return err
}

func (s *sqliteStore) Save(event protocol.EventFrame, errStr string) error {
	headers, _ := json.Marshal(event.Headers)
	query := `INSERT INTO dlq (msg_id, msg_type, payload, headers, error, created_at, attempts) VALUES (?, ?, ?, ?, ?, ?, ?)`
	_, err := s.db.Exec(query, event.MsgID, event.MsgType, event.Payload, string(headers), errStr, time.Now(), 1)
	return err
}

func (s *sqliteStore) List() ([]DLQEntry, error) {
	query := `SELECT id, msg_id, msg_type, payload, headers, error, created_at, attempts FROM dlq ORDER BY created_at DESC`
	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entries []DLQEntry
	for rows.Next() {
		var e DLQEntry
		var headers string
		if err := rows.Scan(&e.ID, &e.MsgID, &e.MsgType, &e.Payload, &headers, &e.Error, &e.CreatedAt, &e.Attempts); err != nil {
			return nil, err
		}
		e.Headers = headers
		entries = append(entries, e)
	}
	return entries, nil
}

func (s *sqliteStore) Delete(id int64) error {
	_, err := s.db.Exec("DELETE FROM dlq WHERE id = ?", id)
	return err
}

func (s *sqliteStore) Close() error {
	return s.db.Close()
}
