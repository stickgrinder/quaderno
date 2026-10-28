// SPDX-License-Identifier: GPL-3.0-or-later

//! Icon search for the icon picker (product spec §9).
//!
//! Matching uses, in order: the icon name, the curated synonym table, Phosphor's
//! own tags, then its categories. Icons used by the fixed scales are excluded,
//! and icons already used by another item of the same kind are listed last.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::timeline::normalize;

/// Phosphor's per-icon metadata: `name \t tags \t categories \t figma category`.
const PHOSPHOR_TAGS: &str = include_str!("../data/phosphor-tags.tsv");
/// The curated synonym table: `term \t icon,icon,…` (English and Italian).
const SYNONYMS: &str = include_str!("../data/icon-synonyms.tsv");

/// Icons used by the fixed scales (ratings and dream flags), which the picker
/// never offers (product spec §9, vault-spec Appendix B).
const FIXED_SCALE_ICONS: &[&str] = &[
    "smiley-sad",
    "smiley-nervous",
    "smiley-meh",
    "smiley",
    "smiley-wink",
    "battery-empty",
    "battery-low",
    "battery-medium",
    "battery-high",
    "battery-full",
    "cell-signal-none",
    "cell-signal-low",
    "cell-signal-medium",
    "cell-signal-high",
    "cell-signal-full",
    "eye",
    "alarm",
    "cloud-moon",
    "moon",
    "bed",
    "lightbulb-filament",
    "ghost",
    "repeat",
];

/// The parsed icon index.
pub struct IconIndex {
    all: Vec<String>,
    tags: HashMap<String, Vec<String>>,
    categories: HashMap<String, Vec<String>>,
    figma: HashMap<String, String>,
    synonyms: Vec<(String, Vec<String>)>,
}

impl IconIndex {
    /// The shared index, parsed once.
    pub fn shared() -> &'static IconIndex {
        static INDEX: OnceLock<IconIndex> = OnceLock::new();
        INDEX.get_or_init(IconIndex::parse)
    }

    fn parse() -> IconIndex {
        let mut all = Vec::new();
        let mut tags = HashMap::new();
        let mut categories = HashMap::new();
        let mut figma = HashMap::new();
        for line in PHOSPHOR_TAGS.lines() {
            let mut fields = line.split('\t');
            let Some(name) = fields.next() else { continue };
            if name.is_empty() {
                continue;
            }
            let icon_tags = fields
                .next()
                .unwrap_or_default()
                .split(',')
                .filter(|tag| !tag.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let icon_categories = fields
                .next()
                .unwrap_or_default()
                .split(',')
                .filter(|category| !category.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let icon_figma = fields.next().unwrap_or_default().to_owned();
            tags.insert(name.to_owned(), icon_tags);
            categories.insert(name.to_owned(), icon_categories);
            figma.insert(name.to_owned(), icon_figma);
            all.push(name.to_owned());
        }
        all.sort();

        let mut synonyms = Vec::new();
        for line in SYNONYMS.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let Some((term, icons)) = line.split_once('\t') else {
                continue;
            };
            let icons = icons
                .split(',')
                .map(str::trim)
                .filter(|icon| !icon.is_empty() && !FIXED_SCALE_ICONS.contains(icon))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !icons.is_empty() {
                synonyms.push((normalize(term), icons));
            }
        }

        IconIndex {
            all,
            tags,
            categories,
            figma,
            synonyms,
        }
    }

    /// The display categories, sorted.
    pub fn categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self.figma.values().cloned().collect();
        categories.sort();
        categories.dedup();
        categories
    }

    /// Icons grouped by their display category.
    pub fn by_category(&self) -> Vec<(String, Vec<String>)> {
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for (category, icon) in &self.figma {
            match groups.iter_mut().find(|(name, _)| name == category) {
                Some((_, icons)) => icons.push(icon.clone()),
                None => groups.push((category.clone(), vec![icon.clone()])),
            }
        }
        for (_, icons) in &mut groups {
            icons.sort();
        }
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        groups
    }

    /// Icons matching `query`, best first. `used` icons are listed last.
    pub fn search(&self, query: &str, used: &HashSet<String>) -> Vec<String> {
        let query = normalize(query.trim());
        if query.is_empty() {
            let mut items: Vec<(bool, &String)> = self
                .all
                .iter()
                .filter(|icon| !FIXED_SCALE_ICONS.contains(&icon.as_str()))
                .map(|icon| (used.contains(icon), icon))
                .collect();
            items.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
            return items.into_iter().map(|(_, icon)| icon.clone()).collect();
        }

        let mut ranked: Vec<(u8, bool, &String)> = Vec::new();
        for icon in &self.all {
            if FIXED_SCALE_ICONS.contains(&icon.as_str()) {
                continue;
            }
            let rank = if normalize(icon).contains(&query) {
                Some(0)
            } else if self.synonym_rank(icon, &query) {
                Some(1)
            } else if self
                .tags
                .get(icon)
                .is_some_and(|tags| tags.iter().any(|tag| normalize(tag).contains(&query)))
            {
                Some(2)
            } else if self.categories.get(icon).is_some_and(|categories| {
                categories
                    .iter()
                    .any(|category| normalize(category).contains(&query))
            }) || self
                .figma
                .get(icon)
                .is_some_and(|category| normalize(category).contains(&query))
            {
                Some(3)
            } else {
                None
            };
            if let Some(rank) = rank {
                ranked.push((rank, used.contains(icon), icon));
            }
        }
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(b.2))
        });
        ranked
            .into_iter()
            .map(|(_, _, icon)| icon.clone())
            .collect()
    }

    /// Whether any synonym entry for `icon` matches the query.
    fn synonym_rank(&self, icon: &str, query: &str) -> bool {
        self.synonyms.iter().any(|(term, icons)| {
            icons.iter().any(|candidate| candidate == icon)
                && (term.contains(query) || query.contains(term.as_str()))
        })
    }

    /// The best icon for a new item called `name` (product spec §9).
    pub fn suggest(&self, name: &str) -> Option<String> {
        if name.trim().is_empty() {
            return None;
        }
        self.search(name, &HashSet::new()).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_has_every_shipped_icon() {
        let index = IconIndex::shared();
        let total: usize = index
            .by_category()
            .iter()
            .map(|(_, icons)| icons.len())
            .sum();
        assert_eq!(total, 1512);
        assert!(
            index
                .search("flower-lotus", &HashSet::new())
                .contains(&"flower-lotus".to_owned())
        );
    }

    #[test]
    fn a_name_match_ranks_first() {
        let index = IconIndex::shared();
        let results = index.search("flower-lotus", &HashSet::new());
        assert_eq!(results.first().map(String::as_str), Some("flower-lotus"));
    }

    #[test]
    fn synonym_terms_match_icons() {
        let index = IconIndex::shared();
        for term in ["calm", "calma", "serene", "relaxed"] {
            let results = index.search(term, &HashSet::new());
            assert!(
                results.iter().take(3).any(|icon| icon == "flower-lotus"),
                "{term} should suggest flower-lotus, got {results:?}"
            );
        }
    }

    #[test]
    fn tags_and_categories_match() {
        let index = IconIndex::shared();
        // A tag match (not part of the icon name or a synonym).
        let results = index.search("savings", &HashSet::new());
        assert!(results.iter().any(|icon| icon == "acorn"));
        // A category match.
        let results = index.search("weather", &HashSet::new());
        assert!(!results.is_empty());
    }

    #[test]
    fn fixed_scale_icons_are_excluded() {
        let index = IconIndex::shared();
        for query in ["smiley", "battery", "signal", "moon"] {
            let results = index.search(query, &HashSet::new());
            assert!(
                !results
                    .iter()
                    .any(|icon| FIXED_SCALE_ICONS.contains(&icon.as_str())),
                "{query} leaked a fixed-scale icon"
            );
        }
    }

    #[test]
    fn used_icons_are_listed_last_within_their_rank() {
        let index = IconIndex::shared();
        let used: HashSet<String> = ["clock".to_owned()].into_iter().collect();
        let results = index.search("clock", &used);
        // Other icons whose name contains "clock" come before the used one.
        let clock = results.iter().position(|icon| icon == "clock");
        assert!(clock.is_some_and(|position| position > 0));
    }

    #[test]
    fn suggests_from_a_new_items_name() {
        let index = IconIndex::shared();
        assert!(index.suggest("calm").is_some());
        assert_eq!(index.suggest(""), None);
    }

    #[test]
    fn category_groups_cover_the_set() {
        let index = IconIndex::shared();
        let groups = index.by_category();
        assert!(!groups.is_empty());
        let total: usize = groups.iter().map(|(_, icons)| icons.len()).sum();
        assert_eq!(total, 1512);
    }
}
