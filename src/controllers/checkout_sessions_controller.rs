use crate::models::{CheckoutSession, Site};
use rocket::{response::status::NotFound, State};
use rocket_contrib::json::Json;

#[post("/", data = "<checkout>")]
pub async fn create(
  site: State<'_, Site>,
  checkout: Json<CheckoutSession>,
) -> Result<Json<CheckoutSession>, NotFound<()>> {
  checkout
    .into_inner()
    .save(&site)
    .await
    .map(Json)
    .ok_or(NotFound(()))
}
