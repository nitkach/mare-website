use anyhow::{anyhow, Result};
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Form, Router};
use chrono::{Utc, DateTime};
use serde::Deserialize;

use crate::database::breed::Breed;
use crate::database::{Database, DatabaseRecord, SetState};
use app_error::AppError;

mod app_error;

pub async fn run() -> Result<()> {
    let shared_state = Database::init().await?;

    // build our application with a single route
    let app = Router::new()
        .route("/", get(get_index))
        .route("/mares", get(get_mare_table))
        .route("/mares", post(post_mares))
        .route("/mares/:id", get(get_mare))
        .route("/mares/:id/delete", post(delete_mare))
        .route("/mares/:id/edit", post(edit_mare))
        .route("/mares/:id/image", get(mare_image))
        .with_state(shared_state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

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
    let vec_of_ponies = pool.list().await?;

    let html = MareTableTemplate {
        ponies: vec_of_ponies,
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

    let _ = pool.add(&form).await?;

    Ok(axum::response::Redirect::to("/mares"))
}

async fn delete_mare(
    State(pool): State<Database>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let Some(_) = pool.remove(id).await? else {
        return Err(AppError::with_status_404(anyhow!("Cannot find record with {id} id.")));
    };

    Ok(axum::response::Redirect::to("/mares"))
}

#[derive(Debug, Template)]
#[template(path = "get_mare.askama.html")]
struct GetMareTemplate {
    name: String,
    breed: Breed,
    id: i32,
    modified_at: DateTime<Utc>
}

async fn get_mare(
    State(pool): State<Database>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let Some(mare) = pool.get(id).await? else {
        return Err(AppError::with_status_404(anyhow!("Cannot find record with {id} id.")));
    };

    let html = GetMareTemplate {
        name: mare.name,
        breed: mare.breed,
        id,
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
    Path(id): Path<i32>,
    form: Form<EditPonyForm>,
) -> Result<impl IntoResponse, AppError> {
    let pony_data = form.0;

    // DateTime::<Utc>::to_rfc3339(&self)
    // // problem: if first user edit data, but not send it, this is problem
    // // fix: add timestamp_updated_at as field -> send to request to update
    // // if timestamp != timestamp of record -> problem
    // // optimistic concurrency

    // chrono::DateTime::
    // // sqlx feature to support Timestamp

    // // html: timestamp (when sended) - hidden form input
    // let Some(_) = pool.set(id, &pony_data).await? else
    let reason = match pool.set(id, &pony_data).await? {
        SetState::Success => return Ok(axum::response::Redirect::to("/mares")),
        SetState::ModifiedAtConflict => " since the mare's record has already changed.",
        SetState::RecordNotFound => " since the mare's record is not found.",
    };

    let mut message = "Unfortunately, it is impossible to save,".to_owned();

    message.push_str(reason);

    // TODO possible to direct user to the mare page with the data he specified
    Err(AppError::new(axum::http::StatusCode::CONFLICT, anyhow!(message))).into()
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

#[derive(Debug, Template)]
#[template(path = "mare_image.askama.html")]
struct MareImageTemplate {
    name: String,
    pony_id: i32,
    image_id: i64,
    image: String,
}

async fn mare_image(
    State(pool): State<Database>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let name = match pool.get(id).await? {
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

    let tags = format!("score.gte:100, {name}, pony, mare, !irl");
    let request = client
        .get("https://derpibooru.org/api/v1/json/search/images")
        .query(&[("per_page", "1"), ("sf", "random"), ("q", &tags)]);

    let response = request.send().await?;

    let mut response = response.json::<ImageResponse>().await?;

    let image = match response.images.pop() {
        Some(image) => image,
        None => {
            // TODO
            return Err(AppError::new(axum::http::StatusCode::BAD_GATEWAY, anyhow!(
                "Cannot find images by \"{name}\" name."
            )))
            .into();
        }
    };

    let html = MareImageTemplate {
        name: name,
        pony_id: id,
        image_id: image.id,
        image: image.representations.medium,
    };

    Ok(html)
}
