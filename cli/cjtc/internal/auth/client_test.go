package auth

import (
	"cjtc/internal/core/config"
	"cjtc/internal/core/telemetry"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// mockPool 简单的 TokenPool 实现用于测试
type mockPool struct {
	token *Token
}

func (m *mockPool) GetAppTicket(profile string) (*Ticket, error) { return nil, nil }
func (m *mockPool) SetAppTicket(profile string, ticket string) error { return nil }
func (m *mockPool) GetAccessToken(profile string) (*Token, error)    { return m.token, nil }
func (m *mockPool) SetAccessToken(profile string, token *Token) error {
	m.token = token
	return nil
}

// mockBarrier 简单的 Barrier 实现用于测试
type mockBarrier struct{}

func (m *mockBarrier) Do(key string, fn func() (interface{}, error)) (interface{}, error) {
	return fn()
}

func TestGetOpenApiSpec_MockData(t *testing.T) {
	// 1. 设置测试环境
	home, _ := os.UserHomeDir()
	cacheDir := filepath.Join(home, ".cjtc")
	profile := "test-tdd-profile"
	cachePath := filepath.Join(cacheDir, profile+"_openapi.json")
	
	// 清理之前的缓存
	_ = os.Remove(cachePath)
	_ = os.Remove(cachePath + ".idx")

	tel, err := telemetry.NewTelemetry("", "debug")
	require.NoError(t, err)
	pool := &mockPool{
		token: &Token{
			Value:     "test-token",
			ExpiresAt: time.Now().Add(1 * time.Hour),
		},
	}
	barrier := &mockBarrier{}
	client := NewClient(pool, barrier, tel)

	cfg := &config.Config{
		OpenApiURL: "http://localhost:8080",
	}

	// 2. 执行获取 Spec (应触发 fetchAndCacheSpec 并生成 Mock 数据)
	spec, err := client.GetOpenApiSpec(profile, cfg)
	require.NoError(t, err)

	// 3. 验证 Spec 结构与内容
	specMap, ok := spec.(map[string]interface{})
	require.True(t, ok)

	assert.Equal(t, "3.0.1", specMap["openapi"])
	info := specMap["info"].(map[string]interface{})
	assert.Equal(t, "1.2.0", info["version"])
	assert.Contains(t, info["description"], "CRM")

	paths := specMap["paths"].(map[string]interface{})
	
	// 验证新增的高质量 Mock 路径是否存在
	assert.Contains(t, paths, "/v1/crm/customer/register")
	assert.Contains(t, paths, "/v1/production/work-order/create")
	assert.Contains(t, paths, "/v1/hr/attendance/summary")
	assert.Contains(t, paths, "/v1/analytics/sales/rank")
	assert.Contains(t, paths, "/v1/logistics/delivery/track")

	// 验证总量是否达到预期 (原有+新增+自动生成 100+)
	assert.GreaterOrEqual(t, len(paths), 100, "Should have at least 100 mock API paths for semantic search testing")

	// 4. 验证缓存文件是否生成
	_, err = os.Stat(cachePath)
	assert.NoError(t, err, "Cache file should be created")

	// 读取缓存文件验证内容
	data, err := os.ReadFile(cachePath)
	assert.NoError(t, err)
	var cachedSpec map[string]interface{}
	err = json.Unmarshal(data, &cachedSpec)
	assert.NoError(t, err)
	assert.Equal(t, "1.2.0", cachedSpec["info"].(map[string]interface{})["version"])
}
