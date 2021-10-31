use crate::models::{PublicStudentForm, BillingSummary, Db, DiscordToken};
use rocket::{
  response::status::BadRequest,
  http::uri::{Uri, Reference}
};
use super::*;

#[post("/", data = "<form>")]
pub async fn create(form: Json<PublicStudentForm>, country: Country, site: &State<Site>) -> JsonResult<String> {
  let student = form.save(&site, country).await?;
  Ok(Json(student.make_profile_link(&site, &site.settings.checkout_domain, 72).await?))
}

#[get("/")]
pub async fn show<'a>(site: &'a State<Site>, session: Session) -> JsonResult<StudentState<'a>> {
  Ok(Json(StudentState::new(&site, session.student).await?))
}

#[post("/pay_now")]
pub async fn pay_now(site: &State<Site>, session: Session) -> JsonResult<Option<Invoice>> {
  BillingSummary::new(&site, session.student).await?.invoice_everything().await.map(Json)
}

#[post("/discord_success?<discord_data..>")]
pub async fn discord_success(site: &State<Site>, discord_data: DiscordToken) -> Result<String> {
  Student::process_discord_response(&site, discord_data).await
}
