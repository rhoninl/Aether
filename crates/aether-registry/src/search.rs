use crate::manifest::WorldCategory;
use crate::registry::{EntryStatus, WorldEntry};

/// Default number of results per page.
const DEFAULT_PAGE_SIZE: usize = 20;
/// Maximum number of results per page.
const MAX_PAGE_SIZE: usize = 100;

/// Sort field for search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortField {
    Relevance,
    Rating,
    PlayerCount,
    Newest,
    VisitCount,
}

impl Default for SortField {
    fn default() -> Self {
        Self::Relevance
    }
}

/// Search query with multiple filter criteria.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub category: Option<WorldCategory>,
    pub tags: Vec<String>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
    pub min_rating: Option<f32>,
    pub sort_by: SortField,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            category: None,
            tags: Vec::new(),
            min_players: None,
            max_players: None,
            min_rating: None,
            sort_by: SortField::default(),
            limit: DEFAULT_PAGE_SIZE,
            offset: 0,
        }
    }
}

/// Paginated search results.
#[derive(Debug)]
pub struct SearchResult<'a> {
    pub worlds: Vec<&'a WorldEntry>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Execute a search query against a set of world entries.
pub fn search<'a>(worlds: &'a [&'a WorldEntry], query: &SearchQuery) -> SearchResult<'a> {
    let limit = query.limit.min(MAX_PAGE_SIZE).max(1);

    let mut filtered: Vec<&WorldEntry> = worlds
        .iter()
        .copied()
        .filter(|w| w.status != EntryStatus::Deleted)
        .filter(|w| match &query.category {
            Some(cat) => w.category == *cat,
            None => true,
        })
        .filter(|w| {
            if query.tags.is_empty() {
                true
            } else {
                query.tags.iter().any(|t| w.tags.contains(t))
            }
        })
        .filter(|w| match query.min_players {
            Some(min) => w.current_players >= min,
            None => true,
        })
        .filter(|w| match query.max_players {
            Some(max) => w.current_players <= max,
            None => true,
        })
        .filter(|w| match query.min_rating {
            Some(min) => w.rating >= min,
            None => true,
        })
        .filter(|w| match &query.text {
            Some(text) => {
                let lower = text.to_lowercase();
                w.name.to_lowercase().contains(&lower)
                    || w.description.to_lowercase().contains(&lower)
            }
            None => true,
        })
        .collect();

    let total = filtered.len();

    match query.sort_by {
        SortField::Rating => filtered.sort_by(|a, b| {
            b.rating
                .partial_cmp(&a.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortField::PlayerCount => {
            filtered.sort_by(|a, b| b.current_players.cmp(&a.current_players))
        }
        SortField::Newest => filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        SortField::VisitCount => filtered.sort_by(|a, b| b.visit_count.cmp(&a.visit_count)),
        SortField::Relevance => {
            // For relevance, featured first, then by visit count
            filtered.sort_by(|a, b| {
                b.featured
                    .cmp(&a.featured)
                    .then(b.visit_count.cmp(&a.visit_count))
            });
        }
    }

    let paginated: Vec<&WorldEntry> = filtered.into_iter().skip(query.offset).take(limit).collect();

    SearchResult {
        worlds: paginated,
        total,
        offset: query.offset,
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::make_entry;
    use uuid::Uuid;

    fn setup_worlds() -> Vec<WorldEntry> {
        let creator = Uuid::new_v4();
        let mut worlds = Vec::new();

        let mut w1 = make_entry("Sunset Social Club", creator);
        w1.category = WorldCategory::Social;
        w1.tags = vec!["chill".to_string(), "music".to_string()];
        w1.current_players = 42;
        w1.rating = 4.5;
        w1.visit_count = 10000;
        w1.featured = true;
        w1.created_at = 3000;
        worlds.push(w1);

        let mut w2 = make_entry("Battle Arena", creator);
        w2.category = WorldCategory::Game;
        w2.tags = vec!["pvp".to_string(), "action".to_string()];
        w2.current_players = 100;
        w2.rating = 3.8;
        w2.visit_count = 50000;
        w2.created_at = 2000;
        worlds.push(w2);

        let mut w3 = make_entry("Art Gallery", creator);
        w3.category = WorldCategory::Art;
        w3.tags = vec!["art".to_string(), "chill".to_string()];
        w3.current_players = 5;
        w3.rating = 4.9;
        w3.visit_count = 2000;
        w3.created_at = 1000;
        worlds.push(w3);

        let mut w4 = make_entry("Music Studio", creator);
        w4.category = WorldCategory::Music;
        w4.tags = vec!["music".to_string(), "creative".to_string()];
        w4.current_players = 15;
        w4.rating = 4.0;
        w4.visit_count = 8000;
        w4.created_at = 4000;
        worlds.push(w4);

        worlds
    }

    fn refs(worlds: &[WorldEntry]) -> Vec<&WorldEntry> {
        worlds.iter().collect()
    }

    #[test]
    fn search_no_filters_returns_all() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(&r, &SearchQuery::default());
        assert_eq!(result.total, 4);
        assert_eq!(result.worlds.len(), 4);
    }

    #[test]
    fn search_by_category() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                category: Some(WorldCategory::Game),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 1);
        assert_eq!(result.worlds[0].name, "Battle Arena");
    }

    #[test]
    fn search_by_tags_any_match() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                tags: vec!["music".to_string()],
                ..Default::default()
            },
        );
        assert_eq!(result.total, 2);
    }

    #[test]
    fn search_by_player_count_range() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                min_players: Some(10),
                max_players: Some(50),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 2);
        assert!(result.worlds.iter().all(|w| w.current_players >= 10));
        assert!(result.worlds.iter().all(|w| w.current_players <= 50));
    }

    #[test]
    fn search_by_min_rating() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                min_rating: Some(4.0),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 3);
        assert!(result.worlds.iter().all(|w| w.rating >= 4.0));
    }

    #[test]
    fn search_text_name() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                text: Some("arena".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 1);
        assert_eq!(result.worlds[0].name, "Battle Arena");
    }

    #[test]
    fn search_text_case_insensitive() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                text: Some("SUNSET".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 1);
        assert_eq!(result.worlds[0].name, "Sunset Social Club");
    }

    #[test]
    fn search_text_description() {
        let mut worlds = setup_worlds();
        worlds[0].description = "A cozy place to hang out with friends".to_string();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                text: Some("cozy".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 1);
    }

    #[test]
    fn search_multi_criteria() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                category: Some(WorldCategory::Social),
                min_rating: Some(4.0),
                tags: vec!["chill".to_string()],
                ..Default::default()
            },
        );
        assert_eq!(result.total, 1);
        assert_eq!(result.worlds[0].name, "Sunset Social Club");
    }

    #[test]
    fn search_empty_results() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                text: Some("nonexistent_xyz".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(result.total, 0);
        assert!(result.worlds.is_empty());
    }

    #[test]
    fn search_sort_by_rating() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::Rating,
                ..Default::default()
            },
        );
        assert_eq!(result.worlds[0].name, "Art Gallery"); // 4.9
        assert_eq!(result.worlds[1].name, "Sunset Social Club"); // 4.5
    }

    #[test]
    fn search_sort_by_player_count() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::PlayerCount,
                ..Default::default()
            },
        );
        assert_eq!(result.worlds[0].name, "Battle Arena"); // 100
    }

    #[test]
    fn search_sort_by_newest() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::Newest,
                ..Default::default()
            },
        );
        assert_eq!(result.worlds[0].name, "Music Studio"); // created_at=4000
    }

    #[test]
    fn search_sort_by_visit_count() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::VisitCount,
                ..Default::default()
            },
        );
        assert_eq!(result.worlds[0].name, "Battle Arena"); // 50000
    }

    #[test]
    fn search_relevance_featured_first() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::Relevance,
                ..Default::default()
            },
        );
        // Featured worlds come first in relevance
        assert!(result.worlds[0].featured);
    }

    #[test]
    fn search_pagination_offset() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                sort_by: SortField::Newest,
                limit: 2,
                offset: 0,
                ..Default::default()
            },
        );
        assert_eq!(result.worlds.len(), 2);
        assert_eq!(result.total, 4);

        let result2 = search(
            &r,
            &SearchQuery {
                sort_by: SortField::Newest,
                limit: 2,
                offset: 2,
                ..Default::default()
            },
        );
        assert_eq!(result2.worlds.len(), 2);
        assert_eq!(result2.total, 4);
        // Different pages should have different worlds
        assert_ne!(result.worlds[0].id, result2.worlds[0].id);
    }

    #[test]
    fn search_pagination_beyond_results() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                offset: 100,
                ..Default::default()
            },
        );
        assert_eq!(result.total, 4);
        assert!(result.worlds.is_empty());
    }

    #[test]
    fn search_limit_clamped_to_max() {
        let worlds = setup_worlds();
        let r = refs(&worlds);
        let result = search(
            &r,
            &SearchQuery {
                limit: 999,
                ..Default::default()
            },
        );
        // Should be clamped to MAX_PAGE_SIZE
        assert!(result.limit <= MAX_PAGE_SIZE);
    }

    #[test]
    fn search_excludes_deleted() {
        let mut worlds = setup_worlds();
        worlds[0].status = EntryStatus::Deleted;
        let r = refs(&worlds);
        let result = search(&r, &SearchQuery::default());
        assert_eq!(result.total, 3);
    }
}
