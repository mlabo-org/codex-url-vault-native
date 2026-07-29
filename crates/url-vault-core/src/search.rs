use std::collections::HashSet;

use crate::model::{Bookmark, SuggestResult};

pub(crate) fn search(mut bookmarks: Vec<Bookmark>, query: &str, limit: usize) -> Vec<Bookmark> {
    let query_lower = query.to_lowercase();
    let query_compact: String = query_lower
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect();
    let tokens = query_tokens(query);

    for bookmark in &mut bookmarks {
        let aliases = bookmark.aliases.join(" ").to_lowercase();
        let intents = bookmark.intents.join(" ").to_lowercase();
        let title = bookmark.title.clone().unwrap_or_default().to_lowercase();
        let folder = bookmark
            .folder_path
            .clone()
            .unwrap_or_default()
            .to_lowercase();
        let tags = bookmark.tags.join(" ").to_lowercase();
        let note = bookmark.note.clone().unwrap_or_default().to_lowercase();
        let project = bookmark.project.clone().unwrap_or_default().to_lowercase();
        let url = bookmark.url.to_lowercase();
        let snapshot = bookmark
            .snapshot
            .as_ref()
            .filter(|value| value.status == "valid")
            .and_then(|value| value.content.clone())
            .unwrap_or_default()
            .to_lowercase();

        let mut score = 0_i64;
        for alias in &bookmark.aliases {
            let alias_lower = alias.to_lowercase();
            let alias_compact: String = alias_lower
                .chars()
                .filter(|value| !value.is_whitespace())
                .collect();
            if !alias_lower.is_empty()
                && (query_lower.contains(&alias_lower) || query_compact.contains(&alias_compact))
            {
                score += 20;
            }
        }
        for intent in &bookmark.intents {
            let intent_lower = intent.to_lowercase();
            let intent_compact: String = intent_lower
                .chars()
                .filter(|value| !value.is_whitespace())
                .collect();
            if !intent_lower.is_empty()
                && (query_lower.contains(&intent_lower) || query_compact.contains(&intent_compact))
            {
                score += 16;
            }
        }
        let title_compact: String = title
            .chars()
            .filter(|value| !value.is_whitespace())
            .collect();
        if !title.is_empty()
            && (query_lower.contains(&title) || query_compact.contains(&title_compact))
        {
            score += 8;
        }
        if !query_lower.is_empty() && snapshot.contains(&query_lower) {
            score += 10;
        }
        for token in &tokens {
            score += if title.contains(token) { 6 } else { 0 };
            score += if aliases.contains(token) { 8 } else { 0 };
            score += if intents.contains(token) { 7 } else { 0 };
            score += if folder.contains(token) { 5 } else { 0 };
            score += if tags.contains(token) { 5 } else { 0 };
            score += if project.contains(token) { 4 } else { 0 };
            score += if note.contains(token) { 3 } else { 0 };
            score += if snapshot.contains(token) { 4 } else { 0 };
            score += if url.contains(token) { 2 } else { 0 };
        }
        if tokens.is_empty() {
            score = 1;
        }
        bookmark.score = (score > 0).then_some(score);
    }

    bookmarks.retain(|bookmark| bookmark.score.is_some());
    bookmarks.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.last_opened_at.cmp(&left.last_opened_at))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    bookmarks.truncate(limit);
    bookmarks
}

pub(crate) fn suggest(bookmarks: Vec<Bookmark>, query: &str, limit: usize) -> SuggestResult {
    let candidates = search(bookmarks, query, limit);
    let open_directly = is_clear_winner(&candidates);
    SuggestResult {
        query: query.to_owned(),
        candidates,
        open_directly,
    }
}

pub fn is_clear_winner(candidates: &[Bookmark]) -> bool {
    let Some(top) = candidates.first().and_then(|value| value.score) else {
        return false;
    };
    if candidates.len() == 1 && top >= 8 {
        return true;
    }
    top >= 20
        && (candidates.len() == 1
            || candidates
                .get(1)
                .and_then(|value| value.score)
                .is_none_or(|second| top >= second + 8))
}

fn query_tokens(query: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for token in query.split_whitespace() {
        let normalized = token.trim().to_lowercase();
        if !normalized.is_empty() && seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    let mut current = String::new();
    for character in query.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            current.push(character.to_ascii_lowercase());
        } else if !current.is_empty() {
            if seen.insert(current.clone()) {
                result.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if !current.is_empty() && seen.insert(current.clone()) {
        result.push(current);
    }
    if result.is_empty() && !query.trim().is_empty() {
        result.push(query.trim().to_lowercase());
    }
    result
}
