use crate::db::{Database, DbError};
use crate::theme::{CATEGORICAL_DARK, CATEGORICAL_LIGHT};
use rusqlite::params;

pub fn assign_category_color(
    db: &Database,
    kind: &str,
    value: &str,
    dark: bool,
) -> Result<String, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT colour FROM category_colors WHERE kind = ?1 AND value = ?2")?;
    let mut rows = stmt.query(params![kind, value])?;
    if let Some(row) = rows.next()? {
        return Ok(row.get(0)?);
    }
    drop(rows);
    drop(stmt);

    let count: usize = db.conn().query_row(
        "SELECT COUNT(*) FROM category_colors WHERE kind = ?1",
        params![kind],
        |r| r.get::<_, i64>(0),
    )? as usize;

    let palette = if dark {
        CATEGORICAL_DARK
    } else {
        CATEGORICAL_LIGHT
    };
    let colour = if count < palette.len() {
        palette[count].to_string()
    } else {
        let hue = ((count as f64) * 137.508) % 360.0;
        format!("hsl({:.1}, 65%, 52%)", hue)
    };

    db.conn().execute(
        "INSERT INTO category_colors (kind, value, colour) VALUES (?1, ?2, ?3)",
        params![kind, value, colour],
    )?;
    Ok(colour)
}
