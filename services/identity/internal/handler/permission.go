package handler

import (
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/service"
)

type PermissionHandler struct {
	permService *service.PermissionService
	logger      *slog.Logger
}

func NewPermissionHandler(permService *service.PermissionService, logger *slog.Logger) *PermissionHandler {
	return &PermissionHandler{
		permService: permService,
		logger:      logger,
	}
}

func (h *PermissionHandler) GetMyPermissions(w http.ResponseWriter, r *http.Request) {
	claims := GetClaims(r.Context())
	if claims == nil {
		writeError(w, http.StatusUnauthorized, "unauthorized")
		return
	}

	userID, err := uuid.Parse(claims.Subject)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid user ID in token")
		return
	}

	perms, err := h.permService.GetPermissions(r.Context(), userID)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to get permissions")
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"permissions": perms,
		"role":        claims.Role,
	})
}

type AssignRoleRequest struct {
	Role string `json:"role"`
}

func (h *PermissionHandler) AssignRole(w http.ResponseWriter, r *http.Request) {
	claims := GetClaims(r.Context())
	if claims == nil {
		writeError(w, http.StatusUnauthorized, "unauthorized")
		return
	}

	adminID, err := uuid.Parse(claims.Subject)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid admin ID in token")
		return
	}

	targetID, err := uuid.Parse(chi.URLParam(r, "user_id"))
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid user ID")
		return
	}

	var req AssignRoleRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()

	if err := h.permService.AssignRole(r.Context(), targetID, model.Role(req.Role), adminID, ip, userAgent); err != nil {
		writeError(w, http.StatusForbidden, err.Error())
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"message": "role assigned"})
}

func (h *PermissionHandler) RevokeRole(w http.ResponseWriter, r *http.Request) {
	claims := GetClaims(r.Context())
	if claims == nil {
		writeError(w, http.StatusUnauthorized, "unauthorized")
		return
	}

	adminID, err := uuid.Parse(claims.Subject)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid admin ID in token")
		return
	}

	targetID, err := uuid.Parse(chi.URLParam(r, "user_id"))
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid user ID")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()

	if err := h.permService.RevokeRole(r.Context(), targetID, adminID, ip, userAgent); err != nil {
		writeError(w, http.StatusForbidden, err.Error())
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"message": "role revoked"})
}
