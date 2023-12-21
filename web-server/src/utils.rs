use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Request},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use protocol::rand::{self, distributions::Alphanumeric, Rng};
use serde_json::json;

pub struct Error {
    pub code: StatusCode,
    pub message: String,
}

pub type ApiResult<T> = Result<T, Error>;

impl Error {
    pub fn new(code: StatusCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl<T: ToString> From<T> for Error {
    fn from(value: T) -> Self {
        eprintln!("Unhandled Error: {}", value.to_string());

        Error {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Internal server error".to_owned(),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (
            self.code,
            Json(json!({ "error": true, "message": self.message })),
        )
            .into_response()
    }
}

pub struct JsonRej<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for JsonRej<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        let req = Request::from_parts(parts, body);

        match Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let payload = json!({
                    "error": true,
                    "message": rejection.body_text(),
                });
                Err((rejection.status(), Json(payload)))
            }
        }
    }
}

pub fn random_str(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
