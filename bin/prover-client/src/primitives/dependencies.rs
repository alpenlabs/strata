use std::collections::HashSet;

use uuid::Uuid;

pub struct Dependencies{
    order: Vec<Uuid>,
    remaining: HashSet<Uuid>
}

impl Dependencies {
    // pub fn new(capacity: usize) -> Self {
    //     Dependencies(Vec::with_capacity(capacity))
    // }

    // pub fn insert(&mut self, task_id: Uuid) {
    //     self.0.push(task_id);
    // }
}
