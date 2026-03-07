use std::collections::{HashMap, HashSet, VecDeque};

use aether_economy::{
    CurrencyLedger, EconomyTransaction, IdempotencyRecord, LedgerEntry, LedgerKind, SettlementState,
    SettlementStream, TransactionCoordinator, TransactionDirection, TransactionKind, TransactionState,
    WalletAccount, WalletOperation, WalletSummary,
};
use aether_economy::{PayoutDestination, PayoutRecord};
use aether_federation::{AuthCheckMode, FederationAuthRequest, FederationAuthResult};
use aether_security::{
    ActionKey, RateLimit, RateLimitBucket, WasmSandboxCapability, WasmSurfaceError,
};
use aether_compliance::{
    ComplianceKeystore, DeleteRequest, DeleteScope, KeyPurpose, KeystoreEntry, LegalHold, ProfileDeletion,
    RetentionRecord, RetentionState,
};
use aether_trust_safety::{
    AnonymousMode, ContentFilter, KickAction, PersonalSpaceBubble, ParentalControl,
    SafetySettings, VisibleScope, VisibilityMode, WorldOwnerToolset,
};
use aether_social::{
    ChatChannel, ChatMessage, ChatType, FriendRequest, FriendState, FriendStatus, Group, GroupConfig, GroupInvite,
    GroupInvite::Accepted, GroupInvite::Sent, GroupStatus, InWorldLocation, PresenceKind, PresenceState,
    PresenceVisibility, ShardMapPolicy,
};
use aether_registry::{
    DiscoveryFilter, DiscoveryResult, DiscoverySort, MatchCriteria, MatchOutcome, PortalResolver, PortalRoute,
    ServerInstance, SessionManager, SessionManagerPolicy, SessionState, WorldManifest, validate_manifest,
};
use crate::auth::{AuthValidationPolicy, AuthzResult, Token};
use crate::route::GeoRoutingPolicy;
use crate::relay::{NatMode, RelayProfile, RelayRegion, RelaySession};

#[derive(Debug, Clone)]
pub struct BackendRuntimeConfig {
    pub token_ttl_ms: u64,
    pub token_refresh_window_ms: u64,
    pub transaction_retention_days: u16,
    pub chat_history_limit: usize,
    pub social_shard_bits: u8,
    pub social_target_shards: u32,
    pub registry_match_policy: SessionManagerPolicy,
    pub auth_backends: Vec<String>,
    pub live_regions: Vec<String>,
    pub relay_profile: RelayProfile,
    pub relay_regions: Vec<String>,
    pub action_rate_limits: Vec<RateLimit>,
    pub rate_limit_window_ms: u64,
    pub abuse_ban_threshold: u32,
    pub abuse_ban_window_ms: u64,
    pub abuse_lockout_ms: u64,
    pub default_sandbox: WasmSandboxCapability,
    pub trusted_default_safety: SafetySettings,
    pub trusted_default_visibility: VisibleScope,
    pub trusted_default_parental: ParentalControl,
    pub trusted_default_tools: WorldOwnerToolset,
}

impl Default for BackendRuntimeConfig {
    fn default() -> Self {
        Self {
            token_ttl_ms: 3_600_000,
            token_refresh_window_ms: 120_000,
            transaction_retention_days: 30,
            chat_history_limit: 128,
            social_shard_bits: 0,
            social_target_shards: 8,
            registry_match_policy: SessionManagerPolicy {
                max_instances_per_region: 3,
                scale_up_threshold: 0.85,
                scale_down_threshold: 0.20,
                instance_idle_timeout_ms: 300_000,
                region_policy: aether_registry::RegionPolicy {
                    preferred_regions: vec!["us-east-1".to_string(), "eu-west-1".to_string()],
                    allow_cross_region_failover: true,
                    latency_budget_ms: 180,
                },
            },
            auth_backends: vec!["local".to_string(), "federation".to_string()],
            live_regions: vec!["us-east-1".to_string(), "eu-west-1".to_string(), "eu-central-1".to_string()],
            relay_profile: RelayProfile {
                service_name: "gateway-relay".to_string(),
                tls_terminated: true,
                nat_mode: NatMode::Stun,
            },
            relay_regions: vec![
                "us-east-1".to_string(),
                "eu-west-1".to_string(),
                "eu-central-1".to_string(),
            ],
            action_rate_limits: vec![
                RateLimit {
                    action: ActionKey::Move,
                    per_user_per_minute: 240,
                    burst: 40,
                },
                RateLimit {
                    action: ActionKey::Chat,
                    per_user_per_minute: 300,
                    burst: 60,
                },
                RateLimit {
                    action: ActionKey::Trade,
                    per_user_per_minute: 120,
                    burst: 20,
                },
                RateLimit {
                    action: ActionKey::ScriptRpc,
                    per_user_per_minute: 400,
                    burst: 100,
                },
                RateLimit {
                    action: ActionKey::VoiceFrame,
                    per_user_per_minute: 240,
                    burst: 60,
                },
                RateLimit {
                    action: ActionKey::InventoryAction,
                    per_user_per_minute: 240,
                    burst: 40,
                },
            ],
            rate_limit_window_ms: 60_000,
            abuse_ban_threshold: 18,
            abuse_ban_window_ms: 60_000,
            abuse_lockout_ms: 300_000,
            default_sandbox: WasmSandboxCapability {
                max_memory_pages: 128,
                max_cpu_ms: 180,
                max_file_reads: 32,
                max_net_calls_per_sec: 250,
                allowed_api: vec![
                    "spawn_entity".into(),
                    "despawn_entity".into(),
                    "set_entity_position".into(),
                    "entity_position".into(),
                    "apply_force".into(),
                    "raycast".into(),
                    "open_panel".into(),
                    "close_panel".into(),
                    "play_sound".into(),
                    "stop_sound".into(),
                    "emit_event".into(),
                    "send_rpc".into(),
                    "world_get".into(),
                    "world_set".into(),
                ],
            },
            trusted_default_safety: SafetySettings {
                personal_space: PersonalSpaceBubble {
                    enabled: false,
                    radius_m: 2.5,
                },
                anonymous_mode: AnonymousMode {
                    enabled: false,
                    expires_ms: None,
                },
                allow_voice: true,
            },
            trusted_default_visibility: VisibleScope {
                mode: VisibilityMode::Visible,
                include_friends: true,
            },
            trusted_default_parental: ParentalControl {
                enabled: false,
                filter: ContentFilter::Off,
                time_limit: Some(aether_trust_safety::TimeLimit {
                    minutes_per_day: 360,
                    hard_stop: true,
                }),
                social_allowed: true,
            },
            trusted_default_tools: WorldOwnerToolset {
                can_mute: true,
                can_kick: true,
                can_ban: false,
            },
        }
    }
}

#[derive(Debug)]
pub struct ScriptExecutionRequest {
    pub user_id: u64,
    pub request_id: String,
    pub artifact_id: String,
    pub requested_api: Vec<String>,
    pub memory_pages: u32,
    pub cpu_ms: u64,
    pub file_reads: u32,
    pub net_calls_per_sec: u32,
}

#[derive(Debug)]
pub enum ModerationCommand {
    Mute {
        actor_id: u64,
        target_id: u64,
        duration_ms: Option<u64>,
    },
    Kick {
        actor_id: u64,
        world_id: String,
        target_id: u64,
        reason: String,
        action: KickAction,
    },
    Ban {
        actor_id: u64,
        world_id: String,
        target_id: u64,
        reason: String,
    },
    Unmute {
        actor_id: u64,
        target_id: u64,
    },
    Unban {
        actor_id: u64,
        world_id: String,
        target_id: u64,
    },
}

#[derive(Debug)]
pub struct TrustSettingsUpdate {
    pub user_id: u64,
    pub safety: SafetySettings,
    pub visibility: VisibleScope,
    pub parental: ParentalControl,
    pub moderation_tools: WorldOwnerToolset,
}

#[derive(Debug)]
pub enum KeystoreCommandMode {
    Add,
    Remove,
}

#[derive(Debug)]
pub struct KeystoreCommand {
    pub requested_by: u64,
    pub mode: KeystoreCommandMode,
    pub entry: KeystoreEntry,
}

#[derive(Debug)]
pub struct PseudonymizationRequest {
    pub requested_by: u64,
    pub user_id: u64,
    pub pseudonym: String,
}

#[derive(Debug, Clone)]
struct RuntimeTrustProfile {
    safety: SafetySettings,
    visibility: VisibleScope,
    parental: ParentalControl,
    moderation_tools: WorldOwnerToolset,
}

#[derive(Debug)]
struct AbuseState {
    violation_count: u32,
    violation_window_start_ms: u64,
    blocked_until_ms: u64,
}

impl Default for RuntimeTrustProfile {
    fn default() -> Self {
        Self {
            safety: SafetySettings {
                personal_space: PersonalSpaceBubble {
                    enabled: false,
                    radius_m: 0.0,
                },
                anonymous_mode: AnonymousMode {
                    enabled: false,
                    expires_ms: None,
                },
                allow_voice: true,
            },
            visibility: VisibleScope {
                mode: VisibilityMode::Visible,
                include_friends: true,
            },
            parental: ParentalControl {
                enabled: false,
                filter: ContentFilter::Off,
                time_limit: Some(aether_trust_safety::TimeLimit {
                    minutes_per_day: 600,
                    hard_stop: true,
                }),
                social_allowed: true,
            },
            moderation_tools: WorldOwnerToolset {
                can_mute: false,
                can_kick: false,
                can_ban: false,
            },
        }
    }
}

impl Default for AbuseState {
    fn default() -> Self {
        Self {
            violation_count: 0,
            violation_window_start_ms: 0,
            blocked_until_ms: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendStepInput {
    pub now_ms: u64,
    pub auth_logins: Vec<u64>,
    pub token_refreshes: Vec<String>,
    pub auth_validations: Vec<(u64, String)>,
    pub federation_requests: Vec<FederationAuthRequest>,
    pub economy_transactions: Vec<EconomyTransaction>,
    pub friend_requests: Vec<FriendRequest>,
    pub group_invites: Vec<GroupInvite>,
    pub presence_updates: Vec<PresenceState>,
    pub chat_messages: Vec<ChatMessage>,
    pub manifest_upserts: Vec<WorldManifest>,
    pub discovery_filters: Vec<DiscoveryFilter>,
    pub route_requests: Vec<(String, String)>,
    pub relay_requests: Vec<(String, String)>,
    pub script_requests: Vec<ScriptExecutionRequest>,
    pub moderation_commands: Vec<ModerationCommand>,
    pub trust_updates: Vec<TrustSettingsUpdate>,
    pub delete_requests: Vec<DeleteRequest>,
    pub pseudonymization_requests: Vec<PseudonymizationRequest>,
    pub retention_updates: Vec<RetentionRecord>,
    pub keystore_commands: Vec<KeystoreCommand>,
}

impl BackendStepInput {
    pub fn empty() -> Self {
        Self {
            now_ms: 0,
            auth_logins: Vec::new(),
            token_refreshes: Vec::new(),
            auth_validations: Vec::new(),
            federation_requests: Vec::new(),
            economy_transactions: Vec::new(),
            friend_requests: Vec::new(),
            group_invites: Vec::new(),
            presence_updates: Vec::new(),
            chat_messages: Vec::new(),
            manifest_upserts: Vec::new(),
            discovery_filters: Vec::new(),
            route_requests: Vec::new(),
            relay_requests: Vec::new(),
            script_requests: Vec::new(),
            moderation_commands: Vec::new(),
            trust_updates: Vec::new(),
            delete_requests: Vec::new(),
            pseudonymization_requests: Vec::new(),
            retention_updates: Vec::new(),
            keystore_commands: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct BackendStepOutput {
    pub issued_tokens: Vec<Token>,
    pub auth_results: Vec<AuthzResult>,
    pub federation_results: Vec<FederationAuthResult>,
    pub transactions_committed: Vec<String>,
    pub transactions_rejected: Vec<String>,
    pub duplicate_transactions: usize,
    pub wallet_summaries: Vec<WalletSummary>,
    pub friend_status: Vec<FriendStatus>,
    pub group_count: usize,
    pub presence_count: usize,
    pub chat_delivered: Vec<String>,
    pub discovery_results: Vec<DiscoveryResult>,
    pub routed_instances: Vec<MatchOutcome>,
    pub shard_assignments: HashMap<u64, u32>,
    pub authz_denials: Vec<String>,
    pub rate_limit_denials: Vec<String>,
    pub abuse_mitigations: Vec<String>,
    pub wasm_rejections: Vec<String>,
    pub trust_rejections: Vec<String>,
    pub compliance_events: Vec<String>,
    pub script_allowance: usize,
    pub relay_sessions: Vec<RelaySession>,
    pub live_region_routing: Vec<String>,
}

#[derive(Debug)]
pub struct BackendRuntimeState {
    next_token_id: u64,
    active_tokens: HashMap<u64, Token>,
    token_user_index: HashMap<u64, String>,
    revoked_tokens: HashSet<String>,
    idempotent_transactions: HashMap<String, IdempotencyRecord>,
    wallets: HashMap<u64, WalletAccount>,
    ledger: CurrencyLedger,
    tx_coordinator: TransactionCoordinator,
    settlement_stream: SettlementStream,
    settlements: Vec<PayoutRecord>,
    friend_state: HashMap<(u64, u64), FriendState>,
    group_state: HashMap<String, Group>,
    presence_state: HashMap<u64, PresenceState>,
    chat_state: HashMap<String, VecDeque<ChatMessage>>,
    portal_routes: HashMap<String, String>,
    worlds: HashMap<String, WorldManifest>,
    session_managers: HashMap<String, SessionManager>,
    trust_profiles: HashMap<u64, RuntimeTrustProfile>,
    rate_limit_buckets: HashMap<(u64, ActionKey), RateLimitBucket>,
    abuse_states: HashMap<u64, AbuseState>,
    muted_users: HashMap<u64, u64>,
    ban_records: HashMap<(String, u64), u64>,
    pseudonyms: HashMap<u64, String>,
    parental_spend_ms: HashMap<u64, u64>,
    deletions: HashMap<u64, ProfileDeletion>,
    retention_records: Vec<RetentionRecord>,
    keystore: ComplianceKeystore,
}

impl Default for BackendRuntimeState {
    fn default() -> Self {
        Self {
            next_token_id: 0,
            active_tokens: HashMap::new(),
            token_user_index: HashMap::new(),
            revoked_tokens: HashSet::new(),
            idempotent_transactions: HashMap::new(),
            wallets: HashMap::new(),
            ledger: CurrencyLedger::new(),
            tx_coordinator: TransactionCoordinator::new(30),
            settlement_stream: SettlementStream::new("gateway::economy"),
            settlements: Vec::new(),
            friend_state: HashMap::new(),
            group_state: HashMap::new(),
            presence_state: HashMap::new(),
            chat_state: HashMap::new(),
            portal_routes: HashMap::new(),
            worlds: HashMap::new(),
            session_managers: HashMap::new(),
            trust_profiles: HashMap::new(),
            rate_limit_buckets: HashMap::new(),
            abuse_states: HashMap::new(),
            muted_users: HashMap::new(),
            ban_records: HashMap::new(),
            pseudonyms: HashMap::new(),
            parental_spend_ms: HashMap::new(),
            deletions: HashMap::new(),
            retention_records: Vec::new(),
            keystore: ComplianceKeystore { keys: Vec::new() },
        }
    }
}

pub struct BackendRuntime {
    cfg: BackendRuntimeConfig,
}

impl BackendRuntime {
    pub fn new(cfg: BackendRuntimeConfig) -> Self {
        Self { cfg }
    }

    pub fn default() -> Self {
        Self::new(BackendRuntimeConfig::default())
    }

    pub fn step(
        &self,
        input: BackendStepInput,
        state: &mut BackendRuntimeState,
        policy: &AuthValidationPolicy,
        geo: Option<&GeoRoutingPolicy>,
    ) -> BackendStepOutput {
        self.purge_expired_tokens(input.now_ms, state);
        self.purge_expired_idempotency_records(input.now_ms, state);
        self.purge_expired_retention_records(input.now_ms, state);
        self.purge_expired_rate_limits(input.now_ms, state);

        let mut output = BackendStepOutput::default();
        self.process_auth(
            input.now_ms,
            &input.auth_logins,
            &input.token_refreshes,
            &input.auth_validations,
            &mut output,
            policy,
            state,
        );
        self.process_trust_updates(
            input.now_ms,
            &input.trust_updates,
            &mut output,
            state,
        );
        self.process_federation(&input.federation_requests, &mut output, state);
        self.process_scripts(
            input.now_ms,
            &input.script_requests,
            &mut output,
            policy,
            state,
        );
        self.process_economy(
            input.now_ms,
            &input.economy_transactions,
            &mut output,
            policy,
            state,
        );
        self.process_social(
            input.now_ms,
            &input.friend_requests,
            &input.group_invites,
            &input.presence_updates,
            &input.chat_messages,
            &mut output,
            state,
            policy,
        );
        self.process_moderation(
            input.now_ms,
            &input.moderation_commands,
            &mut output,
            policy,
            state,
        );
        self.process_compliance(
            input.now_ms,
            &input.delete_requests,
            &input.pseudonymization_requests,
            &input.retention_updates,
            &input.keystore_commands,
            &mut output,
            policy,
            state,
        );
        self.process_registry(
            input.now_ms,
            &input.manifest_upserts,
            &input.discovery_filters,
            &input.route_requests,
            geo,
            &mut output,
            policy,
            state,
        );
        self.process_relay(input.now_ms, &input.relay_requests, geo, &mut output);
        output
    }

    fn purge_expired_rate_limits(&self, now_ms: u64, state: &mut BackendRuntimeState) {
        state.abuse_states.retain(|_, state| {
            now_ms.saturating_sub(state.violation_window_start_ms) <= self.cfg.abuse_ban_window_ms
        });
    }

    fn purge_expired_retention_records(&self, now_ms: u64, state: &mut BackendRuntimeState) {
        state.retention_records.retain(|record| {
            if record.state == RetentionState::Expired {
                return false;
            }
            if record.until_ms == 0 {
                return true;
            }
            now_ms <= record.until_ms
        });
    }

    fn purge_expired_tokens(&self, now_ms: u64, state: &mut BackendRuntimeState) {
        let mut expired = Vec::new();
        for (user_id, token) in state.active_tokens.iter() {
            if token.expires_ms <= now_ms {
                expired.push(*user_id);
            }
        }
        for user_id in expired {
            state.active_tokens.remove(&user_id);
            state.token_user_index.remove(&user_id);
        }
    }

    fn purge_expired_idempotency_records(&self, now_ms: u64, state: &mut BackendRuntimeState) {
        const MILLIS_PER_DAY: u64 = 24 * 60 * 60 * 1000;
        state.idempotent_transactions.retain(|_, record| {
            let ttl_ms = u64::from(record.ttl_days).saturating_mul(MILLIS_PER_DAY);
            now_ms.saturating_sub(record.created_ms) <= ttl_ms
        });
    }

    fn token_issuer(policy: &AuthValidationPolicy) -> String {
        policy
            .accepted_issuers
            .first()
            .cloned()
            .unwrap_or_else(|| "local".to_string())
    }

    fn issue_token(
        &self,
        now_ms: u64,
        user_id: u64,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) -> Token {
        let next_id = state.next_token_id.saturating_add(1);
        state.next_token_id = next_id;
        Token {
            user_id,
            token_id: format!("tok:{}:{}:{}", Self::token_issuer(policy), user_id, next_id),
            expires_ms: now_ms.saturating_add(self.cfg.token_ttl_ms),
        }
    }

    fn validate_token_signature(
        &self,
        user_id: u64,
        token: &Token,
        policy: &AuthValidationPolicy,
    ) -> bool {
        if !policy.require_signature {
            return true;
        }
        let mut parts = token.token_id.split(':');
        if parts.next() != Some("tok") {
            return false;
        }
        let issuer = match parts.next() {
            Some(value) => value,
            None => return false,
        };
        if !policy.accepted_issuers.is_empty() && !policy.accepted_issuers.iter().any(|value| value == issuer) {
            return false;
        }
        if !self.cfg.auth_backends.is_empty()
            && !self.cfg.auth_backends.iter().any(|backend| backend == issuer)
        {
            return false;
        }
        match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(token_user_id) => token_user_id == user_id,
            None => false,
        }
    }

    fn owner_for_token(&self, token_id: &str, state: &BackendRuntimeState) -> Option<u64> {
        state
            .token_user_index
            .iter()
            .find_map(|(user_id, cached_token)| {
                if cached_token == token_id {
                    Some(*user_id)
                } else {
                    None
                }
            })
    }

    fn trust_profile(&self, user_id: u64, state: &BackendRuntimeState) -> RuntimeTrustProfile {
        state
            .trust_profiles
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| RuntimeTrustProfile {
                safety: self.cfg.trusted_default_safety.clone(),
                visibility: self.cfg.trusted_default_visibility.clone(),
                parental: self.cfg.trusted_default_parental.clone(),
                moderation_tools: self.cfg.trusted_default_tools.clone(),
            })
    }

    fn has_active_session(
        &self,
        now_ms: u64,
        user_id: u64,
        policy: &AuthValidationPolicy,
        state: &BackendRuntimeState,
    ) -> bool {
        match state.active_tokens.get(&user_id) {
            Some(token) => {
                if state.revoked_tokens.contains(&token.token_id) {
                    return false;
                }
                if policy.require_signature && !self.validate_token_signature(user_id, token, policy) {
                    return false;
                }
                !policy.require_expiry_check || token.expires_ms >= now_ms
            }
            None => false,
        }
    }

    fn note_action_denial(
        &self,
        output: &mut BackendStepOutput,
        actor_id: u64,
        action: &str,
        reason: &str,
    ) {
        output.authz_denials.push(format!("{actor_id}:{action}:{reason}"));
    }

    fn check_rate_limit(
        &self,
        now_ms: u64,
        actor_id: u64,
        action: ActionKey,
        output: &mut BackendStepOutput,
        state: &mut BackendRuntimeState,
    ) -> bool {
        let limit = self
            .cfg
            .action_rate_limits
            .iter()
            .find(|entry| entry.action == action)
            .cloned()
            .unwrap_or(RateLimit {
                action,
                per_user_per_minute: 120,
                burst: 20,
            });

        let key = (actor_id, action);
        let mut bucket = state
            .rate_limit_buckets
            .get(&key)
            .cloned()
            .unwrap_or_else(|| RateLimitBucket {
                user_id: actor_id,
                action,
                window_start_ms: now_ms,
                allowance: i32::try_from(limit.per_user_per_minute.saturating_add(limit.burst)).unwrap_or(i32::MAX),
            });

        if now_ms.saturating_sub(bucket.window_start_ms) >= self.cfg.rate_limit_window_ms {
            bucket.window_start_ms = now_ms;
            bucket.allowance = i32::try_from(limit.per_user_per_minute.saturating_add(limit.burst))
                .unwrap_or(i32::MAX);
        }

        if bucket.allowance <= 0 {
            let abuse = self.record_abuse(now_ms, actor_id, state);
            output
                .rate_limit_denials
                .push(format!("rl:{action:?}:{actor_id}:{limit.per_user_per_minute}/{limit.burst}"));
            output
                .abuse_mitigations
                .push(format!("user:{actor_id}:rate:{abuse}"));
            state.rate_limit_buckets.insert(key, bucket);
            return false;
        }

        bucket.allowance -= 1;
        state.rate_limit_buckets.insert(key, bucket);
        true
    }

    fn record_abuse(&self, now_ms: u64, actor_id: u64, state: &mut BackendRuntimeState) -> u32 {
        let mut abuse = state
            .abuse_states
            .remove(&actor_id)
            .unwrap_or_else(AbuseState::default);

        if now_ms.saturating_sub(abuse.violation_window_start_ms) > self.cfg.abuse_ban_window_ms {
            abuse.violation_count = 0;
            abuse.violation_window_start_ms = now_ms;
        }

        abuse.violation_count = abuse.violation_count.saturating_add(1);
        abuse.violation_window_start_ms = abuse.violation_window_start_ms.min(now_ms);
        if abuse.violation_count >= self.cfg.abuse_ban_threshold {
            abuse.blocked_until_ms = now_ms.saturating_add(self.cfg.abuse_lockout_ms);
        }
        state.abuse_states.insert(actor_id, abuse.clone());
        abuse.violation_count
    }

    fn is_action_allowed(
        &self,
        now_ms: u64,
        actor_id: u64,
        action: ActionKey,
        action_name: &str,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
        output: &mut BackendStepOutput,
    ) -> bool {
        if let Some(abuse) = state.abuse_states.get(&actor_id)
            && abuse.blocked_until_ms > now_ms
        {
            self.note_action_denial(output, actor_id, action_name, "abuse_lockout");
            return false;
        }

        if !self.has_active_session(now_ms, actor_id, policy, state) {
            self.note_action_denial(output, actor_id, action_name, "missing_auth");
            return false;
        }

        let profile = self.trust_profile(actor_id, state);
        if profile.parental.enabled && !profile.parental.social_allowed {
            self.note_action_denial(output, actor_id, action_name, "parental_social_block");
            return false;
        }

        if !self.check_rate_limit(now_ms, actor_id, action, output, state) {
            return false;
        }
        true
    }

    fn is_muted(&self, actor_id: u64, now_ms: u64, state: &BackendRuntimeState) -> bool {
        state
            .muted_users
            .get(&actor_id)
            .is_some_and(|until_ms| *until_ms == u64::MAX || *until_ms > now_ms)
    }

    fn is_banned_from_world(
        &self,
        actor_id: u64,
        world_id: &str,
        now_ms: u64,
        state: &BackendRuntimeState,
    ) -> bool {
        let global = state
            .ban_records
            .get(&(String::from("GLOBAL"), actor_id))
            .is_some_and(|until_ms| *until_ms == u64::MAX || *until_ms > now_ms);
        let world_ban = state
            .ban_records
            .get(&(world_id.to_string(), actor_id))
            .is_some_and(|until_ms| *until_ms == u64::MAX || *until_ms > now_ms);
        global || world_ban
    }

    fn pseudonym_for(&self, user_id: u64, state: &mut BackendRuntimeState) -> String {
        if let Some(pseudo) = state.pseudonyms.get(&user_id) {
            return pseudo.clone();
        }
        let generated = format!("anon-{user_id}");
        state.pseudonyms.insert(user_id, generated.clone());
        generated
    }

    fn enforce_parental_spend(
        &self,
        now_ms: u64,
        actor_id: u64,
        profile: &RuntimeTrustProfile,
        state: &mut BackendRuntimeState,
        output: &mut BackendStepOutput,
    ) -> bool {
        let Some(limit) = profile.parental.time_limit.as_ref() else {
            return true;
        };
        let spend_entry = state.parental_spend_ms.entry(actor_id).or_insert(0);
        let budget_ms = u64::from(limit.minutes_per_day).saturating_mul(60_000);
        if *spend_entry >= budget_ms {
            if limit.hard_stop {
                self.note_action_denial(output, actor_id, "parental", "daily_budget_exceeded");
                return false;
            }
            return false;
        }
        *spend_entry = spend_entry.saturating_add((now_ms % 60_000).saturating_add(1000));
        if *spend_entry > budget_ms {
            self.note_action_denial(output, actor_id, "parental", "daily_budget_exceeded");
            return false;
        }
        true
    }

    fn check_personal_space(
        &self,
        actor_id: u64,
        target_id: u64,
        now_ms: u64,
        state: &mut BackendRuntimeState,
        output: &mut BackendStepOutput,
    ) -> bool {
        let target_profile = self.trust_profile(target_id, state);
        if !target_profile.safety.personal_space.enabled || target_profile.safety.personal_space.radius_m <= 0.0 {
            return true;
        }

        let actor_loc = state
            .presence_state
            .get(&actor_id)
            .and_then(|presence| presence.in_world.as_ref());
        let target_loc = state
            .presence_state
            .get(&target_id)
            .and_then(|presence| presence.in_world.as_ref());
        if actor_loc.is_none() || target_loc.is_none() {
            return true;
        }
        let actor_loc = actor_loc.unwrap();
        let target_loc = target_loc.unwrap();
        if actor_loc.world_id != target_loc.world_id {
            return true;
        }
        if target_profile.safety.personal_space.radius_m <= 0.0 {
            return true;
        }
        let dx = actor_loc.x - target_loc.x;
        let dy = actor_loc.y - target_loc.y;
        let dz = actor_loc.z - target_loc.z;
        let distance_sq = dx * dx + dy * dy + dz * dz;
        if distance_sq <= target_profile.safety.personal_space.radius_m * target_profile.safety.personal_space.radius_m {
            output
                .trust_rejections
                .push(format!("personal_space:{actor_id}:{target_id}:{now_ms}"));
            return false;
        }
        true
    }

    fn process_auth(
        &self,
        now_ms: u64,
        logins: &[u64],
        refreshes: &[String],
        validations: &[(u64, String)],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for &user_id in logins {
            let token = self.issue_token(now_ms, user_id, policy, state);
            state.active_tokens.insert(user_id, token.clone());
            state.token_user_index.insert(user_id, token.token_id.clone());
            output.issued_tokens.push(token);
        }

        for old in refreshes {
            let user_id = self.owner_for_token(old, state);
            if let Some(user_id) = user_id {
                let token = match state.active_tokens.get(&user_id) {
                    Some(token) => token.clone(),
                    None => continue,
                };

                if token.token_id != *old {
                    continue;
                }
                if self.validate_token_signature(user_id, &token, policy)
                    && now_ms.saturating_sub(token.expires_ms) <= self.cfg.token_refresh_window_ms
                {
                    let renewed = self.issue_token(now_ms, user_id, policy, state);
                    state.active_tokens.insert(user_id, renewed.clone());
                    state.token_user_index.insert(user_id, renewed.token_id.clone());
                    output.issued_tokens.push(renewed);
                }
            }
        }

        for (user_id, token_id) in validations {
            match state.active_tokens.get(user_id) {
                Some(token) => {
                    if token.token_id != *token_id {
                        output.auth_results.push(AuthzResult::Denied("invalid token".to_string()));
                        continue;
                    }
                    if state.revoked_tokens.contains(&token.token_id) {
                        output.auth_results.push(AuthzResult::Denied("revoked token".to_string()));
                        continue;
                    }
                    if policy.require_signature && !self.validate_token_signature(*user_id, token, policy) {
                        output.auth_results.push(AuthzResult::Denied("invalid signature".to_string()));
                        continue;
                    }
                    if policy.require_expiry_check && token.expires_ms < now_ms {
                        output.auth_results.push(AuthzResult::Expired);
                        continue;
                    }
                    output.auth_results.push(AuthzResult::Allowed);
                }
                None => {
                    output.auth_results.push(AuthzResult::Denied("invalid token".to_string()));
                }
            }
        }
    }

    fn process_federation(
        &self,
        requests: &[FederationAuthRequest],
        output: &mut BackendStepOutput,
        state: &BackendRuntimeState,
    ) {
        for req in requests {
            let has_owner = self.owner_for_token(&req.session_token, state) == Some(req.player_id);
            let has_live_token = state
                .active_tokens
                .get(&req.player_id)
                .is_some_and(|token| token.token_id == req.session_token);
            let allowed = match req.mode {
                AuthCheckMode::Disabled => true,
                AuthCheckMode::LocalFallback => self.local_fallback_auth(&req.session_token) || has_live_token,
                AuthCheckMode::CentralToken => {
                    self.central_token_auth(&req.session_token) && has_live_token && has_owner
                }
            };

            output.federation_results.push(FederationAuthResult {
                allowed,
                reason: if allowed {
                    None
                } else {
                    Some("federation auth denied".into())
                },
                central_verified: matches!(req.mode, AuthCheckMode::CentralToken) && allowed,
            });
        }
    }

    fn local_fallback_auth(&self, token: &str) -> bool {
        token.len() > 5
    }

    fn central_token_auth(&self, token: &str) -> bool {
        token.starts_with("federation:")
    }

    fn process_economy(
        &self,
        now_ms: u64,
        txs: &[EconomyTransaction],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for tx in txs {
            if !self.is_action_allowed(
                now_ms,
                tx.player_id,
                ActionKey::Trade,
                "economy",
                policy,
                state,
                output,
            ) {
                output.transactions_rejected.push(tx.tx_id.clone());
                continue;
            }
            if self.is_muted(tx.player_id, now_ms, state) {
                output
                    .trust_rejections
                    .push(format!("economy_muted:{}", tx.player_id));
                output.transactions_rejected.push(tx.tx_id.clone());
                continue;
            }
            let profile = self.trust_profile(tx.player_id, state);
            if !self.enforce_parental_spend(now_ms, tx.player_id, &profile, state, output) {
                output.transactions_rejected.push(tx.tx_id.clone());
                continue;
            }

            if state.deletions.contains_key(&tx.player_id) {
                output
                    .compliance_events
                    .push(format!("delete_gated_economy:{}", tx.player_id));
                output.transactions_rejected.push(tx.tx_id.clone());
                continue;
            }

            if state.idempotent_transactions.contains_key(&tx.tx_id) {
                output.duplicate_transactions = output.duplicate_transactions.saturating_add(1);
                continue;
            }

            let _ = state.tx_coordinator.enqueue(tx.clone());
            state.idempotent_transactions.insert(
                tx.tx_id.clone(),
                IdempotencyRecord {
                    tx_id: tx.tx_id.clone(),
                    key_v7: tx.tx_id.clone(),
                    state: TransactionState::Queued,
                    ttl_days: self.cfg.transaction_retention_days,
                    created_ms: now_ms,
                },
            );

            let operation = match tx.direction {
                TransactionDirection::Purchase | TransactionDirection::Tip | TransactionDirection::Payout => {
                    WalletOperation::Withdraw(tx.amount_minor)
                }
                TransactionDirection::Sale | TransactionDirection::Reward => WalletOperation::Deposit(tx.amount_minor),
            };

            let wallet = state
                .wallets
                .entry(tx.player_id)
                .or_insert_with(|| WalletAccount::new(format!("wallet-{}", tx.player_id), tx.player_id));
            if !wallet.apply(&operation) {
                output.transactions_rejected.push(tx.tx_id.clone());
                state
                    .idempotent_transactions
                    .insert(
                        tx.tx_id.clone(),
                        IdempotencyRecord {
                            tx_id: tx.tx_id.clone(),
                            key_v7: tx.tx_id.clone(),
                            state: TransactionState::Rejected,
                            ttl_days: self.cfg.transaction_retention_days,
                            created_ms: now_ms,
                        },
                    );
                continue;
            }

            let debit_entry = LedgerEntry {
                world_id: tx.world_id.clone(),
                actor_id: tx.player_id,
                tx_id: tx.tx_id.clone(),
                kind: LedgerKind::Debit,
                currency: tx.currency.clone(),
                amount_minor: tx.amount_minor,
                created_ms: now_ms,
                memo: tx.memo.clone(),
            };
            let credit_entry = LedgerEntry {
                world_id: tx.world_id.clone(),
                actor_id: 0,
                tx_id: tx.tx_id.clone(),
                kind: LedgerKind::Credit,
                currency: tx.currency.clone(),
                amount_minor: tx.amount_minor,
                created_ms: now_ms,
                memo: tx.memo.clone(),
            };

            if state.ledger.post_double_entry(debit_entry, credit_entry).is_ok() {
                output.transactions_committed.push(tx.tx_id.clone());
                output.wallet_summaries.push(wallet.summary(now_ms));
                state
                    .idempotent_transactions
                    .insert(
                        tx.tx_id.clone(),
                        IdempotencyRecord {
                            tx_id: tx.tx_id.clone(),
                            key_v7: tx.tx_id.clone(),
                            state: TransactionState::Committed,
                            ttl_days: self.cfg.transaction_retention_days,
                            created_ms: now_ms,
                        },
                    );
                state.tx_coordinator.mark_settled(&tx.tx_id);
            } else {
                output.transactions_rejected.push(tx.tx_id.clone());
                state
                    .idempotent_transactions
                    .insert(
                        tx.tx_id.clone(),
                        IdempotencyRecord {
                            tx_id: tx.tx_id.clone(),
                            key_v7: tx.tx_id.clone(),
                            state: TransactionState::Rejected,
                            ttl_days: self.cfg.transaction_retention_days,
                            created_ms: now_ms,
                        },
                    );
                continue;
            }

            if let TransactionDirection::Payout = tx.direction {
                let payout = PayoutRecord {
                    payout_id: format!("payout-{}", tx.tx_id),
                    tx_id: tx.tx_id.clone(),
                    destination: PayoutDestination::Wallet {
                        wallet_id: wallet.wallet_id.clone(),
                    },
                    amount_minor: tx.amount_minor,
                    fee_minor: 0,
                    state: SettlementState::Queued,
                };
                state.settlement_stream.enqueue(payout.clone());
                state.settlements.push(payout);
            }
        }
    }

    fn process_social(
        &self,
        now_ms: u64,
        friend_requests: &[FriendRequest],
        group_invites: &[GroupInvite],
        presence_updates: &[PresenceState],
        chat_messages: &[ChatMessage],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for request in friend_requests {
            match request {
                FriendRequest::Send { from, to, .. } => {
                    if !self
                        .is_action_allowed(now_ms, *from, ActionKey::InventoryAction, "friend.send", policy, state, output)
                    {
                        continue;
                    }
                    if self.is_muted(*from, now_ms, state) {
                        self.note_action_denial(output, *from, "friend.send", "muted");
                        continue;
                    }
                    if !self.check_personal_space(*from, *to, now_ms, state, output) {
                        self.note_action_denial(output, *from, "friend.send", "personal_space");
                        continue;
                    }
                    let pair = ordered_pair(*from, *to);
                    state.friend_state.insert(pair, FriendState::Pending);
                    output.friend_status.push(FriendStatus {
                        user_a: pair.0,
                        user_b: pair.1,
                        state: FriendState::Pending,
                        initiated_ms: now_ms,
                    });
                }
                FriendRequest::Accept { from, to } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *from,
                        ActionKey::InventoryAction,
                        "friend.accept",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    if self.is_muted(*from, now_ms, state) {
                        continue;
                    }
                    let pair = ordered_pair(*from, *to);
                    state.friend_state.insert(pair, FriendState::Accepted);
                    output.friend_status.push(FriendStatus {
                        user_a: pair.0,
                        user_b: pair.1,
                        state: FriendState::Accepted,
                        initiated_ms: now_ms,
                    });
                }
                FriendRequest::Reject { from, to } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *from,
                        ActionKey::InventoryAction,
                        "friend.reject",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let pair = ordered_pair(*from, *to);
                    state.friend_state.insert(pair, FriendState::Rejected);
                    output.friend_status.push(FriendStatus {
                        user_a: pair.0,
                        user_b: pair.1,
                        state: FriendState::Rejected,
                        initiated_ms: now_ms,
                    });
                }
                FriendRequest::Block { from, to } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *from,
                        ActionKey::InventoryAction,
                        "friend.block",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let pair = ordered_pair(*from, *to);
                    state.friend_state.insert(pair, FriendState::Blocked);
                    output.friend_status.push(FriendStatus {
                        user_a: pair.0,
                        user_b: pair.1,
                        state: FriendState::Blocked,
                        initiated_ms: now_ms,
                    });
                }
            }
        }

        for invite in group_invites {
            match invite {
                Sent {
                    group_id,
                    inviter,
                    invitee,
                } => {
                    if !self
                        .is_action_allowed(now_ms, *inviter, ActionKey::InventoryAction, "group.invite", policy, state, output)
                    {
                        continue;
                    }
                    if !self.check_personal_space(*inviter, *invitee, now_ms, state, output) {
                        self.note_action_denial(output, *inviter, "group.invite", "personal_space");
                        continue;
                    }
                    let group = state.group_state.entry(group_id.clone()).or_insert_with(|| Group {
                        group_id: group_id.clone(),
                        owner_id: *inviter,
                        members: vec![*inviter, *invitee],
                        config: GroupConfig {
                            name: format!("group-{group_id}"),
                            max_members: 64,
                            invite_only: false,
                            public_listing: true,
                        },
                        status: GroupInvite::Sent {
                            group_id: group_id.clone(),
                            inviter: *inviter,
                            invitee: *invitee,
                        }
                        .into_group_status(),
                    });
                    if !group.members.contains(invitee) {
                        group.members.push(*invitee);
                    }
                }
                Accepted {
                    group_id,
                    invitee,
                } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *invitee,
                        ActionKey::InventoryAction,
                        "group.accept",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    if let Some(group) = state.group_state.get_mut(group_id) {
                        if !group.members.contains(invitee) {
                            group.members.push(*invitee);
                        }
                    }
                }
                GroupInvite::Declined { group_id, invitee } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *invitee,
                        ActionKey::InventoryAction,
                        "group.decline",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    if let Some(group) = state.group_state.get_mut(group_id) {
                        group.members.retain(|member| member != invitee);
                        group.status = GroupStatus::Archived;
                    }
                }
            }
        }

        for presence in presence_updates {
            if !self.is_action_allowed(
                now_ms,
                presence.user_id,
                ActionKey::Move,
                "presence",
                policy,
                state,
                output,
            ) {
                continue;
            }
            if self.is_muted(presence.user_id, now_ms, state) {
                self.note_action_denial(output, presence.user_id, "presence", "muted");
                continue;
            }
            let profile = self.trust_profile(presence.user_id, state);
            let world_id = match &presence.in_world {
                Some(location) => location.world_id.clone(),
                None => String::new(),
            };
            if self.is_banned_from_world(presence.user_id, &world_id, now_ms, state) {
                self.note_action_denial(output, presence.user_id, "presence", "banned");
                continue;
            }

            let location = if profile.visibility.mode == VisibilityMode::Invisible {
                None
            } else {
                presence.in_world.clone()
            };

            let mut record = presence.clone();
            if let Some(in_world) = location {
                record.in_world = Some(in_world);
            } else {
                record.in_world = Some(InWorldLocation {
                    world_id: String::new(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    zone: None,
                });
            }
            if profile.safety.anonymous_mode.enabled {
                record.visibility = PresenceVisibility::Hidden;
            }
            record.updated_ms = now_ms;
            state.presence_state.insert(presence.user_id, record);
        }

        for message in chat_messages {
            if !self.is_action_allowed(
                now_ms,
                message.from_user,
                ActionKey::Chat,
                "chat.send",
                policy,
                state,
                output,
            ) {
                output.trust_rejections.push(format!("chat_rejected:{}", message.message_id));
                continue;
            }
            if self.is_muted(message.from_user, now_ms, state) {
                self.note_action_denial(output, message.from_user, "chat.send", "muted");
                continue;
            }
            let profile = self.trust_profile(message.from_user, state);
            if !self.enforce_parental_spend(now_ms, message.from_user, &profile, state, output) {
                continue;
            }
            if profile.safety.anonymous_mode.enabled {
                output
                    .compliance_events
                    .push(format!("anonymous_chat:{}:{}", message.from_user, self.pseudonym_for(message.from_user, state)));
            }
            let mut message = message.clone();
            if profile.visibility.mode == VisibilityMode::Invisible && !matches!(message.kind, ChatType::SystemAnnouncement(_)) {
                self.note_action_denial(output, message.from_user, "chat.send", "invisible");
            } else {
                message.server_ts_ms = now_ms;
                let history = state
                    .chat_state
                    .entry(message.channel_id.clone())
                    .or_insert_with(|| {
                        let kind = if message
                            .channel_id
                            .as_bytes()
                            .first()
                            .is_some_and(|b| b % 2 == 0)
                        {
                            ChatType::World
                        } else {
                            ChatType::DirectMessage
                        };
                        let _ = ChatChannel {
                            channel_id: message.channel_id.clone(),
                            kind,
                            members: Vec::new(),
                        };
                        VecDeque::new()
                    });

                if history.len() >= self.cfg.chat_history_limit {
                    history.pop_front();
                }
                history.push_back(message.clone());
                output.chat_delivered.push(message.message_id.clone());
            }
        }

        if !state.group_state.is_empty() {
            output.group_count = state.group_state.len();
        }
        output.presence_count = state.presence_state.len();

        let users = presence_updates.iter().map(|presence| presence.user_id).collect::<Vec<_>>();
        output.shard_assignments = self.route_social_shards(users);
    }

    fn process_trust_updates(
        &self,
        now_ms: u64,
        updates: &[TrustSettingsUpdate],
        output: &mut BackendStepOutput,
        state: &mut BackendRuntimeState,
    ) {
        for update in updates {
            let existing = self
                .trust_profile(update.user_id, state);
            state.trust_profiles.insert(
                update.user_id,
                RuntimeTrustProfile {
                    safety: update.safety.clone(),
                    visibility: update.visibility.clone(),
                    parental: update.parental.clone(),
                    moderation_tools: update.moderation_tools.clone(),
                },
            );
            output.compliance_events.push(format!(
                "trust_updated:{}:{}:{}",
                update.user_id,
                existing.safety.allow_voice,
                now_ms,
            ));
        }
    }

    fn process_scripts(
        &self,
        now_ms: u64,
        scripts: &[ScriptExecutionRequest],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        let _ = now_ms;
        for request in scripts {
            if !self.is_action_allowed(
                now_ms,
                request.user_id,
                ActionKey::ScriptRpc,
                "script.request",
                policy,
                state,
                output,
            ) {
                output.wasm_rejections.push(format!("script:{}:unauth", request.request_id));
                continue;
            }

            let mut reasons = Vec::new();
            for api in &request.requested_api {
                if !self.cfg.default_sandbox.allowed_api.contains(api) {
                    reasons.push(WasmSurfaceError::ApiViolation);
                    output.wasm_rejections.push(format!(
                        "script:{}:api:{}",
                        request.request_id, api
                    ));
                }
            }
            if request.memory_pages > self.cfg.default_sandbox.max_memory_pages {
                reasons.push(WasmSurfaceError::ResourceQuotaExceeded);
                output.wasm_rejections.push(format!(
                    "script:{}:memory:{}>{}",
                    request.request_id, request.memory_pages, self.cfg.default_sandbox.max_memory_pages
                ));
            }
            if request.cpu_ms > self.cfg.default_sandbox.max_cpu_ms {
                reasons.push(WasmSurfaceError::ResourceQuotaExceeded);
                output.wasm_rejections.push(format!(
                    "script:{}:cpu:{}>{}",
                    request.request_id, request.cpu_ms, self.cfg.default_sandbox.max_cpu_ms
                ));
            }
            if request.file_reads > self.cfg.default_sandbox.max_file_reads {
                reasons.push(WasmSurfaceError::ResourceQuotaExceeded);
                output
                    .wasm_rejections
                    .push(format!("script:{}:files:{}>{}", request.request_id, request.file_reads, self.cfg.default_sandbox.max_file_reads));
            }
            if request.net_calls_per_sec > self.cfg.default_sandbox.max_net_calls_per_sec {
                reasons.push(WasmSurfaceError::ResourceQuotaExceeded);
                output.wasm_rejections.push(format!(
                    "script:{}:net:{}>{}",
                    request.request_id,
                    request.net_calls_per_sec,
                    self.cfg.default_sandbox.max_net_calls_per_sec
                ));
            }

            if reasons.is_empty() {
                output.script_allowance += 1;
            }
        }
    }

    fn process_moderation(
        &self,
        now_ms: u64,
        commands: &[ModerationCommand],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for cmd in commands {
            match cmd {
                ModerationCommand::Mute {
                    actor_id,
                    target_id,
                    duration_ms,
                } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *actor_id,
                        ActionKey::InventoryAction,
                        "mod.mute",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let profile = self.trust_profile(*actor_id, state);
                    if !profile.moderation_tools.can_mute {
                        output
                            .trust_rejections
                            .push(format!("mute_denied:{}:{}:tool"));
                        continue;
                    }
                    let until = duration_ms.unwrap_or(0);
                    state
                        .muted_users
                        .insert(*target_id, if until == 0 { u64::MAX } else { now_ms.saturating_add(until) });
                    output
                        .trust_rejections
                        .push(format!("mute_applied:{}:{}", actor_id, target_id));
                }
                ModerationCommand::Kick {
                    actor_id,
                    world_id,
                    target_id,
                    action,
                    reason,
                } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *actor_id,
                        ActionKey::InventoryAction,
                        "mod.kick",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let profile = self.trust_profile(*actor_id, state);
                    if !profile.moderation_tools.can_kick {
                        output
                            .trust_rejections
                            .push(format!("kick_denied:{}:{}:tool", actor_id, target_id));
                        continue;
                    }
                    let duration = match action {
                        KickAction::Evict => 0,
                        KickAction::Warn => 300,
                    };
                    state
                        .ban_records
                        .insert((world_id.clone(), *target_id), now_ms.saturating_add(duration));
                    output.trust_rejections.push(format!(
                        "kick_applied:{}:{}:{}:{}",
                        actor_id, target_id, world_id, reason
                    ));
                }
                ModerationCommand::Ban {
                    actor_id,
                    world_id,
                    target_id,
                    reason,
                } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *actor_id,
                        ActionKey::InventoryAction,
                        "mod.ban",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let profile = self.trust_profile(*actor_id, state);
                    if !profile.moderation_tools.can_ban {
                        output
                            .trust_rejections
                            .push(format!("ban_denied:{}:{}:tool", actor_id, target_id));
                        continue;
                    }
                    state.ban_records.insert((world_id.clone(), *target_id), u64::MAX);
                    output
                        .trust_rejections
                        .push(format!("ban_applied:{}:{}:{}:{}", actor_id, target_id, world_id, reason));
                }
                ModerationCommand::Unmute { actor_id, target_id } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *actor_id,
                        ActionKey::InventoryAction,
                        "mod.unmute",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let profile = self.trust_profile(*actor_id, state);
                    if !profile.moderation_tools.can_mute {
                        output
                            .trust_rejections
                            .push(format!("unmute_denied:{}:{}:tool", actor_id, target_id));
                        continue;
                    }
                    state.muted_users.remove(target_id);
                    output
                        .trust_rejections
                        .push(format!("unmute_applied:{}:{}", actor_id, target_id));
                }
                ModerationCommand::Unban {
                    actor_id,
                    world_id,
                    target_id,
                } => {
                    if !self.is_action_allowed(
                        now_ms,
                        *actor_id,
                        ActionKey::InventoryAction,
                        "mod.unban",
                        policy,
                        state,
                        output,
                    ) {
                        continue;
                    }
                    let profile = self.trust_profile(*actor_id, state);
                    if !profile.moderation_tools.can_ban {
                        output
                            .trust_rejections
                            .push(format!("unban_denied:{}:{}:tool", actor_id, target_id));
                        continue;
                    }
                    state.ban_records.remove(&(world_id.clone(), *target_id));
                    output
                        .trust_rejections
                        .push(format!("unban_applied:{}:{}:{}", actor_id, target_id, world_id));
                }
            }
        }
    }

    fn process_compliance(
        &self,
        now_ms: u64,
        delete_requests: &[DeleteRequest],
        pseudonyms: &[PseudonymizationRequest],
        retention_updates: &[RetentionRecord],
        keystore_commands: &[KeystoreCommand],
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for request in delete_requests {
            if !self.is_action_allowed(
                now_ms,
                request.requested_by,
                ActionKey::InventoryAction,
                "delete.request",
                policy,
                state,
                output,
            ) {
                continue;
            }
            match &request.legal_hold {
                LegalHold::Active { case_id, .. } => {
                    output
                        .compliance_events
                        .push(format!("delete_blocked:{}:{}", request.user_id, case_id));
                    continue;
                }
                LegalHold::Expired | LegalHold::None => {}
            }

            let deletion = ProfileDeletion {
                user_id: request.user_id,
                scope: request.scope.clone(),
                started_ms: now_ms,
                requested_by: request.requested_by,
                status: "completed".into(),
            };
            state.friend_state.retain(|pair, _| !pair.0.eq(&request.user_id) && !pair.1.eq(&request.user_id));
            state.pseudonyms.remove(&request.user_id);
            let affects_chat = request
                .scope
                .iter()
                .any(|scope| matches!(scope, &DeleteScope::All | &DeleteScope::Chat));
            state.chat_state.values_mut().for_each(|messages| {
                messages.retain(|message| {
                    !affects_chat && message.from_user != request.user_id
                });
            });
            state.presence_state.remove(&request.user_id);
            state.deletions.insert(request.user_id, deletion);
            output
                .compliance_events
                .push(format!("delete_applied:{}", request.user_id));
        }

        for request in pseudonyms {
            if !self.is_action_allowed(
                now_ms,
                request.requested_by,
                ActionKey::InventoryAction,
                "pseudonymize",
                policy,
                state,
                output,
            ) {
                continue;
            }
            state
                .pseudonyms
                .insert(request.user_id, request.pseudonym.clone());
            output
                .compliance_events
                .push(format!("pseudonym_applied:{}:{}", request.user_id, request.pseudonym));
        }

        for request in retention_updates {
            let mut record = request.clone();
            if record.until_ms == 0 {
                record.state = RetentionState::Expired;
            } else {
                record.state = RetentionState::Active;
            }
            state.retention_records.push(record);
            output.compliance_events.push(format!(
                "retention_updated:{}:{}:{}",
                request.table_name, request.row_id, request.until_ms
            ));
        }

        for request in keystore_commands {
            if !self.is_action_allowed(
                now_ms,
                request.requested_by,
                ActionKey::InventoryAction,
                "keystore",
                policy,
                state,
                output,
            ) {
                continue;
            }
            match request.mode {
                KeystoreCommandMode::Add => {
                    if request.entry.approver_ids.len() < 2 && request.entry.purpose == KeyPurpose::LegalHold {
                        output
                            .compliance_events
                            .push(format!("keystore_add_blocked:{}", request.entry.key_id));
                        continue;
                    }
                    if !state.keystore.keys.iter().any(|entry| entry.key_id == request.entry.key_id) {
                        state.keystore.keys.push(request.entry.clone());
                    }
                    output
                        .compliance_events
                        .push(format!("keystore_added:{}", request.entry.key_id));
                }
                KeystoreCommandMode::Remove => {
                    state.keystore.keys.retain(|entry| entry.key_id != request.entry.key_id);
                    output
                        .compliance_events
                        .push(format!("keystore_removed:{}", request.entry.key_id));
                }
            }
        }
    }

    fn route_social_shards(&self, users: Vec<u64>) -> HashMap<u64, u32> {
        let shard_count = if self.cfg.social_shard_bits > 0 {
            1u32.saturating_shl(self.cfg.social_shard_bits.min(31).into())
        } else {
            self.cfg.social_target_shards.max(1)
        };
        users
            .into_iter()
            .map(|user_id| (user_id, ShardMapPolicy::shard_for_user(user_id, shard_count)))
            .collect()
    }

    fn process_registry(
        &self,
        now_ms: u64,
        manifests: &[WorldManifest],
        discovery_filters: &[DiscoveryFilter],
        route_requests: &[(String, String)],
        geo: Option<&GeoRoutingPolicy>,
        output: &mut BackendStepOutput,
        policy: &AuthValidationPolicy,
        state: &mut BackendRuntimeState,
    ) {
        for manifest in manifests {
            if !self.is_action_allowed(
                now_ms,
                manifest.owner_id,
                ActionKey::Move,
                "registry.manifest",
                policy,
                state,
                output,
            ) {
                output
                    .trust_rejections
                    .push(format!("registry_denied:{}", manifest.world_id));
                continue;
            }
            if validate_manifest(manifest).is_err() {
                continue;
            }
            state.worlds.insert(manifest.world_id.clone(), manifest.clone());
            let portal = self.resolve_portal_via_geo(manifest, geo);
            state.portal_routes.insert(manifest.slug.clone(), portal);

            let policy = &self.cfg.registry_match_policy;
            let manager = state
                .session_managers
                .entry(manifest.world_id.clone())
                .or_insert_with(|| SessionManager::new(manifest.world_id.clone()));

            if manager.instances.is_empty() && !policy.region_policy.preferred_regions.is_empty() {
                let seed_region = &policy.region_policy.preferred_regions[0];
                self.create_instance_for_region(manifest.world_id.as_str(), manager, seed_region, policy);
            }
        }

        for filter in discovery_filters {
            output.discovery_results.push(self.discover_worlds(filter, state));
        }

        for (world_id, requested_region) in route_requests {
            let outcome = state
                .session_managers
                .get_mut(world_id)
                .map_or(MatchOutcome::NotFound, |manager| {
                    self.route_world_to_instance(world_id, requested_region, manager, geo)
                });
            output.routed_instances.push(outcome);
        }
    }

    fn process_relay(
        &self,
        now_ms: u64,
        relay_requests: &[(String, String)],
        geo: Option<&GeoRoutingPolicy>,
        output: &mut BackendStepOutput,
    ) {
        let mut sequence: u64 = 0;
        for (request_id, requested_region) in relay_requests {
            sequence = sequence.saturating_add(1);
            let region = self.resolve_live_region(requested_region, geo);
            let relay = RelaySession {
                call_id: format!("{request_id}:{now_ms}:{sequence}"),
                profile: RelayProfile {
                    service_name: self.cfg.relay_profile.service_name.clone(),
                    tls_terminated: self.cfg.relay_profile.tls_terminated,
                    nat_mode: self.cfg.relay_profile.nat_mode,
                },
                relay_region: RelayRegion {
                    region_code: region.clone(),
                    stun_address: format!("stun.{region}:3478"),
                    turn_address: format!("turn.{region}:3478"),
                },
            };
            output.relay_sessions.push(relay);
            output
                .live_region_routing
                .push(format!("relay_route:{request_id}:{region}"));
        }
    }

    fn resolve_live_region(&self, requested_region: &str, geo: Option<&GeoRoutingPolicy>) -> String {
        let mut candidates = Vec::new();
        if !requested_region.is_empty() {
            candidates.push(requested_region.to_string());
        }
        if let Some(geo) = geo {
            candidates.push(geo.fallback_region.clone());
        }
        candidates.extend(self.cfg.relay_regions.iter().cloned());
        candidates
            .into_iter()
            .find(|candidate| self.cfg.live_regions.iter().any(|region| region == candidate))
            .unwrap_or_else(|| self.cfg.live_regions.first().cloned().unwrap_or_else(|| "us-east-1".to_string()))
    }

    fn route_world_to_instance(
        &self,
        world_id: &str,
        requested_region: &str,
        manager: &mut SessionManager,
        geo: Option<&GeoRoutingPolicy>,
    ) -> MatchOutcome {
        let policy = &self.cfg.registry_match_policy;
        let mut candidate_regions = Vec::new();

        if !requested_region.is_empty() {
            candidate_regions.push(requested_region.to_string());
        }
        if let Some(geo) = geo {
            if !candidate_regions.contains(&geo.fallback_region) {
                candidate_regions.push(geo.fallback_region.clone());
            }
        }
        if policy.region_policy.allow_cross_region_failover {
            for region in &policy.region_policy.preferred_regions {
                if !candidate_regions.contains(region) {
                    candidate_regions.push(region.clone());
                }
            }
        }
        if candidate_regions.is_empty() {
            candidate_regions.push("us-east-1".to_string());
        }

        for region in &candidate_regions {
            if let Some(instance_id) = self.pick_instance_for_region(manager, region, policy) {
                return MatchOutcome::Assigned {
                    world_id: world_id.to_string(),
                    instance_id,
                    region: region.clone(),
                };
            }

            if self.can_create_instance_in_region(manager, region, policy) {
                let instance_id = self.create_instance_for_region(world_id, manager, region, policy);
                return MatchOutcome::Assigned {
                    world_id: world_id.to_string(),
                    instance_id,
                    region: region.clone(),
                };
            }
        }
        MatchOutcome::Busy
    }

    fn pick_instance_for_region(
        &self,
        manager: &mut SessionManager,
        region: &str,
        policy: &SessionManagerPolicy,
    ) -> Option<String> {
        let instance_capacity = (128.0_f32 * policy.scale_up_threshold.max(0.1)).max(1.0).round() as u32;
        let mut best_instance = None;
        let mut best_load = u32::MAX;

        for (instance_id, instance) in manager.instances.iter() {
            if instance.region == region
                && instance.population < instance_capacity
                && instance.population < best_load
            {
                best_load = instance.population;
                best_instance = Some(instance_id.clone());
            }
        }

        match best_instance {
            Some(instance_id) => {
                let instance = manager
                    .instances
                    .get_mut(&instance_id)
                    .expect("picked instance must still exist");
                instance.population = instance.population.saturating_add(1);
                Some(instance_id)
            }
            None => None,
        }
    }

    fn can_create_instance_in_region(
        &self,
        manager: &SessionManager,
        region: &str,
        policy: &SessionManagerPolicy,
    ) -> bool {
        if policy.max_instances_per_region == 0 {
            return true;
        }
        let instances_in_region = manager
            .instances
            .values()
            .filter(|instance| instance.region == region)
            .count() as u32;
        instances_in_region < policy.max_instances_per_region
    }

    fn create_instance_for_region(
        &self,
        world_id: &str,
        manager: &mut SessionManager,
        region: &str,
        _policy: &SessionManagerPolicy,
    ) -> String {
        let instance_id = format!("inst-{}-{}", world_id, manager.instances.len());
        manager.add_instance(ServerInstance {
            world_id: world_id.to_string(),
            instance_id: instance_id.clone(),
            region: region.to_string(),
            host: "127.0.0.1".to_string(),
            port: 9000,
            population: 1,
            state: SessionState::Running,
        });
        instance_id
    }

    fn discover_worlds(&self, filter: &DiscoveryFilter, state: &BackendRuntimeState) -> DiscoveryResult {
        let mut results: Vec<WorldManifest> = state
            .worlds
            .values()
            .filter(|manifest| {
                filter.criteria.search.as_ref().is_none_or(|search| {
                    manifest.name.contains(search) || manifest.slug.contains(search)
                })
                    && (!filter.criteria.featured_only || manifest.featured)
                    && self.manifest_matches_category(manifest, filter.criteria.categories.as_slice())
            })
            .filter(|manifest| {
                manifest.max_players >= filter.criteria.min_players && manifest.max_players <= filter.criteria.max_players
            })
            .cloned()
            .collect();

        match filter.sort {
            DiscoverySort::FeaturedFirst => {
                results.sort_by(|a, b| b.featured.cmp(&a.featured).then(a.name.cmp(&b.name)));
            }
            DiscoverySort::PlayerCountDesc => {
                results.sort_by(|a, b| b.max_players.cmp(&a.max_players));
            }
            DiscoverySort::RecentlyUpdated => {
                results.sort_by(|a, b| b.version.cmp(&a.version));
            }
            DiscoverySort::RegionNearest => {
                results.sort_by(|a, b| a.region_preference.cmp(&b.region_preference));
            }
        };

        let page = filter.page;
        let page_size = filter.page_size.max(1);
        let start = page.saturating_mul(page_size) as usize;
        let total = results.len();
        let page_items = if start >= total {
            Vec::new()
        } else {
            let end = (start + page_size as usize).min(total);
            results[start..end].to_vec()
        };

        DiscoveryResult {
            worlds: page_items,
            page,
            page_size,
            total,
        }
    }

    fn manifest_matches_category(&self, manifest: &WorldManifest, categories: &[String]) -> bool {
        if categories.is_empty() {
            return true;
        }
        let manifest_category = match &manifest.category {
            aether_registry::WorldCategory::Social => "Social",
            aether_registry::WorldCategory::PvE => "PvE",
            aether_registry::WorldCategory::PvP => "PvP",
            aether_registry::WorldCategory::Simulation => "Simulation",
            aether_registry::WorldCategory::Creative => "Creative",
            aether_registry::WorldCategory::Sandbox => "Sandbox",
            aether_registry::WorldCategory::Other(category) => category.as_str(),
        };

        categories.iter().any(|category| {
            category.eq_ignore_ascii_case(manifest_category)
        })
    }

    fn resolve_portal_via_geo(&self, manifest: &WorldManifest, geo: Option<&GeoRoutingPolicy>) -> String {
        let region = geo
            .map(|policy| policy.fallback_region.as_str())
            .unwrap_or_else(|| {
                self.cfg
                    .registry_match_policy
                    .region_policy
                    .preferred_regions
                    .first()
                    .map(String::as_str)
                    .unwrap_or("us-east-1")
            });

        let world_slug = if manifest.portal.is_empty() {
            manifest.slug.as_str()
        } else if let Some((_, route_slug)) = PortalResolver::parse(&manifest.portal) {
            route_slug
        } else {
            manifest.portal.as_str()
        };

        PortalResolver::resolve(PortalRoute {
            world_slug: world_slug.to_string(),
            region: region.to_string(),
            session_token: None,
            fallback_http: None,
        })
    }
}

fn ordered_pair(a: u64, b: u64) -> (u64, u64) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

trait InviteStatusExt {
    fn into_group_status(self) -> GroupStatus;
}

impl InviteStatusExt for GroupInvite {
    fn into_group_status(self) -> GroupStatus {
        match self {
            GroupInvite::Sent { .. } => GroupStatus::Created,
            GroupInvite::Accepted { .. } => GroupStatus::Active,
            GroupInvite::Declined { .. } => GroupStatus::Archived,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_and_federation_flow() {
        let mut state = BackendRuntimeState::default();
        let runtime = BackendRuntime::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: true,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };
        let out = runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![42],
                token_refreshes: Vec::new(),
                auth_validations: vec![(42, String::new())],
                federation_requests: vec![FederationAuthRequest {
                    world_id: "test".into(),
                    player_id: 42,
                    session_token: "federation:ok".into(),
                    mode: AuthCheckMode::CentralToken,
                }],
                economy_transactions: vec![],
                friend_requests: vec![],
                group_invites: vec![],
                presence_updates: vec![],
                chat_messages: vec![],
                manifest_upserts: vec![],
                discovery_filters: vec![],
                route_requests: vec![],
                relay_requests: Vec::new(),
                script_requests: Vec::new(),
                moderation_commands: Vec::new(),
                trust_updates: Vec::new(),
                delete_requests: Vec::new(),
                pseudonymization_requests: Vec::new(),
                retention_updates: Vec::new(),
                keystore_commands: Vec::new(),
            },
            &mut state,
            &policy,
            None,
        );

        assert!(!out.issued_tokens.is_empty());
        assert_eq!(out.auth_results.len(), 1);
        match &out.auth_results[0] {
            AuthzResult::Denied(message) => assert!(message.contains("invalid")),
            _ => panic!("expected denied for empty token"),
        }
        assert_eq!(out.federation_results.len(), 1);
        assert!(matches!(out.federation_results[0].allowed, false));
    }

    #[test]
    fn economy_idempotent_replay_and_wallet() {
        let mut state = BackendRuntimeState::default();
        let runtime = BackendRuntime::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };
        let tx = aether_economy::EconomyTransaction {
            tx_id: "tx-1".into(),
            player_id: 1,
            world_id: "w1".into(),
            amount_minor: 42,
            currency: "USD".into(),
            direction: TransactionDirection::Purchase,
            kind: TransactionKind::Sync,
            memo: None,
            created_ms: 1000,
        };

        let _ = runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![1],
                token_refreshes: vec![],
                auth_validations: vec![],
                federation_requests: vec![],
                economy_transactions: vec![tx.clone(), tx],
                friend_requests: vec![],
                group_invites: vec![],
                presence_updates: vec![],
                chat_messages: vec![],
                manifest_upserts: vec![],
                discovery_filters: vec![],
                route_requests: vec![],
                relay_requests: Vec::new(),
                script_requests: Vec::new(),
                moderation_commands: Vec::new(),
                trust_updates: Vec::new(),
                delete_requests: Vec::new(),
                pseudonymization_requests: Vec::new(),
                retention_updates: Vec::new(),
                keystore_commands: Vec::new(),
            },
            &mut state,
            &policy,
            None,
        );
        assert!(!state.wallets.is_empty());
        assert_eq!(state.wallets[&1].balance_minor, -42);
        assert_eq!(state.idempotent_transactions.len(), 1);
    }

    #[test]
    fn social_and_registry_integration() {
        let mut state = BackendRuntimeState::default();
        let runtime = BackendRuntime::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };
        let manifest = WorldManifest {
            world_id: "world-1".into(),
            slug: "w1".into(),
            name: "My World".into(),
            owner_id: 1,
            category: aether_registry::WorldCategory::Social,
            featured: true,
            max_players: 16,
            region_preference: vec!["us-east-1".into()],
            status: aether_registry::WorldStatus::Published,
            version: 1,
            portal: "w1".into(),
        };

        let out = runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![1],
                token_refreshes: vec![],
                auth_validations: vec![],
                federation_requests: vec![],
                economy_transactions: vec![],
                friend_requests: vec![FriendRequest::Send {
                    from: 1,
                    to: 2,
                    message: Some("hi".into()),
                }],
                group_invites: vec![GroupInvite::Sent {
                    group_id: "group".into(),
                    inviter: 1,
                    invitee: 2,
                }],
                presence_updates: vec![PresenceState {
                    user_id: 1,
                    kind: PresenceKind::Online,
                    visibility: PresenceVisibility::Visible,
                    in_world: None,
                    updated_ms: 1000,
                }],
                chat_messages: vec![ChatMessage {
                    message_id: "msg-1".into(),
                    from_user: 1,
                    channel_id: "world".into(),
                    kind: aether_social::MessageKind::Text("hello".into()),
                    server_ts_ms: 0,
                }],
                manifest_upserts: vec![manifest.clone()],
                discovery_filters: vec![DiscoveryFilter {
                    criteria: MatchCriteria {
                        search: Some("World".into()),
                        categories: vec!["Social".into()],
                        featured_only: false,
                        min_players: 0,
                        max_players: 128,
                    },
                    sort: DiscoverySort::FeaturedFirst,
                    page: 0,
                    page_size: 10,
                }],
                route_requests: vec![("world-1".into(), "us-east-1".into())],
                relay_requests: vec![("relay-1".into(), "us-east-1".into())],
                script_requests: Vec::new(),
                moderation_commands: Vec::new(),
                trust_updates: Vec::new(),
                delete_requests: Vec::new(),
                pseudonymization_requests: Vec::new(),
                retention_updates: Vec::new(),
                keystore_commands: Vec::new(),
            },
            &mut state,
            &policy,
            None,
        );

        assert_eq!(out.friend_status.len(), 1);
        assert_eq!(out.discovery_results.len(), 1);
        assert_eq!(out.routed_instances.len(), 1);
        assert_eq!(out.chat_delivered.len(), 1);
        assert!(state.worlds.contains_key("world-1"));
    }
}
