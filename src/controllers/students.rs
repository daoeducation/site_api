use crate::models::{PublicStudentForm, DiscordToken};
use super::*;

#[get("/")]
pub async fn index<'a>(site: &'a State<Site>, _session: AdminSession) -> JsonResult<Vec<Student>> {
  Ok(Json(site.student().select().order_by(StudentOrderBy::Id).all().await?))
}

#[post("/discord_success?<discord_data..>")]
pub async fn discord_success(site: &State<Site>, discord_data: DiscordToken) -> Result<String> {
  site.student().process_discord_response(discord_data).await
}

#[post("/", data = "<form>")]
pub async fn create<'a>(form: Json<PublicStudentForm>, country: Country, site: &'a State<Site>) -> JsonResult<StudentState> {
  let student = site.student().insert()
    .use_struct(form.0.into_insert_student(&country))
    .save_and_subscribe(country.plan()).await?;
  let billing = BillingSummary::new(student).await?;
  billing.invoice_all_not_invoiced_yet().await?;
  billing.student.send_payment_reminder().await?;
  Ok(Json(StudentState::new(billing.student).await?))
}

#[get("/<student_id>")]
pub async fn show<'a>(site: &'a State<Site>, student_id: i32, _session: AdminSession) -> JsonResult<StudentState> {
  let student = site.student().find(&student_id).await?;
  Ok(Json(StudentState::new(student).await?))
}

#[post("/create_guest", data = "<form>")]
pub async fn create_guest<'a>(form: Json<PublicStudentForm>, _session: AdminSession, site: &'a State<Site>) -> JsonResult<StudentState> {
  let student = site.student().insert()
    .use_struct(form.0.into_insert_student(&Country("XX".to_string())))
    .save_and_subscribe(site.settings.pricing.guest.clone()).await?;
  student.subscription().await?.on_paid().await?;
  Ok(Json(StudentState::new(student).await?))
}

#[get("/by_wordpress_id/<wordpress_id>")]
pub async fn by_wordpress_id<'a>(site: &'a State<Site>, wordpress_id: String, _session: AdminSession) -> JsonResult<StudentState> {
  let student = site.student().select().wordpress_user_eq(&Some(wordpress_id)).one().await?;
  Ok(Json(StudentState::new(student).await?))
}
