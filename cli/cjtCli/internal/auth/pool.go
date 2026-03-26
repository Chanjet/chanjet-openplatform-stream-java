package auth

import (
	"cjtCli/internal/core/vault"
	"encoding/json"
	"sync"
	"time"
)

type TokenPool interface {
	GetAppTicket(profile string) (*Ticket, error)
	SetAppTicket(profile string, ticket string) error
	GetAccessToken(profile string) (*Token, error)
	SetAccessToken(profile string, token *Token) error
}

type vaultTokenPool struct {
	v        vault.Vault
	mu       sync.RWMutex
	tickets  map[string]*Ticket
	tokens   map[string]*Token
}

func NewTokenPool(v vault.Vault) TokenPool {
	return &vaultTokenPool{
		v:       v,
		tickets: make(map[string]*Ticket),
		tokens:  make(map[string]*Token),
	}
}

func (p *vaultTokenPool) GetAppTicket(profile string) (*Ticket, error) {
	p.mu.RLock()
	if t, ok := p.tickets[profile]; ok {
		p.mu.RUnlock()
		return t, nil
	}
	p.mu.RUnlock()

	// Load from Vault
	val, err := p.v.Get(profile, "app_ticket")
	if err != nil {
		return nil, err
	}

	var t Ticket
	if err := json.Unmarshal([]byte(val), &t); err != nil {
		return nil, err
	}

	p.mu.Lock()
	p.tickets[profile] = &t
	p.mu.Unlock()

	return &t, nil
}

func (p *vaultTokenPool) SetAppTicket(profile string, ticket string) error {
	t := &Ticket{
		Value:     ticket,
		CreatedAt: time.Now(),
	}

	raw, _ := json.Marshal(t)
	if err := p.v.Set(profile, "app_ticket", string(raw)); err != nil {
		return err
	}

	p.mu.Lock()
	p.tickets[profile] = t
	p.mu.Unlock()
	return nil
}

func (p *vaultTokenPool) GetAccessToken(profile string) (*Token, error) {
	p.mu.RLock()
	if t, ok := p.tokens[profile]; ok {
		p.mu.RUnlock()
		return t, nil
	}
	p.mu.RUnlock()

	// Load from Vault
	val, err := p.v.Get(profile, "access_token")
	if err != nil {
		return nil, err
	}

	var t Token
	if err := json.Unmarshal([]byte(val), &t); err != nil {
		return nil, err
	}

	p.mu.Lock()
	p.tokens[profile] = &t
	p.mu.Unlock()

	return &t, nil
}

func (p *vaultTokenPool) SetAccessToken(profile string, token *Token) error {
	raw, _ := json.Marshal(token)
	if err := p.v.Set(profile, "access_token", string(raw)); err != nil {
		return err
	}

	p.mu.Lock()
	p.tokens[profile] = token
	p.mu.Unlock()
	return nil
}
