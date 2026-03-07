package config

import (
	"crypto/ed25519"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

const (
	DefaultPort              = "8080"
	DefaultAccessTokenTTL    = 15 * time.Minute
	DefaultRefreshTokenTTL   = 30 * 24 * time.Hour // 30 days
	DefaultArgon2Memory      = 65536
	DefaultArgon2Iterations  = 3
	DefaultArgon2Parallelism = 2
	DefaultArgon2SaltLength  = 16
	DefaultArgon2KeyLength   = 32
	DefaultRateLimitLogin    = 10
	DefaultRateLimitRegister = 5
)

type Config struct {
	Port string

	DatabaseURL         string
	DatabaseReadReplicas []string
	RedisURL            string
	NatsURL             string

	JWTPrivateKey ed25519.PrivateKey
	JWTPublicKey  ed25519.PublicKey
	JWTAccessTTL  time.Duration
	JWTRefreshTTL time.Duration

	OAuthGoogleID      string
	OAuthGoogleSecret  string
	OAuthAppleID       string
	OAuthAppleSecret   string
	OAuthDiscordID     string
	OAuthDiscordSecret string
	OAuthSteamKey      string

	Argon2Memory      uint32
	Argon2Iterations  uint32
	Argon2Parallelism uint8
	Argon2SaltLength  uint32
	Argon2KeyLength   uint32

	RateLimitLogin    int
	RateLimitRegister int
}

func Load() (*Config, error) {
	cfg := &Config{
		Port:               getEnvOrDefault("IDENTITY_PORT", DefaultPort),
		DatabaseURL:        os.Getenv("IDENTITY_DB_URL"),
		DatabaseReadReplicas: parseReadReplicas(os.Getenv("IDENTITY_DB_READ_REPLICAS")),
		RedisURL:           os.Getenv("IDENTITY_REDIS_URL"),
		NatsURL:            os.Getenv("IDENTITY_NATS_URL"),

		OAuthGoogleID:      os.Getenv("IDENTITY_OAUTH_GOOGLE_ID"),
		OAuthGoogleSecret:  os.Getenv("IDENTITY_OAUTH_GOOGLE_SECRET"),
		OAuthAppleID:       os.Getenv("IDENTITY_OAUTH_APPLE_ID"),
		OAuthAppleSecret:   os.Getenv("IDENTITY_OAUTH_APPLE_SECRET"),
		OAuthDiscordID:     os.Getenv("IDENTITY_OAUTH_DISCORD_ID"),
		OAuthDiscordSecret: os.Getenv("IDENTITY_OAUTH_DISCORD_SECRET"),
		OAuthSteamKey:      os.Getenv("IDENTITY_OAUTH_STEAM_KEY"),

		Argon2Memory:      uint32(getEnvOrDefaultInt("IDENTITY_ARGON2_MEMORY", DefaultArgon2Memory)),
		Argon2Iterations:  uint32(getEnvOrDefaultInt("IDENTITY_ARGON2_ITERATIONS", DefaultArgon2Iterations)),
		Argon2Parallelism: uint8(getEnvOrDefaultInt("IDENTITY_ARGON2_PARALLELISM", DefaultArgon2Parallelism)),
		Argon2SaltLength:  uint32(DefaultArgon2SaltLength),
		Argon2KeyLength:   uint32(DefaultArgon2KeyLength),

		RateLimitLogin:    getEnvOrDefaultInt("IDENTITY_RATE_LIMIT_LOGIN", DefaultRateLimitLogin),
		RateLimitRegister: getEnvOrDefaultInt("IDENTITY_RATE_LIMIT_REGISTER", DefaultRateLimitRegister),
	}

	if cfg.DatabaseURL == "" {
		return nil, fmt.Errorf("IDENTITY_DB_URL is required")
	}
	if cfg.RedisURL == "" {
		return nil, fmt.Errorf("IDENTITY_REDIS_URL is required")
	}
	if cfg.NatsURL == "" {
		return nil, fmt.Errorf("IDENTITY_NATS_URL is required")
	}

	// Parse JWT key
	keyPEM := os.Getenv("IDENTITY_JWT_PRIVATE_KEY")
	if keyPEM == "" {
		return nil, fmt.Errorf("IDENTITY_JWT_PRIVATE_KEY is required")
	}
	block, _ := pem.Decode([]byte(keyPEM))
	if block == nil {
		return nil, fmt.Errorf("failed to decode PEM block for JWT private key")
	}
	privKey, err := x509.ParsePKCS8PrivateKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("failed to parse JWT private key: %w", err)
	}
	edKey, ok := privKey.(ed25519.PrivateKey)
	if !ok {
		return nil, fmt.Errorf("JWT private key is not Ed25519")
	}
	cfg.JWTPrivateKey = edKey
	cfg.JWTPublicKey = edKey.Public().(ed25519.PublicKey)

	// Parse durations
	cfg.JWTAccessTTL = getEnvOrDefaultDuration("IDENTITY_JWT_ACCESS_TTL", DefaultAccessTokenTTL)
	cfg.JWTRefreshTTL = getEnvOrDefaultDuration("IDENTITY_JWT_REFRESH_TTL", DefaultRefreshTokenTTL)

	return cfg, nil
}

func getEnvOrDefault(key, defaultVal string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return defaultVal
}

func getEnvOrDefaultInt(key string, defaultVal int) int {
	v := os.Getenv(key)
	if v == "" {
		return defaultVal
	}
	i, err := strconv.Atoi(v)
	if err != nil {
		return defaultVal
	}
	return i
}

func getEnvOrDefaultDuration(key string, defaultVal time.Duration) time.Duration {
	v := os.Getenv(key)
	if v == "" {
		return defaultVal
	}
	d, err := time.ParseDuration(v)
	if err != nil {
		return defaultVal
	}
	return d
}

func parseReadReplicas(value string) []string {
	value = strings.TrimSpace(value)
	if value == "" {
		return nil
	}

	parts := strings.Split(value, ",")
	if len(parts) == 0 {
		return nil
	}

	raw := make([]string, 0, len(parts))
	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed == "" {
			continue
		}
		raw = append(raw, trimmed)
	}
	return raw
}
