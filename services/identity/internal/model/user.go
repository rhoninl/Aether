package model

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type Role string

const (
	RolePlayer    Role = "player"
	RoleCreator   Role = "creator"
	RoleModerator Role = "moderator"
	RoleAdmin     Role = "admin"
)

type User struct {
	ID            uuid.UUID       `json:"id"`
	Email         string          `json:"email"`
	Username      string          `json:"username"`
	PasswordHash  string          `json:"-"`
	DisplayName   string          `json:"display_name"`
	Bio           string          `json:"bio"`
	AvatarURL     string          `json:"avatar_url"`
	Settings      json.RawMessage `json:"settings"`
	Role          Role            `json:"role"`
	EmailVerified bool            `json:"email_verified"`
	CreatedAt     time.Time       `json:"created_at"`
	UpdatedAt     time.Time       `json:"updated_at"`
	DeletedAt     *time.Time      `json:"deleted_at,omitempty"`
}

type Session struct {
	ID               uuid.UUID `json:"id"`
	UserID           uuid.UUID `json:"user_id"`
	RefreshTokenHash string    `json:"-"`
	IPAddress        string    `json:"ip_address"`
	UserAgent        string    `json:"user_agent"`
	ExpiresAt        time.Time `json:"expires_at"`
	CreatedAt        time.Time `json:"created_at"`
}

type OAuthAccount struct {
	ID              uuid.UUID `json:"id"`
	UserID          uuid.UUID `json:"user_id"`
	Provider        string    `json:"provider"`
	ProviderUserID  string    `json:"provider_user_id"`
	AccessTokenEnc  string    `json:"-"`
	RefreshTokenEnc string    `json:"-"`
	CreatedAt       time.Time `json:"created_at"`
}

type WebAuthnCredential struct {
	ID           uuid.UUID `json:"id"`
	UserID       uuid.UUID `json:"user_id"`
	CredentialID []byte    `json:"-"`
	PublicKey    []byte    `json:"-"`
	AAGUID       string    `json:"aaguid"`
	SignCount    uint32    `json:"sign_count"`
	CreatedAt    time.Time `json:"created_at"`
}

type UserRole struct {
	ID        uuid.UUID  `json:"id"`
	UserID    uuid.UUID  `json:"user_id"`
	RoleID    uuid.UUID  `json:"role_id"`
	GrantedBy uuid.UUID  `json:"granted_by"`
	GrantedAt time.Time  `json:"granted_at"`
	RevokedAt *time.Time `json:"revoked_at,omitempty"`
}

type RoleDefinition struct {
	ID             uuid.UUID       `json:"id"`
	Name           Role            `json:"name"`
	Permissions    json.RawMessage `json:"permissions"`
	HierarchyLevel int            `json:"hierarchy_level"`
	CreatedAt      time.Time       `json:"created_at"`
}

// Permission constants
const (
	PermProfileRead  = "profile:read"
	PermProfileWrite = "profile:write"
	PermWorldJoin    = "world:join"
	PermWorldCreate  = "world:create"
	PermWorldEdit    = "world:edit"
	PermAvatarEquip  = "avatar:equip"
	PermAvatarUpload = "avatar:upload"
	PermAssetUpload  = "asset:upload"
	PermUserWarn     = "user:warn"
	PermUserMute     = "user:mute"
	PermUserKick     = "user:kick"
	PermUserBan      = "user:ban"
	PermReportReview = "report:review"
	PermRoleAssign   = "role:assign"
	PermRoleRevoke   = "role:revoke"
	PermSystemConfig = "system:configure"
)

// RolePermissions defines which permissions each role has (excluding inherited)
var RolePermissions = map[Role][]string{
	RolePlayer:    {PermProfileRead, PermProfileWrite, PermWorldJoin, PermAvatarEquip},
	RoleCreator:   {PermWorldCreate, PermWorldEdit, PermAvatarUpload, PermAssetUpload},
	RoleModerator: {PermUserWarn, PermUserMute, PermUserKick, PermReportReview},
	RoleAdmin:     {PermUserBan, PermRoleAssign, PermRoleRevoke, PermSystemConfig},
}

// RoleHierarchy defines inheritance order (higher inherits from lower)
var RoleHierarchy = map[Role]int{
	RolePlayer:    0,
	RoleCreator:   10,
	RoleModerator: 50,
	RoleAdmin:     100,
}

// GetPermissions returns all permissions for a role, including inherited ones.
func GetPermissions(role Role) []string {
	level := RoleHierarchy[role]
	var perms []string
	for r, l := range RoleHierarchy {
		if l <= level {
			perms = append(perms, RolePermissions[r]...)
		}
	}
	return perms
}

// HasPermission checks if a role has a specific permission.
func HasPermission(role Role, permission string) bool {
	for _, p := range GetPermissions(role) {
		if p == permission {
			return true
		}
	}
	return false
}
