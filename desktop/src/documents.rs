//! Persistence for saved calculations.
//!
//! Every document is a whole editor buffer. The workspace writes shortly after
//! typing settles, so writes have to be cheap and durable — hence redb, an
//! embedded copy-on-write key/value store, rather than a file per document or a
//! server.
//!
//! A document is named by the user, from the field in the toolbar. Until it is
//! renamed it carries the date it was created, which is stored alongside the
//! content so the name never shifts when the file is re-read.
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
/// Small key/value settings (currently just the last opened document).
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");

const LAST_OPENED: &str = "last_opened";

/// Application data directory name for the document database.
const DATA_DIRECTORY: &str = "dev.impossiblereality.siffra";
const DATABASE_FILE: &str = "documents.redb";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: u64,
    /// The name shown in the toolbar: user-chosen, defaulting to the date the
    /// document was created.
    pub title: String,
    pub content: String,
    /// Where the caret was left, in characters from the start of the content.
    /// Defaulted so records written before cursors were stored still load.
    #[serde(default)]
    pub cursor: usize,
    /// Creation time in milliseconds since the Unix epoch.
    pub created_at: i64,
    pub updated_at: i64,
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
    /// Opens (creating if needed) the store at `path`, ensuring both tables
    /// exist so later read transactions never race against a missing table.
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
            txn.open_table(META)
                .map_err(|err| fail("could not open the settings table", err))?;
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

        Ok(stored.and_then(|value| serde_json::from_str(value.value()).ok()))
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
                if !document.content.trim().is_empty() {
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

    /// Creates a document. Ids are creation timestamps, nudged forward on the
    /// rare collision so that two documents made in the same millisecond both
    /// survive.
    pub fn create(&self, content: &str) -> Result<Document> {
        if content.trim().is_empty() {
            return Err("cannot save a blank calculation".to_string());
        }

        let created_at = now_millis();
        let mut id = created_at as u64;
        while self.read_document(id)?.is_some() {
            id += 1;
        }

        let document = Document {
            id,
            title: date_title(created_at),
            content: content.to_string(),
            cursor: 0,
            created_at,
            updated_at: created_at,
        };
        self.write_document(&document)?;
        Ok(document)
    }

    /// Saves `content` and the caret position over document `id`, keeping its
    /// name. A document that has since been deleted is recreated, so an
    /// in-flight autosave can never silently lose the user's typing.
    ///
    /// Moving the caret is not an edit: `updated_at` only advances when the text
    /// itself changed, so merely reading a calculation doesn't reorder the list.
    pub fn save(&self, id: u64, content: &str, cursor: usize) -> Result<Document> {
        if content.trim().is_empty() {
            return Err("cannot save a blank calculation".to_string());
        }

        let existing = self.read_document(id)?;
        let created_at = existing.as_ref().map_or_else(now_millis, |d| d.created_at);
        let edited = existing.as_ref().is_none_or(|d| d.content != content);
        let updated_at = match &existing {
            Some(existing) if !edited => existing.updated_at,
            _ => now_millis(),
        };
        let title = existing.map_or_else(|| date_title(created_at), |d| d.title);

        let document = Document {
            id,
            title,
            content: content.to_string(),
            cursor,
            created_at,
            updated_at,
        };
        self.write_document(&document)?;
        Ok(document)
    }

    /// Renames document `id`. A blank name restores the creation-date default,
    /// so a document can never end up nameless.
    pub fn rename(&self, id: u64, title: &str) -> Result<Option<Document>> {
        let Some(mut document) = self.read_document(id)? else {
            return Ok(None);
        };

        let title = title.trim();
        document.title = if title.is_empty() {
            date_title(document.created_at)
        } else {
            title.to_string()
        };
        // Renaming does not change the edit order.
        self.write_document(&document)?;
        Ok(Some(document))
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

        if self.last_opened()? == Some(id) {
            self.clear_last_opened()?;
        }
        Ok(())
    }

    pub fn last_opened(&self) -> Result<Option<u64>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|err| fail("could not read the database", err))?;
        let table = txn
            .open_table(META)
            .map_err(|err| fail("could not read app settings", err))?;
        let stored = table
            .get(LAST_OPENED)
            .map_err(|err| fail("could not read app settings", err))?;

        Ok(stored.and_then(|value| value.value().parse::<u64>().ok()))
    }

    pub fn set_last_opened(&self, id: u64) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|err| fail("could not save app settings", err))?;
        {
            let mut table = txn
                .open_table(META)
                .map_err(|err| fail("could not save app settings", err))?;
            table
                .insert(LAST_OPENED, id.to_string().as_str())
                .map_err(|err| fail("could not save app settings", err))?;
        }
        txn.commit()
            .map_err(|err| fail("could not save app settings", err))
    }

    fn clear_last_opened(&self) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|err| fail("could not save app settings", err))?;
        {
            let mut table = txn
                .open_table(META)
                .map_err(|err| fail("could not save app settings", err))?;
            table
                .remove(LAST_OPENED)
                .map_err(|err| fail("could not save app settings", err))?;
        }
        txn.commit()
            .map_err(|err| fail("could not save app settings", err))
    }

    /// The document to show on launch: the last one opened if it still exists,
    /// otherwise the most recently edited. A blank editor is kept in memory by
    /// the workspace until the user enters a calculation.
    pub fn open_latest(&self) -> Result<Option<Document>> {
        if let Some(id) = self.last_opened()?
            && let Some(document) = self.get(id)?
            && !document.content.trim().is_empty()
        {
            return Ok(Some(document));
        }

        if let Some(summary) = self.list()?.into_iter().next() {
            return Ok(self.get(summary.id)?);
        }

        Ok(None)
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

    #[test]
    fn new_documents_are_named_after_their_creation_date() {
        let (store, _dir) = store();
        let document = store.create("2 + 2").expect("create");
        assert_eq!(document.title, date_title(document.created_at));

        // A header line is formatting, not a name.
        let saved = store
            .save(document.id, "# Trip budget\n2 + 3", 0)
            .expect("save");
        assert_eq!(saved.title, date_title(document.created_at));
        assert_eq!(saved.created_at, document.created_at);
    }

    #[test]
    fn renaming_keeps_the_content_and_the_place_in_the_list() {
        let (store, _dir) = store();
        let document = store.create("2 + 2").expect("create");

        let renamed = store
            .rename(document.id, "  Trip budget  ")
            .expect("rename")
            .expect("document");
        assert_eq!(renamed.title, "Trip budget");
        assert_eq!(renamed.content, "2 + 2");
        assert_eq!(renamed.updated_at, document.updated_at);

        let saved = store.save(document.id, "2 + 3", 0).expect("save");
        assert_eq!(saved.title, "Trip budget");

        let cleared = store
            .rename(document.id, "   ")
            .expect("rename")
            .expect("document");
        assert_eq!(cleared.title, date_title(document.created_at));

        assert!(
            store
                .rename(document.id + 999, "gone")
                .expect("rename")
                .is_none()
        );
    }

    #[test]
    fn listing_is_most_recently_edited_first_and_carries_content() {
        let (store, _dir) = store();
        let first = store.create("1 + 1").expect("create");
        let second = store.create("2 + 2").expect("create");
        store.rename(first.id, "One").expect("rename");
        store.rename(second.id, "Two").expect("rename");
        store.save(first.id, "1 + 1\n3 kg to lb", 0).expect("save");

        let listed = store.list().expect("list");
        assert_eq!(
            listed.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(),
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
        let document = store.create("1 + 1").expect("create");

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
    fn the_caret_is_stored_and_moving_it_is_not_an_edit() {
        let (store, _dir) = store();
        let document = store.create("1 + 1\n2 + 2").expect("create");

        let edited = store.save(document.id, "1 + 1\n2 + 3", 8).expect("save");
        assert_eq!(edited.cursor, 8);
        assert!(edited.updated_at >= document.updated_at);

        let moved = store.save(document.id, "1 + 1\n2 + 3", 2).expect("save");
        assert_eq!(moved.cursor, 2);
        assert_eq!(moved.updated_at, edited.updated_at);

        assert_eq!(store.get(document.id).expect("get").unwrap().cursor, 2);
    }

    #[test]
    fn deleting_removes_the_document_and_forgets_it_as_last_opened() {
        let (store, _dir) = store();
        let document = store.create("2 + 2").expect("create");
        store.set_last_opened(document.id).expect("set last opened");

        store.delete(document.id).expect("delete");
        assert!(store.get(document.id).expect("get").is_none());
        assert_eq!(store.last_opened().expect("last opened"), None);
        assert!(store.list().expect("list").is_empty());
    }

    #[test]
    fn launch_reopens_the_last_document_without_seeding_an_empty_one() {
        let (store, _dir) = store();
        assert!(store.open_latest().expect("open").is_none());

        let saved = store.create("2 + 2").expect("create");
        store.set_last_opened(saved.id).expect("set last opened");
        let reopened = store.open_latest().expect("open").expect("document");
        assert_eq!(reopened.id, saved.id);
        assert_eq!(reopened.content, "2 + 2");
    }

    #[test]
    fn blank_content_is_never_saved() {
        let (store, _dir) = store();

        assert!(store.create("   \n\t").is_err());

        let document = store.create("2 + 2").expect("create");
        assert!(store.save(document.id, "\n  \t", 0).is_err());
        assert_eq!(
            store.get(document.id).expect("get").unwrap().content,
            "2 + 2"
        );
    }
}
