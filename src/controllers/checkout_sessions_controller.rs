//use crate::models::{CheckoutSession, Program, Site};
use super::*;

#[get("/zero_to_hero")]
pub async fn zero_to_hero(site: &State<Site>) -> Result<RawHtml<String>> {
  do_checkout(&site, Program::ZeroToHero).await
}

#[get("/academy")]
pub async fn academy(site: &State<Site>) -> Result<RawHtml<String>> {
  do_checkout(&site, Program::Academy).await
}

async fn do_checkout(_site: &Site, _program: Program) -> Result<RawHtml<String>> {
  /*
  let checkout = CheckoutSession::create(&site, program).await?;
  let context = Context::from_serialize(&checkout)?;
  let html = TEMPLATES.render("checkout_sessions/show", &context)?;
  Ok(RawHtml(html))
  */
  todo!("Must be removed");
}
