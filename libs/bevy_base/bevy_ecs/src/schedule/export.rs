#![allow(missing_docs)]

use alloc::{string::String, vec::Vec};

/// Stable-enough, human-readable identifier for a node in a schedule snapshot.
///
/// This is intentionally not the internal `SystemKey` / `SystemSetKey`: external
/// consumers should treat it as an opaque handle owned by the schedule graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshotNodeId {
    pub kind: ScheduleSnapshotNodeKind,
    pub raw: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum ScheduleSnapshotNodeKind {
    System,
    Set,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshotSystem {
    pub id: ScheduleSnapshotNodeId,
    pub name: String,
    pub system_type: String,
    pub is_send: bool,
    pub is_exclusive: bool,
    pub has_deferred: bool,
    pub conditions: Vec<ScheduleSnapshotCondition>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshotSet {
    pub id: ScheduleSnapshotNodeId,
    pub name: String,
    pub conditions: Vec<ScheduleSnapshotCondition>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshotCondition {
    pub name: String,
    pub system_type: String,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshotEdge {
    pub from: ScheduleSnapshotNodeId,
    pub to: ScheduleSnapshotNodeId,
}

/// Read-only exchange representation of the schedule semantic graph.
///
/// Systems and conditions remain owned by the source schedule. This snapshot is
/// meant for debugging, inspection, and lowering into an external executable IR.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ScheduleSnapshot {
    pub systems: Vec<ScheduleSnapshotSystem>,
    pub sets: Vec<ScheduleSnapshotSet>,
    pub hierarchy_edges: Vec<ScheduleSnapshotEdge>,
    pub dependency_edges: Vec<ScheduleSnapshotEdge>,
}
