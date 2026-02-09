use std::collections::HashSet;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::chunk::{ChunkColumnSnapshot, MeshBatch};

#[derive(Resource)]
pub struct MeshAsyncResources {
    pub runtime: ManuallyDrop<Arc<Runtime>>,
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
                    let start = std::time::Instant::now();
                    let chunk_key = job.chunk_key;
                    let mesh = job.build_mesh();
                    let build_ms = start.elapsed().as_secs_f32() * 1000.0;
                    let _ = result_tx.send(MeshResult {
                        chunk_key,
                        mesh,
                        build_ms,
                    });
                });
            }
        });

        Self {
            runtime: ManuallyDrop::new(runtime),
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
    pub use_greedy: bool,
}

impl MeshJob {
    pub fn build_mesh(self) -> MeshBatch {
        self.snapshot.build_mesh_data(self.use_greedy)
    }
}

pub struct MeshResult {
    pub chunk_key: (i32, i32),
    pub mesh: MeshBatch,
    pub build_ms: f32,
}
