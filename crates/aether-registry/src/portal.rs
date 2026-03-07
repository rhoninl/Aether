#[derive(Debug, Clone)]
pub enum PortalScheme {
    Aether,
    Https,
    StaticWorld,
}

#[derive(Debug, Clone)]
pub struct PortalRoute {
    pub world_slug: String,
    pub region: String,
    pub session_token: Option<String>,
    pub fallback_http: Option<String>,
}

#[derive(Debug)]
pub struct PortalResolver;

impl PortalResolver {
    pub fn parse(uri: &str) -> Option<(PortalScheme, &str)> {
        if uri.starts_with("aether://") {
            Some((PortalScheme::Aether, &uri["aether://".len()..]))
        } else if uri.starts_with("https://") {
            Some((PortalScheme::Https, &uri["https://".len()..]))
        } else if uri.starts_with("static://") {
            Some((PortalScheme::StaticWorld, &uri["static://".len()..]))
        } else {
            None
        }
    }

    pub fn resolve(route: PortalRoute) -> String {
        match route.region.as_str() {
            "local" => format!("ws://127.0.0.1:9000/{}", route.world_slug),
            _ => format!("wss://{}.aether.gg/{}", route.region, route.world_slug),
        }
    }
}

