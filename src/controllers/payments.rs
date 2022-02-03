use crate::models::btcpay;
use super::*;

#[post("/handle_stripe_events", data = "<webhook>")]
pub async fn handle_stripe_events(webhook: StripeWebhook, site: &State<Site>) -> JsonResult<&str> {
  site.payment().from_stripe_event(&webhook.event).await?;
  Ok(Json("OK"))
}

#[post("/handle_btcpay_webhooks", data = "<webhook>")]
pub async fn handle_btcpay_webhooks(webhook: btcpay::Webhook, site: &State<Site>) -> JsonResult<&str> {
  site.payment().from_btcpay_webhook(&webhook).await?;
  Ok(Json("OK"))
}

#[post("/from_invoice?<invoice_id>")]
pub async fn from_invoice<'a>(site: &'a State<Site>, invoice_id: i32, _session: AdminSession) -> JsonResult<&str> {
  site.payment().from_invoice(invoice_id).await?;
  Ok(Json("OK"))
}

#[get("/get_pricing")]
pub async fn get_pricing(country: Country, site: &State<Site>) -> Json<(Plan, Plan)> {
  Json((site.settings.pricing.global.clone(), country.plan()))
}
