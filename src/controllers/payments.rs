use crate::models::{Payment, btcpay};
use super::*;

#[post("/handle_stripe_events", data = "<webhook>")]
pub async fn handle_stripe_events(webhook: StripeWebhook, site: &State<Site>) -> JsonResult<&str> {
  Payment::from_stripe_event(&webhook.event, &site).await?;
  Ok(Json("OK"))
}

#[post("/handle_btcpay_webhooks", data = "<webhook>")]
pub async fn handle_btcpay_webhooks(webhook: btcpay::Webhook, site: &State<Site>) -> JsonResult<&str> {
  Payment::from_btcpay_webhook(&webhook, &site).await?;
  Ok(Json("OK"))
}

#[post("/from_invoice?<invoice_id>")]
pub async fn from_invoice<'a>(site: &'a State<Site>, invoice_id: i32, _session: AdminSession) -> JsonResult<&str> {
  Payment::from_invoice(&site, invoice_id).await?;
  Ok(Json("OK"))
}

