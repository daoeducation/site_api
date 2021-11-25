use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: subscriptions,
  Subscription {
    #[sqlx_search_as int4]
    id: i32,
    created_at: UtcDateTime,
    invoicing_day: i32,
    #[sqlx_search_as int4]
    student_id: i32,
    #[sqlx_search_as boolean]
    active: bool,
    price: Decimal,
    paid: bool,
    plan_code: PlanCode,
    paid_at: Option<UtcDateTime>,
    #[sqlx_search_as varchar]
    stripe_subscription_id: Option<String>,
  }
}

impl Subscription {
  pub async fn create_monthly_charge(&self, today: &UtcDate) -> Result<MonthlyCharge> {
    self.site.monthlycharge().build(NewMonthlyChargeAttrs{
      created_at: Utc::now(),
      billing_period: today.and_hms(0,0,0),
      student_id: self.attrs.student_id,
      subscription_id: self.attrs.id,
      price: self.site.settings.pricing.by_code(self.attrs.plan_code).monthly,
      paid: false,
      paid_at: None,
    }).save().await
  }

  pub async fn on_paid(&self) -> Result<()> {
    let mut student = self.site.student().find_by_id(self.attrs.student_id).await?;
    student.setup_discord_verification().await?;
    student.setup_wordpress().await?;
    student.send_welcome_email()?;

    Ok(())
  }

  pub fn next_invoicing_date(&self) -> UtcDateTime {
    use chrono::prelude::*;
    let today = Utc::today();
    let this_months = Utc.ymd(today.year(), today.month(), self.attrs.invoicing_day as u32);
    let date = if today >= this_months {
      this_months + RelativeDuration::months(1)
    }else{
      this_months
    };
    date.and_hms(0,0,0)
  }
}
