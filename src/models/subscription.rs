use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: subscriptions,
  struct Subscription {
    #[sqlx_search_as(int4)]
    id: i32,
    created_at: UtcDateTime,
    #[sqlx_search_as(int4)]
    student_id: i32,
    #[sqlx_search_as(boolean)]
    active: bool,
    price: Decimal,
    paid: bool,
    plan_code: PlanCode,
    paid_at: Option<UtcDateTime>,
    #[sqlx_search_as(varchar)]
    stripe_subscription_id: Option<String>,
  }
}

impl Subscription {
  pub async fn on_paid(&self) -> Result<()> {
    let mut student = self.state.student().find(self.student_id()).await?;
    student.setup_discord_verification().await?;
    student.setup_wordpress().await?;
    student.send_welcome_email()?;

    Ok(())
  }
}

