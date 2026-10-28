use crate::{middlewares::auth_middleware::Authdata, Arcadia};
use actix_web::{
    web::{Data, Json},
    HttpResponse,
};
use arcadia_common::error::{Error, Result};
use arcadia_storage::utils::compute_diff;
use arcadia_storage::{
    models::{
        forum::{EditedForumThread, ForumThreadEnriched},
        user::UserPermission,
        user_edit_change_log::NewUserEditChangeLog,
    },
    redis::RedisPoolInterface,
};

#[utoipa::path(
    put,
    operation_id = "Edit forum thread",
    tag = "Forum",
    path = "/api/forum/thread",
    responses(
        (status = 200, description = "Edits the thread's information", body=ForumThreadEnriched)
    )
)]
pub async fn exec<R: RedisPoolInterface + 'static>(
    arc: Data<Arcadia<R>>,
    user: Authdata,
    edited_forum_thread: Json<EditedForumThread>,
) -> Result<HttpResponse> {
    let original_thread = arc
        .pool
        .find_forum_thread(edited_forum_thread.id, user.sub)
        .await?;

    let is_staff = arc
        .pool
        .user_has_permission(user.sub, &UserPermission::EditForumThread)
        .await?;

    if !is_staff && original_thread.created_by_id != user.sub {
        return Err(Error::InsufficientPermissions(format!(
            "{:?}",
            UserPermission::EditForumThread
        )));
    }

    // Relocating a thread and editing a locked thread are moderation actions. Moving a thread
    // must also not become a way around the destination sub-category's `new_threads_restricted`
    // allow-list, which is enforced on creation but would otherwise be skipped here.
    if !is_staff
        && (edited_forum_thread.forum_sub_category_id != original_thread.forum_sub_category_id
            || original_thread.locked)
    {
        return Err(Error::InsufficientPermissions(format!(
            "{:?}",
            UserPermission::EditForumThread
        )));
    }

    if let Some(edits) = compute_diff(&original_thread, &edited_forum_thread, &["id"]) {
        arc.pool
            .create_user_edit_change_log(&NewUserEditChangeLog {
                item_type: "forum_thread".to_string(),
                item_id: original_thread.id,
                edited_by_id: user.sub,
                edits,
            })
            .await?;
    }

    let updated_thread = arc
        .pool
        .update_forum_thread(&edited_forum_thread, user.sub)
        .await?;

    Ok(HttpResponse::Ok().json(updated_thread))
}
