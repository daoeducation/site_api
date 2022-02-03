use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: payments,
  struct Payment {
    #[sqlx_search_as(int4)]
    id: i32,
    #[sqlx_search_as(int4)]
    student_id: i32,
    created_at: UtcDateTime,
    #[sqlx_search_as(decimal)]
    amount: Decimal,
    fees: Decimal,
    payment_method: PaymentMethod,
    clearing_data: String,
    #[sqlx_search_as(int4)]
    invoice_id: Option<i32>,
  }
}

impl InsertPaymentHub {
  pub async fn create_and_pay_invoice(self) -> Result<Payment> {
    let payment = self.save().await?;

    if let Some(id) = payment.attrs.invoice_id {
      sqlx::query!(
        "UPDATE invoices SET paid = true, payment_id = $2 WHERE id = $1", 
        id,
        payment.attrs.id,
      ).execute(&payment.state.db).await?;
    }

    let student = payment.state.student().find(payment.student_id()).await?;
    BillingSummary::new(student).await?.sync_paid_status().await?;

    Ok(payment)
  }
}

impl PaymentHub {
  pub async fn from_btcpay_webhook(&self, webhook: &btcpay::Webhook) -> Result<Option<Payment>> {
    if webhook.kind != btcpay::WebhookType::InvoiceSettled {
      return Ok(None)
    }

    let maybe_invoice = self.state.invoice().select()
      .external_id_eq(&webhook.invoice_id)
      .payment_method_eq(&PaymentMethod::BtcPay)
      .optional().await?;

    if maybe_invoice.as_ref().and_then(|i| i.attrs.payment_id ).is_some() {
      return Ok(None)
    }

    if let Some(invoice) = maybe_invoice {
      Ok(Some(invoice.make_payment(None).await?))
    } else {
      Ok(None)
    }
  }

  pub async fn from_invoice(&self, invoice_id: i32) -> Result<Option<Payment>> {
    let maybe_invoice = self.state.invoice().select()
      .id_eq(&invoice_id)
      .payment_id_is_set(false)
      .optional().await?;

    if let Some(invoice) = maybe_invoice {
      Ok(Some(invoice.make_payment(None).await?))
    } else {
      Ok(None)
    }
  }

  pub async fn from_stripe_event(&self, e: &stripe::Event) -> Result<Option<Payment>> {
    use stripe::{EventType, EventObject};

    if let (EventType::InvoicePaymentSucceeded, EventObject::Invoice(i)) = (&e.event_type, &e.data.object) {
      if !i.paid.unwrap_or(false) {
        return Ok(None);
      }

      let customer_id = i.customer.as_ref().map(|c| c.id().to_string() ).ok_or(Error::validation("customer","missing"))?;
      let maybe_student = self.state.student().select()
        .stripe_customer_id_eq(&Some(customer_id.clone()))
        .optional().await?;

      if let Some(student) = maybe_student {
        let amount = Decimal::new(i.amount_paid.ok_or(Error::validation("amount_paid", "missing"))?, 2);
        let maybe_invoice = self.state.invoice().select()
          .amount_eq(&amount)
          .student_id_eq(student.id())
          .payment_method_eq(&PaymentMethod::Stripe)
          .optional().await?;

        Ok(Some(self.insert().use_struct(InsertPayment{
          student_id: student.attrs.id,
          created_at: Utc::now(),
          amount: amount,
          fees: Decimal::ZERO,
          payment_method: PaymentMethod::Stripe,
          clearing_data: serde_json::to_string(&i)?,
          invoice_id: maybe_invoice.map(|i| i.attrs.id),
        }).create_and_pay_invoice().await?))
      } else {
        Ok(None)
      }
    } else {
      Ok(None)
    }
  }
}

