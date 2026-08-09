//! Persistence store for saved calculations.
//!
//! Every document is a whole editor buffer. The workspace writes shortly after
//! typing settles, so writes have to be cheap and durable — hence redb, an
//! embedded copy-on-write key/value store, rather than a file per document or a
//! server.
//!
//! A document is named by the user, from the field in the toolbar. A name is
//! only ever one that was typed: an unnamed calculation has none, and the date
//! it was created stands in for one wherever it is shown. The date is stored in
//! its own right, so naming a calculation adds something rather than replacing
//! what was there.
//!
//! Only calculations with something in them are kept here. An empty one that has
//! been named is the workspace's to hold in memory; an empty one that has not is
//! nothing to keep at all.
//!
//! The record format is stable so documents already saved on disk remain
//! readable.

use chrono::{Local, TimeZone};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Documents keyed by id, stored as JSON so the record can gain fields without
/// a migration.
const DOCUMENTS: TableDefinition<u64, &str> = TableDefinition::new("documents");
/// Application data directory name for the document database.
const DATA_DIRECTORY: &str = "dev.impossiblereality.siffra";
const DATABASE_FILE: &str = "documents.redb";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: u64,
    /// The name the user gave it, or `None` while it has none. What is shown for
    /// an unnamed calculation is [`Document::display_title`], its creation date.
    /// Defaulted so a record written without the field still loads.
    #[serde(default)]
    pub title: Option<String>,
    pub content: String,
    /// Where the caret was left, in characters from the start of the content.
    /// Defaulted so records written before cursors were stored still load.
    #[serde(default)]
    pub cursor: usize,
    /// Creation time in milliseconds since the Unix epoch.
    pub created_at: i64,
    pub updated_at: i64,
}

impl Document {
    /// An empty, unnamed calculation. Nothing keeps it yet: it is written to the
    /// store once it has content, and held in memory once it has a name.
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

    /// What the calculation is called on screen: its name, or the date it was
    /// created while it has none.
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| date_title(self.created_at))
    }

    /// Names the calculation, or takes its name away again — a blank name is no
    /// name rather than an empty one.
    pub fn set_title(&mut self, title: &str) {
        self.title = match title.trim() {
            "" => None,
            title => Some(title.to_string()),
        };
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Whether anything should outlive the moment the calculation is left:
    /// content is kept on disk, a name alone is kept in memory, and neither is
    /// kept at all.
    pub fn is_worth_keeping(&self) -> bool {
        !self.is_empty() || self.title.is_some()
    }
}

/// Reconciles a record with how names are stored now. Documents written before
/// names and dates were kept apart hold their creation date as their name; one
/// that still matches its own date was never actually named.
fn normalized(mut document: Document) -> Document {
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The default name for a document the user has not named: its creation date.
pub fn date_title(created_at: i64) -> String {
    match Local.timestamp_millis_opt(created_at).single() {
        Some(dt) => dt.format("%b %-d, %Y at %-I:%M %p").to_string(),
        None => "Untitled".to_string(),
    }
}

/// Where the store lives. `None` when the platform has no data directory, which
/// leaves the session unsaved rather than guessing at a path.
pub fn database_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join(DATA_DIRECTORY).join(DATABASE_FILE))
}

pub struct DocumentStore {
    db: Database,
}

type Result<T> = std::result::Result<T, String>;

fn fail(context: &str, err: impl std::fmt::Display) -> String {
    format!("{context}: {err}")
}

impl DocumentStore {
    /// Opens (creating if needed) the store at `path`, ensuring its table
    /// exists so later read transactions never race against a missing table.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| fail("could not create the data directory", err))?;
        }

        let db = Database::create(path).map_err(|err| fail("could not open the database", err))?;
        let txn = db
            .begin_write()
            .map_err(|err| fail("could not open the database", err))?;
        {
            txn.open_table(DOCUMENTS)
                .map_err(|err| fail("could not open the documents table", err))?;
        }
        txn.commit()
            .map_err(|err| fail("could not open the database", err))?;

        Ok(Self { db })
    }

    fn read_document(&self, id: u64) -> Result<Option<Document>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|err| fail("could not read the database", err))?;
        let table = txn
            .open_table(DOCUMENTS)
            .map_err(|err| fail("could not read saved calculations", err))?;
        let stored = table
            .get(id)
            .map_err(|err| fail("could not read the calculation", err))?;

        Ok(stored
            .and_then(|value| serde_json::from_str(value.value()).ok())
            .map(normalized))
    }

    /// Writes `document` over whatever is stored under its id. Only calculations
    /// with content belong here, so a blank one is refused rather than quietly
    /// leaving an empty record behind.
    pub fn put(&self, document: &Document) -> Result<()> {
        if document.is_empty() {
            return Err("cannot save a blank calculation".to_string());
        }
        self.write_document(document)
    }

    fn write_document(&self, document: &Document) -> Result<()> {
        let encoded = serde_json::to_string(document)
            .map_err(|err| fail("could not encode the calculation", err))?;

        let txn = self
            .db
            .begin_write()
            .map_err(|err| fail("could not save the calculation", err))?;
        {
            let mut table = txn
                .open_table(DOCUMENTS)
                .map_err(|err| fail("could not save the calculation", err))?;
            table
                .insert(document.id, encoded.as_str())
                .map_err(|err| fail("could not save the calculation", err))?;
        }
        txn.commit()
            .map_err(|err| fail("could not save the calculation", err))
    }

    /// Every non-blank document, contents included: the search modal matches
    /// against the text of each calculation, not just its name.
    pub fn list(&self) -> Result<Vec<Document>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|err| fail("could not read the database", err))?;
        let table = txn
            .open_table(DOCUMENTS)
            .map_err(|err| fail("could not read saved calculations", err))?;

        let mut documents = Vec::new();
        for entry in table
            .iter()
            .map_err(|err| fail("could not read saved calculations", err))?
        {
            let (_, value) = entry.map_err(|err| fail("could not read saved calculations", err))?;
            if let Ok(document) = serde_json::from_str::<Document>(value.value()) {
                let document = normalized(document);
                if !document.is_empty() {
                    documents.push(document);
                }
            }
        }

        // Keep the search list in most-recently-edited order.
        documents.sort_by_key(|document| std::cmp::Reverse(document.updated_at));
        Ok(documents)
    }

    pub fn get(&self, id: u64) -> Result<Option<Document>> {
        self.read_document(id)
    }

    /// An id nothing is stored under. Ids are creation timestamps, nudged
    /// forward on the rare collision so that two calculations started in the
    /// same millisecond both survive.
    pub fn reserve_id(&self, preferred: u64) -> Result<u64> {
        let mut id = preferred;
        while self.read_document(id)?.is_some() {
            id += 1;
        }
        Ok(id)
    }

    pub fn delete(&self, id: u64) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|err| fail("could not delete the calculation", err))?;
        {
            let mut table = txn
                .open_table(DOCUMENTS)
                .map_err(|err| fail("could not delete the calculation", err))?;
            table
                .remove(id)
                .map_err(|err| fail("could not delete the calculation", err))?;
        }
        txn.commit()
            .map_err(|err| fail("could not delete the calculation", err))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (DocumentStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = DocumentStore::open(&dir.path().join("test.redb")).expect("store");
        (store, dir)
    }

    /// A calculation with `content` already written to `store`.
    fn saved(store: &DocumentStore, content: &str) -> Document {
        let created_at = now_millis();
        let id = store.reserve_id(created_at as u64).expect("reserve id");
        let mut document = Document::new(id, created_at);
        document.content = content.to_string();
        store.put(&document).expect("put");
        document
    }

    #[test]
    fn an_unnamed_document_shows_its_creation_date_without_storing_it_as_a_name() {
        let (store, _dir) = store();
        let document = saved(&store, "2 + 2");

        assert_eq!(document.title, None);
        assert_eq!(document.display_title(), date_title(document.created_at));

        let loaded = store.get(document.id).expect("get").expect("document");
        assert_eq!(loaded.title, None);
        assert_eq!(loaded.created_at, document.created_at);
    }

    #[test]
    fn naming_and_unnaming_a_document() {
        let (store, _dir) = store();
        let mut document = saved(&store, "2 + 2");

        document.set_title("  Trip budget  ");
        assert_eq!(document.title.as_deref(), Some("Trip budget"));
        assert_eq!(document.display_title(), "Trip budget");
        store.put(&document).expect("put");
        assert_eq!(
            store
                .get(document.id)
                .expect("get")
                .expect("document")
                .title,
            document.title
        );

        document.set_title("   ");
        assert_eq!(document.title, None);
        assert_eq!(document.display_title(), date_title(document.created_at));
    }

    /// Records written before names and dates were kept apart carry the date as
    /// the name; reading one back must not make it look named.
    #[test]
    fn a_document_named_after_its_own_date_reads_back_unnamed() {
        let (store, _dir) = store();
        let mut document = saved(&store, "2 + 2");
        document.title = Some(date_title(document.created_at));
        store.write_document(&document).expect("write");

        let loaded = store.get(document.id).expect("get").expect("document");
        assert_eq!(loaded.title, None);
        assert_eq!(loaded.display_title(), date_title(document.created_at));
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
    fn listing_is_most_recently_edited_first_and_carries_content() {
        let (store, _dir) = store();
        let mut first = saved(&store, "1 + 1");
        let mut second = saved(&store, "2 + 2");
        first.set_title("One");
        second.set_title("Two");
        store.put(&second).expect("put");
        first.content = "1 + 1\n3 kg to lb".to_string();
        first.updated_at = now_millis() + 1;
        store.put(&first).expect("put");

        let listed = store.list().expect("list");
        assert_eq!(
            listed.iter().map(|d| d.display_title()).collect::<Vec<_>>(),
            ["One", "Two"]
        );
        assert_eq!(listed[0].content, "1 + 1\n3 kg to lb");
        assert_ne!(first.id, second.id);
    }

    /// The field names stored in `documents.redb`. Renaming one makes every
    /// document already saved unreadable.
    #[test]
    fn records_serialize_with_the_field_names_already_on_disk() {
        let (store, _dir) = store();
        let document = saved(&store, "1 + 1");

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

    #[test]
    fn the_caret_travels_with_the_document() {
        let (store, _dir) = store();
        let mut document = saved(&store, "1 + 1\n2 + 2");

        document.cursor = 8;
        store.put(&document).expect("put");
        assert_eq!(store.get(document.id).expect("get").unwrap().cursor, 8);
    }

    #[test]
    fn deleting_removes_the_document() {
        let (store, _dir) = store();
        let document = saved(&store, "2 + 2");

        store.delete(document.id).expect("delete");
        assert!(store.get(document.id).expect("get").is_none());
        assert!(store.list().expect("list").is_empty());
    }

    #[test]
    fn ids_are_reserved_around_what_is_already_stored() {
        let (store, _dir) = store();
        let document = saved(&store, "2 + 2");

        assert_eq!(
            store.reserve_id(document.id).expect("reserve id"),
            document.id + 1
        );
    }

    #[test]
    fn blank_content_is_never_saved() {
        let (store, _dir) = store();
        let document = saved(&store, "2 + 2");

        let mut blank = document.clone();
        blank.content = "\n  \t".to_string();
        blank.set_title("Trip budget");
        assert!(store.put(&blank).is_err());
        assert_eq!(
            store.get(document.id).expect("get").unwrap().content,
            "2 + 2"
        );
    }
}
