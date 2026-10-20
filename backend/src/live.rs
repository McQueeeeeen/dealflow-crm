use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use sqlx::{PgConnection, PgPool};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::ApiJson;
use crate::state::AppState;

pub const CHANNEL: &str = "dealflow_changes";
const STREAM_LIFETIME: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub org: Uuid,
    pub entity: String,
    pub id: Uuid,
    pub action: String,
    pub actor: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Viewer {
    pub user_id: Uuid,
    pub name: String,
    pub view: Option<String>,
}

#[derive(Debug, Clone)]
enum Signal {
    Change(Change),
    Presence(Uuid),
}

pub struct Hub {
    signals: broadcast::Sender<Signal>,
    viewers: Mutex<HashMap<Uuid, (Uuid, Viewer)>>,
    listening: watch::Receiver<bool>,
}

impl Hub {
    pub fn start(pool: PgPool) -> Arc<Self> {
        let (signals, _) = broadcast::channel(256);
        let (ready, listening) = watch::channel(false);
        tokio::spawn(listen(pool, signals.clone(), ready));
        Arc::new(Self {
            signals,
            viewers: Mutex::default(),
            listening,
        })
    }

    pub async fn listening(&self) {
        let _ = self.listening.clone().wait_for(|ready| *ready).await;
    }

    pub fn viewers(&self, org: Uuid) -> Vec<Viewer> {
        let viewers = self.viewers.lock().expect("presence lock");
        viewers
            .values()
            .filter(|(o, _)| *o == org)
            .map(|(_, viewer)| viewer.clone())
            .collect()
    }

    fn update(&self, org: Uuid, edit: impl FnOnce(&mut HashMap<Uuid, (Uuid, Viewer)>) -> bool) {
        let changed = edit(&mut self.viewers.lock().expect("presence lock"));
        if changed {
            let _ = self.signals.send(Signal::Presence(org));
        }
    }
}

async fn listen(pool: PgPool, signals: broadcast::Sender<Signal>, ready: watch::Sender<bool>) {
    loop {
        match PgListener::connect_with(&pool).await {
            Ok(mut listener) => {
                match listener.listen(CHANNEL).await {
                    Ok(()) => {
                        let _ = ready.send(true);
                    }
                    Err(err) => tracing::warn!(error = %err, "could not listen for changes"),
                }
                while let Ok(notification) = listener.recv().await {
                    match serde_json::from_str(notification.payload()) {
                        Ok(change) => {
                            let _ = signals.send(Signal::Change(change));
                        }
                        Err(err) => tracing::warn!(error = %err, "malformed change payload"),
                    }
                }
            }
            Err(err) => tracing::warn!(error = %err, "change listener could not connect"),
        }
        if pool.is_closed() {
            return;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub async fn notify(conn: &mut PgConnection, change: &Change) -> AppResult<()> {
    let payload = serde_json::to_string(change).map_err(anyhow::Error::from)?;
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(payload)
        .execute(conn)
        .await?;
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/live", get(stream))
        .route("/live/presence", put(set_view))
}

async fn stream(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Sse<ReceiverStream<Result<Event, Infallible>>>> {
    let name: String = sqlx::query_scalar("SELECT full_name FROM users WHERE id = $1")
        .bind(user.user_id)
        .fetch_one(&state.pool)
        .await?;
    let hub = state.hub.clone();
    let conn = Uuid::now_v7();
    let mut signals = hub.signals.subscribe();
    let (tx, rx) = mpsc::channel(32);
    let viewer = Viewer {
        user_id: user.user_id,
        name,
        view: None,
    };
    hub.update(user.org_id, |map| map.insert(conn, (user.org_id, viewer)).is_none());

    tokio::spawn(async move {
        let mut open = tx
            .send(Ok(Event::default().event("hello").data(conn.to_string())))
            .await
            .is_ok();
        let deadline = tokio::time::sleep(STREAM_LIFETIME);
        tokio::pin!(deadline);
        while open {
            let event = tokio::select! {
                () = &mut deadline => break,
                () = tx.closed() => break,
                signal = signals.recv() => match signal {
                    Ok(Signal::Change(change)) if change.org == user.org_id => {
                        Event::default().event("change").json_data(&change)
                    }
                    Ok(Signal::Presence(org)) if org == user.org_id => {
                        Event::default().event("presence").json_data(hub.viewers(org))
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        Ok(Event::default().event("resync").data("lagged"))
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            if let Ok(event) = event {
                open = tx.send(Ok(event)).await.is_ok();
            }
        }
        hub.update(user.org_id, |map| map.remove(&conn).is_some());
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

#[derive(Deserialize)]
struct ViewUpdate {
    connection: Uuid,
    view: Option<String>,
}

async fn set_view(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<ViewUpdate>,
) -> AppResult<StatusCode> {
    if req.view.as_ref().is_some_and(|v| v.chars().count() > 80) {
        return Err(AppError::validation("view", "must be at most 80 characters"));
    }
    state.hub.update(user.org_id, |map| match map.get_mut(&req.connection) {
        Some((_, viewer)) if viewer.user_id == user.user_id && viewer.view != req.view => {
            viewer.view = req.view;
            true
        }
        _ => false,
    });
    Ok(StatusCode::NO_CONTENT)
}
