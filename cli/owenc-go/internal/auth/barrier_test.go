package auth

import (
	"fmt"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestBarrier(t *testing.T) {
	barrier := NewBarrier()
	var callCount int32
	var wg sync.WaitGroup

	numGoroutines := 50
	wg.Add(numGoroutines)

	results := make([]string, numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			val, err := barrier.Do("token-refresh", func() (interface{}, error) {
				atomic.AddInt32(&callCount, 1)
				time.Sleep(100 * time.Millisecond) // Simulate slow operation
				return "new-token", nil
			})
			assert.NoError(t, err)
			results[idx] = val.(string)
		}(i)
	}

	wg.Wait()

	assert.Equal(t, int32(1), callCount, "Operation should only be called once")
	for i := 0; i < numGoroutines; i++ {
		assert.Equal(t, "new-token", results[i], "All goroutines should get the same result")
	}
}

func TestBarrierError(t *testing.T) {
	barrier := NewBarrier()
	expectedErr := fmt.Errorf("auth failed")
	
	val, err := barrier.Do("error-key", func() (interface{}, error) {
		return nil, expectedErr
	})
	
	assert.Error(t, err)
	assert.Equal(t, expectedErr, err)
	assert.Nil(t, val)
}
