#[macro_use]
extern crate rocket;

use tera::Tera;
lazy_static::lazy_static! {
  pub static ref TEMPLATES: Tera = {
    let mut tera = Tera::default();
    tera.add_raw_templates([
      ("emails/welcome", include_str!("templates/emails/welcome.html.tera")),
      ("emails/payment_link", include_str!("templates/emails/payment_link.html.tera"))
    ]).expect("No static");
    tera
  };
}

pub mod models;
pub mod error;
pub mod controllers; 
pub use controllers::*;

