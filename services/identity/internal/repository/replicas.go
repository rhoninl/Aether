package repository

import (
	"sync/atomic"

	"github.com/jackc/pgx/v5/pgxpool"
)

type ReadReplicaRouter struct {
	writer    *pgxpool.Pool
	readPools []*pgxpool.Pool
	readIdx   uint64
}

func NewReadReplicaRouter(writer *pgxpool.Pool, readPools []*pgxpool.Pool) *ReadReplicaRouter {
	return &ReadReplicaRouter{
		writer:    writer,
		readPools: readPools,
	}
}

func (r *ReadReplicaRouter) Writer() *pgxpool.Pool {
	if r == nil {
		return nil
	}
	return r.writer
}

func (r *ReadReplicaRouter) Reader() *pgxpool.Pool {
	if r == nil || len(r.readPools) == 0 {
		return r.writer
	}
	index := atomic.AddUint64(&r.readIdx, 1)
	return r.readPools[(index-1)%uint64(len(r.readPools))]
}

