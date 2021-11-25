use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: invoices,
  Invoice {
    #[sqlx_search_as int4]
    id: i32,
    #[sqlx_search_as int4]
    student_id: i32,
    created_at: UtcDateTime,
    #[sqlx_search_as payment_method]
    payment_method: PaymentMethod,
    #[sqlx_search_as varchar]
    external_id: String,
    #[sqlx_search_as decimal]
    amount: Decimal,
    description: String,
    url: String,
    #[sqlx_search_as boolean]
    paid: bool,
    #[sqlx_search_as boolean]
    expired: bool,
    #[sqlx_search_as int4]
    payment_id: Option<i32>,
    notified_on: Option<UtcDateTime>,
  }
}

impl Invoice {
  pub async fn make_payment(&self, clearing_data: Option<&str>) -> Result<Payment> {
    self.site.payment().build(NewPaymentAttrs{
      student_id: self.attrs.student_id,
      created_at: Utc::now(),
      amount: self.attrs.amount,
      fees: Decimal::ZERO,
      payment_method: self.attrs.payment_method,
      clearing_data: clearing_data.unwrap_or("").to_string(),
      invoice_id: Some(self.attrs.id),
    }).create_and_pay_invoice().await
  }
}

