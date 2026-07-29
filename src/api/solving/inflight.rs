//! Answering a question once, however many callers asked it.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use crate::system::EvaluationError;

/// What a solve settled on, in a form every waiter can be handed a copy of.
type Answer<V> = Result<Arc<V>, EvaluationError>;

/// The solves being computed right now, keyed by design and question.
pub(in crate::api) struct InFlight<V> {
    running: Arc<Mutex<HashMap<(String, String), broadcast::Sender<Answer<V>>>>>,
}

impl<V> Default for InFlight<V> {
    fn default() -> Self {
        Self {
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<V: Send + Sync + 'static> InFlight<V> {
    /// Waits on the answer to this question, computing it if nobody else is.
    ///
    /// The solve is owned by a task of its own rather than by whichever caller
    /// happened to start it, so a client that navigates away does not abandon
    /// the answer everybody else is waiting on.
    pub(in crate::api) async fn answer<F>(&self, design: &str, question: &str, solve: F) -> Answer<V>
    where
        F: FnOnce() -> Result<V, EvaluationError> + Send + 'static,
    {
        let key = (design.to_owned(), question.to_owned());
        let mut waiting = {
            let mut running = self.running.lock().unwrap_or_else(|held| held.into_inner());
            if let Some(sender) = running.get(&key) {
                sender.subscribe()
            } else {
                let (sender, waiting) = broadcast::channel(1);
                running.insert(key.clone(), sender.clone());
                let finished = Arc::clone(&self.running);
                tokio::spawn(async move {
                    let answer = match tokio::task::spawn_blocking(solve).await {
                        Ok(answer) => answer.map(Arc::new),
                        Err(_) => Err(abandoned()),
                    };
                    finished
                        .lock()
                        .unwrap_or_else(|held| held.into_inner())
                        .remove(&key);
                    let _ = sender.send(answer);
                });
                waiting
            }
        };
        waiting.recv().await.unwrap_or_else(|_| Err(abandoned()))
    }
}

fn abandoned() -> EvaluationError {
    EvaluationError::Evaluation {
        location: "the solver".to_owned(),
        message: "the solve was abandoned before it answered".to_owned(),
    }
}
