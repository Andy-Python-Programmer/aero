// Copyright (C) 2021-2023 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use alloc::sync::Arc;
use hashbrown::HashMap;

use crate::utils::sync::Mutex;

use super::{Task, TaskId};

/// Process Group
///
/// A process group is a collection of one or more processes that are grouped together so that
/// they can be manipulated as a single entity.
pub struct Group {
    /// Unique identifier of the process group.
    id: usize,
    /// Processes part of the process group.
    tasks: Mutex<HashMap<TaskId, Arc<Task>>>,
}

impl Group {
    /// Creates a new process group.
    pub fn new(leader: Arc<Task>) -> Arc<Self> {
        let mut tasks = HashMap::new();
        tasks.insert(leader.pid(), leader.clone());

        leader.set_group_id(leader.pid().as_usize());

        Arc::new(Self {
            id: leader.pid().as_usize(),
            tasks: Mutex::new(tasks),
        })
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn register_task(&self, task: Arc<Task>) {
        assert!(self.tasks.lock_irq().insert(task.pid(), task).is_none());
    }

    pub fn signal(&self, target: usize) {
        for (_, task) in self.tasks.lock_irq().iter() {
            log::error!("Sending signal to task: {:?}", task.path());

            task.signal(target);
        }
    }
}

/// Process Session
pub struct Session {
    groups: Mutex<HashMap<usize, Arc<Group>>>,
}

impl Session {
    /// Creates a new process session.
    pub fn new(leader: Arc<Task>) -> Arc<Self> {
        let mut groups = HashMap::new();
        groups.insert(leader.pid().as_usize(), Group::new(leader.clone()));

        leader.set_session_id(leader.pid().as_usize());

        Arc::new(Self {
            groups: Mutex::new(groups),
        })
    }

    pub fn find(&self, target: Arc<Task>) -> Option<Arc<Group>> {
        self.groups.lock_irq().get(&target.group_id()).cloned()
    }

    pub fn register_task(&self, task: Arc<Task>) {
        assert!(!task.is_session_leader());

        let mut groups = self.groups.lock_irq();
        if let Some(group) = groups.get(&task.group_id()) {
            assert!(!task.is_group_leader());
            group.register_task(task);
        } else {
            assert!(task.is_group_leader());
            groups.insert(task.group_id(), Group::new(task));
        }
    }
}

pub struct SessionList(Mutex<HashMap<usize, Arc<Session>>>);

impl SessionList {
    /// Creates and registers a new process session with the given `leader` as the leader task.
    pub fn create_session(&self, leader: Arc<Task>) {
        self.0
            .lock_irq()
            .insert(leader.pid().as_usize(), Session::new(leader));
    }

    pub fn find_group(&self, target: Arc<Task>) -> Option<Arc<Group>> {
        self.0.lock_irq().get(&target.session_id())?.find(target)
    }

    pub fn register_task(&self, task: Arc<Task>) {
        assert!(task.is_process_leader());

        let sessions = self.0.lock_irq();

        if let Some(session) = sessions.get(&task.session_id()) {
            session.register_task(task);
        } else {
            drop(sessions);
            self.create_session(task);
        }
    }

    pub fn isolate(&self, task: Arc<Task>) {
        assert!(!task.is_group_leader() && !task.is_session_leader());

        let leader = task.process_leader();

        {
            let mut sessions = self.0.lock_irq();
            sessions.remove(&task.session_id());
        }

        self.create_session(leader)
    }
}

lazy_static::lazy_static! {
    pub static ref SESSIONS: SessionList = SessionList(Mutex::new(HashMap::new()));
}
