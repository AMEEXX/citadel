use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use crate::persistence::{save_candidate_state, CandidateState};

#[derive(Debug)]
pub enum PersistJob {
    CandidateState(CandidateState),
    Flush(tokio::sync::oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct PersistQueue {
    tx: UnboundedSender<PersistJob>,
    state_dir: PathBuf,
    is_write_behind: bool,
}

impl PersistQueue {
    pub fn new(state_dir: PathBuf) -> Self {
        let is_write_behind = std::env::var("CITADEL_PERSIST_WRITE_BEHIND")
            .map(|v| v != "0")
            .unwrap_or(true);

        let (tx, mut rx) = unbounded_channel::<PersistJob>();

        if is_write_behind {
            let dir = state_dir.clone();
            tokio::spawn(async move {
                let mut dirty: HashMap<String, CandidateState> = HashMap::new();
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));

                loop {
                    tokio::select! {
                        job = rx.recv() => {
                            match job {
                                Some(PersistJob::CandidateState(state)) => {
                                    dirty.insert(state.candidate_id.clone(), state);
                                    if dirty.len() >= 64 {
                                        flush_dirty(&dir, &mut dirty).await;
                                    }
                                }
                                Some(PersistJob::Flush(ack)) => {
                                    flush_dirty(&dir, &mut dirty).await;
                                    let _ = ack.send(());
                                }
                                None => {
                                    flush_dirty(&dir, &mut dirty).await;
                                    break;
                                }
                            }
                        }
                        _ = interval.tick() => {
                            if !dirty.is_empty() {
                                flush_dirty(&dir, &mut dirty).await;
                            }
                        }
                    }
                }
            });
        }

        Self {
            tx,
            state_dir,
            is_write_behind,
        }
    }

    pub fn enqueue(&self, state: CandidateState) {
        if self.is_write_behind {
            let _ = self.tx.send(PersistJob::CandidateState(state));
        } else {
            let _ = save_candidate_state(&self.state_dir, &state);
        }
    }

    pub async fn flush(&self) {
        if self.is_write_behind {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if self.tx.send(PersistJob::Flush(tx)).is_ok() {
                let _ = rx.await;
            }
        }
    }
}

async fn flush_dirty(dir: &PathBuf, dirty: &mut HashMap<String, CandidateState>) {
    if dirty.is_empty() {
        return;
    }
    let to_flush: Vec<CandidateState> = dirty.drain().map(|(_, v)| v).collect();
    let dir_clone = dir.clone();
    tokio::task::spawn_blocking(move || {
        for st in to_flush {
            let _ = save_candidate_state(&dir_clone, &st);
        }
    }).await.ok();
}
