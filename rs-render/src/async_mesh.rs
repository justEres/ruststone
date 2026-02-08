use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::chunk::{ChunkColumnSnapshot, MeshData};

#[derive(Resource)]
pub struct MeshAsyncResources {
    pub runtime: Arc<Runtime>,
    pub job_tx: UnboundedSender<MeshJob>,
    pub result_rx: Mutex<UnboundedReceiver<MeshResult>>,
}

impl FromWorld for MeshAsyncResources {
    fn from_world(_world: &mut World) -> Self {
        let runtime = Arc::new(Runtime::new().expect("Failed to create tokio runtime"));
        let (job_tx, mut job_rx) = unbounded_channel::<MeshJob>();
        let (result_tx, result_rx) = unbounded_channel::<MeshResult>();
        let runtime_clone = runtime.clone();

        runtime.spawn(async move {
            while let Some(job) = job_rx.recv().await {
                let result_tx = result_tx.clone();
                let runtime_clone = runtime_clone.clone();
                runtime_clone.spawn_blocking(move || {
                    let chunk_key = job.chunk_key;
                    let mesh = job.build_mesh();
                    let _ = result_tx.send(MeshResult { chunk_key, mesh });
                });
            }
        });

        Self {
            runtime,
            job_tx,
            result_rx: Mutex::new(result_rx),
        }
    }
}

#[derive(Resource, Default)]
pub struct MeshInFlight {
    pub chunks: HashSet<(i32, i32)>,
}

pub struct MeshJob {
    pub chunk_key: (i32, i32),
    pub snapshot: ChunkColumnSnapshot,
}

impl MeshJob {
    pub fn build_mesh(self) -> MeshData {
        self.snapshot.build_mesh_data()
    }
}

pub struct MeshResult {
    pub chunk_key: (i32, i32),
    pub mesh: MeshData,
}
