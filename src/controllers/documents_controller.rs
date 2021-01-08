/*
 * use crate::models::document::Document;

#[get("/")]
pub async fn index(pool: Pool) -> Json<Vec<Document>> {
  Json(pool.run(|c| Document::all(c)).await)
}

#[post("/", data = "<document>")]
pub async fn create(document: Json<Document>, pool: Pool) -> Json<Document> {
  let new = Document {
    id: None,
    ..document.into_inner()
  };
  Json(pool.run(|c| Document::insert(new, c)).await)
}

#[get("/<id>")]
pub async fn show(id: i32, pool: Pool) -> Json<Document> {
  Json(pool.run(move |c| Document::find(id, c)).await)
}
*/
