use crate::models::{PublicStudentForm, BillingSummary, Db};
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
  Ok(Json(StudentState::new(&site, session.student)))
}

#[post("/pay_now")]
pub async fn pay_now(site: &State<Site>, session: Session) -> JsonResult<Option<Invoice>> {
  BillingSummary::new(&site, session.student).await?.invoice_everything().await.map(Json)
}

#[post("/discord_success")]
pub async fn discord_success(site: &State<Site>, uri: Uri<'_>) -> Result<String> {
  match uri.reference().and_then(|x| x.fragment() ).map(|x| x.url_decode_lossy() ) {
    Some(discord_data) => {
      Student::process_discord_response(&site, &discord_data).await?;
      Ok(format!("Ya puedes acceder a https://discord.com/channels/{}", site.settings.discord.guild_id))
    },
    _ => Ok((BadRequest, format!("Invalid uri {}", &uri)))
  }
}
