/// One bit per ASCII letter, plus one for digits and one for everything else.
/// Both sides must already be lowercased.
pub fn mask(text: &str) -> u32 {
    text.bytes().fold(0, |mask, byte| {
        mask | match byte.to_ascii_lowercase() {
            letter @ b'a'..=b'z' => 1 << (letter - b'a'),
            b'0'..=b'9' => 1 << 26,
            _ => 1 << 27,
        }
    })
}

/// Whether `text` holds every character `query` needs. A cheap way to rule out
/// text that cannot possibly match before scoring it.
pub fn reachable(query: u32, text: u32) -> bool {
    query & !text == 0
}

pub fn score_lowercase(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    if text == query {
        return Some(1000);
    }
    if text.starts_with(query) {
        return Some(800 - text.chars().count() as i32);
    }

    let mut target = text.chars();
    let mut consumed = 0;
    let mut previous = None;
    let mut run = 0;
    let mut score = 0;

    for needle in query.chars() {
        let mut found = false;
        for character in target.by_ref() {
            consumed += 1;
            if character == needle {
                let boundary = consumed == 1 || previous == Some(' ') || previous == Some('-');
                score += 10 + run * 5 + if boundary { 15 } else { 0 };
                run += 1;
                previous = Some(character);
                found = true;
                break;
            }
            run = 0;
            previous = Some(character);
        }
        if !found {
            return None;
        }
    }
    Some(score - (consumed + target.count()) as i32)
}

#[cfg(test)]
mod tests {
    use super::{mask, reachable, score_lowercase as score};

    #[test]
    fn ranking_rules_match_the_legacy_matcher() {
        assert!(score("kph", "kph").unwrap() > score("kph", "kilometer per hour").unwrap());
        assert!(score("root", "root_n(x)").is_some());
        assert!(score("xyz", "sqrt").is_none());
    }

    #[test]
    fn a_word_boundary_is_worth_more_than_the_same_letters_mid_word() {
        assert!(score("kp", "km per hour").unwrap() > score("kp", "skipper").unwrap());
        assert!(score("k", "a-kilo").unwrap() > score("k", "aikilo").unwrap());
    }

    #[test]
    fn longer_text_scores_lower_for_the_same_match() {
        assert!(score("kg", "kg x").unwrap() > score("kg", "kg xxxxx").unwrap());
    }

    #[test]
    fn the_mask_rules_out_only_what_can_never_match() {
        let text = mask("kilometer per hour");
        assert!(reachable(mask("kph"), text));
        assert!(reachable(mask("KPH"), text));
        assert!(!reachable(mask("kgz"), text));
        assert!(reachable(mask(""), text));
    }

    #[test]
    fn anything_that_scores_is_reachable() {
        let candidates = ["root_n(x)", "12 kg to lb", "Trip budget", "√2 ≈ 1.41"];
        for text in candidates {
            let text = text.to_lowercase();
            for query in ["root", "kg", "trip", "2", "√", "zzz", "9x"] {
                if score(query, &text).is_some() {
                    assert!(
                        reachable(mask(query), mask(&text)),
                        "{query:?} scored against {text:?} but the mask ruled it out"
                    );
                }
            }
        }
    }
}
