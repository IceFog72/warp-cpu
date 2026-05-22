//! One process-wide `Mutex<SqliteConnection>` shared by the SSH manager.
//!
//! Background: Warp's primary write connection runs on a dedicated write thread
//! (see `app/src/persistence/sqlite.rs`) and handles writes via a `ModelEvent`
//! channel asynchronously.  Wiring the SSH manager into that event bus would
//! require 6+ new enum variants and cross-crate type exposure — too costly.
//!
//! Alternative: **SQLite WAL mode natively supports multiple write connections**
//! (writes are serialised, with a `busy_timeout` retry).  We open a separate
//! independent write connection whose behaviour is entirely local to this crate.
//! SSH-manager writes are user-driven (create/delete node) and extremely
//! infrequent, so contention with the primary write thread is negligible.
//!
//! The path is supplied by the caller at init time via `set_database_path`,
//! keeping this crate free of any dependency on `app`'s `database_file_path()`.
//! If the path has not been set, `with_conn` returns `Err(NotInitialized)`.

use anyhow::{Result, anyhow};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static CONN: OnceLock<Mutex<SqliteConnection>> = OnceLock::new();

/// Called once at app startup with the path to the SQLite DB file.
/// Repeated calls are silently ignored (OnceLock semantics).
pub fn set_database_path(path: PathBuf) {
    let _ = DB_PATH.set(path);
}

fn open() -> Result<SqliteConnection> {
    let path = DB_PATH
        .get()
        .ok_or_else(|| anyhow!("warp_ssh_manager::db: database path not initialized"))?;
    let url = path.to_string_lossy();
    let mut conn = SqliteConnection::establish(&url)?;
    conn.batch_execute(
        "PRAGMA foreign_keys = ON; \
         PRAGMA busy_timeout = 2000; \
         PRAGMA journal_mode = WAL;",
    )?;
    Ok(conn)
}

/// Executes the closure under the connection lock.
/// On the first call the connection is opened lazily; subsequent calls reuse it.
pub fn with_conn<R>(f: impl FnOnce(&mut SqliteConnection) -> Result<R>) -> Result<R> {
    let mtx = CONN.get_or_init(|| Mutex::new(open().expect("warp_ssh_manager db open")));
    let mut guard = mtx
        .lock()
        .map_err(|_| anyhow!("warp_ssh_manager db mutex poisoned"))?;
    f(&mut *guard)
}

/// Injects a pre-built connection for use in tests (bypasses the `OnceLock`).
/// **Not repeatable** — once set the `OnceLock` cannot be overwritten.
/// Tests intentionally share the same in-memory DB; each test is responsible
/// for its own cleanup.
#[cfg(test)]
pub(crate) fn install_for_test(conn: SqliteConnection) {
    let _ = CONN.set(Mutex::new(conn));
}

/// Initialises an in-memory SQLite database for tests that instantiate views
/// which use the SSH manager (e.g. `SshManagerPanel`).  Safe to call multiple
/// times — only the first call takes effect (OnceLock semantics).
///
/// Intended for test fixtures only; no-op if the connection is already set.
pub fn init_in_memory_for_test() {
    if CONN.get().is_some() {
        return;
    }
    match SqliteConnection::establish(":memory:") {
        Ok(conn) => {
            let _ = CONN.set(Mutex::new(conn));
        }
        Err(e) => log::error!("warp_ssh_manager: failed to open in-memory db for test: {e:#}"),
    }
}
