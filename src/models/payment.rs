use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: payments,
  Payment {
    #[sqlx_search_as int4]
    id: i32,
    #[sqlx_search_as int4]
    student_id: i32,
    created_at: UtcDateTime,
    #[sqlx_search_as decimal]
    amount: Decimal,
    fees: Decimal,
    payment_method: PaymentMethod,
    clearing_data: String,
    #[sqlx_search_as int4]
    invoice_id: Option<i32>,
  }
}

impl NewPayment {
  pub async fn create_and_pay_invoice(self) -> Result<Payment> {
    let payment = self.save().await?;

    if let Some(id) = payment.attrs.invoice_id {
      sqlx::query!(
        "UPDATE invoices SET paid = true, payment_id = $2 WHERE id = $1", 
        id,
        payment.attrs.id,
      ).execute(&payment.site.db).await?;
    }

    let student = payment.site.student().find_by_id(payment.attrs.student_id).await?;
    BillingSummary::new(student).await?.sync_paid_status().await?;

    Ok(payment)
  }
}

impl PaymentHub {
  pub async fn from_btcpay_webhook(&self, webhook: &btcpay::Webhook) -> Result<Option<Payment>> {
    if webhook.kind != btcpay::WebhookType::InvoiceSettled {
      return Ok(None)
    }

    let maybe_invoice = self.site.invoice().find_optional(&InvoiceQuery{
      external_id_eq: Some(webhook.invoice_id.clone()),
      payment_method_eq: Some(PaymentMethod::BtcPay),
      ..Default::default()
    }).await?;

    if let Some(invoice) = maybe_invoice {
      Ok(Some(invoice.make_payment(None).await?))
    } else {
      Ok(None)
    }
  }

  pub async fn from_invoice(&self, invoice_id: i32) -> Result<Option<Payment>> {
    let maybe_invoice = self.site.invoice().find_optional(&InvoiceQuery{
      id_eq: Some(invoice_id),
      ..Default::default()
    }).await?;

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
      let maybe_student = self.site.student().find_optional(&StudentQuery{
        stripe_customer_id_eq: Some(Some(customer_id.clone())),
        ..Default::default()
      }).await?;

      if let Some(student) = maybe_student {
        let amount = Decimal::new(i.amount_paid.ok_or(Error::validation("amount_paid", "missing"))?, 2);
        let maybe_invoice = self.site.invoice().find_optional(&InvoiceQuery{
          amount_eq: Some(amount),
          student_id_eq: Some(student.attrs.id),
          payment_method_eq: Some(PaymentMethod::Stripe),
          ..Default::default()
        }).await?;

        Ok(Some(self.build(NewPaymentAttrs{
          student_id: student.attrs.id,
          created_at: Utc::now(),
          amount: Decimal::new(i.tax.unwrap_or(0), 2),
          fees: Decimal::ZERO,
          payment_method: PaymentMethod::Stripe,
          clearing_data: serde_json::to_string(&i)?,
          invoice_id: maybe_invoice.map(|i| i.attrs.id),
        }).save().await?))
      } else {
        Ok(None)
      }
    } else {
      Ok(None)
    }
  }
}

