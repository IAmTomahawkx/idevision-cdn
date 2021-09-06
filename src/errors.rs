use actix_web::{client::PayloadError, http::StatusCode, HttpResponse};
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum Errors {
    #[display(fmt = "You are not authorized to use this route")]
    UnauthorizedRoute,

    #[display(fmt = "You are not authorized to access {}", img)]
    NoPermission { img: String },

    #[display(fmt = "Anonymous caller does not have access to protected {}", img)]
    AnonymousPermission { img: String },

    #[display(fmt = "{} cannot be found", img)]
    NotFound { img: String },

    #[display(
        fmt = "Misconfiguration of node {} (this is node {})",
        requested_node,
        this_node
    )]
    BadNode {
        requested_node: String,
        this_node: String,
    },

    #[display(fmt = "An internal server error has occurred")]
    InternalServerError,

    #[display(fmt = err)]
    BadRequest { err: String },

    #[display(fmt = "Bad query argument '{}'. {}", query, reason)]
    BadQuery { query: String, reason: String },
}

impl actix_web::error::ResponseError for Errors {
    fn error_response(&self) -> HttpResponse {
        actix_web::dev::HttpResponseBuilder::new(self.status_code())
            .set_header(
                actix_web::http::header::CONTENT_TYPE,
                "text/html; charset=utf-8",
            )
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Errors::AnonymousPermission { .. }
            | Errors::NoPermission { .. }
            | Errors::UnauthorizedRoute { .. } => StatusCode::UNAUTHORIZED,
            Errors::NotFound { .. } => StatusCode::NOT_FOUND,
            Errors::BadNode { .. } | Errors::InternalServerError => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Errors::BadRequest { .. } | Errors::BadQuery { .. } => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<actix_web::error::PayloadError> for Errors {
    fn from(err: PayloadError) -> Self {
        Errors::BadRequest {
            err: err.to_string(),
        }
    }
}

impl From<std::io::Error> for Errors {
    fn from(err: std::io::Error) -> Self {
        Errors::BadRequest {
            err: err.to_string(),
        }
    }
}
