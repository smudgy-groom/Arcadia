use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_common::error::Result;
use arcadia_storage::{models::user::UserPermission, redis::RedisPoolInterface};
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RemoveAffiliatedArtistsForm {
    pub affiliation_ids: Vec<i64>,
}

#[utoipa::path(
    delete,
    operation_id = "Delete artist affiliation",
    tag = "Affiliated Artist",
    path = "/api/affiliated-artists",
    security(
      ("http" = ["Bearer"])
    ),
    responses(
        (status = 200, description = "Successfully removed the artist affiliations"),
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    arc: Data<Arcadia<R>>,
    user: Authdata,
    form: Json<RemoveAffiliatedArtistsForm>,
) -> Result<HttpResponse> {
    // Removing an affiliation edits somebody else's title group: the repository only deletes the
    // affiliations that belong to a title group the caller created, unless they may edit any.
    let can_edit_any_title_group = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::EditTitleGroup)
        .await?;

    arc.pool
        .delete_artists_affiliation(
            &form.affiliation_ids,
            user.sub,
            can_edit_any_title_group,
        )
        .await?;

    Ok(HttpResponse::Ok().json(json!({"result": "success"})))
}
