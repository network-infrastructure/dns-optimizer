use serde::{Deserialize, Serialize};

const EMBEDDED_CATALOG: &str = include_str!("../../../data/dns_servers.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalProfile {
    Speed,
    Privacy,
    AdBlock,
    Family,
}

impl GoalProfile {
    pub fn label(&self) -> &'static str {
        match self {
            GoalProfile::Speed => "Maximum speed",
            GoalProfile::Privacy => "Privacy",
            GoalProfile::AdBlock => "Block ads",
            GoalProfile::Family => "Family protection",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "speed" | "fast" | "fastest" | "max-speed" | "maximum" => Some(Self::Speed),
            "privacy" | "private" | "no-log" => Some(Self::Privacy),
            "adblock" | "ad-block" | "ads" | "block-ads" | "adblocking" => Some(Self::AdBlock),
            "family" | "kids" | "safe" | "family-safe" => Some(Self::Family),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogFile {
    pub metadata: CatalogMetadata,
    pub servers: Vec<DnsServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogMetadata {
    pub title: String,
    pub version: String,
    pub verified_date: String,
    pub total_servers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsServer {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub category: String,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub doh: Option<String>,
    pub dot: Option<String>,
    pub doq: Option<String>,
    pub probe_url: Option<String>,
    #[serde(default)]
    pub probe_cors: bool,
    #[serde(default)]
    pub protocols: Vec<String>,
    #[serde(default)]
    pub dnssec: bool,
    #[serde(default)]
    pub qname_minimization: bool,
    #[serde(default)]
    pub ecs: String,
    #[serde(default)]
    pub filtering: Vec<String>,
    #[serde(default)]
    pub logging: String,
    #[serde(default)]
    pub privacy_score: u8,
    #[serde(default)]
    pub jurisdiction: String,
    #[serde(default)]
    pub anycast: bool,
    #[serde(default)]
    pub official_url: String,
    #[serde(default)]
    pub privacy_url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub routing: String,
}

impl DnsServer {
    pub fn primary_ipv4(&self) -> Option<&str> {
        self.ipv4.first().map(|s| s.as_str())
    }

    pub fn blocks_ads(&self) -> bool {
        self.filtering.iter().any(|f| {
            let l = f.to_ascii_lowercase();
            l.contains("ad") || l.contains("tracker")
        })
    }

    pub fn is_family_safe(&self) -> bool {
        self.filtering.iter().any(|f| {
            let l = f.to_ascii_lowercase();
            l.contains("adult") || l.contains("family") || l.contains("safe search")
        })
    }

    pub fn blocks_malware(&self) -> bool {
        self.filtering.iter().any(|f| {
            let l = f.to_ascii_lowercase();
            l.contains("malware") || l.contains("phishing") || l.contains("threat")
        })
    }

    pub fn matches_profile(&self, profile: GoalProfile) -> bool {
        match profile {
            GoalProfile::Speed => !self.ipv4.is_empty(),
            GoalProfile::Privacy => self.privacy_score >= 4 && self.dnssec,
            GoalProfile::AdBlock => self.blocks_ads(),
            GoalProfile::Family => self.is_family_safe(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub metadata: CatalogMetadata,
    pub servers: Vec<DnsServer>,
}

impl Catalog {
    pub fn load_embedded() -> Result<Self, serde_json::Error> {
        let parsed: CatalogFile = serde_json::from_str(EMBEDDED_CATALOG)?;
        Ok(Self {
            metadata: parsed.metadata,
            servers: parsed.servers,
        })
    }

    pub fn find_by_id(&self, id: &str) -> Option<&DnsServer> {
        self.servers
            .iter()
            .find(|s| s.id.eq_ignore_ascii_case(id) || s.primary_ipv4() == Some(id))
    }

    pub fn filter_by_profile(&self, profile: GoalProfile) -> Vec<&DnsServer> {
        self.servers
            .iter()
            .filter(|s| s.matches_profile(profile))
            .collect()
    }

    pub fn find_by_ip(&self, ip: &str) -> Option<&DnsServer> {
        self.servers
            .iter()
            .find(|s| s.ipv4.iter().any(|v| v == ip) || s.ipv6.iter().any(|v| v == ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_catalog() {
        let catalog = Catalog::load_embedded().expect("embedded catalog should parse");
        assert!(catalog.servers.len() >= 30);
        assert!(catalog.find_by_id("cloudflare-standard").is_some());
    }

    #[test]
    fn profile_filters_return_servers() {
        let catalog = Catalog::load_embedded().unwrap();
        assert!(!catalog.filter_by_profile(GoalProfile::Speed).is_empty());
        assert!(!catalog.filter_by_profile(GoalProfile::Privacy).is_empty());
        assert!(!catalog.filter_by_profile(GoalProfile::AdBlock).is_empty());
        assert!(!catalog.filter_by_profile(GoalProfile::Family).is_empty());
    }
}
