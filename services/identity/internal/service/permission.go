package service

import (
	"context"
	"fmt"
	"log/slog"

	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/repository"
)

type PermissionService struct {
	users  *repository.UserRepository
	audit  *repository.AuditRepository
	logger *slog.Logger
}

func NewPermissionService(
	users *repository.UserRepository,
	audit *repository.AuditRepository,
	logger *slog.Logger,
) *PermissionService {
	return &PermissionService{
		users:  users,
		audit:  audit,
		logger: logger,
	}
}

func (s *PermissionService) GetPermissions(ctx context.Context, userID uuid.UUID) ([]string, error) {
	user, err := s.users.GetByID(ctx, userID)
	if err != nil {
		return nil, fmt.Errorf("user not found")
	}
	return model.GetPermissions(user.Role), nil
}

func (s *PermissionService) AssignRole(ctx context.Context, targetUserID uuid.UUID, role model.Role, adminID uuid.UUID, ip, userAgent string) error {
	// Validate role exists
	if _, ok := model.RoleHierarchy[role]; !ok {
		return fmt.Errorf("invalid role: %s", role)
	}

	// Check admin has higher hierarchy level
	admin, err := s.users.GetByID(ctx, adminID)
	if err != nil {
		return fmt.Errorf("admin not found")
	}

	adminLevel := model.RoleHierarchy[admin.Role]
	targetLevel := model.RoleHierarchy[role]
	if adminLevel <= targetLevel {
		return fmt.Errorf("insufficient privileges: cannot assign role with equal or higher hierarchy")
	}

	if err := s.users.UpdateRole(ctx, targetUserID, role); err != nil {
		return fmt.Errorf("failed to assign role: %w", err)
	}

	authSvc := &AuthService{audit: s.audit, logger: s.logger}
	authSvc.logAudit(ctx, &adminID, model.AuditEventPermissionChange, ip, userAgent, map[string]string{
		"target_user": targetUserID.String(),
		"new_role":    string(role),
		"action":      "assign",
	})

	s.logger.Info("role assigned", "admin_id", adminID, "target_user_id", targetUserID, "role", role)
	return nil
}

func (s *PermissionService) RevokeRole(ctx context.Context, targetUserID uuid.UUID, adminID uuid.UUID, ip, userAgent string) error {
	// Revoke to player (base role)
	return s.AssignRole(ctx, targetUserID, model.RolePlayer, adminID, ip, userAgent)
}
