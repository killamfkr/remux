use axum::{Json, extract::State, response::IntoResponse};
use axum_extra::extract::Query;
use remux_macros::get;

use crate::{AppState, api, db::auth};
use axum_anyhow::ApiResult as Result;

#[get("/search/hints")]
pub async fn search_hints(
    State(state): State<AppState>,
    session: auth::AuthSession,
    Query(q): Query<api::SearchHintsQuery>,
) -> Result<impl IntoResponse> {
    let term = q
        .search_term
        .unwrap_or_default();
    if term.is_empty() {
        return Ok(Json(api::SearchHintResult {
            search_hints: vec![],
            total_record_count: 0,
        }));
    }

    let items_query = api::GetItemsQuery {
        search_term: Some(term.clone()),
        limit: q
            .limit
            .or(Some(20)),
        start_index: q.start_index,
        include_item_types: q.include_item_types,
        ..Default::default()
    };

    let result = super::items::get_items(state, session, items_query, false)
        .await?
        .with_client_patches()
        .build();

    let hints: Vec<api::SearchHint> = result
        .items
        .into_iter()
        .map(|item| item_to_hint(item, &term))
        .collect();
    let total = hints.len() as i64;

    Ok(Json(api::SearchHintResult {
        search_hints: hints,
        total_record_count: total,
    }))
}

fn item_to_hint(item: api::BaseItemDto, matched_term: &str) -> api::SearchHint {
    let media_type = search_hint_media_type(&item.type_, &item.media_type);
    api::SearchHint {
        id: item.id,
        item_id: item.id,
        name: item.name,
        matched_term: Some(matched_term.to_string()),
        type_: item.type_,
        primary_image_tag: item
            .image_tags
            .and_then(|t| t.primary),
        production_year: item.production_year,
        run_time_ticks: item.run_time_ticks,
        is_folder: Some(item.is_folder),
        media_type: Some(media_type),
        series_id: item.series_id,
        series_name: item.series_name,
        ..Default::default()
    }
}

/// Jellyfin's `SearchHint.MediaType` is the content category (Video/Audio/…),
/// not the item kind (Movie/Series/…).
fn search_hint_media_type(
    item_type: &api::MediaType,
    fallback: &api::MediaType,
) -> String {
    use api::MediaType;
    match item_type {
        MediaType::Audio
        | MediaType::MusicAlbum
        | MediaType::MusicArtist
        | MediaType::MusicVideo
        | MediaType::AudioBook => "Audio".to_string(),
        MediaType::Photo | MediaType::PhotoAlbum => "Photo".to_string(),
        MediaType::Book => "Book".to_string(),
        _ => match fallback {
            MediaType::Audio
            | MediaType::MusicAlbum
            | MediaType::MusicArtist
            | MediaType::MusicVideo
            | MediaType::AudioBook => "Audio".to_string(),
            MediaType::Unknown => "Video".to_string(),
            other => other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn search_hint_includes_id_and_video_media_type_for_series() {
        let item = api::BaseItemDto {
            id: Uuid::new_v4(),
            name: Some("Breaking Bad".to_string()),
            type_: api::MediaType::Series,
            media_type: api::MediaType::Video,
            is_folder: true,
            ..Default::default()
        };
        let hint = item_to_hint(item, "break");
        assert_eq!(hint.id, hint.item_id);
        assert_eq!(hint.media_type.as_deref(), Some("Video"));
        assert_eq!(hint.matched_term.as_deref(), Some("break"));
        assert_eq!(hint.type_, api::MediaType::Series);
    }
}
