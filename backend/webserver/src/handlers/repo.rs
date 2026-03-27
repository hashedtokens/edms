use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use crate::{db, state::AppState};

pub async fn export_collection(
    State(state): State<AppState>,
    Path((collection, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let collection = collection.clone();
        move || {
            let q = "SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY timestamp DESC";
            let ids: Vec<String> = st.core.cproc(q, &[&collection], |row| row.get(0))?;
            db::endpoints_for_ids(&st.core, &st.queries, &ids)
        }
    }).await;

    match res {
        Ok(Ok(endpoints)) => {
            let mut md = String::new();
            md.push_str(&format!("# Export: {collection}\n\n"));
            md.push_str(&format!("Generated as `{filename}`\n\n"));
            for ep in endpoints {
                md.push_str(&format!("- **{}**: `{}`", ep.endpoint_id, ep.endpoint_str));
                if let Some(a) = ep.annotation {
                    md.push_str(&format!(" — {}", a));
                }
                md.push('\n');
            }

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                md,
            )
                .into_response()
        }
        _ => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "collection not found or export failed".to_string(),
        )
            .into_response(),
    }
}