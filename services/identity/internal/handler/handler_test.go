package handler

import (
	"context"
	"crypto/ed25519"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/aether-engine/identity/internal/config"
	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/service"
)

func newTestAuthService(t *testing.T) *service.AuthService {
	t.Helper()

	pub, priv, err := ed25519.GenerateKey(nil)
	require.NoError(t, err)

	cfg := &config.Config{
		JWTPrivateKey:     priv,
		JWTPublicKey:      pub,
		JWTAccessTTL:      15 * time.Minute,
		JWTRefreshTTL:     30 * 24 * time.Hour,
		Argon2Memory:      65536,
		Argon2Iterations:  3,
		Argon2Parallelism: 2,
		Argon2SaltLength:  16,
		Argon2KeyLength:   32,
	}

	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	svc, err := service.NewAuthService(cfg, nil, nil, nil, nil, nil, logger)
	require.NoError(t, err)
	return svc
}

func TestGetClientIP_XForwardedFor(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.Header.Set("X-Forwarded-For", "1.2.3.4, 5.6.7.8")
	assert.Equal(t, "1.2.3.4", GetClientIP(req))
}

func TestGetClientIP_XRealIP(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.Header.Set("X-Real-IP", "9.8.7.6")
	assert.Equal(t, "9.8.7.6", GetClientIP(req))
}

func TestGetClientIP_RemoteAddr(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.RemoteAddr = "192.168.1.1:12345"
	assert.Equal(t, "192.168.1.1:12345", GetClientIP(req))
}

func TestGetClaims_NoClaims(t *testing.T) {
	ctx := context.Background()
	claims := GetClaims(ctx)
	assert.Nil(t, claims)
}

func TestGetClaims_WithClaims(t *testing.T) {
	claims := &service.AccessClaims{
		Role: model.RoleAdmin,
	}
	ctx := context.WithValue(context.Background(), ContextKeyClaims, claims)
	result := GetClaims(ctx)
	assert.NotNil(t, result)
	assert.Equal(t, model.RoleAdmin, result.Role)
}

func TestAuthMiddleware_NoHeader(t *testing.T) {
	authSvc := newTestAuthService(t)
	handler := AuthMiddleware(authSvc)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusUnauthorized, rec.Code)
}

func TestAuthMiddleware_InvalidFormat(t *testing.T) {
	authSvc := newTestAuthService(t)
	handler := AuthMiddleware(authSvc)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.Header.Set("Authorization", "InvalidFormat")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusUnauthorized, rec.Code)
}

func TestAuthMiddleware_InvalidToken(t *testing.T) {
	authSvc := newTestAuthService(t)
	handler := AuthMiddleware(authSvc)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.Header.Set("Authorization", "Bearer invalid.token.here")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusUnauthorized, rec.Code)
}

func TestRequirePermission_NoContext(t *testing.T) {
	handler := RequirePermission(model.PermUserBan)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusUnauthorized, rec.Code)
}

func TestRequirePermission_HasPermission(t *testing.T) {
	claims := &service.AccessClaims{
		Role:        model.RoleAdmin,
		Permissions: model.GetPermissions(model.RoleAdmin),
	}

	handler := RequirePermission(model.PermUserBan)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	ctx := context.WithValue(req.Context(), ContextKeyClaims, claims)
	req = req.WithContext(ctx)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusOK, rec.Code)
}

func TestRequirePermission_MissingPermission(t *testing.T) {
	claims := &service.AccessClaims{
		Role:        model.RolePlayer,
		Permissions: model.GetPermissions(model.RolePlayer),
	}

	handler := RequirePermission(model.PermUserBan)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	ctx := context.WithValue(req.Context(), ContextKeyClaims, claims)
	req = req.WithContext(ctx)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	assert.Equal(t, http.StatusForbidden, rec.Code)
}

func TestTokenHandler_JWKS(t *testing.T) {
	authSvc := newTestAuthService(t)
	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	tokenHandler := NewTokenHandler(authSvc, logger)

	req := httptest.NewRequest(http.MethodGet, "/auth/.well-known/jwks.json", nil)
	rec := httptest.NewRecorder()
	tokenHandler.JWKS(rec, req)

	assert.Equal(t, http.StatusOK, rec.Code)

	var jwks JWKSResponse
	err := json.NewDecoder(rec.Body).Decode(&jwks)
	require.NoError(t, err)
	assert.Len(t, jwks.Keys, 1)
	assert.Equal(t, "OKP", jwks.Keys[0].Kty)
	assert.Equal(t, "Ed25519", jwks.Keys[0].Crv)
	assert.Equal(t, "EdDSA", jwks.Keys[0].Alg)
	assert.Equal(t, "sig", jwks.Keys[0].Use)
	assert.NotEmpty(t, jwks.Keys[0].X)
}

func TestTokenHandler_Validate_EmptyToken(t *testing.T) {
	authSvc := newTestAuthService(t)
	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	tokenHandler := NewTokenHandler(authSvc, logger)

	body := strings.NewReader(`{"token":""}`)
	req := httptest.NewRequest(http.MethodPost, "/auth/validate", body)
	rec := httptest.NewRecorder()
	tokenHandler.Validate(rec, req)

	assert.Equal(t, http.StatusBadRequest, rec.Code)
}

func TestTokenHandler_Validate_InvalidToken(t *testing.T) {
	authSvc := newTestAuthService(t)
	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	tokenHandler := NewTokenHandler(authSvc, logger)

	body := strings.NewReader(`{"token":"invalid.token"}`)
	req := httptest.NewRequest(http.MethodPost, "/auth/validate", body)
	rec := httptest.NewRecorder()
	tokenHandler.Validate(rec, req)

	assert.Equal(t, http.StatusOK, rec.Code)

	var resp ValidateResponse
	err := json.NewDecoder(rec.Body).Decode(&resp)
	require.NoError(t, err)
	assert.False(t, resp.Valid)
}

func TestWriteJSON(t *testing.T) {
	rec := httptest.NewRecorder()
	writeJSON(rec, http.StatusOK, map[string]string{"key": "value"})

	assert.Equal(t, http.StatusOK, rec.Code)
	assert.Equal(t, "application/json", rec.Header().Get("Content-Type"))

	var result map[string]string
	err := json.NewDecoder(rec.Body).Decode(&result)
	require.NoError(t, err)
	assert.Equal(t, "value", result["key"])
}

func TestWriteError(t *testing.T) {
	rec := httptest.NewRecorder()
	writeError(rec, http.StatusBadRequest, "something went wrong")

	assert.Equal(t, http.StatusBadRequest, rec.Code)

	var result map[string]string
	err := json.NewDecoder(rec.Body).Decode(&result)
	require.NoError(t, err)
	assert.Equal(t, "something went wrong", result["error"])
}
