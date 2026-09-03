//! History reading, searching, and the Copy escape hatch.

use tauri::{AppHandle, Manager};

use crate::database::transcripts::{self, HistoryQuery, Transcript};
use crate::dictation::events;
use crate::insertion::clipboard;
use crate::state::AppState;

/// Run a history query. With no filters this is "the most recent transcripts".
#[tauri::command]
pub fn get_history(app: AppHandle, query: Option<HistoryQuery>) -> Result<Vec<Transcript>, String> {
    let state = app.state::<AppState>();
    let query = query.unwrap_or_default();
    let rows = transcripts::query(&state.db.lock(), &query);
    rows.map_err(|error| error.to_string())
}

/// Convenience wrapper for the search field.
#[tauri::command]
pub fn search_history(
    app: AppHandle,
    search: String,
    limit: Option<u32>,
) -> Result<Vec<Transcript>, String> {
    get_history(
        app,
        Some(HistoryQuery {
            search: Some(search),
            limit,
            ..Default::default()
        }),
    )
}

#[tauri::command]
pub fn delete_transcript(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let removed =
        transcripts::delete(&state.db.lock(), &id).map_err(|error| error.to_string())?;
    if removed {
        events::emit_bare(&app, events::HISTORY_CHANGED);
    }
    Ok(removed)
}

/// Applications that have received dictation, for the history filter.
#[tauri::command]
pub fn get_source_apps(app: AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let apps = transcripts::known_source_apps(&state.db.lock());
    apps.map_err(|error| error.to_string())
}

/// Put text on the clipboard.
///
/// This is what stands behind every Copy affordance: if insertion failed, the
/// transcript is still one click from being usable.
#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    if clipboard::set_text(&text) {
        Ok(())
    } else {
        Err("The clipboard could not be written.".into())
    }
}

/// Counts over the transcripts actually stored. See `transcripts::usage`.
#[tauri::command]
pub fn get_usage(app: AppHandle) -> Result<transcripts::Usage, String> {
    let state = app.state::<AppState>();
    let connection = state.db.lock();
    transcripts::usage(&connection, crate::database::now_ms()).map_err(|error| error.to_string())
}
