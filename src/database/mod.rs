use std::fmt::Display;

use anyhow::Result;
use chrono::Utc;
use log::LevelFilter;
use serde::Deserialize;
use sqlx::{postgres::PgConnectOptions, ConnectOptions, PgPool};
use tracing::{info, instrument, warn, Level};
use url::{self, Url};

use crate::app::{AddPonyForm, EditPonyForm};

pub(crate) mod breed;

#[derive(Debug, Deserialize)]
struct SetStatus {
    code: SetState,
}

#[repr(i32)]
#[derive(Debug, Deserialize, sqlx::Type)]
pub(crate) enum SetState {
    Success = 0,
    ModifiedAtConflict = 1,
    RecordNotFound = 2,
}

impl Display for SetState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<i32> for SetState {
    fn from(value: i32) -> Self {
        match value {
            0 => SetState::Success,
            1 => SetState::ModifiedAtConflict,
            2 => SetState::RecordNotFound,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DatabaseRecord {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) breed: breed::Breed,
    pub(crate) modified_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct Database {
    pool: PgPool,
}

impl Database {
    #[instrument(level = Level::INFO)]
    pub(crate) async fn init() -> Result<Self> {
        let database_url = Url::parse(&std::env::var("DATABASE_URL")?)?;

        let options = PgConnectOptions::from_url(&database_url)?
            .log_statements(LevelFilter::Debug)
            .log_slow_statements(LevelFilter::Warn, core::time::Duration::from_secs(1));

        let pool = PgPool::connect_with(options).await?;

        info!(
            database_url = database_url.to_string(),
            "Established connection to database"
        );

        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    #[instrument(level = Level::INFO, skip(self))]
    pub(crate) async fn add(&self, data: &AddPonyForm) -> Result<i64> {
        let breed: i32 = data.breed.into();

        let query = sqlx::query_as!(
            DatabaseRecord,
            r#"insert into mares (name, breed, modified_at)
            values ($1, $2, CURRENT_TIMESTAMP)
            returning id as "id!", name as "name!", breed as "breed!", modified_at as "modified_at!";
            "#,
            data.name,
            breed
        );

        let record = query.fetch_one(&self.pool).await?;

        // TODO rows_affected=1 rows_returned=0 elapsed=3.8952ms
        // structured logging
        info!(
            name = record.name,
            breed = record.breed.to_string(),
            created_at = record.modified_at.to_string(),
            id = record.id,
            "Added new record: \"{}\", with id = {}",
            record.name,
            record.id
        );

        Ok(record.id)
    }

    #[instrument(level = Level::INFO, skip(self))]
    pub(crate) async fn get(&self, id: i32) -> Result<Option<DatabaseRecord>> {
        let query = sqlx::query_as!(
            DatabaseRecord,
            r#"
            select id as "id!", name as "name!", breed as "breed!", modified_at as "modified_at!"
            from mares
            where id = $1
            "#,
            id
        );

        let record = query.fetch_optional(&self.pool).await?;

        if let Some(record) = &record {
            // TODO
            info!(
                name = record.name,
                breed = record.breed.to_string(),
                created_at = record.modified_at.to_string(),
                id = record.id,
                "Received record: \"{}\", with id = {id}",
                record.name
            );
        } else {
            warn!("Record with id = {id} not found in database.");
        }

        Ok(record)
    }

    #[instrument(level = Level::INFO, skip(self))]
    pub(crate) async fn set(&self, id: i32, data: &EditPonyForm) -> Result<SetState> {
        // TODO return previous record data
        // SQLite doesn't support this feature :/
        // https://stackoverflow.com/questions/6725964/sqlite-get-the-old-value-after-update

        let breed: i32 = data.breed.into();

        //     info!(
        //         "Record with id = {id} modified to {} | {} | {}",
        //         record.name, record.breed, record.modified_at
        //     );
        // warn!(
        //         "Record with id = {id} and timestamp = {} not found in database.",
        //         data.modified_at
        //     );

        let query = sqlx::query_as!(
            SetStatus,
            r#"
            select code as "code!"
            from set_mare_record($1, $2, $3, $4)
            "#,
            id,
            data.name,
            breed,
            data.modified_at
        );

        let set_status = query.fetch_one(&self.pool).await?;

        Ok(set_status.code)
    }

    #[instrument(level = Level::INFO, skip(self))]
    pub(crate) async fn list(&self) -> Result<Vec<DatabaseRecord>> {
        let query = sqlx::query_as!(
            DatabaseRecord,
            r#"
            select *
            from mares
            "#
        );

        let records = query.fetch_all(&self.pool).await?;

        info!(
            "GET: list of records. Total records found: {}.",
            records.len()
        );

        Ok(records)
    }

    #[instrument(level = Level::INFO, skip(self))]
    pub(crate) async fn remove(&self, id: i32) -> Result<Option<DatabaseRecord>> {
        let query = sqlx::query_as!(
            DatabaseRecord,
            r#"
            delete from mares
            where id = $1
            returning name as "name!", breed as "breed!", id as "id!", modified_at as "modified_at!"
            "#,
            id
        );

        let record = query.fetch_optional(&self.pool).await?;

        if let Some(record) = &record {
            info!("REMOVE: record with id = {id} removed from database.");
        } else {
            warn!("Record with id = {id} not found in database.",);
        }

        Ok(record)
    }
}
