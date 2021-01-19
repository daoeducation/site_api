use crate::models::{CheckoutSession, Program, Site};
use rocket::{response::status::NotFound, State};
use tera::Context;
use crate::TEMPLATES;

use rocket::response::content::Content;
use rocket::http::ContentType;


#[get("/zero_to_hero")]
pub async fn zero_to_hero(site: State<'_, Site>) -> Result<Content<String>, NotFound<()>> {
  do_checkout(&site, Program::ZeroToHero).await
}

#[get("/coding_bootcamp")]
pub async fn coding_bootcamp(site: State<'_, Site>) -> Result<Content<String>, NotFound<()>> {
  do_checkout(&site, Program::CodingBootcamp).await
}

async fn do_checkout(site: &Site, program: Program) -> Result<Content<String>, NotFound<()>> {
  let checkout = CheckoutSession::create(&site, program).await.ok_or(NotFound(()))?;
  let context = Context::from_serialize(&checkout).map_err(|_| NotFound(()))?;
  let content = TEMPLATES.render("checkout_sessions/show", &context).map_err(|_| NotFound(()))?;
  Ok(Content(ContentType::HTML, content))
}
