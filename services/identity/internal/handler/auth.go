package handler

import (
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/service"
)

type AuthHandler struct {
	authService *service.AuthService
	logger      *slog.Logger
}

func NewAuthHandler(authService *service.AuthService, logger *slog.Logger) *AuthHandler {
	return &AuthHandler{
		authService: authService,
		logger:      logger,
	}
}

type RegisterRequest struct {
	Email    string `json:"email"`
	Username string `json:"username"`
	Password string `json:"password"`
}

type LoginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type RefreshRequest struct {
	RefreshToken string `json:"refresh_token"`
}

func (h *AuthHandler) Register(w http.ResponseWriter, r *http.Request) {
	var req RegisterRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	if req.Email == "" || req.Username == "" || req.Password == "" {
		writeError(w, http.StatusBadRequest, "email, username, and password are required")
		return
	}

	if len(req.Password) < 8 {
		writeError(w, http.StatusBadRequest, "password must be at least 8 characters")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()

	tokens, err := h.authService.Register(r.Context(), req.Email, req.Username, req.Password, ip, userAgent)
	if err != nil {
		h.logger.Error("registration failed", "error", err, "email", req.Email)
		writeError(w, http.StatusConflict, "registration failed: "+err.Error())
		return
	}

	writeJSON(w, http.StatusCreated, tokens)
}

func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	var req LoginRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	if req.Email == "" || req.Password == "" {
		writeError(w, http.StatusBadRequest, "email and password are required")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()

	tokens, err := h.authService.Login(r.Context(), req.Email, req.Password, ip, userAgent)
	if err != nil {
		writeError(w, http.StatusUnauthorized, "invalid credentials")
		return
	}

	writeJSON(w, http.StatusOK, tokens)
}

func (h *AuthHandler) Refresh(w http.ResponseWriter, r *http.Request) {
	var req RefreshRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	if req.RefreshToken == "" {
		writeError(w, http.StatusBadRequest, "refresh_token is required")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()

	tokens, err := h.authService.RefreshToken(r.Context(), req.RefreshToken, ip, userAgent)
	if err != nil {
		writeError(w, http.StatusUnauthorized, "invalid refresh token")
		return
	}

	writeJSON(w, http.StatusOK, tokens)
}

func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	claims := GetClaims(r.Context())
	if claims == nil {
		writeError(w, http.StatusUnauthorized, "unauthorized")
		return
	}

	sessionID, err := uuid.Parse(claims.SessionID)
	if err != nil {
		writeError(w, http.StatusBadRequest, "missing or invalid session id")
		return
	}
	userID, err := uuid.Parse(claims.Subject)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid token subject")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()
	if err := h.authService.Logout(r.Context(), sessionID, userID, ip, userAgent); err != nil {
		writeError(w, http.StatusInternalServerError, "logout failed")
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"message": "logged out"})
}
