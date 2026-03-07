package repository

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/model"
)

type OAuthAccountRepository struct {
	db *ReadReplicaRouter
}

func NewOAuthAccountRepository(db *pgxpool.Pool) *OAuthAccountRepository {
	return NewOAuthAccountRepositoryWithReadReplicas(db, nil)
}

func NewOAuthAccountRepositoryWithReadReplicas(db *pgxpool.Pool, readReplicas []*pgxpool.Pool) *OAuthAccountRepository {
	return &OAuthAccountRepository{db: NewReadReplicaRouter(db, readReplicas)}
}

func (r *OAuthAccountRepository) Create(ctx context.Context, account *model.OAuthAccount) error {
	query := `
		INSERT INTO oauth_accounts (id, user_id, provider, provider_user_id, access_token_enc, refresh_token_enc, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`

	if account.ID == uuid.Nil {
		account.ID = uuid.New()
	}
	account.CreatedAt = time.Now().UTC()

	_, err := r.db.Writer().Exec(ctx, query,
		account.ID, account.UserID, account.Provider, account.ProviderUserID,
		account.AccessTokenEnc, account.RefreshTokenEnc, account.CreatedAt,
	)
	return err
}

func (r *OAuthAccountRepository) Upsert(ctx context.Context, account *model.OAuthAccount) error {
	query := `
		INSERT INTO oauth_accounts (id, user_id, provider, provider_user_id, access_token_enc, refresh_token_enc, created_at)
		VALUES (uuid_generate_v4(), $1, $2, $3, $4, $5, NOW())
		ON CONFLICT (provider, provider_user_id) DO UPDATE SET
			user_id = EXCLUDED.user_id,
			access_token_enc = EXCLUDED.access_token_enc,
			refresh_token_enc = EXCLUDED.refresh_token_enc`

	_, err := r.db.Writer().Exec(ctx, query,
		account.UserID, account.Provider, account.ProviderUserID,
		account.AccessTokenEnc, account.RefreshTokenEnc,
	)
	return err
}

func (r *OAuthAccountRepository) GetByProviderUserID(ctx context.Context, provider, providerUserID string) (*model.OAuthAccount, error) {
	query := `
		SELECT id, user_id, provider, provider_user_id, access_token_enc, refresh_token_enc, created_at
		FROM oauth_accounts WHERE provider = $1 AND provider_user_id = $2`

	account := new(model.OAuthAccount)
	err := r.db.Reader().QueryRow(ctx, query, provider, providerUserID).Scan(
		&account.ID, &account.UserID, &account.Provider, &account.ProviderUserID,
		&account.AccessTokenEnc, &account.RefreshTokenEnc, &account.CreatedAt,
	)
	if err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return nil, nil
		}
		return nil, err
	}
	return account, nil
}

func (r *OAuthAccountRepository) GetByUserAndProvider(ctx context.Context, userID uuid.UUID, provider string) (*model.OAuthAccount, error) {
	query := `
		SELECT id, user_id, provider, provider_user_id, access_token_enc, refresh_token_enc, created_at
		FROM oauth_accounts WHERE user_id = $1 AND provider = $2`

	account := new(model.OAuthAccount)
	err := r.db.Reader().QueryRow(ctx, query, userID, provider).Scan(
		&account.ID, &account.UserID, &account.Provider, &account.ProviderUserID,
		&account.AccessTokenEnc, &account.RefreshTokenEnc, &account.CreatedAt,
	)
	if err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return nil, nil
		}
		return nil, err
	}
	return account, nil
}

func (r *OAuthAccountRepository) DeleteByUserAndProvider(ctx context.Context, userID uuid.UUID, provider string) error {
	query := `DELETE FROM oauth_accounts WHERE user_id = $1 AND provider = $2`
	result, err := r.db.Writer().Exec(ctx, query, userID, provider)
	if err != nil {
		return fmt.Errorf("delete oauth account: %w", err)
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("oauth account not found")
	}
	return nil
}

