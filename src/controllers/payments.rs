use crate::models::{Db, Payment, btcpay};
use sqlx::types::Decimal;
use super::*;

#[post("/handle_stripe_events", data = "<event>")]
pub async fn handle_stripe_events(event: Json<stripe::Event>, site: &State<Site>) -> JsonResult<&str> {
  Payment::from_stripe_event(&event, &site).await?;
  Ok(Json("OK"))
}

#[post("/handle_btcpay_webhooks", data = "<webhook>")]
pub async fn handle_coingate_callbacks(webhook: btcpay::Webhook, site: &State<Site>) -> JsonResult<&str> {
  Payment::from_btcpay_webhook(&webhook, &site).await?;
  Ok(Json("OK"))
}
