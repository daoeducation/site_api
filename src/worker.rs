use daoe_api::{
  error::Result,
  models::{Site, SiteSettings, BillingSummary, StudentQuery}
};
use chrono::Utc;

#[tokio::main]
async fn main() {
  let site = SiteSettings::default()
    .into_site()
    .await
    .expect("Could not validate site state");

  loop {
    if let Err(e) = process_students_billing(&site).await {
      println!("Unexpected error ocurred {}", e);
    }
  }
}

async fn process_students_billing(site: &Site) -> Result<()> {
  let students = site.student().all(&StudentQuery::default()).await?;
  for student in students.into_iter() {
    let billing_summary = BillingSummary::new(student).await?;
    billing_summary.create_monthly_charges_for(&Utc::today()).await?;
  }
  Ok(())
}
