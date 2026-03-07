package handler

import (
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/service"
)

type OAuthHandler struct {
	authService *service.AuthService
	logger      *slog.Logger
}

func NewOAuthHandler(authService *service.AuthService, logger *slog.Logger) *OAuthHandler {
	return &OAuthHandler{
		authService: authService,
		logger:      logger,
	}
}

type OAuthLoginRequest struct {
	ProviderUserID string `json:"provider_user_id"`
	Email          string `json:"email"`
	Username       string `json:"username"`
	DisplayName    string `json:"display_name"`
	AvatarURL      string `json:"avatar_url"`
}

type OAuthLoginResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	TokenType    string `json:"token_type"`
}

func (h *OAuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	provider := chi.URLParam(r, "provider")
	if provider == "" {
		writeError(w, http.StatusBadRequest, "provider is required")
		return
	}

	var req OAuthLoginRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	if req.ProviderUserID == "" {
		writeError(w, http.StatusBadRequest, "provider_user_id is required")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()
	tokens, err := h.authService.OAuthLogin(
		r.Context(),
		provider,
		req.ProviderUserID,
		req.Email,
		req.Username,
		req.DisplayName,
		req.AvatarURL,
		ip,
		userAgent,
	)
	if err != nil {
		h.logger.Error("oauth login failed", "error", err, "provider", provider)
		writeError(w, http.StatusUnauthorized, "oauth login failed")
		return
	}

	writeJSON(w, http.StatusOK, tokens)
}

func (h *OAuthHandler) Link(w http.ResponseWriter, r *http.Request) {
	claims := GetClaims(r.Context())
	if claims == nil {
		writeError(w, http.StatusUnauthorized, "unauthorized")
		return
	}

	userID, err := uuid.Parse(claims.Subject)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid user id in token")
		return
	}

	provider := chi.URLParam(r, "provider")
	if provider == "" {
		writeError(w, http.StatusBadRequest, "provider is required")
		return
	}

	var req OAuthLoginRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	if req.ProviderUserID == "" {
		writeError(w, http.StatusBadRequest, "provider_user_id is required")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()
	if err := h.authService.LinkOAuthAccount(r.Context(), userID, provider, req.ProviderUserID, ip, userAgent); err != nil {
		h.logger.Error("oauth link failed", "error", err, "provider", provider, "user_id", userID)
		writeError(w, http.StatusConflict, err.Error())
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "linked"})
}
