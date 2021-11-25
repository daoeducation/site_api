use super::*;

make_sqlx_model!{
  state: Site,
  table: monthly_charges,
  MonthlyCharge {
    #[sqlx_search_as int4]
    id: i32,
    created_at: UtcDateTime,
    billing_period: UtcDateTime,
    #[sqlx_search_as int4]
    student_id: i32,
    #[sqlx_search_as int4]
    subscription_id: i32,
    price: Decimal,
    #[sqlx_search_as boolean]
    paid: bool,
    paid_at: Option<UtcDateTime>,
  }
}
