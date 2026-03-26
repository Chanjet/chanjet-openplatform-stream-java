package auth

import "time"

// Token represents an Access Token
type Token struct {
	Value     string    `json:"access_token"`
	ExpiresAt time.Time `json:"expires_at"`
}

func (t *Token) IsExpired() bool {
	// 5-minute buffer
	return time.Now().Add(5 * time.Minute).After(t.ExpiresAt)
}

// Ticket represents an App Ticket
type Ticket struct {
	Value     string    `json:"app_ticket"`
	CreatedAt time.Time `json:"created_at"`
}

func (t *Ticket) IsExpired() bool {
	// App tickets usually last 24h, but platform recommends 10min refresh.
	// We check for 20 minutes as a safe boundary for proactive refresh.
	return time.Since(t.CreatedAt) > 20*time.Minute
}
