use std::collections::HashMap;

use crate::{
    AssetIntegrityPolicy, CentralServiceGate, FederationAssetReference, HashMismatchAction, ModifiedSinceApproval,
    FederationAuthRequest, FederationAuthResult, RegistrationState, SelfHostedWorld,
};

#[derive(Debug)]
pub struct FederationRuntimeConfig {
    pub gate: CentralServiceGate,
    pub integrity: AssetIntegrityPolicy,
    pub token_ttl_ms: u64,
    pub max_cache_tokens: usize,
}

impl Default for FederationRuntimeConfig {
    fn default() -> Self {
        Self {
            gate: CentralServiceGate {
                require_aec_routing: true,
                require_auth_service: true,
                require_registry_moderation: true,
            },
            integrity: AssetIntegrityPolicy {
                verify_download: true,
                require_signature: true,
                on_mismatch: HashMismatchAction::Report,
            },
            token_ttl_ms: 300_000,
            max_cache_tokens: 1_000,
        }
    }
}

#[derive(Debug)]
pub struct FederationRuntimeInput {
    pub now_ms: u64,
    pub auth_requests: Vec<FederationAuthRequest>,
    pub registered_worlds: Vec<SelfHostedWorld>,
    pub tx_requests: Vec<FederationTransactionRequest>,
    pub asset_checks: Vec<FederationAssetReference>,
}

#[derive(Debug)]
pub struct FederationRuntimeOutput {
    pub now_ms: u64,
    pub auth_results: Vec<FederationAuthResult>,
    pub world_state_events: Vec<String>,
    pub tx_routes: Vec<FederationTransactionResult>,
    pub asset_events: Vec<String>,
}

#[derive(Debug)]
pub struct FederationRuntime {
    cfg: FederationRuntimeConfig,
    state: FederationRuntimeState,
}

#[derive(Debug)]
struct FederationRuntimeState {
    auth_cache: HashMap<String, u64>,
    worlds: HashMap<String, SelfHostedWorld>,
    tx_seen: HashMap<String, u64>,
}

impl Default for FederationRuntimeState {
    fn default() -> Self {
        Self {
            auth_cache: HashMap::new(),
            worlds: HashMap::new(),
            tx_seen: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FederationTransactionRequest {
    pub tx_id: String,
    pub player_id: u64,
    pub world_id: String,
    pub session_token: String,
    pub central_only: bool,
    pub amount_minor: i64,
}

#[derive(Debug)]
pub struct FederationTransactionResult {
    pub tx_id: String,
    pub world_id: String,
    pub routed_to_central: bool,
    pub accepted: bool,
    pub reason: Option<String>,
}

impl Default for FederationRuntime {
    fn default() -> Self {
        Self::new(FederationRuntimeConfig::default())
    }
}

impl FederationRuntime {
    pub fn new(cfg: FederationRuntimeConfig) -> Self {
        Self {
            cfg,
            state: FederationRuntimeState::default(),
        }
    }

    pub fn step(&mut self, input: FederationRuntimeInput) -> FederationRuntimeOutput {
        let mut output = FederationRuntimeOutput {
            now_ms: input.now_ms,
            auth_results: Vec::new(),
            world_state_events: Vec::new(),
            tx_routes: Vec::new(),
            asset_events: Vec::new(),
        };

        for req in input.auth_requests {
            let result = self.process_auth(&req, input.now_ms);
            output.auth_results.push(result);
        }

        for world in input.registered_worlds {
            self.upsert_world(world, &mut output);
        }

        for tx in input.tx_requests {
            let result = self.route_transaction(tx, input.now_ms);
            output.tx_routes.push(result);
        }

        for asset in input.asset_checks {
            self.validate_asset(asset, &mut output);
        }

        self.purge_expired_cache(input.now_ms);
        output
    }

    fn process_auth(
        &mut self,
        req: &FederationAuthRequest,
        now_ms: u64,
    ) -> FederationAuthResult {
        let has_live_token = self
            .state
            .auth_cache
            .contains_key(&req.session_token)
            && self
                .state
                .auth_cache
                .get(&req.session_token)
                .copied()
                .is_some_and(|issued| now_ms.saturating_sub(issued) <= self.cfg.token_ttl_ms);
        let allowed = if self.cfg.gate.require_auth_service && req.mode == crate::auth::AuthCheckMode::CentralToken {
            req.session_token.starts_with("federation:")
                && has_live_token
        } else {
            !req.world_id.is_empty() && req.player_id > 0
        };

        if allowed {
            if self.state.auth_cache.len() >= self.cfg.max_cache_tokens {
                if let Some(oldest) = self.state.auth_cache.iter().map(|(k, v)| (k.clone(), *v)).min_by_key(|(_, v)| *v)
                {
                    self.state.auth_cache.remove(&oldest.0);
                }
            }
            self.state.auth_cache.insert(req.session_token.clone(), now_ms);
        }

        FederationAuthResult {
            allowed,
            reason: if allowed {
                None
            } else {
                Some("federation auth rejected".into())
            },
            central_verified: allowed && self.cfg.gate.require_auth_service && req.mode == crate::auth::AuthCheckMode::CentralToken,
        }
    }

    fn upsert_world(&mut self, mut world: SelfHostedWorld, output: &mut FederationRuntimeOutput) {
        let existed = self
            .state
            .worlds
            .insert(world.world_id.clone(), world.clone())
            .is_some();

        let approval = if self.cfg.gate.require_registry_moderation {
            match world.state {
                RegistrationState::Approved => ModifiedSinceApproval::No,
                RegistrationState::NeedsReview | RegistrationState::ModerationPending => {
                    ModifiedSinceApproval::Yes
                }
                _ => ModifiedSinceApproval::Yes,
            }
        } else {
            ModifiedSinceApproval::No
        };
        let endpoint_ok = !world.endpoint.is_empty();
        if !endpoint_ok {
            output.world_state_events.push(format!(
                "world_reject:{}:missing_endpoint",
                world.world_id
            ));
            world.state = RegistrationState::Rejected;
            self.state.worlds.insert(world.world_id.clone(), world);
            return;
        }

        if !matches!(approval, ModifiedSinceApproval::No) {
            output
                .world_state_events
                .push(format!("world_moderation:{}", world.world_id));
        }
        output.world_state_events.push(format!(
            "world:{}:{}:{}",
            if existed { "updated" } else { "registered" },
            world.world_id,
            if world.discovered { "discovered" } else { "pending_discovery" }
        ));
    }

    fn route_transaction(&mut self, tx: FederationTransactionRequest, now_ms: u64) -> FederationTransactionResult {
        if self.state.tx_seen.contains_key(&tx.tx_id) {
            return FederationTransactionResult {
                tx_id: tx.tx_id,
                world_id: tx.world_id,
                routed_to_central: false,
                accepted: false,
                reason: Some("duplicate_tx".into()),
            };
        }
        if tx.tx_id.is_empty() || tx.world_id.is_empty() || tx.amount_minor == 0 {
            return FederationTransactionResult {
                tx_id: tx.tx_id,
                world_id: tx.world_id,
                routed_to_central: false,
                accepted: false,
                reason: Some("invalid_payload".into()),
            };
        }

        let world = self.state.worlds.get(&tx.world_id);
        let can_route_central = self.cfg.gate.require_aec_routing && world.is_some_and(|entry| {
            if self.cfg.gate.require_auth_service {
                self.state
                    .auth_cache
                    .contains_key(&tx.session_token)
                    && !tx.session_token.is_empty()
            } else {
                entry.discovered
            }
        });
        let accepted = self.state.auth_cache.contains_key(&tx.session_token) || tx.central_only == false;
        self.state.tx_seen.insert(tx.tx_id.clone(), now_ms);

        FederationTransactionResult {
            tx_id: tx.tx_id,
            world_id: tx.world_id,
            routed_to_central: can_route_central,
            accepted,
            reason: if !accepted {
                Some("missing_auth".into())
            } else if can_route_central || self.cfg.gate.require_aec_routing {
                Some("routed_to_central_services".into())
            } else {
                Some("routed_local".into())
            },
        }
    }

    fn validate_asset(&self, asset: FederationAssetReference, output: &mut FederationRuntimeOutput) {
        if self.cfg.integrity.verify_download && !asset.approved {
            match self.cfg.integrity.on_mismatch {
                HashMismatchAction::Reject => {
                    output.asset_events.push(format!(
                        "asset_reject:{}:reject_unapproved",
                        asset.asset_id
                    ));
                }
                HashMismatchAction::Report => {
                    output.asset_events.push(format!(
                        "asset_warn:{}:requires_review",
                        asset.asset_id
                    ));
                }
                HashMismatchAction::Quarantine => {
                    output.asset_events.push(format!(
                        "asset_quarantine:{}:checksum_not_valid",
                        asset.asset_id
                    ));
                }
            }
            return;
        }

        if self.cfg.integrity.require_signature && !asset.sha256.starts_with("sig:") {
            output.asset_events.push(format!("asset_warn:{}:sig_missing", asset.asset_id));
            return;
        }
        output
            .asset_events
            .push(format!("asset_ok:{}:{}", asset.asset_id, if asset.approved { "approved" } else { "pending" }));
    }

    fn purge_expired_cache(&mut self, now_ms: u64) {
        self.state
            .auth_cache
            .retain(|_, expires_at| now_ms.saturating_sub(*expires_at) <= self.cfg.token_ttl_ms);
    }
}
