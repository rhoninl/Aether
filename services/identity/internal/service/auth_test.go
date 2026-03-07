package service

import (
	"crypto/ed25519"
	"crypto/x509"
	"encoding/pem"
	"log/slog"
	"os"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/aether-engine/identity/internal/config"
	"github.com/aether-engine/identity/internal/model"
)

func newTestAuthService(t *testing.T) *AuthService {
	t.Helper()

	pub, priv, err := ed25519.GenerateKey(nil)
	require.NoError(t, err)

	cfg := &config.Config{
		JWTPrivateKey:    priv,
		JWTPublicKey:     pub,
		JWTAccessTTL:     15 * time.Minute,
		JWTRefreshTTL:    30 * 24 * time.Hour,
		Argon2Memory:     65536,
		Argon2Iterations: 3,
		Argon2Parallelism: 2,
		Argon2SaltLength:  16,
		Argon2KeyLength:   32,
	}

	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	svc, err := NewAuthService(cfg, nil, nil, nil, nil, nil, logger)
	require.NoError(t, err)

	return svc
}

func TestPasswordHashAndVerify(t *testing.T) {
	svc := newTestAuthService(t)

	password := "securePassword123!"
	hash, err := svc.hashPassword(password)
	require.NoError(t, err)
	assert.NotEmpty(t, hash)
	assert.Contains(t, hash, "$argon2id$")

	// Verify correct password
	assert.True(t, svc.verifyPassword(password, hash))

	// Verify wrong password
	assert.False(t, svc.verifyPassword("wrongPassword", hash))

	// Verify empty password against hash
	assert.False(t, svc.verifyPassword("", hash))
}

func TestPasswordHashUniqueness(t *testing.T) {
	svc := newTestAuthService(t)

	hash1, err := svc.hashPassword("samePassword")
	require.NoError(t, err)

	hash2, err := svc.hashPassword("samePassword")
	require.NoError(t, err)

	// Same password should produce different hashes (different salts)
	assert.NotEqual(t, hash1, hash2)

	// But both should verify correctly
	assert.True(t, svc.verifyPassword("samePassword", hash1))
	assert.True(t, svc.verifyPassword("samePassword", hash2))
}

func TestCreateAndValidateAccessToken(t *testing.T) {
	svc := newTestAuthService(t)

	user := &model.User{
		ID:   uuid.New(),
		Role: model.RoleCreator,
	}

	tokenString, expiresIn, err := svc.createAccessToken(user, uuid.Nil)
	require.NoError(t, err)
	assert.NotEmpty(t, tokenString)
	assert.Equal(t, int64(900), expiresIn) // 15 minutes = 900 seconds

	// Validate the token
	claims, err := svc.ValidateToken(tokenString)
	require.NoError(t, err)
	assert.Equal(t, user.ID.String(), claims.Subject)
	assert.Equal(t, model.RoleCreator, claims.Role)
	assert.Equal(t, JWTIssuer, claims.Issuer)

	// Verify permissions include creator + player (inherited)
	assert.Contains(t, claims.Permissions, model.PermWorldCreate)
	assert.Contains(t, claims.Permissions, model.PermProfileRead)
}

func TestValidateToken_InvalidToken(t *testing.T) {
	svc := newTestAuthService(t)

	_, err := svc.ValidateToken("invalid.token.string")
	assert.Error(t, err)
}

func TestValidateToken_WrongKey(t *testing.T) {
	svc := newTestAuthService(t)

	user := &model.User{
		ID:   uuid.New(),
		Role: model.RolePlayer,
	}

	tokenString, _, err := svc.createAccessToken(user, uuid.Nil)
	require.NoError(t, err)

	// Create a different service with different keys
	svc2 := newTestAuthService(t)
	_, err = svc2.ValidateToken(tokenString)
	assert.Error(t, err)
}

func TestValidateToken_ExpiredToken(t *testing.T) {
	pub, priv, err := ed25519.GenerateKey(nil)
	require.NoError(t, err)

	cfg := &config.Config{
		JWTPrivateKey:    priv,
		JWTPublicKey:     pub,
		JWTAccessTTL:     -1 * time.Minute, // Already expired
		JWTRefreshTTL:    30 * 24 * time.Hour,
		Argon2Memory:     65536,
		Argon2Iterations: 3,
		Argon2Parallelism: 2,
		Argon2SaltLength:  16,
		Argon2KeyLength:   32,
	}

	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	svc, err := NewAuthService(cfg, nil, nil, nil, nil, nil, logger)
	require.NoError(t, err)

	user := &model.User{ID: uuid.New(), Role: model.RolePlayer}
	tokenString, _, err := svc.createAccessToken(user, uuid.Nil)
	require.NoError(t, err)

	_, err = svc.ValidateToken(tokenString)
	assert.Error(t, err)
}

func TestTokenRolePermissions(t *testing.T) {
	svc := newTestAuthService(t)

	roles := []model.Role{model.RolePlayer, model.RoleCreator, model.RoleModerator, model.RoleAdmin}

	for _, role := range roles {
		t.Run(string(role), func(t *testing.T) {
			user := &model.User{ID: uuid.New(), Role: role}
			tokenString, _, err := svc.createAccessToken(user, uuid.Nil)
			require.NoError(t, err)

			claims, err := svc.ValidateToken(tokenString)
			require.NoError(t, err)
			assert.Equal(t, role, claims.Role)

			expectedPerms := model.GetPermissions(role)
			assert.ElementsMatch(t, expectedPerms, claims.Permissions)
		})
	}
}

func TestGenerateRefreshToken(t *testing.T) {
	token1, err := generateRefreshToken()
	require.NoError(t, err)
	assert.Len(t, token1, 64) // 32 bytes hex encoded

	token2, err := generateRefreshToken()
	require.NoError(t, err)
	assert.NotEqual(t, token1, token2)
}

func TestHashRefreshToken(t *testing.T) {
	token := "test-refresh-token"
	hash1 := hashRefreshToken(token)
	hash2 := hashRefreshToken(token)

	// Same input should produce same hash
	assert.Equal(t, hash1, hash2)

	// Different input should produce different hash
	hash3 := hashRefreshToken("different-token")
	assert.NotEqual(t, hash1, hash3)
}

func TestSplitLast(t *testing.T) {
	tests := []struct {
		input    string
		sep      string
		expected []string
	}{
		{"a$b$c", "$", []string{"a$b", "c"}},
		{"abc", "$", []string{"abc"}},
		{"a$b", "$", []string{"a", "b"}},
	}

	for _, tt := range tests {
		result := splitLast(tt.input, tt.sep)
		assert.Equal(t, tt.expected, result)
	}
}

func TestGetPublicKey(t *testing.T) {
	svc := newTestAuthService(t)
	pubKey := svc.GetPublicKey()
	assert.NotNil(t, pubKey)
	assert.Len(t, pubKey, ed25519.PublicKeySize)
}

// Helper to generate a PEM-encoded Ed25519 private key for config tests
func TestConfigKeyParsing(t *testing.T) {
	_, priv, err := ed25519.GenerateKey(nil)
	require.NoError(t, err)

	pkcs8Bytes, err := x509.MarshalPKCS8PrivateKey(priv)
	require.NoError(t, err)

	pemBlock := pem.EncodeToMemory(&pem.Block{
		Type:  "PRIVATE KEY",
		Bytes: pkcs8Bytes,
	})
	assert.NotNil(t, pemBlock)
	assert.Contains(t, string(pemBlock), "BEGIN PRIVATE KEY")
}

