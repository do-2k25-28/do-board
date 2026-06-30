use axum::{
    body::Bytes,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use rust_xlsxwriter::Workbook;
use shared::BirthdayEntry;
use std::io::Cursor;

pub async fn get_template() -> Response {
    let mut workbook = Workbook::new();
    let ws = workbook.add_worksheet();
    let _ = ws.write(0, 0, "Name");
    let _ = ws.write(0, 1, "Date (dd-mm-yyyy)");
    let _ = ws.write(1, 0, "Alice Martin");
    let _ = ws.write(1, 1, "15-03-1990");
    let _ = ws.write(2, 0, "Bob Dupont");
    let _ = ws.write(2, 1, "28-06-1985");

    let bytes = match workbook.save_to_buffer() {
        Ok(b) => b,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"birthdays-template.xlsx\"",
            ),
        ],
        bytes,
    )
        .into_response()
}

pub async fn import_xlsx(body: Bytes) -> Result<Json<Vec<BirthdayEntry>>, StatusCode> {
    let cursor = Cursor::new(body.to_vec());
    let mut workbook: Xlsx<_> =
        open_workbook_from_rs(cursor).map_err(|_| StatusCode::BAD_REQUEST)?;

    let range = workbook
        .worksheet_range_at(0)
        .ok_or(StatusCode::BAD_REQUEST)?
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let entries: Vec<BirthdayEntry> = range
        .rows()
        .skip(1) // skip header row
        .filter_map(|row| {
            let name = match row.first()? {
                Data::String(s) => s.trim().to_string(),
                _ => return None,
            };
            let date = match row.get(1)? {
                Data::String(s) => s.trim().to_string(),
                _ => return None,
            };
            if name.is_empty() || date.is_empty() {
                return None;
            }
            Some(BirthdayEntry { name, date })
        })
        .collect();

    Ok(Json(entries))
}
