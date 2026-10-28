use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_common::error::{Error, Result};
use arcadia_storage::{
    models::{
        artist::{AffiliatedArtistHierarchy, UserCreatedAffiliatedArtist},
        user::UserPermission,
    },
    redis::RedisPoolInterface,
};

#[utoipa::path(
    post,
    operation_id = "Create artist affiliation",
    tag = "Affiliated Artist",
    path = "/api/affiliated-artists",
    security(
      ("http" = ["Bearer"])
    ),
    responses(
        (status = 200, description = "Successfully created the artist affiliations", body=Vec<AffiliatedArtistHierarchy>),
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    artists: Json<Vec<UserCreatedAffiliatedArtist>>,
    arc: Data<Arcadia<R>>,
    user: Authdata,
) -> Result<HttpResponse> {
    // Adding an affiliation edits somebody else's title group: require ownership of every title
    // group referenced, or the permission that allows editing any title group.
    let can_edit_any_title_group = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::EditTitleGroup)
        .await?;

    if !can_edit_any_title_group {
        for affiliation in artists.iter() {
            let title_group = arc.pool.find_title_group(affiliation.title_group_id).await?;
            if title_group.created_by_id != user.sub {
                return Err(Error::InsufficientPermissions(format!(
                    "{:?}",
                    UserPermission::EditTitleGroup
                )));
            }
        }
    }

    let affiliations = arc
        .pool
        .create_artists_affiliation(&artists, user.sub, &arc.notification_sender)
        .await?;

    Ok(HttpResponse::Created().json(affiliations))
}
