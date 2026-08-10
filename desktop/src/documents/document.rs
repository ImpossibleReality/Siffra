//! Saved calculation data and its display conventions.

use jiff::{Timestamp, tz::TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: u64,
    /// User-provided name; omitted names display the creation date.
    #[serde(default)]
    pub title: Option<String>,
    pub content: String,
    /// Caret offset in bytes.
    #[serde(default)]
    pub cursor: usize,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Document {
    pub fn new(id: u64, created_at: i64) -> Self {
        Self {
            id,
            title: None,
            content: String::new(),
            cursor: 0,
            created_at,
            updated_at: created_at,
        }
    }

    /// Returns the user name or a formatted creation date.
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| date_title(self.created_at))
    }

    /// Sets or clears the user name.
    pub fn set_title(&mut self, title: &str) {
        self.title = match title.trim() {
            "" => None,
            title => Some(title.to_string()),
        };
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Whether this document has content or a name.
    pub fn is_worth_keeping(&self) -> bool {
        !self.is_empty() || self.title.is_some()
    }
}

pub(super) fn normalized(mut document: Document) -> Document {
    let named_after_its_date = document
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty() || title == date_title(document.created_at));
    if named_after_its_date {
        document.title = None;
    }
    document
}

pub fn now_millis() -> i64 {
    Timestamp::now().as_millisecond()
}

pub fn date_title(created_at: i64) -> String {
    Timestamp::from_millisecond(created_at)
        .map(|timestamp| timestamp.to_zoned(TimeZone::system()))
        .map(|date| date.strftime("%b %-d, %Y at %-I:%M %p").to_string())
        .unwrap_or_else(|_| "Untitled".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unnamed_document_shows_its_creation_date_without_storing_it_as_a_name() {
        let document = Document::new(1, now_millis());

        assert_eq!(document.title, None);
        assert_eq!(document.display_title(), date_title(document.created_at));
    }

    #[test]
    fn naming_and_unnaming_a_document() {
        let mut document = Document::new(1, now_millis());

        document.set_title("  Trip budget  ");
        assert_eq!(document.title.as_deref(), Some("Trip budget"));
        assert_eq!(document.display_title(), "Trip budget");

        document.set_title("   ");
        assert_eq!(document.title, None);
        assert_eq!(document.display_title(), date_title(document.created_at));
    }

    #[test]
    fn only_a_document_with_content_or_a_name_is_worth_keeping() {
        let mut document = Document::new(1, now_millis());
        assert!(!document.is_worth_keeping());

        document.set_title("Trip budget");
        assert!(document.is_worth_keeping());

        document.set_title("");
        document.content = "2 + 2".to_string();
        assert!(document.is_worth_keeping());
    }

    #[test]
    fn records_serialize_with_the_field_names_already_on_disk() {
        let document = Document::new(1, now_millis());

        let json = serde_json::to_value(&document).expect("document json");
        let mut keys: Vec<_> = json.as_object().expect("object").keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "content",
                "created_at",
                "cursor",
                "id",
                "title",
                "updated_at"
            ]
        );
    }
}
