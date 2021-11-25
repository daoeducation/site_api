use crate::models::{PublicStudentForm, DiscordToken};
use super::*;

#[post("/", data = "<form>")]
pub async fn create<'a>(form: Json<PublicStudentForm>, country: Country, site: &'a State<Site>) -> JsonResult<StudentState> {
  let new_student = form.0.into_new_student(country, &site);
  let student = new_student.save_and_subscribe().await?;
  let billing = BillingSummary::new(student).await?;
  billing.invoice_all_not_invoiced_yet().await?;
  billing.student.send_payment_reminder().await?;
  Ok(Json(StudentState::new(billing.student).await?))
}

#[get("/<student_id>")]
pub async fn show<'a>(site: &'a State<Site>, student_id: i32, _session: AdminSession) -> JsonResult<StudentState> {
  let student = site.student().find_by_id(student_id).await?;
  Ok(Json(StudentState::new(student).await?))
}
#[post("/discord_success?<discord_data..>")]
pub async fn discord_success(site: &State<Site>, discord_data: DiscordToken) -> Result<String> {
  site.student().process_discord_response(discord_data).await
}
