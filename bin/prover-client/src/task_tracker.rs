use alpen_express_db::types::WitnessType;
use anyhow::Ok;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::primitives::{
    prover_input::{ProverInput, WitnessData},
    tasks_scheduler::{DependencyStatus, ProvingTask, TaskStatus},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct CheckpointInfo {
    pub(crate) cp_height: u64,
    pub(crate) l2_final_height: u64,
    pub(crate) l1_final_height: u64,
}

pub(crate) struct TaskTracker {
    tasks: Mutex<Vec<ProvingTask>>,
    running_checkpoint: CheckpointInfo,
}

impl TaskTracker {
    pub fn new() -> Self {
        TaskTracker {
            tasks: Mutex::new(Vec::new()),
            running_checkpoint: CheckpointInfo::default(),
        }
    }

    pub async fn order_tasks_by_priority(&self, tasks: &mut [ProvingTask], task_pos: usize) {
        // if the table is already sorted,
        // then we only need to reorg based on the info from new task
        // let mut _tasks = self.tasks.lock().await;
        // if new_task.checkpoint_index == self.running_checkpoint.cp_height {
        //     todo!()
        // }
        let new_task = &mut tasks[task_pos];
        // if it is a newly created task, check if it has blockers
        if new_task.progress_status == TaskStatus::Created
            && new_task.witness_type == WitnessType::EL
        {
            new_task.dependency_status = DependencyStatus::Open;
        }
    }

    // when checkpoint rpc is called
    // add these witness
    pub async fn create_tasks_using_checkpoint_info(&self, cp_info: CheckpointInfo) {
        let mut tasks = self.tasks.lock().await;
        for block_num in self.running_checkpoint.l1_final_height..cp_info.l1_final_height {
            let task_id = Uuid::new_v4();
            let task = ProvingTask {
                id: task_id,
                checkpoint_index: cp_info.cp_height,
                block_height: block_num,
                witness_type: WitnessType::BlkSpace,
                prover_input: ProverInput::BlkSpace(WitnessData { data: vec![] }),
                progress_status: TaskStatus::Created,
                dependency_status: DependencyStatus::Blocked,
                retry_count: 0,
            };
            tasks.push(task);
        }
        for block_num in self.running_checkpoint.l2_final_height..cp_info.l2_final_height {
            let task_id = Uuid::new_v4();
            let task = ProvingTask {
                id: task_id,
                checkpoint_index: cp_info.cp_height,
                block_height: block_num,
                witness_type: WitnessType::EL,
                prover_input: ProverInput::ElBlock(WitnessData { data: vec![] }),
                progress_status: TaskStatus::Created,
                dependency_status: DependencyStatus::Blocked,
                retry_count: 0,
            };
            tasks.push(task);
            let task_id = Uuid::new_v4();
            let task = ProvingTask {
                id: task_id,
                checkpoint_index: cp_info.cp_height,
                block_height: block_num,
                witness_type: WitnessType::CL,
                prover_input: ProverInput::ClBlock(WitnessData { data: vec![] }),
                progress_status: TaskStatus::Created,
                dependency_status: DependencyStatus::Blocked,
                retry_count: 0,
            };
            tasks.push(task);
        }
        let task_id = Uuid::new_v4();
        let task = ProvingTask {
            id: task_id,
            checkpoint_index: cp_info.cp_height,
            block_height: 0,
            witness_type: WitnessType::CLAgg,
            prover_input: ProverInput::ClAgg,
            progress_status: TaskStatus::Created,
            dependency_status: DependencyStatus::Blocked,
            retry_count: 0,
        };
        tasks.push(task);
        let task_id = Uuid::new_v4();
        let task = ProvingTask {
            id: task_id,
            checkpoint_index: cp_info.cp_height,
            block_height: 0,
            witness_type: WitnessType::BlkSpaceAgg,
            prover_input: ProverInput::BlkSpaceAgg,
            progress_status: TaskStatus::Created,
            dependency_status: DependencyStatus::Blocked,
            retry_count: 0,
        };
        tasks.push(task);
        let task_id = Uuid::new_v4();
        let task = ProvingTask {
            id: task_id,
            checkpoint_index: cp_info.cp_height,
            block_height: 0,
            witness_type: WitnessType::Checkpoint,
            prover_input: ProverInput::Checkpoint,
            progress_status: TaskStatus::Created,
            dependency_status: DependencyStatus::Blocked,
            retry_count: 0,
        };
        tasks.push(task);
    }

    // add witness to tasks which are blocked from executing otherwise
    // some tasks (CL) also require corresponding proofs (EL) to be unblocked
    // Hence the primary task is to add witness and check if the task is unblocked from executing
    pub async fn include_task_witness(
        &self,
        cp_index: u64,
        block_num: u64,
        witness: ProverInput,
        witness_type: WitnessType,
    ) -> Result<Uuid, anyhow::Error> {
        if witness_type != WitnessType::EL
            && witness_type != WitnessType::CL
            && witness_type != WitnessType::BlkSpace
        {
            return Err(anyhow::anyhow!(
                "Unexpected Witness Type {:?}",
                witness_type
            ));
        }
        let mut tasks = self.tasks.lock().await;
        let matched_doc = tasks.iter_mut().find(|t| {
            t.checkpoint_index == cp_index
                && t.block_height == block_num
                && t.witness_type == witness_type
        });
        match matched_doc {
            Some(existing_task) => {
                if existing_task.dependency_status != DependencyStatus::Blocked {
                    return Err(anyhow::anyhow!("Expected blocked dependency"));
                }
                existing_task.prover_input = witness;
                if witness_type == WitnessType::EL || witness_type == WitnessType::BlkSpace {
                    existing_task.dependency_status = DependencyStatus::Open;
                } else if witness_type == WitnessType::CL {
                    let tasks = self.tasks.lock().await; // ISSUE:  concurrency issue here
                    let matched_el_doc_for_cl = tasks.iter().find(|t| {
                        t.checkpoint_index == cp_index
                            && t.block_height == block_num
                            && t.witness_type == WitnessType::EL
                    });
                    if let Some(el_doc_for_cl) = matched_el_doc_for_cl {
                        if el_doc_for_cl.progress_status == TaskStatus::ProvingSuccessful {
                            existing_task.dependency_status = DependencyStatus::Open;
                        }
                    };
                }
                Ok(existing_task.id)
            }
            None => Err(anyhow::anyhow!(
                "no matching doc exist {:?} {:?} {:?}",
                cp_index,
                witness_type,
                block_num,
            )),
        }
        // {
        //     task.progress_status = status;
        //     if status == TaskStatus::WitnessSubmitted {
        //         task.prover_input.make_empty(); // free because it has now been saved in db
        //     }
        //     self.order_tasks_by_priority(&mut tasks, idx).await;
        // }

        // let task_id = Uuid::new_v4();
        // let task = ProvingTask {
        //     id: task_id,
        //     checkpoint_index: cp_index,
        //     block_height: block_num,
        //     witness_type,
        //     prover_input: witness,
        //     progress_status: TaskStatus::Created,
        //     dependency_status: DependencyStatus::Blocked,
        //     retry_count: 0,
        // };
        // let mut tasks = self.tasks.lock().await;
        // // todo: ensure the same task id isn't already present
        // tasks.push(task);
        // let new_task_index = tasks.len() - 1;
        // self.order_tasks_by_priority(&mut tasks, new_task_index)
        //     .await;
    }

    // when a task is updated
    // check if the task unblocks other tasks from executing
    pub async fn update_task_status(&self, task_id: Uuid, status: TaskStatus) {
        let mut tasks = self.tasks.lock().await;
        if let Some((idx, task)) = tasks.iter_mut().enumerate().find(|(_, t)| t.id == task_id) {
            task.progress_status = status;
            if status == TaskStatus::WitnessSubmitted {
                task.prover_input.make_empty(); // free because it has now been saved in db
            }
            self.order_tasks_by_priority(&mut tasks, idx).await;
        }
    }

    pub async fn get_pending_task(&self) -> Option<ProvingTask> {
        let tasks = self.tasks.lock().await;
        if let Some(index) = tasks.iter().position(|t| {
            t.progress_status == TaskStatus::WitnessSubmitted // todo
                || t.progress_status == TaskStatus::Created
                || t.progress_status == TaskStatus::ProvingBegin
        }) {
            let task = tasks[index].clone();
            Some(task)
        } else {
            None
        }
    }
}
