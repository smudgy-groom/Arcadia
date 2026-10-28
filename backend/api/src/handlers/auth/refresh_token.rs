use crate::{services::auth::generate_login_tokens, Arcadia};
use actix_web::{web, HttpResponse};
use arcadia_common::error::{Error, Result};
use arcadia_storage::{
    models::user::{Claims, LoginResponse, RefreshToken, TokenType},
    redis::RedisPoolInterface,
};
use chrono::prelude::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation};

#[utoipa::path(
    post,
    operation_id = "Refresh token",
    tag = "Auth",
    path = "/api/auth/refresh-token",
    responses(
        (status = 200, description = "Successfully refreshed the token", body=LoginResponse),
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    arc: web::Data<Arcadia<R>>,
    form: web::Json<RefreshToken>,
) -> Result<HttpResponse> {
    let old_refresh_token = decode::<Claims>(
        &form.refresh_token,
        &DecodingKey::from_secret(arc.api.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| Error::InvalidOrExpiredRefreshToken)?;

    // an access token must not be exchangeable for a 90 day credential
    if old_refresh_token.claims.typ != TokenType::Refresh {
        return Err(Error::InvalidOrExpiredRefreshToken);
    }

    let is_invalidated = arc
        .auth
        .is_invalidated(old_refresh_token.claims.sub, old_refresh_token.claims.iat)
        .await?;
    if is_invalidated {
        return Err(Error::InvalidatedToken);
    }

    let user = arc
        .pool
        .find_user_with_id(old_refresh_token.claims.sub)
        .await?;
    if user.banned {
        return Err(Error::AccountBanned);
    }

    // the presented token is single use: it is redeemed before a new pair is issued, so a
    // token that has already been exchanged can never be replayed
    if !arc
        .auth
        .consume_refresh_token(&form.refresh_token)
        .await?
    {
        return Err(Error::InvalidOrExpiredRefreshToken);
    }

    let tokens = generate_login_tokens(
        old_refresh_token.claims.sub,
        &arc.api.jwt_secret,
        true,
        Utc::now(),
    )?;
    arc.auth
        .register_refresh_token(&tokens.refresh_token)
        .await?;

    Ok(HttpResponse::Ok().json(tokens))
}
