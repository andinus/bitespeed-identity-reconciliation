use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::{Row, SqlitePool};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyBody {
    email: Option<String>,
    phone_number: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactResponse {
    // primaryContatctId -- typo in task documentation.
    primary_contact_id: i32,
    emails: Vec<String>,
    phone_numbers: Vec<String>,
    secondary_contact_ids: Vec<i32>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactWrapper {
    contact: ContactResponse,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct Contact {
    id: i32,
    linked_id: Option<i32>,
    email: Option<String>,
    phone_number: Option<String>,
    link_precedence: String,
}

/// identify handler takes an email, phone_number and returns the required body.
/// it might create a secondary contact or might turn a primary contact into a
/// secondary contact if required.
pub async fn handler_identify(
    State(pool): State<SqlitePool>,
    Json(payload): Json<IdentifyBody>,
) -> impl IntoResponse {
    let email = payload.email;
    let phone_number = payload.phone_number;

    // one of email, phone_number should be specified.
    if email.is_none() && phone_number.is_none() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // check if any contact with this email, phone_number combination exists.
    let result = sqlx::query_as::<_, Contact>(
        "SELECT id, email, phone_number, link_precedence, linked_id
         FROM contacts
         WHERE email = ? OR phone_number = ?
         ORDER BY created_at",
    )
    .bind(&email)
    .bind(&phone_number)
    .fetch_all(&pool)
    .await
    .unwrap();

    // if no such contact exists then we create one.
    let primary_contact_id = if result.is_empty() {
        sqlx::query(
            "INSERT INTO contacts (email, phone_number, link_precedence)
               VALUES (?, ?, ?)
               RETURNING id",
        )
        .bind(&email)
        .bind(&phone_number)
        .bind("primary")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get::<i32, &str>("id")
    } else {
        // if a contact already exists then we need to merge the current
        // details, we might have to update multiple rows for this step.
        let mut tx = pool.begin().await.unwrap();

        let primary_contact = result
            .iter()
            .filter(|r| &r.link_precedence == "primary")
            .collect::<Vec<&Contact>>();

        // at least one existing row is going to be "primary". identify the
        // primary_contact_id.
        let primary_contact_id = if primary_contact.is_empty() {
            result
                .iter()
                .filter(|r| &r.link_precedence == "secondary")
                .collect::<Vec<&Contact>>()[0]
                .linked_id
                .unwrap()
        } else {
            primary_contact[0].id
        };

        // if we have multiple "primary" rows then all except one needs to
        // become secondary. the oldest contact will remain "primary".
        for contact in primary_contact.iter().skip(1) {
            sqlx::query(
                "UPDATE contacts SET link_precedence = 'secondary', linked_id = ?
                 WHERE id = ?",
            )
            .bind(primary_contact_id)
            .bind(contact.id)
            .execute(&mut *tx)
            .await
            .unwrap();
        }

        // a secondary contact has to be created if either email or phone_number
        // is not present already.
        let email_absent = result.iter().filter(|r| r.email == email).count() == 0;
        let phone_number_absent = result
            .iter()
            .filter(|r| r.phone_number == phone_number)
            .count()
            == 0;

        if (email.is_some() && email_absent) || (phone_number.is_some() && phone_number_absent) {
            sqlx::query(
                "INSERT INTO contacts (email, phone_number, link_precedence, linked_id)
                   VALUES (?, ?, ?, ?)",
            )
            .bind(&email)
            .bind(&phone_number)
            .bind("secondary")
            .bind(primary_contact_id)
            .execute(&mut *tx)
            .await
            .unwrap();
        }

        tx.commit().await.unwrap();

        primary_contact_id
    };

    // get all details for the requested `primary_contact_id'.
    let contact = get_contacts_for_id(primary_contact_id, &pool).await;

    Json(ContactWrapper { contact }).into_response()
}

async fn get_contacts_for_id(primary_contact_id: i32, pool: &SqlitePool) -> ContactResponse {
    let emails = sqlx::query(
        "SELECT DISTINCT email
         FROM contacts
         WHERE (id = $1 OR linked_id = $1) AND email IS NOT NULL",
    )
    .bind(primary_contact_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, &str>("email"))
    .collect::<Vec<String>>();

    let phone_numbers = sqlx::query(
        "SELECT DISTINCT phone_number
         FROM contacts
         WHERE (id = $1 OR linked_id = $1) AND phone_number IS NOT NULL",
    )
    .bind(primary_contact_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, &str>("phone_number"))
    .collect::<Vec<String>>();

    let secondary_contact_ids = sqlx::query(
        "SELECT DISTINCT id
         FROM contacts
         WHERE linked_id = $1",
    )
    .bind(primary_contact_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<i32, &str>("id"))
    .collect::<Vec<i32>>();

    ContactResponse {
        primary_contact_id,
        emails,
        phone_numbers,
        secondary_contact_ids,
    }
}
