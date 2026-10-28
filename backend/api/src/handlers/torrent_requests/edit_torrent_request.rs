use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_storage::utils::compute_diff;
use arcadia_storage::{
    models::{
        torrent_request::{EditedTorrentRequest, TorrentRequest},
        user::UserPermission,
        user_edit_change_log::NewUserEditChangeLog,
    },
    redis::RedisPoolInterface,
};

use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use arcadia_common::error::{Error, Result};

#[utoipa::path(
    put,
    operation_id = "Edit torrent request",
    tag = "Torrent Request",
    path = "/api/torrent-requests",
    security(
        ("http" = ["Bearer"])
    ),
    responses(
        (status = 200, description = "Successfully edited the torrent request", body=TorrentRequest),
    )
)]

pub async fn exec<R: RedisPoolInterface + 'static>(
    form: Json<EditedTorrentRequest>,
    arc: Data<Arcadia<R>>,
    user: Authdata,
) -> Result<HttpResponse> {
    let torrent_request = arc.pool.find_torrent_request(form.id).await?;

    let is_staff = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::EditTorrentRequest)
        .await?;

    if !is_staff && torrent_request.created_by_id != user.sub {
        return Err(Error::InsufficientPermissions(format!(
            "{:?}",
            UserPermission::EditTorrentRequest
        )));
    }

    // Votes are cast on the title group the request currently targets: letting the author
    // move the request would redirect a bounty they did not fund to another title group.
    if !is_staff && form.title_group_id != torrent_request.title_group_id {
        return Err(Error::InsufficientPermissions(format!(
            "{:?}",
            UserPermission::EditTorrentRequest
        )));
    }

    if let Some(edits) = compute_diff(&torrent_request, &form, &["id", "initial_vote"]) {
        arc.pool
            .create_user_edit_change_log(&NewUserEditChangeLog {
                item_type: "torrent_request".to_string(),
                item_id: torrent_request.id,
                edited_by_id: user.sub,
                edits,
            })
            .await?;
    }

    let updated_torrent_request = arc
        .pool
        .update_torrent_request(&form, torrent_request.id)
        .await?;

    Ok(HttpResponse::Ok().json(updated_torrent_request))
}
