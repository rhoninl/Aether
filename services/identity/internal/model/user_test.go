package model

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestRoleHierarchy(t *testing.T) {
	assert.Equal(t, 0, RoleHierarchy[RolePlayer])
	assert.Equal(t, 10, RoleHierarchy[RoleCreator])
	assert.Equal(t, 50, RoleHierarchy[RoleModerator])
	assert.Equal(t, 100, RoleHierarchy[RoleAdmin])
}

func TestGetPermissions_Player(t *testing.T) {
	perms := GetPermissions(RolePlayer)
	assert.Contains(t, perms, PermProfileRead)
	assert.Contains(t, perms, PermProfileWrite)
	assert.Contains(t, perms, PermWorldJoin)
	assert.Contains(t, perms, PermAvatarEquip)
	assert.NotContains(t, perms, PermWorldCreate)
	assert.NotContains(t, perms, PermUserBan)
}

func TestGetPermissions_Creator(t *testing.T) {
	perms := GetPermissions(RoleCreator)
	// Own permissions
	assert.Contains(t, perms, PermWorldCreate)
	assert.Contains(t, perms, PermWorldEdit)
	assert.Contains(t, perms, PermAvatarUpload)
	assert.Contains(t, perms, PermAssetUpload)
	// Inherited from player
	assert.Contains(t, perms, PermProfileRead)
	assert.Contains(t, perms, PermWorldJoin)
	// Should NOT have moderator/admin permissions
	assert.NotContains(t, perms, PermUserWarn)
	assert.NotContains(t, perms, PermUserBan)
}

func TestGetPermissions_Moderator(t *testing.T) {
	perms := GetPermissions(RoleModerator)
	// Own permissions
	assert.Contains(t, perms, PermUserWarn)
	assert.Contains(t, perms, PermUserMute)
	assert.Contains(t, perms, PermUserKick)
	assert.Contains(t, perms, PermReportReview)
	// Inherited from creator
	assert.Contains(t, perms, PermWorldCreate)
	// Inherited from player
	assert.Contains(t, perms, PermProfileRead)
	// Should NOT have admin permissions
	assert.NotContains(t, perms, PermUserBan)
	assert.NotContains(t, perms, PermRoleAssign)
}

func TestGetPermissions_Admin(t *testing.T) {
	perms := GetPermissions(RoleAdmin)
	// Own permissions
	assert.Contains(t, perms, PermUserBan)
	assert.Contains(t, perms, PermRoleAssign)
	assert.Contains(t, perms, PermRoleRevoke)
	assert.Contains(t, perms, PermSystemConfig)
	// Inherited from all lower roles
	assert.Contains(t, perms, PermProfileRead)
	assert.Contains(t, perms, PermWorldCreate)
	assert.Contains(t, perms, PermUserWarn)
}

func TestHasPermission(t *testing.T) {
	tests := []struct {
		name       string
		role       Role
		permission string
		expected   bool
	}{
		{"player can read profile", RolePlayer, PermProfileRead, true},
		{"player cannot create world", RolePlayer, PermWorldCreate, false},
		{"player cannot ban user", RolePlayer, PermUserBan, false},
		{"creator can create world", RoleCreator, PermWorldCreate, true},
		{"creator can read profile (inherited)", RoleCreator, PermProfileRead, true},
		{"creator cannot kick user", RoleCreator, PermUserKick, false},
		{"moderator can kick user", RoleModerator, PermUserKick, true},
		{"moderator can create world (inherited)", RoleModerator, PermWorldCreate, true},
		{"moderator cannot ban user", RoleModerator, PermUserBan, false},
		{"admin can ban user", RoleAdmin, PermUserBan, true},
		{"admin can assign roles", RoleAdmin, PermRoleAssign, true},
		{"admin can read profile (inherited)", RoleAdmin, PermProfileRead, true},
		{"admin can kick user (inherited)", RoleAdmin, PermUserKick, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := HasPermission(tt.role, tt.permission)
			assert.Equal(t, tt.expected, result)
		})
	}
}

func TestRolePermissions_NoOverlap(t *testing.T) {
	// Each role should have unique permissions (not duplicated from parent)
	seen := make(map[string]bool)
	for _, perms := range RolePermissions {
		for _, p := range perms {
			if seen[p] {
				t.Errorf("permission %s appears in multiple role definitions", p)
			}
			seen[p] = true
		}
	}
}
