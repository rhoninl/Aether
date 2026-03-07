package service

import (
	"context"
	"fmt"
	"log/slog"

	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/repository"
)

type ProfileService struct {
	users  *repository.UserRepository
	audit  *repository.AuditRepository
	logger *slog.Logger
}

func NewProfileService(
	users *repository.UserRepository,
	audit *repository.AuditRepository,
	logger *slog.Logger,
) *ProfileService {
	return &ProfileService{
		users:  users,
		audit:  audit,
		logger: logger,
	}
}

type UpdateProfileRequest struct {
	DisplayName *string `json:"display_name,omitempty"`
	Bio         *string `json:"bio,omitempty"`
	AvatarURL   *string `json:"avatar_url,omitempty"`
}

func (s *ProfileService) GetByID(ctx context.Context, id uuid.UUID) (*model.User, error) {
	user, err := s.users.GetByID(ctx, id)
	if err != nil {
		return nil, fmt.Errorf("user not found")
	}
	return user, nil
}

func (s *ProfileService) Update(ctx context.Context, userID uuid.UUID, req *UpdateProfileRequest, ip, userAgent string) (*model.User, error) {
	user, err := s.users.GetByID(ctx, userID)
	if err != nil {
		return nil, fmt.Errorf("user not found")
	}

	if req.DisplayName != nil {
		user.DisplayName = *req.DisplayName
	}
	if req.Bio != nil {
		user.Bio = *req.Bio
	}
	if req.AvatarURL != nil {
		user.AvatarURL = *req.AvatarURL
	}

	if err := s.users.Update(ctx, user); err != nil {
		return nil, fmt.Errorf("failed to update profile: %w", err)
	}

	authSvc := &AuthService{audit: s.audit, logger: s.logger}
	authSvc.logAudit(ctx, &userID, model.AuditEventProfileUpdate, ip, userAgent, nil)

	s.logger.Info("profile updated", "user_id", userID)
	return user, nil
}

func (s *ProfileService) Search(ctx context.Context, query string, limit, offset int) ([]*model.User, error) {
	if limit <= 0 || limit > 100 {
		limit = 20
	}
	return s.users.Search(ctx, query, limit, offset)
}
