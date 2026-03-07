package handler

import (
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/google/uuid"

	"github.com/aether-engine/identity/internal/service"
)

type WebAuthnHandler struct {
	authService *service.AuthService
	logger      *slog.Logger
}

func NewWebAuthnHandler(authService *service.AuthService, logger *slog.Logger) *WebAuthnHandler {
	return &WebAuthnHandler{
		authService: authService,
		logger:      logger,
	}
}

type WebAuthnRegisterRequest struct {
	CredentialID string `json:"credential_id"`
	PublicKey    string `json:"public_key"`
	AAGUID       string `json:"aaguid"`
}

type WebAuthnLoginRequest struct {
	CredentialID string `json:"credential_id"`
}

func (h *WebAuthnHandler) Register(w http.ResponseWriter, r *http.Request) {
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

	var req WebAuthnRegisterRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()
	if err := h.authService.RegisterWebAuthnCredential(r.Context(), userID, req.CredentialID, req.PublicKey, req.AAGUID, ip, userAgent); err != nil {
		h.logger.Error("webauthn registration failed", "error", err, "user_id", userID)
		writeError(w, http.StatusConflict, err.Error())
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "registered"})
}

func (h *WebAuthnHandler) Login(w http.ResponseWriter, r *http.Request) {
	var req WebAuthnLoginRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	ip := GetClientIP(r)
	userAgent := r.UserAgent()
	tokens, err := h.authService.LoginWithWebAuthn(r.Context(), req.CredentialID, ip, userAgent)
	if err != nil {
		h.logger.Error("webauthn login failed", "error", err)
		writeError(w, http.StatusUnauthorized, "webauthn login failed")
		return
	}

	writeJSON(w, http.StatusOK, tokens)
}

