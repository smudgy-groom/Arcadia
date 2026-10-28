use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_common::error::{Error, Result};
use arcadia_storage::{
    models::{title_group::TitleGroup, user::UserPermission},
    redis::RedisPoolInterface,
};
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddTitleGroupToSeriesRequest {
    pub series_id: i64,
    pub title_group_id: i32,
}

#[utoipa::path(
    post,
    operation_id = "Add title group to series",
    tag = "Series",
    path = "/api/series/title-group",
    security(
      ("http" = ["Bearer"])
    ),
    responses(
        (status = 200, description = "Successfully attached the title group to the series", body=TitleGroup),
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    form: Json<AddTitleGroupToSeriesRequest>,
    arc: Data<Arcadia<R>>,
    user: Authdata,
) -> Result<HttpResponse> {
    // Attaching a title group to a series rewrites catalogue structure: the caller must either
    // have created that title group or hold the permission that governs series membership.
    let can_edit_any_series = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::RemoveTitleGroupFromSeries)
        .await?;

    if !can_edit_any_series {
        let title_group = arc.pool.find_title_group(form.title_group_id).await?;
        if title_group.created_by_id != user.sub {
            return Err(Error::InsufficientPermissions(format!(
                "{:?}",
                UserPermission::RemoveTitleGroupFromSeries
            )));
        }
    }

    arc.pool
        .assign_title_group_to_series(form.title_group_id, form.series_id)
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"result": "success"})))
}
