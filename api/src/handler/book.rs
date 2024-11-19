use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use registry::AppRegistry;
use shared::error::{AppError};
use kernel::model::id::BookId;
use crate::model::book::{BookResponse, CreateBookRequest};

#[axum::debug_handler]
pub async fn register_book(
    State(registry): State<AppRegistry>,
    Json(req): Json<CreateBookRequest>,
) -> Result<StatusCode, AppError> {
    registry
        .book_repository()
        .create(req.into())
        .await
        .map(|_| StatusCode::CREATED)
}

#[axum::debug_handler]
pub async fn show_book_list(
    State(registry): State<AppRegistry>,
) -> Result<Json<Vec<BookResponse>>, AppError> {
    registry
        .book_repository()
        .find_all()
        .await
        .map(|v| v.into_iter().map(BookResponse::from).collect::<Vec<_>>())
        .map(Json)
}

#[axum::debug_handler]
pub async fn show_book(
    Path(book_id): Path<BookId>,
    State(registry): State<AppRegistry>,
) -> Result<Json<BookResponse>,AppError> {
    registry
        .book_repository()
        .find_by_id(book_id)
        .await
        .and_then(|bc| match bc {
            Some(bc) => Ok(Json(bc.into())),
            None => Err(AppError::EntityNotFound("not found".into())),
        })
}
