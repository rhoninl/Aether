package repository

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/model"
)

type SessionRepository struct {
	db *ReadReplicaRouter
}

func NewSessionRepository(db *pgxpool.Pool) *SessionRepository {
	return NewSessionRepositoryWithReadReplicas(db, nil)
}

func NewSessionRepositoryWithReadReplicas(db *pgxpool.Pool, readReplicas []*pgxpool.Pool) *SessionRepository {
	return &SessionRepository{db: NewReadReplicaRouter(db, readReplicas)}
}

func (r *SessionRepository) Create(ctx context.Context, session *model.Session) error {
	query := `
		INSERT INTO sessions (id, user_id, refresh_token_hash, ip_address, user_agent, expires_at, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`

	if session.ID == uuid.Nil {
		session.ID = uuid.New()
	}
	session.CreatedAt = time.Now().UTC()

	_, err := r.db.Writer().Exec(ctx, query,
		session.ID, session.UserID, session.RefreshTokenHash,
		session.IPAddress, session.UserAgent, session.ExpiresAt, session.CreatedAt,
	)
	return err
}

func (r *SessionRepository) GetByRefreshTokenHash(ctx context.Context, hash string) (*model.Session, error) {
	reader := r.db.Reader()
	query := `
		SELECT id, user_id, refresh_token_hash, ip_address, user_agent, expires_at, created_at
		FROM sessions WHERE refresh_token_hash = $1 AND expires_at > NOW()`

	var session model.Session
	err := reader.QueryRow(ctx, query, hash).Scan(
		&session.ID, &session.UserID, &session.RefreshTokenHash,
		&session.IPAddress, &session.UserAgent, &session.ExpiresAt, &session.CreatedAt,
	)
	if err != nil {
		return nil, err
	}
	return &session, nil
}

func (r *SessionRepository) Delete(ctx context.Context, id uuid.UUID) error {
	query := `DELETE FROM sessions WHERE id = $1`
	result, err := r.db.Writer().Exec(ctx, query, id)
	if err != nil {
		return err
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("session not found")
	}
	return nil
}

func (r *SessionRepository) DeleteByUserID(ctx context.Context, userID uuid.UUID) error {
	query := `DELETE FROM sessions WHERE user_id = $1`
	_, err := r.db.Writer().Exec(ctx, query, userID)
	return err
}

func (r *SessionRepository) DeleteExpired(ctx context.Context) (int64, error) {
	query := `DELETE FROM sessions WHERE expires_at < NOW()`
	result, err := r.db.Writer().Exec(ctx, query)
	if err != nil {
		return 0, err
	}
	return result.RowsAffected(), nil
}

func (r *SessionRepository) UpdateRefreshTokenHash(ctx context.Context, sessionID uuid.UUID, newHash string, newExpiry time.Time) error {
	query := `UPDATE sessions SET refresh_token_hash = $1, expires_at = $2 WHERE id = $3`
	result, err := r.db.Writer().Exec(ctx, query, newHash, newExpiry, sessionID)
	if err != nil {
		return err
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("session not found")
	}
	return nil
}
