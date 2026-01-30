//! Room/channel management for WebSocket subscriptions.
//!
//! Rooms allow grouping connections by their subscriptions for efficient
//! message broadcasting.

use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::handler::ConnectionId;

/// Identifier for a room/channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoomId {
    /// Room for a specific task's updates
    Task(String),
    /// Room for all task updates
    AllTasks,
    /// Room for a specific agent's updates
    Agent(String),
    /// Room for all agent updates
    AllAgents,
    /// Room for a specific DAG's updates
    Dag(String),
    /// Room for all DAG updates
    AllDags,
    /// Room for system metrics
    Metrics,
    /// Room for approval notifications
    Approvals,
    /// Room for error notifications
    Errors,
    /// Custom room with arbitrary name
    Custom(String),
}

impl RoomId {
    /// Get a string representation of the room ID.
    pub fn as_str(&self) -> String {
        match self {
            RoomId::Task(id) => format!("task:{}", id),
            RoomId::AllTasks => "tasks:all".to_string(),
            RoomId::Agent(id) => format!("agent:{}", id),
            RoomId::AllAgents => "agents:all".to_string(),
            RoomId::Dag(id) => format!("dag:{}", id),
            RoomId::AllDags => "dags:all".to_string(),
            RoomId::Metrics => "metrics".to_string(),
            RoomId::Approvals => "approvals".to_string(),
            RoomId::Errors => "errors".to_string(),
            RoomId::Custom(name) => format!("custom:{}", name),
        }
    }

    /// Get the room type.
    pub fn room_type(&self) -> RoomType {
        match self {
            RoomId::Task(_) | RoomId::AllTasks => RoomType::Task,
            RoomId::Agent(_) | RoomId::AllAgents => RoomType::Agent,
            RoomId::Dag(_) | RoomId::AllDags => RoomType::Dag,
            RoomId::Metrics => RoomType::Metrics,
            RoomId::Approvals => RoomType::Approval,
            RoomId::Errors => RoomType::Error,
            RoomId::Custom(_) => RoomType::Custom,
        }
    }
}

/// Type classification for rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    Task,
    Agent,
    Dag,
    Metrics,
    Approval,
    Error,
    Custom,
}

/// A room containing a set of subscribed connections.
#[derive(Debug)]
pub struct Room {
    /// Room identifier
    pub id: RoomId,
    /// Connected clients
    members: HashSet<ConnectionId>,
    /// Room creation time
    pub created_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Message count sent to this room
    pub message_count: u64,
}

impl Room {
    /// Create a new room.
    pub fn new(id: RoomId) -> Self {
        let now = Utc::now();
        Self {
            id,
            members: HashSet::new(),
            created_at: now,
            last_activity: now,
            message_count: 0,
        }
    }

    /// Add a connection to the room.
    pub fn add_member(&mut self, conn_id: ConnectionId) -> bool {
        let added = self.members.insert(conn_id);
        if added {
            self.last_activity = Utc::now();
        }
        added
    }

    /// Remove a connection from the room.
    pub fn remove_member(&mut self, conn_id: ConnectionId) -> bool {
        let removed = self.members.remove(&conn_id);
        if removed {
            self.last_activity = Utc::now();
        }
        removed
    }

    /// Check if a connection is in the room.
    pub fn has_member(&self, conn_id: ConnectionId) -> bool {
        self.members.contains(&conn_id)
    }

    /// Get all members of the room.
    pub fn members(&self) -> impl Iterator<Item = &ConnectionId> {
        self.members.iter()
    }

    /// Get the number of members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if the room is empty.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Record that a message was sent to this room.
    pub fn record_message(&mut self) {
        self.message_count += 1;
        self.last_activity = Utc::now();
    }
}

/// Statistics about a room.
#[derive(Debug, Clone, Serialize)]
pub struct RoomStats {
    pub id: String,
    pub room_type: RoomType,
    pub member_count: usize,
    pub message_count: u64,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

impl From<&Room> for RoomStats {
    fn from(room: &Room) -> Self {
        Self {
            id: room.id.as_str(),
            room_type: room.id.room_type(),
            member_count: room.member_count(),
            message_count: room.message_count,
            created_at: room.created_at,
            last_activity: room.last_activity,
        }
    }
}

/// Manager for all rooms.
#[derive(Debug, Default)]
pub struct RoomManager {
    /// All active rooms
    rooms: HashMap<RoomId, Room>,
    /// Index of connection ID to rooms they're in (for fast cleanup)
    connection_rooms: HashMap<ConnectionId, HashSet<RoomId>>,
}

impl RoomManager {
    /// Create a new room manager.
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            connection_rooms: HashMap::new(),
        }
    }

    /// Add a connection to a room, creating the room if necessary.
    pub fn join_room(&mut self, conn_id: ConnectionId, room_id: RoomId) -> bool {
        // Get or create the room
        let room = self.rooms
            .entry(room_id.clone())
            .or_insert_with(|| Room::new(room_id.clone()));

        let added = room.add_member(conn_id);

        if added {
            // Update connection index
            self.connection_rooms
                .entry(conn_id)
                .or_default()
                .insert(room_id);
        }

        added
    }

    /// Remove a connection from a room.
    pub fn leave_room(&mut self, conn_id: ConnectionId, room_id: &RoomId) -> bool {
        let removed = if let Some(room) = self.rooms.get_mut(room_id) {
            let removed = room.remove_member(conn_id);

            // Clean up empty rooms (except system rooms)
            if room.is_empty() && !self.is_system_room(room_id) {
                self.rooms.remove(room_id);
            }

            removed
        } else {
            false
        };

        if removed {
            // Update connection index
            if let Some(rooms) = self.connection_rooms.get_mut(&conn_id) {
                rooms.remove(room_id);
                if rooms.is_empty() {
                    self.connection_rooms.remove(&conn_id);
                }
            }
        }

        removed
    }

    /// Remove a connection from all rooms (for disconnection cleanup).
    pub fn remove_connection_from_all(&mut self, conn_id: ConnectionId) {
        if let Some(rooms) = self.connection_rooms.remove(&conn_id) {
            for room_id in rooms {
                if let Some(room) = self.rooms.get_mut(&room_id) {
                    room.remove_member(conn_id);

                    // Clean up empty non-system rooms
                    if room.is_empty() && !self.is_system_room(&room_id) {
                        self.rooms.remove(&room_id);
                    }
                }
            }
        }
    }

    /// Check if a room is a system room (should not be auto-deleted).
    fn is_system_room(&self, room_id: &RoomId) -> bool {
        matches!(
            room_id,
            RoomId::AllTasks
            | RoomId::AllAgents
            | RoomId::AllDags
            | RoomId::Metrics
            | RoomId::Approvals
            | RoomId::Errors
        )
    }

    /// Get a room by ID.
    pub fn get_room(&self, room_id: &RoomId) -> Option<&Room> {
        self.rooms.get(room_id)
    }

    /// Get mutable reference to a room.
    pub fn get_room_mut(&mut self, room_id: &RoomId) -> Option<&mut Room> {
        self.rooms.get_mut(room_id)
    }

    /// Get all members of a room.
    pub fn get_room_members(&self, room_id: &RoomId) -> Vec<ConnectionId> {
        self.rooms
            .get(room_id)
            .map(|r| r.members().copied().collect())
            .unwrap_or_default()
    }

    /// Get all rooms a connection is in.
    pub fn get_connection_rooms(&self, conn_id: ConnectionId) -> Vec<RoomId> {
        self.connection_rooms
            .get(&conn_id)
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of active rooms.
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Get statistics for all rooms.
    pub fn get_all_stats(&self) -> Vec<RoomStats> {
        self.rooms.values().map(RoomStats::from).collect()
    }

    /// Get statistics for a specific room type.
    pub fn get_stats_by_type(&self, room_type: RoomType) -> Vec<RoomStats> {
        self.rooms
            .values()
            .filter(|r| r.id.room_type() == room_type)
            .map(RoomStats::from)
            .collect()
    }

    /// Broadcast to a room and get member list.
    pub fn get_broadcast_targets(&self, room_id: &RoomId) -> Vec<ConnectionId> {
        let mut targets = Vec::new();

        // Get direct room members
        if let Some(room) = self.rooms.get(room_id) {
            targets.extend(room.members().copied());
        }

        // Also include "all" room subscribers
        let all_room_id = match room_id {
            RoomId::Task(_) => Some(RoomId::AllTasks),
            RoomId::Agent(_) => Some(RoomId::AllAgents),
            RoomId::Dag(_) => Some(RoomId::AllDags),
            _ => None,
        };

        if let Some(all_id) = all_room_id {
            if let Some(all_room) = self.rooms.get(&all_id) {
                for conn_id in all_room.members() {
                    if !targets.contains(conn_id) {
                        targets.push(*conn_id);
                    }
                }
            }
        }

        targets
    }

    /// Clean up stale rooms (no activity for specified duration).
    pub fn cleanup_stale_rooms(&mut self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;

        // Extract system room check to avoid borrowing issue
        let is_system = |room_id: &RoomId| -> bool {
            matches!(
                room_id,
                RoomId::AllTasks
                | RoomId::AllAgents
                | RoomId::AllDags
                | RoomId::Metrics
                | RoomId::Approvals
                | RoomId::Errors
            )
        };

        self.rooms.retain(|room_id, room| {
            // Keep system rooms and rooms with activity
            is_system(room_id)
                || !room.is_empty()
                || room.last_activity > cutoff
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_id_string() {
        assert_eq!(
            RoomId::Task("abc-123".to_string()).as_str(),
            "task:abc-123"
        );
        assert_eq!(RoomId::AllTasks.as_str(), "tasks:all");
        assert_eq!(RoomId::Metrics.as_str(), "metrics");
    }

    #[test]
    fn test_room_membership() {
        let mut room = Room::new(RoomId::Task("test".to_string()));
        let conn1 = ConnectionId::new();
        let conn2 = ConnectionId::new();

        assert!(room.add_member(conn1));
        assert!(!room.add_member(conn1)); // Already a member
        assert!(room.add_member(conn2));

        assert_eq!(room.member_count(), 2);
        assert!(room.has_member(conn1));
        assert!(room.has_member(conn2));

        assert!(room.remove_member(conn1));
        assert!(!room.remove_member(conn1)); // Already removed
        assert_eq!(room.member_count(), 1);
    }

    #[test]
    fn test_room_manager_join_leave() {
        let mut manager = RoomManager::new();
        let conn1 = ConnectionId::new();
        let conn2 = ConnectionId::new();
        let room_id = RoomId::Task("task-1".to_string());

        // Join room
        assert!(manager.join_room(conn1, room_id.clone()));
        assert!(manager.join_room(conn2, room_id.clone()));
        assert_eq!(manager.get_room_members(&room_id).len(), 2);

        // Leave room
        assert!(manager.leave_room(conn1, &room_id));
        assert_eq!(manager.get_room_members(&room_id).len(), 1);
    }

    #[test]
    fn test_room_manager_connection_cleanup() {
        let mut manager = RoomManager::new();
        let conn1 = ConnectionId::new();
        let room1 = RoomId::Task("task-1".to_string());
        let room2 = RoomId::Agent("agent-1".to_string());

        // Join multiple rooms
        manager.join_room(conn1, room1.clone());
        manager.join_room(conn1, room2.clone());

        assert_eq!(manager.get_connection_rooms(conn1).len(), 2);

        // Remove from all
        manager.remove_connection_from_all(conn1);
        assert!(manager.get_connection_rooms(conn1).is_empty());

        // Rooms should be cleaned up (they're empty and not system rooms)
        assert_eq!(manager.room_count(), 0);
    }

    #[test]
    fn test_broadcast_targets_includes_all_subscribers() {
        let mut manager = RoomManager::new();
        let conn1 = ConnectionId::new();
        let conn2 = ConnectionId::new();
        let conn3 = ConnectionId::new();

        let specific_room = RoomId::Task("task-1".to_string());

        // conn1 subscribes to specific task
        manager.join_room(conn1, specific_room.clone());
        // conn2 subscribes to all tasks
        manager.join_room(conn2, RoomId::AllTasks);
        // conn3 subscribes to both
        manager.join_room(conn3, specific_room.clone());
        manager.join_room(conn3, RoomId::AllTasks);

        let targets = manager.get_broadcast_targets(&specific_room);

        // Should include all three connections
        assert!(targets.contains(&conn1));
        assert!(targets.contains(&conn2));
        assert!(targets.contains(&conn3));
        assert_eq!(targets.len(), 3); // No duplicates
    }
}
