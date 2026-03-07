package handler

import (
	"crypto/ed25519"
	"encoding/base64"
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/aether-engine/identity/internal/service"
)

type TokenHandler struct {
	authService *service.AuthService
	logger      *slog.Logger
}

func NewTokenHandler(authService *service.AuthService, logger *slog.Logger) *TokenHandler {
	return &TokenHandler{
		authService: authService,
		logger:      logger,
	}
}

type ValidateRequest struct {
	Token string `json:"token"`
}

type ValidateResponse struct {
	Valid       bool     `json:"valid"`
	Subject     string   `json:"subject,omitempty"`
	Role        string   `json:"role,omitempty"`
	Permissions []string `json:"permissions,omitempty"`
}

func (h *TokenHandler) Validate(w http.ResponseWriter, r *http.Request) {
	var req ValidateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	if req.Token == "" {
		writeError(w, http.StatusBadRequest, "token is required")
		return
	}

	claims, err := h.authService.ValidateToken(req.Token)
	if err != nil {
		writeJSON(w, http.StatusOK, ValidateResponse{Valid: false})
		return
	}

	writeJSON(w, http.StatusOK, ValidateResponse{
		Valid:       true,
		Subject:     claims.Subject,
		Role:        string(claims.Role),
		Permissions: claims.Permissions,
	})
}

type JWKSResponse struct {
	Keys []JWK `json:"keys"`
}

type JWK struct {
	Kty string `json:"kty"`
	Crv string `json:"crv"`
	X   string `json:"x"`
	Use string `json:"use"`
	Alg string `json:"alg"`
	Kid string `json:"kid"`
}

func (h *TokenHandler) JWKS(w http.ResponseWriter, r *http.Request) {
	pubKey := h.authService.GetPublicKey()

	jwks := JWKSResponse{
		Keys: []JWK{
			{
				Kty: "OKP",
				Crv: "Ed25519",
				X:   base64.RawURLEncoding.EncodeToString(ed25519.PublicKey(pubKey)),
				Use: "sig",
				Alg: "EdDSA",
				Kid: "aether-identity-1",
			},
		},
	}

	writeJSON(w, http.StatusOK, jwks)
}
