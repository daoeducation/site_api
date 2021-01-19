use crate::models::{CheckoutSession, Program, Site};
use rocket::{response::status::NotFound, State};
use rocket_contrib::templates::Template;

#[get("/zero_to_hero")]
pub async fn zero_to_hero(site: State<'_, Site>) -> Result<Template, NotFound<()>> {
  do_checkout(&site, Program::ZeroToHero).await
}

#[get("/coding_bootcamp")]
pub async fn coding_bootcamp(site: State<'_, Site>) -> Result<Template, NotFound<()>> {
  do_checkout(&site, Program::CodingBootcamp).await
}

async fn do_checkout(site: &Site, program: Program) -> Result<Template, NotFound<()>> {
  let checkout = CheckoutSession::create(&site, program).await.ok_or(NotFound(()))?;
  Ok(Template::render("checkout_sessions/show", &checkout))
}
