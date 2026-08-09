use crate::daemon::DaemonState;
use luci_common::{Job, JobRequirements, NodeId, NodeState, NodeStatus};

pub struct Scheduler;

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    pub fn find_best_node(&self, state: &DaemonState, job: &Job) -> Option<NodeId> {
        let requirements = &job.requirements;

        let mut candidates: Vec<(&NodeState, f64)> = state
            .nodes
            .values()
            .filter(|node| self.is_suitable(node, requirements))
            .map(|node| (node, self.score_node(node, requirements)))
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        candidates.first().map(|(node, _)| node.info.node_id.clone())
    }

    fn is_suitable(&self, node: &NodeState, requirements: &JobRequirements) -> bool {
        if !node.is_available() {
            return false;
        }

        if let Some(resources) = &node.resources {
            if resources.memory.available_bytes < requirements.min_ram_bytes {
                return false;
            }

            if resources.cpu.cores < requirements.min_cpu_cores {
                return false;
            }

            if requirements.requires_gpu {
                match &resources.gpu {
                    Some(gpu) => {
                        if let Some(min_vram) = requirements.min_vram_bytes {
                            if gpu.vram_available_bytes < min_vram {
                                return false;
                            }
                        }
                    }
                    None => return false,
                }
            }

            true
        } else {
            false
        }
    }

    fn score_node(&self, node: &NodeState, requirements: &JobRequirements) -> f64 {
        let mut score = 0.0;

        if let Some(resources) = &node.resources {
            // RAM score (higher is better)
            score += resources.memory.available_gb() * 10.0;

            // CPU score (lower load is better)
            score += (100.0 - resources.cpu.load_percent as f64) * 0.5;

            // Idle bonus
            if resources.idle_seconds > 300 {
                score += 20.0;
            }

            // Charging bonus
            if resources.is_charging.unwrap_or(false) {
                score += 5.0;
            }

            // GPU bonus if available
            if let Some(gpu) = &resources.gpu {
                score += 10.0;
                // Extra bonus if node meets VRAM requirements
                if let Some(min_vram) = requirements.min_vram_bytes {
                    if gpu.vram_available_bytes >= min_vram {
                        score += 5.0;
                    }
                }
            }
        }

        // Preferred node bonus
        if requirements.preferred_nodes.contains(&node.info.node_id) {
            score += 25.0;
        }

        // Status bonus
        match node.status {
            NodeStatus::Idle => score += 15.0,
            NodeStatus::Online => score += 10.0,
            _ => {}
        }

        score
    }

    pub fn rank_nodes(&self, state: &DaemonState) -> Vec<(NodeId, f64)> {
        let mut nodes: Vec<(NodeId, f64)> = state
            .nodes
            .values()
            .filter(|node| node.is_available())
            .map(|node| {
                let score = node.score();
                (node.info.node_id.clone(), score)
            })
            .collect();

        nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        nodes
    }
}
