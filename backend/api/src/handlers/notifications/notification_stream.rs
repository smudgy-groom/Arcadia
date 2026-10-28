use crate::{middlewares::auth_middleware, Arcadia};
use actix_web::web::{Data, Query};
use actix_web_lab::sse;
use arcadia_storage::redis::RedisPoolInterface;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;

/// How many notification streams a single user may hold open at once. A stream keeps its socket
/// for as long as the client leaves it open, so without a limit one account can exhaust the
/// process' file descriptors and the server stops accepting connections altogether.
const MAX_NOTIFICATION_STREAMS_PER_USER: usize = 5;

/// How many notification streams the whole instance may hold open at once, so that many accounts
/// together cannot consume every file descriptor either.
const MAX_OPEN_NOTIFICATION_STREAMS: usize = 512;

/// The streams currently open, in total and per user.
#[derive(Default)]
struct OpenStreams {
    total: usize,
    per_user: HashMap<i32, usize>,
}

static OPEN_NOTIFICATION_STREAMS: LazyLock<Mutex<OpenStreams>> =
    LazyLock::new(|| Mutex::new(OpenStreams::default()));

/// Gives back the stream slot when the stream ends, however it ends.
struct OpenNotificationStream(i32);

impl Drop for OpenNotificationStream {
    fn drop(&mut self) {
        let mut streams = OPEN_NOTIFICATION_STREAMS.lock().unwrap();
        streams.total -= 1;
        if let Some(user_streams) = streams.per_user.get_mut(&self.0) {
            *user_streams -= 1;
            if *user_streams == 0 {
                streams.per_user.remove(&self.0);
            }
        }
    }
}

/// Claims a stream slot for the user, or returns `None` when they already hold the maximum or the
/// instance is at its own limit.
fn open_notification_stream(user_id: i32) -> Option<OpenNotificationStream> {
    let mut streams = OPEN_NOTIFICATION_STREAMS.lock().unwrap();
    if streams.total >= MAX_OPEN_NOTIFICATION_STREAMS {
        return None;
    }
    let user_streams = streams.per_user.entry(user_id).or_insert(0);
    if *user_streams >= MAX_NOTIFICATION_STREAMS_PER_USER {
        return None;
    }
    *user_streams += 1;
    streams.total += 1;
    Some(OpenNotificationStream(user_id))
}

#[derive(Debug, Deserialize)]
pub struct NotificationStreamQuery {
    pub token: String,
}

pub async fn exec<R: RedisPoolInterface + 'static>(
    query: Query<NotificationStreamQuery>,
    arc: Data<Arcadia<R>>,
) -> Result<
    sse::Sse<impl futures::Stream<Item = Result<sse::Event, std::convert::Infallible>>>,
    actix_web::Error,
> {
    let user_id = auth_middleware::validate_token::<R>(&query.token, &arc).await?;

    let Some(stream_slot) = open_notification_stream(user_id) else {
        return Err(actix_web::error::ErrorTooManyRequests(
            "too many open notification streams",
        ));
    };

    let mut receiver = arc.notification_sender.subscribe();

    let stream = async_stream::stream! {
        let _stream_slot = stream_slot;
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if event.user_ids().contains(&user_id) {
                        yield Ok(sse::Event::Data(sse::Data::new(event.event_type())));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    yield Ok(sse::Event::Data(sse::Data::new("refresh")));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(sse::Sse::from_stream(stream).with_keep_alive(Duration::from_secs(30)))
}
