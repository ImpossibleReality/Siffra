pub fn score(query: &str, text: &str) -> Option<i32> {
    score_lowercase(&query.to_lowercase(), &text.to_lowercase())
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

    let target: Vec<char> = text.chars().collect();
    let mut target_index = 0;
    let mut run = 0;
    let mut score = 0;
    for needle in query.chars() {
        let mut found = false;
        while target_index < target.len() {
            if target[target_index] == needle {
                let boundary = target_index == 0
                    || target[target_index - 1] == ' '
                    || target[target_index - 1] == '-';
                score += 10 + run * 5 + if boundary { 15 } else { 0 };
                run += 1;
                target_index += 1;
                found = true;
                break;
            }
            run = 0;
            target_index += 1;
        }
        if !found {
            return None;
        }
    }
    Some(score - target.len() as i32)
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn ranking_rules_match_the_legacy_matcher() {
        assert!(score("kph", "kph").unwrap() > score("kph", "kilometer per hour").unwrap());
        assert!(score("root", "root_n(x)").is_some());
        assert!(score("xyz", "sqrt").is_none());
    }

    #[test]
    fn matching_ignores_case_on_both_sides() {
        assert_eq!(score("KPH", "kph"), score("kph", "KPH"));
    }
}
