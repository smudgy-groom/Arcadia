use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_common::error::{Error, Result};
use arcadia_storage::{
    models::{
        collage::{CollageEntry, UserCreatedCollageEntry},
        user::UserPermission,
    },
    redis::RedisPoolInterface,
};

#[utoipa::path(
    post,
    operation_id = "Inserts entries into a collage",
    tag = "Collages",
    path = "/api/collages/entries",
    security(
      ("http" = ["Bearer"])
    ),
    responses(
        (status = 200, description = "Successfully created the collage entries", body=Vec<CollageEntry>),
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    collage_entries: Json<Vec<UserCreatedCollageEntry>>,
    arc: Data<Arcadia<R>>,
    user: Authdata,
) -> Result<HttpResponse> {
    // Adding a title group to a collage is an edit of that collage: only its owner, or a user
    // allowed to edit any collage, may add entries to it.
    let can_edit_any_collage = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::EditCollage)
        .await?;

    for entry in collage_entries.iter() {
        let collage = arc.pool.find_collage(entry.collage_id).await?;
        if !can_edit_any_collage && collage.created_by_id != user.sub {
            return Err(Error::InsufficientPermissions(format!(
                "{:?}",
                UserPermission::EditCollage
            )));
        }
    }

    // TODO: instead, we should return Vec<CollageAndAssociatedData> so the frontend can hydrate the page
    // without having to refetch everything
    let collage_entries = arc
        .pool
        .create_collage_entries(&collage_entries, user.sub, &arc.notification_sender)
        .await?;

    Ok(HttpResponse::Created().json(collage_entries))
}
