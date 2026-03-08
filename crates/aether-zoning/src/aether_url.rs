//! `aether://` URL scheme parser and resolver.
//!
//! Format: `aether://<host>/<world_id>[/<zone_id>][?spawn=<x,y,z>][&instance=<id>]`

/// Default port for aether:// URLs when none is specified.
const DEFAULT_AETHER_PORT: u16 = 7700;
/// The scheme prefix for aether URLs.
const AETHER_SCHEME: &str = "aether://";

/// A parsed `aether://` URL.
#[derive(Debug, Clone, PartialEq)]
pub struct AetherUrl {
    /// Server host (e.g., "worlds.aether.io").
    pub host: String,
    /// Port number (defaults to 7700).
    pub port: u16,
    /// Unique world identifier.
    pub world_id: String,
    /// Optional target zone within the world.
    pub zone_id: Option<String>,
    /// Optional spawn coordinates in the target world.
    pub spawn: Option<[f32; 3]>,
    /// Optional specific instance id.
    pub instance: Option<String>,
}

/// Errors that can occur when parsing an aether URL.
#[derive(Debug, Clone, PartialEq)]
pub enum AetherUrlError {
    /// URL does not start with `aether://`.
    InvalidScheme,
    /// Missing or empty host component.
    MissingHost,
    /// Missing world_id path component.
    MissingWorldId,
    /// Spawn coordinate string could not be parsed as three floats.
    InvalidSpawnCoordinates(String),
    /// Port is not a valid number.
    InvalidPort(String),
    /// General malformed URL.
    Malformed(String),
}

impl std::fmt::Display for AetherUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidScheme => write!(f, "URL must start with aether://"),
            Self::MissingHost => write!(f, "missing host in aether URL"),
            Self::MissingWorldId => write!(f, "missing world_id in aether URL"),
            Self::InvalidSpawnCoordinates(s) => {
                write!(f, "invalid spawn coordinates: {}", s)
            }
            Self::InvalidPort(s) => write!(f, "invalid port: {}", s),
            Self::Malformed(s) => write!(f, "malformed aether URL: {}", s),
        }
    }
}

impl AetherUrl {
    /// Parse an `aether://` URL string.
    pub fn parse(input: &str) -> Result<Self, AetherUrlError> {
        let remainder = input
            .strip_prefix(AETHER_SCHEME)
            .ok_or(AetherUrlError::InvalidScheme)?;

        if remainder.is_empty() {
            return Err(AetherUrlError::MissingHost);
        }

        // Split off query string
        let (path_part, query_part) = match remainder.find('?') {
            Some(idx) => (&remainder[..idx], Some(&remainder[idx + 1..])),
            None => (remainder, None),
        };

        // Split host from path segments
        let (host_port, path_segments) = match path_part.find('/') {
            Some(idx) => (&path_part[..idx], Some(&path_part[idx + 1..])),
            None => (path_part, None),
        };

        // Parse host and optional port
        let (host, port) = parse_host_port(host_port)?;

        if host.is_empty() {
            return Err(AetherUrlError::MissingHost);
        }

        // Parse path segments: /<world_id>[/<zone_id>]
        let path_str = path_segments.unwrap_or("");
        let segments: Vec<&str> = path_str
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err(AetherUrlError::MissingWorldId);
        }

        let world_id = segments[0].to_string();
        let zone_id = segments.get(1).map(|s| s.to_string());

        // Parse query parameters
        let mut spawn = None;
        let mut instance = None;

        if let Some(query) = query_part {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    match key {
                        "spawn" => {
                            spawn = Some(parse_spawn(value)?);
                        }
                        "instance" => {
                            instance = Some(value.to_string());
                        }
                        _ => {} // ignore unknown query params
                    }
                }
            }
        }

        Ok(AetherUrl {
            host,
            port,
            world_id,
            zone_id,
            spawn,
            instance,
        })
    }

    /// Reconstruct the URL as a string.
    pub fn to_url_string(&self) -> String {
        let mut url = format!("aether://{}", self.host);
        if self.port != DEFAULT_AETHER_PORT {
            url.push_str(&format!(":{}", self.port));
        }
        url.push_str(&format!("/{}", self.world_id));
        if let Some(ref zone) = self.zone_id {
            url.push_str(&format!("/{}", zone));
        }

        let mut query_parts = Vec::new();
        if let Some(ref spawn) = self.spawn {
            query_parts.push(format!("spawn={},{},{}", spawn[0], spawn[1], spawn[2]));
        }
        if let Some(ref instance) = self.instance {
            query_parts.push(format!("instance={}", instance));
        }
        if !query_parts.is_empty() {
            url.push('?');
            url.push_str(&query_parts.join("&"));
        }
        url
    }

    /// Returns the authority string (host:port).
    pub fn authority(&self) -> String {
        if self.port == DEFAULT_AETHER_PORT {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// Parse "host" or "host:port" into (host, port).
fn parse_host_port(input: &str) -> Result<(String, u16), AetherUrlError> {
    if let Some((host, port_str)) = input.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| AetherUrlError::InvalidPort(port_str.to_string()))?;
        Ok((host.to_string(), port))
    } else {
        Ok((input.to_string(), DEFAULT_AETHER_PORT))
    }
}

/// Parse "x,y,z" into [f32; 3].
fn parse_spawn(input: &str) -> Result<[f32; 3], AetherUrlError> {
    let parts: Vec<&str> = input.split(',').collect();
    if parts.len() != 3 {
        return Err(AetherUrlError::InvalidSpawnCoordinates(input.to_string()));
    }
    let x = parts[0]
        .trim()
        .parse::<f32>()
        .map_err(|_| AetherUrlError::InvalidSpawnCoordinates(input.to_string()))?;
    let y = parts[1]
        .trim()
        .parse::<f32>()
        .map_err(|_| AetherUrlError::InvalidSpawnCoordinates(input.to_string()))?;
    let z = parts[2]
        .trim()
        .parse::<f32>()
        .map_err(|_| AetherUrlError::InvalidSpawnCoordinates(input.to_string()))?;
    Ok([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Scheme validation ---

    #[test]
    fn rejects_http_scheme() {
        let result = AetherUrl::parse("http://example.com/world1");
        assert_eq!(result.unwrap_err(), AetherUrlError::InvalidScheme);
    }

    #[test]
    fn rejects_empty_string() {
        let result = AetherUrl::parse("");
        assert_eq!(result.unwrap_err(), AetherUrlError::InvalidScheme);
    }

    #[test]
    fn rejects_partial_scheme() {
        let result = AetherUrl::parse("aether:/missing-slash");
        assert_eq!(result.unwrap_err(), AetherUrlError::InvalidScheme);
    }

    // --- Host parsing ---

    #[test]
    fn rejects_missing_host() {
        let result = AetherUrl::parse("aether://");
        assert_eq!(result.unwrap_err(), AetherUrlError::MissingHost);
    }

    #[test]
    fn rejects_empty_host_with_path() {
        let result = AetherUrl::parse("aether:///world1");
        assert_eq!(result.unwrap_err(), AetherUrlError::MissingHost);
    }

    // --- World ID parsing ---

    #[test]
    fn rejects_missing_world_id() {
        let result = AetherUrl::parse("aether://example.com");
        assert_eq!(result.unwrap_err(), AetherUrlError::MissingWorldId);
    }

    #[test]
    fn rejects_missing_world_id_trailing_slash() {
        let result = AetherUrl::parse("aether://example.com/");
        assert_eq!(result.unwrap_err(), AetherUrlError::MissingWorldId);
    }

    // --- Successful minimal parse ---

    #[test]
    fn parses_minimal_url() {
        let url = AetherUrl::parse("aether://example.com/world1").unwrap();
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, DEFAULT_AETHER_PORT);
        assert_eq!(url.world_id, "world1");
        assert_eq!(url.zone_id, None);
        assert_eq!(url.spawn, None);
        assert_eq!(url.instance, None);
    }

    // --- Zone ID ---

    #[test]
    fn parses_url_with_zone_id() {
        let url = AetherUrl::parse("aether://example.com/world1/zone-north").unwrap();
        assert_eq!(url.world_id, "world1");
        assert_eq!(url.zone_id, Some("zone-north".to_string()));
    }

    // --- Port ---

    #[test]
    fn parses_custom_port() {
        let url = AetherUrl::parse("aether://example.com:9000/world1").unwrap();
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 9000);
    }

    #[test]
    fn rejects_invalid_port() {
        let result = AetherUrl::parse("aether://example.com:notaport/world1");
        assert!(matches!(result.unwrap_err(), AetherUrlError::InvalidPort(_)));
    }

    // --- Spawn coordinates ---

    #[test]
    fn parses_spawn_coordinates() {
        let url =
            AetherUrl::parse("aether://example.com/world1?spawn=10.5,20.0,30.5").unwrap();
        assert_eq!(url.spawn, Some([10.5, 20.0, 30.5]));
    }

    #[test]
    fn rejects_spawn_with_two_components() {
        let result = AetherUrl::parse("aether://example.com/world1?spawn=10,20");
        assert!(matches!(
            result.unwrap_err(),
            AetherUrlError::InvalidSpawnCoordinates(_)
        ));
    }

    #[test]
    fn rejects_spawn_with_non_numeric() {
        let result = AetherUrl::parse("aether://example.com/world1?spawn=x,y,z");
        assert!(matches!(
            result.unwrap_err(),
            AetherUrlError::InvalidSpawnCoordinates(_)
        ));
    }

    // --- Instance ---

    #[test]
    fn parses_instance_param() {
        let url = AetherUrl::parse("aether://example.com/world1?instance=abc-123").unwrap();
        assert_eq!(url.instance, Some("abc-123".to_string()));
    }

    // --- Combined ---

    #[test]
    fn parses_full_url_with_all_params() {
        let url = AetherUrl::parse(
            "aether://worlds.aether.io:8080/my-world/zone-a?spawn=1.0,2.0,3.0&instance=inst-1",
        )
        .unwrap();
        assert_eq!(url.host, "worlds.aether.io");
        assert_eq!(url.port, 8080);
        assert_eq!(url.world_id, "my-world");
        assert_eq!(url.zone_id, Some("zone-a".to_string()));
        assert_eq!(url.spawn, Some([1.0, 2.0, 3.0]));
        assert_eq!(url.instance, Some("inst-1".to_string()));
    }

    #[test]
    fn ignores_unknown_query_params() {
        let url =
            AetherUrl::parse("aether://example.com/world1?foo=bar&instance=x").unwrap();
        assert_eq!(url.instance, Some("x".to_string()));
        assert_eq!(url.spawn, None);
    }

    // --- Roundtrip ---

    #[test]
    fn roundtrip_minimal() {
        let url = AetherUrl::parse("aether://example.com/world1").unwrap();
        let s = url.to_url_string();
        assert_eq!(s, "aether://example.com/world1");
        let url2 = AetherUrl::parse(&s).unwrap();
        assert_eq!(url, url2);
    }

    #[test]
    fn roundtrip_with_port_and_zone() {
        let url = AetherUrl::parse("aether://host:9000/w/z").unwrap();
        let s = url.to_url_string();
        assert_eq!(s, "aether://host:9000/w/z");
        let url2 = AetherUrl::parse(&s).unwrap();
        assert_eq!(url, url2);
    }

    #[test]
    fn roundtrip_full() {
        let url = AetherUrl::parse(
            "aether://host:8080/world/zone?spawn=1,2,3&instance=i1",
        )
        .unwrap();
        let s = url.to_url_string();
        let url2 = AetherUrl::parse(&s).unwrap();
        assert_eq!(url.host, url2.host);
        assert_eq!(url.port, url2.port);
        assert_eq!(url.world_id, url2.world_id);
        assert_eq!(url.zone_id, url2.zone_id);
        assert_eq!(url.instance, url2.instance);
    }

    // --- Authority ---

    #[test]
    fn authority_without_custom_port() {
        let url = AetherUrl::parse("aether://example.com/world1").unwrap();
        assert_eq!(url.authority(), "example.com");
    }

    #[test]
    fn authority_with_custom_port() {
        let url = AetherUrl::parse("aether://example.com:9000/world1").unwrap();
        assert_eq!(url.authority(), "example.com:9000");
    }

    // --- Default port constant ---

    #[test]
    fn default_port_is_7700() {
        assert_eq!(DEFAULT_AETHER_PORT, 7700);
    }

    // --- Negative spawn values ---

    #[test]
    fn parses_negative_spawn() {
        let url =
            AetherUrl::parse("aether://example.com/w?spawn=-10.5,0,-30.5").unwrap();
        assert_eq!(url.spawn, Some([-10.5, 0.0, -30.5]));
    }
}
