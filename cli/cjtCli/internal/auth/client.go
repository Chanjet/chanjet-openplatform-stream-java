package auth

import (
	"bytes"
	"cjtCli/internal/core/config"
	"cjtCli/internal/core/telemetry"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"go.uber.org/zap"
)

type Client interface {
	GetAppAccessToken(profile string, cfg *config.Config) (*Token, error)
	TriggerPush(profile string, cfg *config.Config) error
	// GetOpenApiSpec 获取完整的 OpenAPI 3.0 规范文件
	GetOpenApiSpec(profile string, cfg *config.Config) (interface{}, error)
}

type authClient struct {
	pool    TokenPool
	barrier Barrier
	tel     *telemetry.Telemetry
	client  *http.Client
}

func NewClient(pool TokenPool, barrier Barrier, tel *telemetry.Telemetry) Client {
	return &authClient{
		pool:    pool,
		barrier: barrier,
		tel:     tel,
		client:  &http.Client{Timeout: 10 * time.Second},
	}
}

type appTokenResponse struct {
	Result bool        `json:"result"`
	Error  interface{} `json:"error"`
	Value  struct {
		AccessToken string `json:"accessToken"`
		ExpiresIn   int    `json:"expiresIn"`
	} `json:"value"`
}

func (c *authClient) GetAppAccessToken(profile string, cfg *config.Config) (*Token, error) {
	// 1. Check pool
	token, err := c.pool.GetAccessToken(profile)
	if err == nil && !token.IsExpired() {
		return token, nil
	}

	// 2. Barrier for refresh
	val, err := c.barrier.Do("refresh-app-token:"+profile, func() (interface{}, error) {
		// Double check inside barrier
		token, err := c.pool.GetAccessToken(profile)
		if err == nil && !token.IsExpired() {
			return token, nil
		}

		// 3. Perform network refresh
		ticket, err := c.pool.GetAppTicket(profile)
		if err != nil {
			return nil, fmt.Errorf("missing app_ticket, please ensure daemon is running and app_ticket is received: %w", err)
		}

		url := fmt.Sprintf("%s/v1/common/auth/selfBuiltApp/generateToken", cfg.AuthURL)
		body := map[string]string{
			"appTicket":   ticket.Value,
			"certificate": cfg.Certificate,
		}
		rawBody, _ := json.Marshal(body)

		req, err := http.NewRequest("POST", url, bytes.NewBuffer(rawBody))
		if err != nil {
			return nil, err
		}
		req.Header.Set("appKey", cfg.AppKey)
		req.Header.Set("appSecret", cfg.AppSecret)
		req.Header.Set("Content-Type", "application/json")

		resp, err := c.client.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close()

		respData, _ := io.ReadAll(resp.Body)
		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("platform auth failed (HTTP %d): %s", resp.StatusCode, string(respData))
		}

		var tokenResp appTokenResponse
		if err := json.Unmarshal(respData, &tokenResp); err != nil {
			return nil, err
		}

		if !tokenResp.Result {
			return nil, fmt.Errorf("platform error: %v", tokenResp.Error)
		}

		newToken := &Token{
			Value:     tokenResp.Value.AccessToken,
			ExpiresAt: time.Now().Add(time.Duration(tokenResp.Value.ExpiresIn) * time.Second),
		}

		// 4. Save to pool
		if err := c.pool.SetAccessToken(profile, newToken); err != nil {
			return nil, err
		}

		return newToken, nil
	})

	if err != nil {
		return nil, err
	}
	return val.(*Token), nil
}

func (c *authClient) TriggerPush(profile string, cfg *config.Config) error {
	url := fmt.Sprintf("%s/auth/appTicket/resend", cfg.AuthURL)
	body := map[string]string{}
	rawBody, _ := json.Marshal(body)

	req, err := http.NewRequest("POST", url, bytes.NewBuffer(rawBody))
	if err != nil {
		return err
	}
	req.Header.Set("appKey", cfg.AppKey)
	req.Header.Set("appSecret", cfg.AppSecret)
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	respData, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to trigger push (HTTP %d): %s", resp.StatusCode, string(respData))
	}

	var resendResp struct {
		Code    string `json:"code"`
		Message string `json:"message"`
		Result  string `json:"result"`
	}
	if err := json.Unmarshal(respData, &resendResp); err != nil {
		return fmt.Errorf("failed to parse resend response: %w", err)
	}

	if resendResp.Code != "200" {
		return fmt.Errorf("platform error: %s - %s", resendResp.Code, resendResp.Message)
	}

	c.tel.Sys().Info("AppTicket push triggered successfully", zap.String("profile", profile))
	return nil
}

func (c *authClient) GetOpenApiSpec(profile string, cfg *config.Config) (interface{}, error) {
	// TODO: 等待开放平台 PR 实现 GET /metadata/v1/openapi/spec?app_key={AppKey}
	return nil, fmt.Errorf("interface 'GetOpenApiSpec' is pending platform implementation (PR v0.1.1)")
}
