package config

import (
	"crypto/ed25519"
	"crypto/x509"
	"encoding/pem"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func generateTestKeyPEM(t *testing.T) string {
	t.Helper()
	_, priv, err := ed25519.GenerateKey(nil)
	require.NoError(t, err)

	pkcs8Bytes, err := x509.MarshalPKCS8PrivateKey(priv)
	require.NoError(t, err)

	pemBytes := pem.EncodeToMemory(&pem.Block{
		Type:  "PRIVATE KEY",
		Bytes: pkcs8Bytes,
	})

	return string(pemBytes)
}

func setRequiredEnvVars(t *testing.T) {
	t.Helper()
	t.Setenv("IDENTITY_DB_URL", "postgres://test:test@localhost:5432/test")
	t.Setenv("IDENTITY_REDIS_URL", "redis://localhost:6379")
	t.Setenv("IDENTITY_NATS_URL", "nats://localhost:4222")
	t.Setenv("IDENTITY_JWT_PRIVATE_KEY", generateTestKeyPEM(t))
}

func TestLoad_Success(t *testing.T) {
	setRequiredEnvVars(t)

	cfg, err := Load()
	require.NoError(t, err)
	assert.NotNil(t, cfg)
	assert.Equal(t, DefaultPort, cfg.Port)
	assert.NotNil(t, cfg.JWTPrivateKey)
	assert.NotNil(t, cfg.JWTPublicKey)
	assert.Equal(t, DefaultAccessTokenTTL, cfg.JWTAccessTTL)
	assert.Equal(t, DefaultRefreshTokenTTL, cfg.JWTRefreshTTL)
	assert.Equal(t, uint32(DefaultArgon2Memory), cfg.Argon2Memory)
	assert.Equal(t, uint32(DefaultArgon2Iterations), cfg.Argon2Iterations)
	assert.Equal(t, uint8(DefaultArgon2Parallelism), cfg.Argon2Parallelism)
}

func TestLoad_MissingDBURL(t *testing.T) {
	t.Setenv("IDENTITY_REDIS_URL", "redis://localhost:6379")
	t.Setenv("IDENTITY_NATS_URL", "nats://localhost:4222")
	t.Setenv("IDENTITY_JWT_PRIVATE_KEY", generateTestKeyPEM(t))

	_, err := Load()
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "IDENTITY_DB_URL")
}

func TestLoad_MissingRedisURL(t *testing.T) {
	t.Setenv("IDENTITY_DB_URL", "postgres://test:test@localhost:5432/test")
	t.Setenv("IDENTITY_NATS_URL", "nats://localhost:4222")
	t.Setenv("IDENTITY_JWT_PRIVATE_KEY", generateTestKeyPEM(t))

	_, err := Load()
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "IDENTITY_REDIS_URL")
}

func TestLoad_MissingNatsURL(t *testing.T) {
	t.Setenv("IDENTITY_DB_URL", "postgres://test:test@localhost:5432/test")
	t.Setenv("IDENTITY_REDIS_URL", "redis://localhost:6379")
	t.Setenv("IDENTITY_JWT_PRIVATE_KEY", generateTestKeyPEM(t))

	_, err := Load()
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "IDENTITY_NATS_URL")
}

func TestLoad_MissingJWTKey(t *testing.T) {
	t.Setenv("IDENTITY_DB_URL", "postgres://test:test@localhost:5432/test")
	t.Setenv("IDENTITY_REDIS_URL", "redis://localhost:6379")
	t.Setenv("IDENTITY_NATS_URL", "nats://localhost:4222")

	_, err := Load()
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "IDENTITY_JWT_PRIVATE_KEY")
}

func TestLoad_InvalidJWTKey(t *testing.T) {
	t.Setenv("IDENTITY_DB_URL", "postgres://test:test@localhost:5432/test")
	t.Setenv("IDENTITY_REDIS_URL", "redis://localhost:6379")
	t.Setenv("IDENTITY_NATS_URL", "nats://localhost:4222")
	t.Setenv("IDENTITY_JWT_PRIVATE_KEY", "not-a-pem-key")

	_, err := Load()
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "PEM")
}

func TestLoad_CustomPort(t *testing.T) {
	setRequiredEnvVars(t)
	t.Setenv("IDENTITY_PORT", "9090")

	cfg, err := Load()
	require.NoError(t, err)
	assert.Equal(t, "9090", cfg.Port)
}

func TestLoad_CustomDurations(t *testing.T) {
	setRequiredEnvVars(t)
	t.Setenv("IDENTITY_JWT_ACCESS_TTL", "30m")
	t.Setenv("IDENTITY_JWT_REFRESH_TTL", "168h")

	cfg, err := Load()
	require.NoError(t, err)
	assert.Equal(t, 30*time.Minute, cfg.JWTAccessTTL)
	assert.Equal(t, 168*time.Hour, cfg.JWTRefreshTTL)
}

func TestLoad_CustomArgon2Params(t *testing.T) {
	setRequiredEnvVars(t)
	t.Setenv("IDENTITY_ARGON2_MEMORY", "131072")
	t.Setenv("IDENTITY_ARGON2_ITERATIONS", "5")
	t.Setenv("IDENTITY_ARGON2_PARALLELISM", "4")

	cfg, err := Load()
	require.NoError(t, err)
	assert.Equal(t, uint32(131072), cfg.Argon2Memory)
	assert.Equal(t, uint32(5), cfg.Argon2Iterations)
	assert.Equal(t, uint8(4), cfg.Argon2Parallelism)
}

func TestLoad_OAuthProviders(t *testing.T) {
	setRequiredEnvVars(t)
	t.Setenv("IDENTITY_OAUTH_GOOGLE_ID", "google-client-id")
	t.Setenv("IDENTITY_OAUTH_GOOGLE_SECRET", "google-secret")
	t.Setenv("IDENTITY_OAUTH_DISCORD_ID", "discord-client-id")

	cfg, err := Load()
	require.NoError(t, err)
	assert.Equal(t, "google-client-id", cfg.OAuthGoogleID)
	assert.Equal(t, "google-secret", cfg.OAuthGoogleSecret)
	assert.Equal(t, "discord-client-id", cfg.OAuthDiscordID)
}

func TestGetEnvOrDefault(t *testing.T) {
	assert.Equal(t, "default", getEnvOrDefault("NONEXISTENT_VAR_12345", "default"))

	t.Setenv("TEST_VAR_EXISTING", "custom")
	assert.Equal(t, "custom", getEnvOrDefault("TEST_VAR_EXISTING", "default"))
}

func TestGetEnvOrDefaultInt(t *testing.T) {
	assert.Equal(t, 42, getEnvOrDefaultInt("NONEXISTENT_VAR_12345", 42))

	t.Setenv("TEST_INT_VAR", "100")
	assert.Equal(t, 100, getEnvOrDefaultInt("TEST_INT_VAR", 42))

	t.Setenv("TEST_INT_INVALID", "not-a-number")
	assert.Equal(t, 42, getEnvOrDefaultInt("TEST_INT_INVALID", 42))
}

func TestGetEnvOrDefaultDuration(t *testing.T) {
	assert.Equal(t, 5*time.Minute, getEnvOrDefaultDuration("NONEXISTENT_VAR_12345", 5*time.Minute))

	t.Setenv("TEST_DUR_VAR", "30s")
	assert.Equal(t, 30*time.Second, getEnvOrDefaultDuration("TEST_DUR_VAR", 5*time.Minute))

	t.Setenv("TEST_DUR_INVALID", "not-a-duration")
	assert.Equal(t, 5*time.Minute, getEnvOrDefaultDuration("TEST_DUR_INVALID", 5*time.Minute))
}

func TestDefaultConstants(t *testing.T) {
	assert.Equal(t, "8080", DefaultPort)
	assert.Equal(t, 15*time.Minute, DefaultAccessTokenTTL)
	assert.Equal(t, 30*24*time.Hour, DefaultRefreshTokenTTL)
	assert.Equal(t, 65536, DefaultArgon2Memory)
	assert.Equal(t, 3, DefaultArgon2Iterations)
	assert.Equal(t, 2, DefaultArgon2Parallelism)
	assert.Equal(t, 16, DefaultArgon2SaltLength)
	assert.Equal(t, 32, DefaultArgon2KeyLength)
	assert.Equal(t, 10, DefaultRateLimitLogin)
	assert.Equal(t, 5, DefaultRateLimitRegister)
}

func TestParseReadReplicas(t *testing.T) {
	assert.Nil(t, parseReadReplicas(""))
	assert.Equal(t, []string{"postgres://replica1:5432/db"}, parseReadReplicas("postgres://replica1:5432/db"))
	assert.Equal(t, []string{
		"postgres://replica1:5432/db",
		"postgres://replica2:5432/db",
	}, parseReadReplicas("postgres://replica1:5432/db, postgres://replica2:5432/db,,"))
}

func TestLoad_ReadReplicas(t *testing.T) {
	setRequiredEnvVars(t)
	t.Setenv("IDENTITY_DB_READ_REPLICAS", "postgres://replica1:5432/db,postgres://replica2:5432/db")

	cfg, err := Load()
	require.NoError(t, err)
	assert.Equal(t, []string{
		"postgres://replica1:5432/db",
		"postgres://replica2:5432/db",
	}, cfg.DatabaseReadReplicas)
}
