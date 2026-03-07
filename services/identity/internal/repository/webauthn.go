package repository

import (
	"context"
	"errors"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/model"
)

type WebAuthnCredentialRepository struct {
	db *ReadReplicaRouter
}

func NewWebAuthnCredentialRepository(db *pgxpool.Pool) *WebAuthnCredentialRepository {
	return NewWebAuthnCredentialRepositoryWithReadReplicas(db, nil)
}

func NewWebAuthnCredentialRepositoryWithReadReplicas(db *pgxpool.Pool, readReplicas []*pgxpool.Pool) *WebAuthnCredentialRepository {
	return &WebAuthnCredentialRepository{db: NewReadReplicaRouter(db, readReplicas)}
}

func (r *WebAuthnCredentialRepository) Create(ctx context.Context, credential *model.WebAuthnCredential) error {
	query := `
		INSERT INTO webauthn_credentials (id, user_id, credential_id, public_key, aaguid, sign_count, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`

	if credential.ID == uuid.Nil {
		credential.ID = uuid.New()
	}
	credential.CreatedAt = time.Now().UTC()

	_, err := r.db.Writer().Exec(ctx, query,
		credential.ID, credential.UserID, credential.CredentialID,
		credential.PublicKey, credential.AAGUID, credential.SignCount, credential.CreatedAt,
	)
	return err
}

func (r *WebAuthnCredentialRepository) GetByCredentialID(ctx context.Context, credentialID []byte) (*model.WebAuthnCredential, error) {
	query := `
		SELECT id, user_id, credential_id, public_key, aaguid, sign_count, created_at
		FROM webauthn_credentials WHERE credential_id = $1`

	var credential model.WebAuthnCredential
	err := r.db.Reader().QueryRow(ctx, query, credentialID).Scan(
		&credential.ID, &credential.UserID, &credential.CredentialID,
		&credential.PublicKey, &credential.AAGUID, &credential.SignCount, &credential.CreatedAt,
	)
	if err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return nil, nil
		}
		return nil, err
	}
	return &credential, nil
}

func (r *WebAuthnCredentialRepository) GetByUserID(ctx context.Context, userID uuid.UUID) ([]*model.WebAuthnCredential, error) {
	query := `
		SELECT id, user_id, credential_id, public_key, aaguid, sign_count, created_at
		FROM webauthn_credentials WHERE user_id = $1`

	rows, err := r.db.Reader().Query(ctx, query, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var credentials []*model.WebAuthnCredential
	for rows.Next() {
		var credential model.WebAuthnCredential
		if err := rows.Scan(
			&credential.ID, &credential.UserID, &credential.CredentialID,
			&credential.PublicKey, &credential.AAGUID, &credential.SignCount, &credential.CreatedAt,
		); err != nil {
			return nil, err
		}
		credentials = append(credentials, &credential)
	}
	return credentials, rows.Err()
}

func (r *WebAuthnCredentialRepository) IncrementSignCount(ctx context.Context, credentialID []byte, newSignCount uint32) error {
	query := `UPDATE webauthn_credentials SET sign_count = $1 WHERE credential_id = $2`
	_, err := r.db.Writer().Exec(ctx, query, newSignCount, credentialID)
	return err
}

