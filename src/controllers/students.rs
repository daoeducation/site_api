use crate::models::{PublicStudentForm, DiscordToken};
use super::*;

#[post("/", data = "<form>")]
pub async fn create<'a>(form: Json<PublicStudentForm>, country: Country, site: &'a State<Site>) -> JsonResult<StudentState<'a
>> {
  let student = form.save(&site, country).await?;
  let billing = BillingSummary::new(site, student).await?;
  billing.invoice_all_not_invoiced_yet().await?;
  billing.student.send_payment_reminder(&site).await?;
  Ok(Json(StudentState::new(&site, billing.student).await?))
}

#[get("/<student_id>")]
pub async fn show<'a>(site: &'a State<Site>, student_id: i32, _session: AdminSession) -> JsonResult<StudentState<'a>> {
  let student = Student::find_by_id(&site, student_id).await?;
  Ok(Json(StudentState::new(&site, student).await?))
}
#[post("/discord_success?<discord_data..>")]
pub async fn discord_success(site: &State<Site>, discord_data: DiscordToken) -> Result<String> {
  Student::process_discord_response(&site, discord_data).await
}
