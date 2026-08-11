//! The change feed, carried by a channel instead of a socket.
//!
//! A subscription is numbered because the window has to be able to end one it
//! no longer wants: moving between designs leaves the old feed running, and a
//! process that never stops watching would accumulate one per design visited.

use tauri::{State, ipc::Channel};

use super::{Desktop, Failure};

#[tauri::command]
pub(super) async fn feed_subscribe(
    desktop: State<'_, Desktop>,
    design: String,
    channel: Channel<String>,
) -> Result<u32, Failure> {
    subscribe(desktop.inner(), &design, channel).await
}

#[tauri::command]
pub(super) fn feed_unsubscribe(desktop: State<'_, Desktop>, id: u32) {
    unsubscribe(desktop.inner(), id);
}

async fn subscribe(
    desktop: &Desktop,
    design: &str,
    channel: Channel<String>,
) -> Result<u32, Failure> {
    let mut feed = match desktop.service().feed(design) {
        Ok(feed) => feed,
        Err(refusal) => return Err(Failure::read(refusal).await),
    };

    let watching = tauri::async_runtime::spawn(async move {
        while let Some(message) = feed.next().await {
            // The channel goes with the window that opened it, and there is
            // nobody left to tell once it has.
            if channel.send(message).is_err() {
                return;
            }
        }
    });

    let id = desktop.subscription();
    desktop.feeds().insert(id, watching);
    Ok(id)
}

fn unsubscribe(desktop: &Desktop, id: u32) {
    if let Some(watching) = desktop.feeds().remove(&id) {
        watching.abort();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::Value;
    use tauri::ipc::InvokeResponseBody;

    use crate::desktop::{bridge, tests::workspace};

    use super::*;

    /// Watches a design and throws away what it says.
    pub(crate) async fn watching(desktop: &Desktop, design: &str) {
        subscribe(desktop, design, Channel::new(|_| Ok(())))
            .await
            .expect("subscribes");
    }

    /// Collects what the window would have been sent.
    fn recorder() -> (Channel<String>, Arc<Mutex<Vec<Value>>>) {
        let received = Arc::new(Mutex::new(Vec::new()));
        let recording = Arc::clone(&received);
        let channel = Channel::new(move |body: InvokeResponseBody| {
            let InvokeResponseBody::Json(message) = body else {
                return Ok(());
            };
            // The channel carries the feed's own JSON as a JSON string.
            let text: String = serde_json::from_str(&message).expect("a message");
            recording
                .lock()
                .expect("recording")
                .push(serde_json::from_str(&text).expect("an update"));
            Ok(())
        });
        (channel, received)
    }

    /// Waits for the messages a feed sends of its own accord to arrive.
    async fn settled(received: &Mutex<Vec<Value>>, count: usize) -> Vec<Value> {
        for _ in 0..200 {
            if received.lock().expect("recording").len() >= count {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        received.lock().expect("recording").clone()
    }

    /// A watcher is told what the design says before anything happens to it.
    #[tokio::test]
    async fn opens_with_the_design_and_what_is_being_solved() {
        let (desktop, _root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;

        let (channel, received) = recorder();
        subscribe(&desktop, "checkout", channel)
            .await
            .expect("subscribes");

        let opening = settled(&received, 2).await;
        assert_eq!(opening[0]["type"], "snapshot");
        assert_eq!(opening[1]["type"], "active");
    }

    /// An edit reaches everyone watching, as the edit rather than as a design.
    #[tokio::test]
    async fn carries_the_changes_that_follow() {
        let (desktop, _root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;

        let (channel, received) = recorder();
        subscribe(&desktop, "checkout", channel)
            .await
            .expect("subscribes");
        settled(&received, 2).await;

        bridge::call(
            &desktop,
            "POST",
            "/api/v1/designs/checkout/mutations",
            Some(serde_json::json!({
                "mutations": [{
                    "kind": "set_scratchpad_entry",
                    "entry": { "name": "peak_rate", "expression": "50", "unit": "op/s", "summary": "" },
                }],
            })),
        )
        .await
        .expect("applies");

        let seen = settled(&received, 3).await;
        assert_eq!(seen[2]["type"], "change");
    }

    #[tokio::test]
    async fn refuses_to_watch_a_design_that_is_not_there() {
        let (desktop, _root) = workspace();
        let (channel, _received) = recorder();

        let failure = subscribe(&desktop, "missing", channel)
            .await
            .expect_err("is refused");

        assert_eq!(
            serde_json::to_value(&failure).expect("serialises")["status"],
            404
        );
    }

    /// A subscription nobody ended would run for the rest of the session.
    #[tokio::test]
    async fn stops_a_subscription_it_is_told_to_end() {
        let (desktop, _root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;

        let (channel, _received) = recorder();
        let id = subscribe(&desktop, "checkout", channel)
            .await
            .expect("subscribes");

        assert_eq!(desktop.feeds().len(), 1);
        unsubscribe(&desktop, id);
        assert!(desktop.feeds().is_empty());
    }
}
