//! Transcript storage and search.

use rusqlite::{params_from_iter, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptSource {
    Dictation,
    Import,
}

impl TranscriptSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::Import => "import",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "import" => Self::Import,
            _ => Self::Dictation,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub text: String,
    /// Unix milliseconds.
    pub created_at: i64,
    pub source: TranscriptSource,
    pub source_app: Option<String>,
}

impl Transcript {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let source: String = row.get("source")?;
        Ok(Self {
            id: row.get("id")?,
            text: row.get("text")?,
            created_at: row.get("created_at")?,
            source: TranscriptSource::parse(&source),
            source_app: row.get("source_app")?,
        })
    }
}

/// A new transcript on its way into history.
#[derive(Clone, Debug)]
pub struct NewTranscript {
    pub text: String,
    pub source: TranscriptSource,
    pub source_app: Option<String>,
}

/// Filters the history view can apply. All are optional and combine with AND.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryQuery {
    /// Free text. Runs through FTS5 when present.
    pub search: Option<String>,
    pub source: Option<TranscriptSource>,
    pub source_app: Option<String>,
    /// Unix milliseconds, inclusive.
    pub since: Option<i64>,
    /// Unix milliseconds, exclusive.
    pub until: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 500;

pub fn insert(
    connection: &Connection,
    transcript: NewTranscript,
    created_at: i64,
) -> rusqlite::Result<Transcript> {
    let id = uuid::Uuid::new_v4().to_string();

    connection.execute(
        "INSERT INTO transcripts (id, text, created_at, source, source_app)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            id,
            transcript.text,
            created_at,
            transcript.source.as_str(),
            transcript.source_app,
        ],
    )?;

    Ok(Transcript {
        id,
        text: transcript.text,
        created_at,
        source: transcript.source,
        source_app: transcript.source_app,
    })
}

pub fn delete(connection: &Connection, id: &str) -> rusqlite::Result<bool> {
    let affected = connection.execute("DELETE FROM transcripts WHERE id = ?1", [id])?;
    Ok(affected > 0)
}

pub fn count(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("SELECT COUNT(*) FROM transcripts", [], |row| row.get(0))
}

/// The distinct applications that have received dictation, most recent first.
/// Feeds the history view's source-app filter.
pub fn known_source_apps(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT source_app FROM transcripts
         WHERE source_app IS NOT NULL
         GROUP BY source_app
         ORDER BY MAX(created_at) DESC
         LIMIT 40",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect()
}

/// Run a history query.
///
/// With a search term the rows come back in FTS relevance order; without one,
/// newest first.
pub fn query(connection: &Connection, request: &HistoryQuery) -> rusqlite::Result<Vec<Transcript>> {
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = request.offset.unwrap_or(0);

    let mut wheres: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    let search = request
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(to_fts_query);

    if let Some(ref term) = search {
        wheres.push(format!("transcripts_fts MATCH ?{}", values.len() + 1));
        values.push(Box::new(term.clone()));
    }
    if let Some(source) = request.source {
        wheres.push(format!("t.source = ?{}", values.len() + 1));
        values.push(Box::new(source.as_str().to_string()));
    }
    if let Some(ref app) = request.source_app {
        wheres.push(format!("t.source_app = ?{}", values.len() + 1));
        values.push(Box::new(app.clone()));
    }
    if let Some(since) = request.since {
        wheres.push(format!("t.created_at >= ?{}", values.len() + 1));
        values.push(Box::new(since));
    }
    if let Some(until) = request.until {
        wheres.push(format!("t.created_at < ?{}", values.len() + 1));
        values.push(Box::new(until));
    }

    let from = if search.is_some() {
        "transcripts t JOIN transcripts_fts ON transcripts_fts.rowid = t.rowid"
    } else {
        "transcripts t"
    };
    let filter = if wheres.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", wheres.join(" AND "))
    };
    let order = if search.is_some() {
        "ORDER BY rank, t.created_at DESC"
    } else {
        "ORDER BY t.created_at DESC"
    };

    let sql = format!(
        "SELECT t.id, t.text, t.created_at, t.source, t.source_app
         FROM {from} {filter} {order}
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        limit_index = values.len() + 1,
        offset_index = values.len() + 2,
    );
    values.push(Box::new(limit));
    values.push(Box::new(offset));

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), Transcript::from_row)?;
    rows.collect()
}

/// Turn arbitrary user input into a safe FTS5 MATCH expression.
///
/// Every token is quoted so FTS operators typed by accident (`AND`, `*`, `"`,
/// `NEAR`) are searched for literally instead of producing a syntax error, and
/// the last token gets a prefix `*` so search feels live as the user types.
fn to_fts_query(input: &str) -> String {
    let tokens: Vec<String> = input
        .split_whitespace()
        .map(|token| {
            let cleaned: String = token
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
                .collect();
            cleaned
        })
        .filter(|token| !token.is_empty())
        .collect();

    if tokens.is_empty() {
        // Matches nothing rather than everything: an all-punctuation search
        // should not dump the whole history.
        return "\"\"".into();
    }

    let last = tokens.len() - 1;
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            if index == last {
                format!("\"{token}\"*")
            } else {
                format!("\"{token}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::schema;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        schema::apply(&connection).unwrap();
        connection
    }

    fn add(connection: &Connection, text: &str, app: &str, at: i64) -> Transcript {
        insert(
            connection,
            NewTranscript {
                text: text.into(),
                source: TranscriptSource::Dictation,
                source_app: Some(app.into()),
            },
            at,
        )
        .unwrap()
    }

    #[test]
    fn a_transcript_round_trips() {
        let connection = db();
        let saved = add(&connection, "hello from clide", "TextEdit", 1_000);

        let rows = query(&connection, &HistoryQuery::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, saved.id);
        assert_eq!(rows[0].text, "hello from clide");
        assert_eq!(rows[0].source_app.as_deref(), Some("TextEdit"));
        assert_eq!(rows[0].source, TranscriptSource::Dictation);
    }

    #[test]
    fn history_is_newest_first() {
        let connection = db();
        add(&connection, "oldest", "Notes", 1);
        add(&connection, "newest", "Notes", 99);

        let rows = query(&connection, &HistoryQuery::default()).unwrap();
        assert_eq!(rows[0].text, "newest");
        assert_eq!(rows[1].text, "oldest");
    }

    #[test]
    fn full_text_search_finds_words_anywhere_in_the_transcript() {
        let connection = db();
        add(&connection, "remember to renew the domain", "Notes", 1);
        add(&connection, "buy oat milk", "Notes", 2);

        let rows = query(
            &connection,
            &HistoryQuery {
                search: Some("domain".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert!(rows[0].text.contains("domain"));
    }

    #[test]
    fn search_matches_prefixes_so_it_works_while_typing() {
        let connection = db();
        add(&connection, "transcription pipeline", "Notes", 1);

        for partial in ["trans", "transcri", "transcription"] {
            let rows = query(
                &connection,
                &HistoryQuery {
                    search: Some(partial.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(rows.len(), 1, "prefix {partial:?} found nothing");
        }
    }

    #[test]
    fn fts_operators_typed_by_a_user_do_not_break_search() {
        let connection = db();
        add(&connection, "ship the beta", "Notes", 1);

        for hostile in ["\"", "AND", "beta OR", "NEAR(", "*", "()", "  "] {
            let result = query(
                &connection,
                &HistoryQuery {
                    search: Some(hostile.into()),
                    ..Default::default()
                },
            );
            assert!(result.is_ok(), "search {hostile:?} errored: {result:?}");
        }
    }

    #[test]
    fn filters_combine() {
        let connection = db();
        add(&connection, "in notes", "Notes", 10);
        add(&connection, "in textedit", "TextEdit", 20);
        add(&connection, "later in notes", "Notes", 30);

        let rows = query(
            &connection,
            &HistoryQuery {
                source_app: Some("Notes".into()),
                since: Some(20),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "later in notes");
    }

    #[test]
    fn the_search_index_follows_deletions() {
        let connection = db();
        let saved = add(&connection, "ephemeral thought", "Notes", 1);

        assert!(delete(&connection, &saved.id).unwrap());
        assert_eq!(count(&connection).unwrap(), 0);

        let rows = query(
            &connection,
            &HistoryQuery {
                search: Some("ephemeral".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(rows.is_empty(), "FTS index kept a deleted transcript");
    }

    #[test]
    fn deleting_something_that_is_gone_is_not_an_error() {
        let connection = db();
        assert!(!delete(&connection, "no-such-id").unwrap());
    }

    #[test]
    fn source_apps_are_listed_most_recent_first() {
        let connection = db();
        add(&connection, "a", "Notes", 1);
        add(&connection, "b", "TextEdit", 5);
        add(&connection, "c", "Notes", 9);

        assert_eq!(
            known_source_apps(&connection).unwrap(),
            vec!["Notes".to_string(), "TextEdit".to_string()]
        );
    }

    #[test]
    fn limits_are_capped_so_the_ui_cannot_ask_for_everything() {
        let connection = db();
        for i in 0..10 {
            add(&connection, &format!("entry {i}"), "Notes", i);
        }

        let rows = query(
            &connection,
            &HistoryQuery {
                limit: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rows.len(), 3);

        let paged = query(
            &connection,
            &HistoryQuery {
                limit: Some(3),
                offset: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
        assert_ne!(rows[0].id, paged[0].id);
    }

    #[test]
    fn transcripts_never_store_audio_or_credentials() {
        // Guards the record shape itself: adding a column here should be a
        // deliberate decision, not something that drifts in.
        let connection = db();
        let mut statement = connection.prepare("SELECT * FROM transcripts LIMIT 0").unwrap();
        let _ = statement.query([]).unwrap();
        let columns: Vec<String> = statement
            .column_names()
            .iter()
            .map(|c| c.to_string())
            .collect();
        assert_eq!(
            columns,
            vec!["id", "text", "created_at", "source", "source_app"]
        );
    }
}

/// Real counts over the transcripts actually stored.
///
/// Every figure here is a `COUNT` or a `SUM` over rows the user produced.
/// Nothing is estimated, and nothing is invented to fill space —
/// `AGENTS.md` is explicit that the dashboard must not carry fake statistics.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub total_transcripts: u32,
    pub transcripts_this_week: u32,
    pub words_this_week: u32,
    /// Distinct applications dictated into over the last week.
    pub apps_this_week: u32,
    /// Longest run of consecutive days with at least one transcript,
    /// counting back from today. Zero when nothing was dictated today.
    pub day_streak: u32,
}

pub fn usage(connection: &Connection, now_ms: i64) -> rusqlite::Result<Usage> {
    const DAY_MS: i64 = 86_400_000;
    let week_ago = now_ms - 7 * DAY_MS;

    let total_transcripts: u32 =
        connection.query_row("SELECT COUNT(*) FROM transcripts", [], |row| row.get(0))?;

    let (transcripts_this_week, apps_this_week): (u32, u32) = connection.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT source_app) FROM transcripts WHERE created_at >= ?1",
        [week_ago],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Counting words in SQL keeps the whole transcript corpus out of the IPC
    // boundary just to compute a number.
    let words_this_week: u32 = {
        let mut statement = connection
            .prepare("SELECT text FROM transcripts WHERE created_at >= ?1")?;
        let rows = statement.query_map([week_ago], |row| row.get::<_, String>(0))?;
        rows.filter_map(Result::ok)
            .map(|text| text.split_whitespace().count() as u32)
            .sum()
    };

    let day_streak = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT created_at / 86400000 FROM transcripts ORDER BY 1 DESC",
        )?;
        let days: Vec<i64> = statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(Result::ok)
            .collect();

        let today = now_ms / DAY_MS;
        let mut streak = 0u32;
        for (offset, day) in days.iter().enumerate() {
            if *day == today - offset as i64 {
                streak += 1;
            } else {
                break;
            }
        }
        streak
    };

    Ok(Usage {
        total_transcripts,
        transcripts_this_week,
        words_this_week,
        apps_this_week,
        day_streak,
    })
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;

    fn seeded() -> (Connection, i64) {
        let connection = Connection::open_in_memory().unwrap();
        crate::database::schema::apply(&connection).unwrap();
        // A fixed "now" well past the epoch so day arithmetic is stable.
        (connection, 1_000 * DAY_MS + 3_600_000)
    }

    fn add(connection: &Connection, text: &str, app: &str, at: i64) {
        insert(
            connection,
            NewTranscript {
                text: text.into(),
                source: TranscriptSource::Dictation,
                source_app: Some(app.into()),
            },
            at,
        )
        .unwrap();
    }

    #[test]
    fn an_empty_history_reports_zeroes_rather_than_failing() {
        let (connection, now) = seeded();
        let usage = usage(&connection, now).unwrap();
        assert_eq!(usage.total_transcripts, 0);
        assert_eq!(usage.words_this_week, 0);
        assert_eq!(usage.day_streak, 0);
    }

    #[test]
    fn words_and_apps_are_counted_over_the_last_week_only() {
        let (connection, now) = seeded();
        add(&connection, "three words here", "Notes", now - DAY_MS);
        add(&connection, "two words", "Mail", now - 2 * DAY_MS);
        // Older than a week: must not be counted.
        add(&connection, "ignored entirely please", "Slack", now - 30 * DAY_MS);

        let usage = usage(&connection, now).unwrap();
        assert_eq!(usage.transcripts_this_week, 2);
        assert_eq!(usage.words_this_week, 5);
        assert_eq!(usage.apps_this_week, 2);
        assert_eq!(usage.total_transcripts, 3, "the total spans all time");
    }

    #[test]
    fn a_streak_counts_back_from_today_and_stops_at_the_first_gap() {
        let (connection, now) = seeded();
        add(&connection, "today", "Notes", now);
        add(&connection, "yesterday", "Notes", now - DAY_MS);
        add(&connection, "the day before", "Notes", now - 2 * DAY_MS);
        // Gap on day 3, then more activity that must not extend the streak.
        add(&connection, "long ago", "Notes", now - 4 * DAY_MS);

        assert_eq!(usage(&connection, now).unwrap().day_streak, 3);
    }

    #[test]
    fn a_streak_is_zero_when_nothing_was_dictated_today() {
        let (connection, now) = seeded();
        add(&connection, "yesterday only", "Notes", now - DAY_MS);
        assert_eq!(usage(&connection, now).unwrap().day_streak, 0);
    }

    #[test]
    fn several_transcripts_in_one_day_count_as_one_day_of_streak() {
        let (connection, now) = seeded();
        add(&connection, "one", "Notes", now);
        add(&connection, "two", "Notes", now - 3_600_000);
        assert_eq!(usage(&connection, now).unwrap().day_streak, 1);
    }
}
