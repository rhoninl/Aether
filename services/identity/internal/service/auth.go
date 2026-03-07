package service

import (
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"strings"
	"time"

	"github.com/go-jose/go-jose/v4"
	"github.com/go-jose/go-jose/v4/jwt"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"golang.org/x/crypto/argon2"

	"github.com/aether-engine/identity/internal/config"
	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/repository"
)

const (
	JWTIssuer        = "aether-identity"
	JWTAudienceAPI   = "aether-api"
	JWTAudienceWorld = "aether-world"
)

type TokenPair struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	TokenType    string `json:"token_type"`
}

type AccessClaims struct {
	jwt.Claims
	Role        model.Role `json:"role"`
	Permissions []string   `json:"permissions"`
	SessionID   string     `json:"sid"`
}

type AuthService struct {
	cfg           *config.Config
	users         *repository.UserRepository
	sessions      *repository.SessionRepository
	oauthAccounts *repository.OAuthAccountRepository
	webauthnCreds *repository.WebAuthnCredentialRepository
	audit         *repository.AuditRepository
	signer        jose.Signer
	logger        *slog.Logger
}

func NewAuthService(
	cfg *config.Config,
	users *repository.UserRepository,
	sessions *repository.SessionRepository,
	oauthAccounts *repository.OAuthAccountRepository,
	webauthnCreds *repository.WebAuthnCredentialRepository,
	audit *repository.AuditRepository,
	logger *slog.Logger,
) (*AuthService, error) {
	signingKey := jose.SigningKey{Algorithm: jose.EdDSA, Key: cfg.JWTPrivateKey}
	signer, err := jose.NewSigner(signingKey, (&jose.SignerOptions{}).WithType("JWT"))
	if err != nil {
		return nil, fmt.Errorf("failed to create JWT signer: %w", err)
	}

	return &AuthService{
		cfg:           cfg,
		users:         users,
		sessions:      sessions,
		oauthAccounts: oauthAccounts,
		webauthnCreds: webauthnCreds,
		audit:         audit,
		signer:        signer,
		logger:        logger,
	}, nil
}

func (s *AuthService) Register(ctx context.Context, email, username, password, ip, userAgent string) (*TokenPair, error) {
	hash, err := s.hashPassword(password)
	if err != nil {
		return nil, fmt.Errorf("failed to hash password: %w", err)
	}

	user := &model.User{
		ID:           uuid.New(),
		Email:        email,
		Username:     username,
		PasswordHash: hash,
		Role:         model.RolePlayer,
	}

	if err := s.users.Create(ctx, user); err != nil {
		return nil, fmt.Errorf("failed to create user: %w", err)
	}

	tokens, err := s.createSession(ctx, user, ip, userAgent)
	if err != nil {
		return nil, err
	}

	s.logAudit(ctx, &user.ID, model.AuditEventRegister, ip, userAgent, nil)
	s.logger.Info("user registered", "user_id", user.ID, "email", email)

	return tokens, nil
}

func (s *AuthService) Login(ctx context.Context, email, password, ip, userAgent string) (*TokenPair, error) {
	user, err := s.users.GetByEmail(ctx, email)
	if err != nil {
		return nil, fmt.Errorf("invalid credentials")
	}

	if !s.verifyPassword(password, user.PasswordHash) {
		s.logAudit(ctx, &user.ID, model.AuditEventLogin, ip, userAgent, map[string]string{"status": "failed"})
		return nil, fmt.Errorf("invalid credentials")
	}

	tokens, err := s.createSession(ctx, user, ip, userAgent)
	if err != nil {
		return nil, err
	}

	s.logAudit(ctx, &user.ID, model.AuditEventLogin, ip, userAgent, map[string]string{"status": "success"})
	s.logger.Info("user logged in", "user_id", user.ID)

	return tokens, nil
}

func (s *AuthService) RefreshToken(ctx context.Context, refreshToken, ip, userAgent string) (*TokenPair, error) {
	hash := hashRefreshToken(refreshToken)

	session, err := s.sessions.GetByRefreshTokenHash(ctx, hash)
	if err != nil {
		return nil, fmt.Errorf("invalid refresh token")
	}

	user, err := s.users.GetByID(ctx, session.UserID)
	if err != nil {
		return nil, fmt.Errorf("user not found")
	}

	newRefreshToken, err := generateRefreshToken()
	if err != nil {
		return nil, fmt.Errorf("failed to generate refresh token: %w", err)
	}
	newHash := hashRefreshToken(newRefreshToken)
	newExpiry := time.Now().UTC().Add(s.cfg.JWTRefreshTTL)

	if err := s.sessions.UpdateRefreshTokenHash(ctx, session.ID, newHash, newExpiry); err != nil {
		return nil, fmt.Errorf("failed to rotate refresh token: %w", err)
	}

	accessToken, expiresIn, err := s.createAccessToken(user, session.ID)
	if err != nil {
		return nil, err
	}

	s.logAudit(ctx, &user.ID, model.AuditEventTokenRefresh, ip, userAgent, nil)

	return &TokenPair{
		AccessToken:  accessToken,
		RefreshToken: newRefreshToken,
		ExpiresIn:    expiresIn,
		TokenType:    "Bearer",
	}, nil
}

func (s *AuthService) Logout(ctx context.Context, sessionID uuid.UUID, userID uuid.UUID, ip, userAgent string) error {
	if err := s.sessions.Delete(ctx, sessionID); err != nil {
		return fmt.Errorf("failed to delete session: %w", err)
	}
	s.logAudit(ctx, &userID, model.AuditEventLogout, ip, userAgent, nil)
	return nil
}

func (s *AuthService) ValidateToken(tokenString string) (*AccessClaims, error) {
	tok, err := jwt.ParseSigned(tokenString, []jose.SignatureAlgorithm{jose.EdDSA})
	if err != nil {
		return nil, fmt.Errorf("invalid token format")
	}

	claims := &AccessClaims{}
	if err := tok.Claims(s.cfg.JWTPublicKey, claims); err != nil {
		return nil, fmt.Errorf("invalid token signature")
	}

	expected := jwt.Expected{
		Issuer: JWTIssuer,
		Time:   time.Now(),
	}
	if err := claims.Claims.Validate(expected); err != nil {
		return nil, fmt.Errorf("token validation failed: %w", err)
	}

	return claims, nil
}

func (s *AuthService) GetPublicKey() ed25519.PublicKey {
	return s.cfg.JWTPublicKey
}

func (s *AuthService) OAuthLogin(ctx context.Context, provider, providerUserID, email, username, displayName, avatarURL, ip, userAgent string) (*TokenPair, error) {
	provider = strings.ToLower(strings.TrimSpace(provider))
	if err := s.validateOAuthProvider(provider); err != nil {
		return nil, err
	}
	if providerUserID == "" {
		return nil, fmt.Errorf("provider_user_id is required")
	}
	if s.oauthAccounts == nil || s.users == nil || s.sessions == nil {
		return nil, fmt.Errorf("oauth repository is not configured")
	}

	// Existing linked account path
	existingAccount, err := s.oauthAccounts.GetByProviderUserID(ctx, provider, providerUserID)
	if err != nil {
		return nil, err
	}
	if existingAccount != nil {
		user, err := s.users.GetByID(ctx, existingAccount.UserID)
		if err != nil {
			return nil, fmt.Errorf("linked user not found")
		}

		tokens, err := s.createSession(ctx, user, ip, userAgent)
		if err == nil {
			s.logAudit(ctx, &user.ID, model.AuditEventOAuthLogin, ip, userAgent, map[string]string{"provider": provider, "flow": "linked"})
		}
		return tokens, err
	}

	// If user exists by email, link this provider account to it
	var user *model.User
	if email != "" {
		user, err = s.users.GetByEmail(ctx, email)
		if err != nil && !errors.Is(err, pgx.ErrNoRows) {
			return nil, fmt.Errorf("lookup user failed: %w", err)
		}
	}

	// Otherwise create a new identity anchored account
	if user == nil {
		if username == "" && email != "" {
			username = s.makeUniqueUsername(ctx, email)
		}
		if username == "" {
			username = "player-" + shortRandomString()
		}

		if displayName == "" && email != "" {
			if parts := strings.Split(email, "@"); len(parts) > 0 {
				displayName = parts[0]
			}
		}

		user = &model.User{
			ID:        uuid.New(),
			Email:     email,
			Username:  username,
			Role:      model.RolePlayer,
			AvatarURL: avatarURL,
		}
		if displayName != "" {
			user.DisplayName = displayName
		}

		if err := s.users.Create(ctx, user); err != nil {
			return nil, fmt.Errorf("failed to create user: %w", err)
		}
	}

	// Link identity provider credential to local account
	account := &model.OAuthAccount{
		UserID:         user.ID,
		Provider:       provider,
		ProviderUserID: providerUserID,
	}
	if err := s.oauthAccounts.Upsert(ctx, account); err != nil {
		return nil, fmt.Errorf("failed to link oauth account: %w", err)
	}

	tokens, err := s.createSession(ctx, user, ip, userAgent)
	if err == nil {
		s.logAudit(ctx, &user.ID, model.AuditEventOAuthLogin, ip, userAgent, map[string]string{"provider": provider, "flow": "new"})
	}
	return tokens, err
}

func (s *AuthService) LinkOAuthAccount(ctx context.Context, userID uuid.UUID, provider, providerUserID, ip, userAgent string) error {
	provider = strings.ToLower(strings.TrimSpace(provider))
	if err := s.validateOAuthProvider(provider); err != nil {
		return err
	}
	if providerUserID == "" {
		return fmt.Errorf("provider_user_id is required")
	}
	if s.oauthAccounts == nil || s.users == nil {
		return fmt.Errorf("oauth repository is not configured")
	}

	if _, err := s.users.GetByID(ctx, userID); err != nil {
		return fmt.Errorf("user not found")
	}

	existing, err := s.oauthAccounts.GetByProviderUserID(ctx, provider, providerUserID)
	if err != nil {
		return err
	}
	if existing != nil {
		if existing.UserID != userID {
			return fmt.Errorf("provider already linked to another user")
		}
		return nil
	}

	account := &model.OAuthAccount{
		UserID:         userID,
		Provider:       provider,
		ProviderUserID: providerUserID,
	}
	if err := s.oauthAccounts.Upsert(ctx, account); err != nil {
		return fmt.Errorf("failed to link oauth account: %w", err)
	}

	s.logAudit(ctx, &userID, model.AuditEventOAuthLogin, ip, userAgent, map[string]string{"provider": provider, "action": "link"})
	return nil
}

func (s *AuthService) RegisterWebAuthnCredential(ctx context.Context, userID uuid.UUID, credentialIDBase64, publicKeyBase64, aaguid, ip, userAgent string) error {
	if s.webauthnCreds == nil || s.users == nil {
		return fmt.Errorf("webauthn is not configured")
	}

	if _, err := s.users.GetByID(ctx, userID); err != nil {
		return fmt.Errorf("user not found")
	}

	credentialID, err := decodeBase64Bytes(credentialIDBase64)
	if err != nil {
		return fmt.Errorf("invalid credential_id: %w", err)
	}
	if len(credentialID) == 0 {
		return fmt.Errorf("credential_id is required")
	}

	publicKey, err := decodeBase64Bytes(publicKeyBase64)
	if err != nil {
		return fmt.Errorf("invalid public_key: %w", err)
	}
	if len(publicKey) == 0 {
		return fmt.Errorf("public_key is required")
	}

	existing, err := s.webauthnCreds.GetByCredentialID(ctx, credentialID)
	if err != nil {
		return err
	}
	if existing != nil {
		if existing.UserID == userID {
			return fmt.Errorf("credential already registered")
		}
		return fmt.Errorf("credential already linked to another user")
	}

	credential := &model.WebAuthnCredential{
		ID:           uuid.New(),
		UserID:       userID,
		CredentialID: credentialID,
		PublicKey:    publicKey,
		AAGUID:       aaguid,
		SignCount:    0,
	}

	if err := s.webauthnCreds.Create(ctx, credential); err != nil {
		return fmt.Errorf("failed to register webauthn credential: %w", err)
	}

	s.logAudit(ctx, &userID, model.AuditEventWebAuthnLogin, ip, userAgent, map[string]string{"flow": "register"})
	return nil
}

func (s *AuthService) LoginWithWebAuthn(ctx context.Context, credentialIDBase64, ip, userAgent string) (*TokenPair, error) {
	if s.webauthnCreds == nil || s.users == nil || s.sessions == nil {
		return nil, fmt.Errorf("webauthn is not configured")
	}

	credentialID, err := decodeBase64Bytes(credentialIDBase64)
	if err != nil {
		return nil, fmt.Errorf("invalid credential_id: %w", err)
	}
	if len(credentialID) == 0 {
		return nil, fmt.Errorf("credential_id is required")
	}

	credential, err := s.webauthnCreds.GetByCredentialID(ctx, credentialID)
	if err != nil {
		return nil, err
	}
	if credential == nil {
		return nil, fmt.Errorf("invalid credential")
	}

	user, err := s.users.GetByID(ctx, credential.UserID)
	if err != nil {
		return nil, fmt.Errorf("user not found")
	}

	tokens, err := s.createSession(ctx, user, ip, userAgent)
	if err != nil {
		return nil, err
	}

	// Best-effort credential replay protection counter update.
	if err := s.webauthnCreds.IncrementSignCount(ctx, credentialID, credential.SignCount+1); err != nil {
		s.logger.Warn("failed to update webauthn sign counter", "error", err, "user_id", user.ID)
	}

	s.logAudit(ctx, &user.ID, model.AuditEventWebAuthnLogin, ip, userAgent, map[string]string{"flow": "login"})
	return tokens, nil
}

func (s *AuthService) createSession(ctx context.Context, user *model.User, ip, userAgent string) (*TokenPair, error) {
	refreshToken, err := generateRefreshToken()
	if err != nil {
		return nil, fmt.Errorf("failed to generate refresh token: %w", err)
	}

	session := &model.Session{
		ID:               uuid.New(),
		UserID:           user.ID,
		RefreshTokenHash: hashRefreshToken(refreshToken),
		IPAddress:        ip,
		UserAgent:        userAgent,
		ExpiresAt:        time.Now().UTC().Add(s.cfg.JWTRefreshTTL),
	}

	if err := s.sessions.Create(ctx, session); err != nil {
		return nil, fmt.Errorf("failed to create session: %w", err)
	}

	accessToken, expiresIn, err := s.createAccessToken(user, session.ID)
	if err != nil {
		return nil, err
	}

	return &TokenPair{
		AccessToken:  accessToken,
		RefreshToken: refreshToken,
		ExpiresIn:    expiresIn,
		TokenType:    "Bearer",
	}, nil
}

func (s *AuthService) createAccessToken(user *model.User, sessionID uuid.UUID) (string, int64, error) {
	now := time.Now().UTC()
	expiry := now.Add(s.cfg.JWTAccessTTL)

	claims := AccessClaims{
		Claims: jwt.Claims{
			Issuer:   JWTIssuer,
			Subject:  user.ID.String(),
			Audience: jwt.Audience{JWTAudienceAPI, JWTAudienceWorld},
			IssuedAt: jwt.NewNumericDate(now),
			Expiry:   jwt.NewNumericDate(expiry),
			ID:       uuid.New().String(),
		},
		Role:        user.Role,
		Permissions: model.GetPermissions(user.Role),
	}
	if sessionID != uuid.Nil {
		claims.SessionID = sessionID.String()
	}

	token, err := jwt.Signed(s.signer).Claims(claims).Serialize()
	if err != nil {
		return "", 0, fmt.Errorf("failed to sign token: %w", err)
	}

	return token, int64(s.cfg.JWTAccessTTL.Seconds()), nil
}

func (s *AuthService) validateOAuthProvider(provider string) error {
	switch provider {
	case "google":
		if s.cfg.OAuthGoogleID == "" || s.cfg.OAuthGoogleSecret == "" {
			return fmt.Errorf("google provider is not configured")
		}
	case "apple":
		if s.cfg.OAuthAppleID == "" || s.cfg.OAuthAppleSecret == "" {
			return fmt.Errorf("apple provider is not configured")
		}
	case "discord":
		if s.cfg.OAuthDiscordID == "" || s.cfg.OAuthDiscordSecret == "" {
			return fmt.Errorf("discord provider is not configured")
		}
	case "steam":
		if s.cfg.OAuthSteamKey == "" {
			return fmt.Errorf("steam provider is not configured")
		}
	default:
		return fmt.Errorf("unsupported provider")
	}
	return nil
}

func (s *AuthService) makeUniqueUsername(ctx context.Context, email string) string {
	base := ""
	if email != "" {
		parts := strings.Split(email, "@")
		if len(parts) > 0 && parts[0] != "" {
			base = sanitizeIdentifier(parts[0])
		}
	}
	if base == "" {
		base = "player"
	}

	candidate := base
	for i := 0; i < 64; i++ {
		existing, err := s.users.GetByUsername(ctx, candidate)
		if err != nil && !errors.Is(err, pgx.ErrNoRows) {
			return base
		}
		if errors.Is(err, pgx.ErrNoRows) {
			return candidate
		}
		if i == 0 {
			candidate = fmt.Sprintf("%s-%d", base, i+1)
		} else {
			candidate = fmt.Sprintf("%s-%d", base, i+1)
		}
	}
	return base
}

func (s *AuthService) hashPassword(password string) (string, error) {
	salt := make([]byte, s.cfg.Argon2SaltLength)
	if _, err := rand.Read(salt); err != nil {
		return "", err
	}

	hash := argon2.IDKey(
		[]byte(password), salt,
		s.cfg.Argon2Iterations, s.cfg.Argon2Memory,
		s.cfg.Argon2Parallelism, uint32(s.cfg.Argon2KeyLength),
	)

	// Encode as: $argon2id$v=19$m=MEMORY,t=ITERATIONS,p=PARALLELISM$SALT$HASH
	return fmt.Sprintf("$argon2id$v=19$m=%d,t=%d,p=%d$%s$%s",
		s.cfg.Argon2Memory, s.cfg.Argon2Iterations, s.cfg.Argon2Parallelism,
		hex.EncodeToString(salt), hex.EncodeToString(hash),
	), nil
}

func (s *AuthService) verifyPassword(password, encodedHash string) bool {
	var memory, iterations uint32
	var parallelism uint8
	var saltHex, hashHex string

	_, err := fmt.Sscanf(encodedHash, "$argon2id$v=19$m=%d,t=%d,p=%d$%s",
		&memory, &iterations, &parallelism, &saltHex)
	if err != nil {
		return false
	}

	parts := splitLast(saltHex, "$")
	if len(parts) != 2 {
		return false
	}
	saltHex = parts[0]
	hashHex = parts[1]

	salt, err := hex.DecodeString(saltHex)
	if err != nil {
		return false
	}

	expectedHash, err := hex.DecodeString(hashHex)
	if err != nil {
		return false
	}

	computedHash := argon2.IDKey([]byte(password), salt, iterations, memory, parallelism, uint32(len(expectedHash)))

	if len(computedHash) != len(expectedHash) {
		return false
	}
	result := byte(0)
	for i := range computedHash {
		result |= computedHash[i] ^ expectedHash[i]
	}
	return result == 0
}

func (s *AuthService) logAudit(ctx context.Context, userID *uuid.UUID, eventType model.AuditEventType, ip, userAgent string, meta map[string]string) {
	var metadata json.RawMessage
	if meta != nil {
		metadata, _ = json.Marshal(meta)
	}

	log := &model.AuditLog{
		UserID:    userID,
		EventType: eventType,
		IPAddress: ip,
		UserAgent: userAgent,
		Metadata:  metadata,
	}

	if err := s.audit.Create(ctx, log); err != nil {
		s.logger.Error("failed to write audit log", "error", err, "event_type", eventType)
	}
}

func generateRefreshToken() (string, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}

func hashRefreshToken(token string) string {
	h := sha256.Sum256([]byte(token))
	return hex.EncodeToString(h[:])
}

func splitLast(s, sep string) []string {
	for i := len(s) - 1; i >= 0; i-- {
		if string(s[i]) == sep {
			return []string{s[:i], s[i+1:]}
		}
	}
	return []string{s}
}

func decodeBase64Bytes(v string) ([]byte, error) {
	value := strings.TrimSpace(v)
	if value == "" {
		return nil, nil
	}

	if decoded, err := base64.StdEncoding.DecodeString(value); err == nil {
		return decoded, nil
	}
	if decoded, err := base64.RawURLEncoding.DecodeString(value); err == nil {
		return decoded, nil
	}
	return nil, fmt.Errorf("invalid base64 value")
}

func shortRandomString() string {
	r := make([]byte, 4)
	if _, err := rand.Read(r); err != nil {
		return "user"
	}
	hexValue := hex.EncodeToString(r)
	return strings.ToLower(hexValue[:8])
}

func sanitizeIdentifier(input string) string {
	lower := strings.ToLower(strings.TrimSpace(input))
	if lower == "" {
		return ""
	}

	filtered := make([]rune, 0, len(lower))
	for _, ch := range lower {
		if (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') {
			filtered = append(filtered, ch)
		} else if ch == '_' || ch == '-' {
			filtered = append(filtered, ch)
		}
	}

	result := string(filtered)
	result = strings.Trim(result, "_-")
	return result
}
