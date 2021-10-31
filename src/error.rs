use sqlx::postgres::PgDatabaseError;
use std::error::Error as ErrorTrait;

use rocket::{
  http::Status,
  request::Request,
  response::{self, Responder},
  serde::json::{json, Json},
  warn,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  IOError(#[from] std::io::Error),
  #[error(transparent)]
  DatabaseError(sqlx::Error),
  #[error("Invalid {field}: {message}")]
  Validation { field: String, message: String },
  #[error(transparent)]
  ValidationError(#[from] validator::ValidationErrors),
  #[error(transparent)]
  Config(#[from] rocket::figment::Error),
  #[error(transparent)]
  Template(#[from] tera::Error),
  #[error(transparent)]
  Utf8Error(#[from] std::str::Utf8Error),
  #[error(transparent)]
  JsonSerde(#[from] serde_json::Error),
  #[error("A configured stripe price is wrong")]
  InvalidStripePrice,
  #[error(transparent)]
  Stripe(#[from] stripe::Error),
  #[error(transparent)]
  ParseIdError(#[from] stripe::ParseIdError),
  #[error(transparent)]
  UreqError(#[from] ureq::Error),
}

impl From<rocket::form::Errors<'_>> for Error {
  fn from(err: rocket::form::Errors<'_>) -> Error {
    Error::validation("form", &format!("{}", err))
  }
}

impl From<sqlx::Error> for Error {
  fn from(err: sqlx::Error) -> Error {
    match err {
      sqlx::Error::Database(ref inner_error) => {
        let pg_error = inner_error.downcast_ref::<PgDatabaseError>();
        match pg_error.code() {
          "23505" => Error::validation(
            "uniqueness",
            pg_error.detail().unwrap_or("id already exists"),
          ),
          "23503" => Error::validation("nonexistent", "references a nonexistent resource"),
          _ => Error::DatabaseError(err),
        }
      }
      _ => Error::DatabaseError(err),
    }
  }
}

impl Error {
  pub fn validation(field: &str, message: &str) -> Error {
    Error::Validation {
      field: field.to_string(),
      message: message.to_string(),
    }
  }
}

impl<'r> Responder<'r, 'static> for Error {
  fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
    let response = match self {
      Error::ValidationError(_) | Error::Validation { .. } => (
        Status::UnprocessableEntity,
        Json(json![{"error": self.to_string()}]),
      ),
      Error::DatabaseError(sqlx::Error::RowNotFound) => {
        (Status::NotFound, Json(json![{ "error": "Not found" }]))
      }
      _ => {
        warn!(
          "A wild error appeared: {:?}\n\n{:?}\n",
          &self,
          &self.source()
        );
        (
          Status::InternalServerError,
          Json(json![{ "error": "Unexpected Error" }]),
        )
      }
    };

    response.respond_to(request)
  }
}

pub type Result<T> = std::result::Result<T, Error>;
