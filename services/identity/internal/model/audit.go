package model

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type AuditEventType string

const (
	AuditEventLogin            AuditEventType = "login"
	AuditEventLogout           AuditEventType = "logout"
	AuditEventRegister         AuditEventType = "register"
	AuditEventTokenRefresh     AuditEventType = "token_refresh"
	AuditEventPermissionChange AuditEventType = "permission_change"
	AuditEventProfileUpdate    AuditEventType = "profile_update"
	AuditEventOAuthLogin       AuditEventType = "oauth_login"
	AuditEventWebAuthnLogin    AuditEventType = "webauthn_login"
	AuditEventPasswordChange   AuditEventType = "password_change"
	AuditEventAccountDelete    AuditEventType = "account_delete"
)

type AuditLog struct {
	ID        uuid.UUID       `json:"id"`
	UserID    *uuid.UUID      `json:"user_id,omitempty"`
	EventType AuditEventType  `json:"event_type"`
	IPAddress string          `json:"ip_address"`
	UserAgent string          `json:"user_agent"`
	Metadata  json.RawMessage `json:"metadata,omitempty"`
	CreatedAt time.Time       `json:"created_at"`
}
