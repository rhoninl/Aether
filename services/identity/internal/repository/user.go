package repository

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/model"
)

type UserRepository struct {
	db *ReadReplicaRouter
}

func NewUserRepository(db *pgxpool.Pool) *UserRepository {
	return NewUserRepositoryWithReadReplicas(db, nil)
}

func NewUserRepositoryWithReadReplicas(db *pgxpool.Pool, readReplicas []*pgxpool.Pool) *UserRepository {
	return &UserRepository{db: NewReadReplicaRouter(db, readReplicas)}
}

func (r *UserRepository) Create(ctx context.Context, user *model.User) error {
	writer := r.db.Writer()
	query := `
		INSERT INTO users (id, email, username, password_hash, display_name, bio, avatar_url, settings, role, email_verified, created_at, updated_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`

	now := time.Now().UTC()
	if user.ID == uuid.Nil {
		user.ID = uuid.New()
	}
	user.CreatedAt = now
	user.UpdatedAt = now

	settings := user.Settings
	if settings == nil {
		settings = json.RawMessage("{}")
	}

	_, err := writer.Exec(ctx, query,
		user.ID, user.Email, user.Username, user.PasswordHash,
		user.DisplayName, user.Bio, user.AvatarURL, settings,
		user.Role, user.EmailVerified, user.CreatedAt, user.UpdatedAt,
	)
	return err
}

func (r *UserRepository) GetByID(ctx context.Context, id uuid.UUID) (*model.User, error) {
	reader := r.db.Reader()
	query := `
		SELECT id, email, username, password_hash, display_name, bio, avatar_url, settings, role, email_verified, created_at, updated_at, deleted_at
		FROM users WHERE id = $1 AND deleted_at IS NULL`

	return r.scanUser(reader.QueryRow(ctx, query, id))
}

func (r *UserRepository) GetByEmail(ctx context.Context, email string) (*model.User, error) {
	reader := r.db.Reader()
	query := `
		SELECT id, email, username, password_hash, display_name, bio, avatar_url, settings, role, email_verified, created_at, updated_at, deleted_at
		FROM users WHERE email = $1 AND deleted_at IS NULL`

	return r.scanUser(reader.QueryRow(ctx, query, email))
}

func (r *UserRepository) GetByUsername(ctx context.Context, username string) (*model.User, error) {
	reader := r.db.Reader()
	query := `
		SELECT id, email, username, password_hash, display_name, bio, avatar_url, settings, role, email_verified, created_at, updated_at, deleted_at
		FROM users WHERE username = $1 AND deleted_at IS NULL`

	return r.scanUser(reader.QueryRow(ctx, query, username))
}

func (r *UserRepository) Update(ctx context.Context, user *model.User) error {
	writer := r.db.Writer()
	query := `
		UPDATE users SET display_name = $1, bio = $2, avatar_url = $3, settings = $4, updated_at = $5
		WHERE id = $6 AND deleted_at IS NULL`

	user.UpdatedAt = time.Now().UTC()
	settings := user.Settings
	if settings == nil {
		settings = json.RawMessage("{}")
	}

	result, err := writer.Exec(ctx, query,
		user.DisplayName, user.Bio, user.AvatarURL, settings, user.UpdatedAt, user.ID,
	)
	if err != nil {
		return err
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("user not found")
	}
	return nil
}

func (r *UserRepository) UpdateRole(ctx context.Context, userID uuid.UUID, role model.Role) error {
	writer := r.db.Writer()
	query := `UPDATE users SET role = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL`
	result, err := writer.Exec(ctx, query, role, time.Now().UTC(), userID)
	if err != nil {
		return err
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("user not found")
	}
	return nil
}

func (r *UserRepository) SoftDelete(ctx context.Context, id uuid.UUID) error {
	writer := r.db.Writer()
	query := `UPDATE users SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL`
	result, err := writer.Exec(ctx, query, time.Now().UTC(), id)
	if err != nil {
		return err
	}
	if result.RowsAffected() == 0 {
		return fmt.Errorf("user not found")
	}
	return nil
}

func (r *UserRepository) Search(ctx context.Context, query string, limit, offset int) ([]*model.User, error) {
	reader := r.db.Reader()
	sql := `
		SELECT id, email, username, password_hash, display_name, bio, avatar_url, settings, role, email_verified, created_at, updated_at, deleted_at
		FROM users
		WHERE deleted_at IS NULL AND (username ILIKE $1 OR display_name ILIKE $1)
		ORDER BY created_at DESC
		LIMIT $2 OFFSET $3`

	rows, err := reader.Query(ctx, sql, "%"+query+"%", limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []*model.User
	for rows.Next() {
		user, err := r.scanUser(rows)
		if err != nil {
			return nil, err
		}
		users = append(users, user)
	}
	return users, rows.Err()
}

func (r *UserRepository) scanUser(row pgx.Row) (*model.User, error) {
	var user model.User
	err := row.Scan(
		&user.ID, &user.Email, &user.Username, &user.PasswordHash,
		&user.DisplayName, &user.Bio, &user.AvatarURL, &user.Settings,
		&user.Role, &user.EmailVerified, &user.CreatedAt, &user.UpdatedAt,
		&user.DeletedAt,
	)
	if err != nil {
		return nil, err
	}
	return &user, nil
}
