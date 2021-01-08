use crate::models::{Config, Site};
use rocket::State;
use rocket_contrib::json::Json;

#[get("/")]
pub fn index(site: State<Site>) -> Json<Config> {
  Json(site.config.clone())
}
