package repository

import (
	"context"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/model"
)

type AuditRepository struct {
	db *ReadReplicaRouter
}

func NewAuditRepository(db *pgxpool.Pool) *AuditRepository {
	return NewAuditRepositoryWithReadReplicas(db, nil)
}

func NewAuditRepositoryWithReadReplicas(db *pgxpool.Pool, readReplicas []*pgxpool.Pool) *AuditRepository {
	return &AuditRepository{db: NewReadReplicaRouter(db, readReplicas)}
}

func (r *AuditRepository) Create(ctx context.Context, log *model.AuditLog) error {
	query := `
		INSERT INTO audit_logs (id, user_id, event_type, ip_address, user_agent, metadata, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`

	if log.ID == uuid.Nil {
		log.ID = uuid.New()
	}
	log.CreatedAt = time.Now().UTC()

	_, err := r.db.Writer().Exec(ctx, query,
		log.ID, log.UserID, log.EventType,
		log.IPAddress, log.UserAgent, log.Metadata, log.CreatedAt,
	)
	return err
}

func (r *AuditRepository) ListByUserID(ctx context.Context, userID uuid.UUID, limit, offset int) ([]*model.AuditLog, error) {
	reader := r.db.Reader()
	query := `
		SELECT id, user_id, event_type, ip_address, user_agent, metadata, created_at
		FROM audit_logs WHERE user_id = $1
		ORDER BY created_at DESC
		LIMIT $2 OFFSET $3`

	rows, err := reader.Query(ctx, query, userID, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*model.AuditLog
	for rows.Next() {
		var log model.AuditLog
		if err := rows.Scan(&log.ID, &log.UserID, &log.EventType, &log.IPAddress, &log.UserAgent, &log.Metadata, &log.CreatedAt); err != nil {
			return nil, err
		}
		logs = append(logs, &log)
	}
	return logs, rows.Err()
}
