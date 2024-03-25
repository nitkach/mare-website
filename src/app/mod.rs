use anyhow::{anyhow, Result};
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{debug_handler, Form, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tower_http::trace::{self, TraceLayer};
use tracing::{error, info, warn, Level};

use crate::database::breed::Breed;
use crate::database::{Database, DatabaseRecord, PagingState, SetState};
use app_error::AppError;

mod app_error;

pub async fn run() -> Result<()> {
    let shared_state = Database::init().await?;

    // build our application with a single route
    let layer = TraceLayer::new_for_http()
        .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
        .on_response(trace::DefaultOnResponse::new().level(Level::INFO));

    let routes = Router::new()
        .route("/", get(get_index))
        .route("/mares", get(get_mare_table))
        .route("/mares", post(post_mares))
        .route("/mares/page/:page/:state/:id", get(get_paged_mare_table))
        .route("/mares/:id", get(get_mare))
        .route("/mares/:id/delete", post(delete_mare))
        .route("/mares/:id/edit", post(edit_mare))
        .route("/mares/:id/image", get(mare_image))
        .layer(layer)
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let (ip, port) = {
        let x = listener.local_addr().unwrap();
        (x.ip(), x.port())
    };

    info!(ip = ?ip, port = ?port, "Bound IP address and port.");

    axum::serve(listener, routes.into_make_service()).await?;

    Ok(())
}

#[derive(Debug, Template)]
#[template(path = "index.askama.html")]
struct IndexTemplate;

async fn get_index() -> Result<impl IntoResponse, AppError> {
    let html = IndexTemplate;

    Ok(html)
}

#[derive(Debug, Template)]
#[template(path = "mare_table.askama.html")]
struct MareTableTemplate {
    ponies: Vec<DatabaseRecord>,
}

async fn get_mare_table(State(pool): State<Database>) -> Result<impl IntoResponse, AppError> {
    let mare_records = pool.list().await?;

    let html = MareTableTemplate {
        ponies: mare_records,
    };

    Ok(html)
}

#[derive(Deserialize)]
struct PagingParameters {
    page: u32,
    id: String,
    state: PagingState,
}

#[derive(Debug, Template)]
#[template(path = "paged_mare_table.askama.html")]
struct PagedMareTableTemplate {
    ponies: Vec<DatabaseRecord>,
    first_id: Option<String>,
    last_id: Option<String>,
    page: u32,
}

#[debug_handler]
async fn get_paged_mare_table(
    State(pool): State<Database>,
    Path(params): Path<PagingParameters>,
) -> Result<impl IntoResponse, AppError> {
    let mare_records = pool.get_paged_records(&params.id, params.state).await?;

    let (first_id, last_id) = match (mare_records.first(), mare_records.last()) {
        (None, None) => (None, None),
        (Some(first), Some(last)) => (Some(first.id.to_string()), Some(last.id.to_string())),

        _ => unreachable!(),
    };

    let html = PagedMareTableTemplate {
        ponies: mare_records,
        first_id,
        last_id,
        page: params.page,
    };

    Ok(html)
}

#[derive(Deserialize, Debug)]
pub(crate) struct AddPonyForm {
    pub(crate) name: String,
    pub(crate) breed: Breed,
}

async fn post_mares(
    State(pool): State<Database>,
    form: Form<AddPonyForm>,
) -> Result<impl IntoResponse, AppError> {
    let form = form.0;

    if form.name.len() > 100 {
        return Err(AppError::new(
            axum::http::StatusCode::BAD_REQUEST,
            anyhow!(
                "Allowed name length has been exceeded.\nCurrent length: {}, maximum: 100.",
                form.name.len()
            ),
        ));
    }

    let _ = pool.add(&form).await?;

    Ok(axum::response::Redirect::to("/mares"))
}

async fn delete_mare(
    State(pool): State<Database>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let Some(_) = pool.remove(&id).await? else {
        return Err(AppError::with_status_404(anyhow!(
            "Cannot find record with {id} id."
        )));
    };

    Ok(axum::response::Redirect::to("/mares"))
}

#[derive(Debug, Template)]
#[template(path = "get_mare.askama.html")]
struct GetMareTemplate {
    name: String,
    breed: Breed,
    id: String,
    modified_at: DateTime<Utc>,
}

async fn get_mare(
    State(pool): State<Database>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let Some(mare) = pool.get(&id).await? else {
        return Err(AppError::with_status_404(anyhow!(
            "Cannot find record with {id} id."
        )));
    };

    let html = GetMareTemplate {
        name: mare.name,
        breed: mare.breed,
        id: id.to_string(),
        modified_at: mare.modified_at,
    };

    Ok(html)
}

#[derive(Debug, Deserialize)]
pub(crate) struct EditPonyForm {
    pub(crate) name: String,
    pub(crate) breed: Breed,
    pub(crate) modified_at: DateTime<Utc>,
}

async fn edit_mare(
    State(pool): State<Database>,
    Path(id): Path<String>,
    form: Form<EditPonyForm>,
) -> Result<impl IntoResponse, AppError> {
    let pony_data = form.0;

    // problem: if first user edit data, but not send it, this is problem
    // fix: add timestamp_updated_at as field -> send to request to update
    // if timestamp != timestamp of record -> problem
    // optimistic concurrency

    // sqlx feature to support Timestamp

    // html: timestamp (when sended) - hidden form input
    let reason = match pool.set(&id, &pony_data).await? {
        SetState::Success => return Ok(axum::response::Redirect::to("/mares")),
        SetState::ModifiedAtConflict => "has already changed.",
        SetState::RecordNotFound => "not found.",
    };

    warn!("Cannot modify record with id = {id}, since record {reason}");
    let message =
        format!("Unfortunately, it is impossible to save, since the mare's record {reason}");

    // TODO possible to direct user to the mare page with the data he specified
    Err(AppError::new(
        axum::http::StatusCode::CONFLICT,
        anyhow!(message),
    ))
}

#[derive(Debug, Deserialize)]
struct ImageResponse {
    images: Vec<Image>,
}

#[derive(Debug, Deserialize)]
struct Image {
    id: i64,
    representations: Representations,
}

#[derive(Debug, Deserialize)]
struct Representations {
    // large: String,
    medium: String,
    // small: String,
}

#[derive(Serialize)]
struct MareQuery {
    per_page: usize,
    sf: String,
    q: String,
}

#[derive(Debug, Template)]
#[template(path = "mare_image.askama.html")]
struct MareImageTemplate {
    name: String,
    pony_id: String,
    image_id: i64,
    image: String,
}

async fn mare_image(
    State(pool): State<Database>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let name = match pool.get(&id).await? {
        Some(record) => record.name,
        None => {
            return Err(AppError::with_status_404(anyhow!(
                "Cannot find record with {id} id."
            )));
        }
    };

    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "MareWebsite",
            env!("CARGO_PKG_VERSION"),
            "https://github.com/nitkach",
        ))
        .build()?;

    let url = "https://derpibooru.org/api/v1/json/search/images";
    let tags = format!("score.gte:100, {name}, pony, mare, !irl");
    let query = [("per_page", "1"), ("sf", "random"), ("q", &tags)];
    let request = client.get(url).query(&query);

    info!(url = url, query = ?query, "Request created, sending...");
    let response = request.send().await?;

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(err) => {
            warn!("An error came from the server when trying to get an image");
            let code = if let Some(code) = err.status() {
                axum::http::StatusCode::from_u16(code.as_u16())?
            } else {
                error!("Error was generated from a response, but the status code is not found");
                axum::http::StatusCode::NOT_FOUND
            };
            return Err(AppError::new(
                code,
                anyhow!("Cannot find images by \"{name}\" name."),
            ));
        }
    };

    let mut response = response.json::<ImageResponse>().await?;

    let Some(image) = response.images.pop() else {
        // TODO
        return Err(AppError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            anyhow!("Cannot find images by \"{name}\" name."),
        ));
    };

    let html = MareImageTemplate {
        name,
        pony_id: id,
        image_id: image.id,
        image: image.representations.medium,
    };

    Ok(html)
}
