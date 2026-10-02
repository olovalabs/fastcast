//! Quick links: named URLs with optional keywords, persisted locally.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuickLink {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub keyword: String,
}

pub fn load_links() -> Vec<QuickLink> {
    crate::store::load_json("links.json")
}

pub fn save_links(links: &[QuickLink]) {
    crate::store::save_json("links.json", links);
}

pub fn search_links(links: &[QuickLink], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut scored: Vec<(usize, u8)> = links
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            // An exact keyword match always wins.
            if !l.keyword.is_empty() && l.keyword.to_lowercase() == q {
                return Some((i, 0));
            }
            let hay = format!("{} {} {}", l.name, l.url, l.keyword).to_lowercase();
            words
                .iter()
                .all(|w| hay.contains(w))
                .then_some((i, 1))
        })
        .collect();
    scored.sort_by_key(|&(i, rank)| (rank, links[i].name.to_lowercase()));
    scored.into_iter().map(|(i, _)| i).collect()
}

/// Parse `add link <name> | <url>` (the `|` separator is optional; the last
/// whitespace-separated token is treated as the URL).
pub fn parse_add_link(query: &str) -> Option<(String, String)> {
    let rest = query.strip_prefix("add link")?.trim();
    if rest.is_empty() {
        return None;
    }
    if let Some((name, url)) = rest.split_once('|') {
        let (name, url) = (name.trim().to_string(), url.trim().to_string());
        if !name.is_empty() && !url.is_empty() {
            return Some((name, url));
        }
        return None;
    }
    let mut parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let url = parts.pop()?.to_string();
    Some((parts.join(" "), url))
}

/// Parse `edit link <name> | <new url>`.
pub fn parse_edit_link(query: &str) -> Option<(String, String)> {
    let rest = query.strip_prefix("edit link")?.trim();
    let (name, url) = rest.split_once('|')?;
    let (name, url) = (name.trim().to_string(), url.trim().to_string());
    (!name.is_empty() && !url.is_empty()).then_some((name, url))
}

#[cfg(test)]
mod tests {
    use super::{parse_add_link, parse_edit_link};

    #[test]
    fn parses_commands() {
        assert_eq!(
            parse_add_link("add link GitHub https://github.com"),
            Some(("GitHub".into(), "https://github.com".into()))
        );
        assert_eq!(
            parse_add_link("add link My Site | example.com"),
            Some(("My Site".into(), "example.com".into()))
        );
        assert_eq!(parse_add_link("add link onlyname"), None);
        assert_eq!(
            parse_edit_link("edit link GitHub | https://gh.com"),
            Some(("GitHub".into(), "https://gh.com".into()))
        );
    }
}
