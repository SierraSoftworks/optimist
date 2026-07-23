use crate::{
    command::{ChangeSet, ChangeStreamMessage},
    domain::ProjectId,
};
use axum::{
    Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
    routing::get,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/projects/{project}/changes/ws", get(connect))
}

#[derive(serde::Deserialize)]
struct StreamQuery {
    #[serde(default)]
    after: u64,
}

async fn connect(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Query(query): Query<StreamQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let receiver = state.subscribe(&project).await;
    let replay = state
        .catalog
        .write()
        .await
        .replay_changes_with_snapshot(&project, query.after)?;
    Ok(upgrade.on_upgrade(move |socket| stream(socket, receiver, replay)))
}

async fn stream(
    mut socket: WebSocket,
    mut receiver: tokio::sync::broadcast::Receiver<ChangeSet>,
    replay: crate::command::ChangeSetReplay,
) {
    let mut delivered = replay.after_revision;
    if let Some(snapshot) = replay.snapshot {
        delivered = snapshot.revision;
        if send(
            &mut socket,
            ChangeStreamMessage::Snapshot(Box::new(snapshot)),
        )
        .await
        .is_err()
        {
            return;
        }
    }
    for change in replay.changes {
        delivered = change.project_revision;
        if send(&mut socket, ChangeStreamMessage::Change(Box::new(change)))
            .await
            .is_err()
        {
            return;
        }
    }
    delivered = delivered.max(replay.current_revision);
    if send(
        &mut socket,
        ChangeStreamMessage::CaughtUp {
            revision: replay.current_revision,
        },
    )
    .await
    .is_err()
    {
        return;
    }
    loop {
        match receiver.recv().await {
            Ok(change) if change.project_revision > delivered => {
                delivered = change.project_revision;
                if send(&mut socket, ChangeStreamMessage::Change(Box::new(change)))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let _ = send(
                    &mut socket,
                    ChangeStreamMessage::ReplayRequired {
                        after_revision: delivered,
                    },
                )
                .await;
                return;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn send(socket: &mut WebSocket, message: ChangeStreamMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(&message).expect("ChangeSet stream messages serialize");
    socket.send(Message::Text(text.into())).await
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use tokio::{net::TcpListener, task::JoinHandle};

    use crate::{
        command::{ChangeStreamMessage, CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload},
        project::{CreateProject, Project},
        server,
    };

    async fn server() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        (format!("http://{address}"), task)
    }

    fn node(name: &str) -> GraphCommand {
        GraphCommand::CreateNode(CreateNode {
            name: name.to_owned(),
            title: name.to_owned(),
            payload: NodePayload::Factor(Factor {
                controllable: true,
                evidence: vec![],
            }),
        })
    }

    async fn commit(client: &reqwest::Client, base: &str, request: &CommandRequest) {
        let response = client
            .post(format!("{base}/api/v1/projects/A/commands"))
            .json(request)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    async fn message<S>(socket: &mut S) -> ChangeStreamMessage
    where
        S: futures_util::Stream<
                Item = Result<
                    tokio_tungstenite::tungstenite::Message,
                    tokio_tungstenite::tungstenite::Error,
                >,
            > + Unpin,
    {
        let value = socket.next().await.unwrap().unwrap().into_text().unwrap();
        serde_json::from_str(&value).unwrap()
    }

    #[tokio::test]
    async fn replays_then_streams_ordered_project_changes() {
        let (base, server) = server().await;
        let client = reqwest::Client::new();
        let project: Project = client
            .post(format!("{base}/api/v1/projects"))
            .json(&CreateProject {
                name: "Delivery".to_owned(),
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(project.id.as_str(), "A");
        commit(&client, &base, &CommandRequest::new(0, node("first"))).await;

        let websocket = base.replacen("http://", "ws://", 1);
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "{websocket}/api/v1/projects/A/changes/ws?after=0"
        ))
        .await
        .unwrap();
        assert!(matches!(
            message(&mut socket).await,
            ChangeStreamMessage::Change(change) if change.project_revision == 1
        ));
        assert_eq!(
            message(&mut socket).await,
            ChangeStreamMessage::CaughtUp { revision: 1 }
        );

        commit(&client, &base, &CommandRequest::new(1, node("second"))).await;
        assert!(matches!(
            message(&mut socket).await,
            ChangeStreamMessage::Change(change) if change.project_revision == 2
        ));
        server.abort();
    }

    #[tokio::test]
    async fn broadcasts_once_to_two_subscribers_and_suppresses_retries() {
        let (base, server) = server().await;
        let client = reqwest::Client::new();
        client
            .post(format!("{base}/api/v1/projects"))
            .json(&CreateProject {
                name: "Delivery".to_owned(),
            })
            .send()
            .await
            .unwrap();
        commit(&client, &base, &CommandRequest::new(0, node("first"))).await;
        let websocket = base.replacen("http://", "ws://", 1);
        let url = format!("{websocket}/api/v1/projects/A/changes/ws?after=1");
        let (mut first, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut second, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        assert_eq!(
            message(&mut first).await,
            ChangeStreamMessage::CaughtUp { revision: 1 }
        );
        assert_eq!(
            message(&mut second).await,
            ChangeStreamMessage::CaughtUp { revision: 1 }
        );

        let request = CommandRequest::new(1, node("second"));
        commit(&client, &base, &request).await;
        commit(&client, &base, &request).await;
        for socket in [&mut first, &mut second] {
            assert!(matches!(
                message(socket).await,
                ChangeStreamMessage::Change(change) if change.project_revision == 2
            ));
        }
        let replay: crate::command::ChangeSetReplay = client
            .get(format!("{base}/api/v1/projects/A/changes?after=1"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(replay.current_revision, 2);
        assert_eq!(replay.changes.len(), 1);
        server.abort();
    }

    #[tokio::test]
    async fn sends_snapshot_before_live_changes_when_history_has_a_gap() {
        let (base, server) = server().await;
        let client = reqwest::Client::new();
        let project: Project = client
            .post(format!("{base}/api/v1/projects"))
            .json(&CreateProject {
                name: "Delivery".to_owned(),
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        commit(&client, &base, &CommandRequest::new(0, node("first"))).await;
        let archive: crate::project::ProjectArchive = client
            .get(format!("{base}/api/v1/projects/A/archive"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        client
            .post(format!(
                "{base}/api/v1/project-archives?replace=true&yes=true"
            ))
            .json(&archive)
            .send()
            .await
            .unwrap();

        let websocket = base.replacen("http://", "ws://", 1);
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "{websocket}/api/v1/projects/A/changes/ws?after=0"
        ))
        .await
        .unwrap();
        assert!(matches!(
            message(&mut socket).await,
            ChangeStreamMessage::Snapshot(snapshot)
                if snapshot.revision == 1 && snapshot.archive.project.id == project.id
        ));
        assert_eq!(
            message(&mut socket).await,
            ChangeStreamMessage::CaughtUp { revision: 1 }
        );
        commit(&client, &base, &CommandRequest::new(1, node("second"))).await;
        assert!(matches!(
            message(&mut socket).await,
            ChangeStreamMessage::Change(change) if change.project_revision == 2
        ));
        server.abort();
    }
}
