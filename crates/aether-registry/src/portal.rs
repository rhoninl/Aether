use uuid::Uuid;

/// Supported portal URI schemes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalScheme {
    Aether,
    Https,
    StaticWorld,
}

/// A resolved portal route to a world instance.
#[derive(Debug, Clone)]
pub struct PortalRoute {
    pub world_slug: String,
    pub region: String,
    pub session_token: Option<String>,
    pub fallback_http: Option<String>,
}

/// Parsed aether:// URL with structured components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUrl {
    pub world_id: Uuid,
    pub spawn_point: Option<String>,
    pub instance_hint: Option<String>,
}

/// Errors when parsing a portal URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalError {
    InvalidScheme,
    MissingWorldId,
    InvalidWorldId,
    MalformedUrl,
}

/// Portal URL resolver for aether://, https://, and static:// schemes.
#[derive(Debug)]
pub struct PortalResolver;

impl PortalResolver {
    /// Parse a URI string into its scheme and remainder.
    pub fn parse(uri: &str) -> Option<(PortalScheme, &str)> {
        if let Some(rest) = uri.strip_prefix("aether://") {
            Some((PortalScheme::Aether, rest))
        } else if let Some(rest) = uri.strip_prefix("https://") {
            Some((PortalScheme::Https, rest))
        } else if let Some(rest) = uri.strip_prefix("static://") {
            Some((PortalScheme::StaticWorld, rest))
        } else {
            None
        }
    }

    /// Resolve a portal route to a WebSocket endpoint URL.
    pub fn resolve(route: &PortalRoute) -> String {
        match route.region.as_str() {
            "local" => format!("ws://127.0.0.1:9000/{}", route.world_slug),
            _ => format!("wss://{}.aether.gg/{}", route.region, route.world_slug),
        }
    }

    /// Parse a full aether:// URL into a structured `PortalUrl`.
    ///
    /// Format: `aether://<world_id>[/<spawn_point>][?instance=<hint>]`
    pub fn parse_portal_url(uri: &str) -> Result<PortalUrl, PortalError> {
        let rest = uri.strip_prefix("aether://").ok_or(PortalError::InvalidScheme)?;

        if rest.is_empty() {
            return Err(PortalError::MissingWorldId);
        }

        // Split on '?' first to separate query params
        let (path_part, query_part) = match rest.find('?') {
            Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
            None => (rest, None),
        };

        // Split path on '/' to get world_id and optional spawn_point
        let segments: Vec<&str> = path_part.split('/').filter(|s| !s.is_empty()).collect();

        if segments.is_empty() {
            return Err(PortalError::MissingWorldId);
        }

        let world_id =
            Uuid::parse_str(segments[0]).map_err(|_| PortalError::InvalidWorldId)?;

        let spawn_point = if segments.len() > 1 {
            Some(segments[1..].join("/"))
        } else {
            None
        };

        // Parse query parameters for instance hint
        let instance_hint = query_part.and_then(|q| {
            q.split('&')
                .find_map(|param| {
                    let (key, value) = param.split_once('=')?;
                    if key == "instance" {
                        Some(value.to_string())
                    } else {
                        None
                    }
                })
        });

        Ok(PortalUrl {
            world_id,
            spawn_point,
            instance_hint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aether_scheme() {
        let result = PortalResolver::parse("aether://some-world");
        assert!(result.is_some());
        let (scheme, rest) = result.unwrap();
        assert_eq!(scheme, PortalScheme::Aether);
        assert_eq!(rest, "some-world");
    }

    #[test]
    fn parse_https_scheme() {
        let result = PortalResolver::parse("https://example.com/world");
        assert!(result.is_some());
        let (scheme, rest) = result.unwrap();
        assert_eq!(scheme, PortalScheme::Https);
        assert_eq!(rest, "example.com/world");
    }

    #[test]
    fn parse_static_scheme() {
        let result = PortalResolver::parse("static://local-world");
        assert!(result.is_some());
        let (scheme, _) = result.unwrap();
        assert_eq!(scheme, PortalScheme::StaticWorld);
    }

    #[test]
    fn parse_unknown_scheme_returns_none() {
        assert!(PortalResolver::parse("ftp://something").is_none());
        assert!(PortalResolver::parse("just-text").is_none());
        assert!(PortalResolver::parse("").is_none());
    }

    #[test]
    fn resolve_local_region() {
        let route = PortalRoute {
            world_slug: "my-world".to_string(),
            region: "local".to_string(),
            session_token: None,
            fallback_http: None,
        };
        assert_eq!(
            PortalResolver::resolve(&route),
            "ws://127.0.0.1:9000/my-world"
        );
    }

    #[test]
    fn resolve_remote_region() {
        let route = PortalRoute {
            world_slug: "my-world".to_string(),
            region: "us-west".to_string(),
            session_token: None,
            fallback_http: None,
        };
        assert_eq!(
            PortalResolver::resolve(&route),
            "wss://us-west.aether.gg/my-world"
        );
    }

    #[test]
    fn parse_portal_url_basic() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.world_id, wid);
        assert!(result.spawn_point.is_none());
        assert!(result.instance_hint.is_none());
    }

    #[test]
    fn parse_portal_url_with_spawn_point() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}/lobby", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.world_id, wid);
        assert_eq!(result.spawn_point.as_deref(), Some("lobby"));
    }

    #[test]
    fn parse_portal_url_with_nested_spawn_point() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}/zone/entrance", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.spawn_point.as_deref(), Some("zone/entrance"));
    }

    #[test]
    fn parse_portal_url_with_instance_hint() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}?instance=abc-123", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.world_id, wid);
        assert_eq!(result.instance_hint.as_deref(), Some("abc-123"));
    }

    #[test]
    fn parse_portal_url_with_spawn_and_instance() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}/arena?instance=match-42", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.world_id, wid);
        assert_eq!(result.spawn_point.as_deref(), Some("arena"));
        assert_eq!(result.instance_hint.as_deref(), Some("match-42"));
    }

    #[test]
    fn parse_portal_url_invalid_scheme() {
        let result = PortalResolver::parse_portal_url("https://not-aether");
        assert_eq!(result, Err(PortalError::InvalidScheme));
    }

    #[test]
    fn parse_portal_url_missing_world_id() {
        let result = PortalResolver::parse_portal_url("aether://");
        assert_eq!(result, Err(PortalError::MissingWorldId));
    }

    #[test]
    fn parse_portal_url_invalid_world_id() {
        let result = PortalResolver::parse_portal_url("aether://not-a-uuid");
        assert_eq!(result, Err(PortalError::InvalidWorldId));
    }

    #[test]
    fn parse_portal_url_ignores_unknown_query_params() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}?foo=bar&instance=hint&baz=qux", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert_eq!(result.instance_hint.as_deref(), Some("hint"));
    }

    #[test]
    fn parse_portal_url_no_instance_param() {
        let wid = Uuid::new_v4();
        let url = format!("aether://{}?foo=bar", wid);
        let result = PortalResolver::parse_portal_url(&url).unwrap();
        assert!(result.instance_hint.is_none());
    }
}
