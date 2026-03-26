package auth

import (
	"golang.org/x/sync/singleflight"
)

// Barrier provides a single-flight barrier for concurrent operations
type Barrier interface {
	Do(key string, fn func() (interface{}, error)) (interface{}, error)
}

type singleFlightBarrier struct {
	group singleflight.Group
}

func NewBarrier() Barrier {
	return &singleFlightBarrier{}
}

func (b *singleFlightBarrier) Do(key string, fn func() (interface{}, error)) (interface{}, error) {
	val, err, _ := b.group.Do(key, fn)
	return val, err
}
