use super::*;

make_sqlx_model!{
  state: Site,
  table: degrees,
  struct Degree {
    #[sqlx_search_as(int4)]
    id: i32,
    #[sqlx_search_as(int4)]
    subscription_id: i32,
    #[sqlx_search_as(int4)]
    student_id: i32,
    created_at: UtcDateTime,
    description: String,
    poap_link: Option<String>,
    constata_certificate_id: Option<String>,
    price: Decimal,
    #[sqlx_search_as(bool)]
    paid: bool,
    paid_at: Option<UtcDateTime>,
  }
}
