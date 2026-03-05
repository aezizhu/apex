//! Comprehensive unit tests for Worker Pool concurrency management.
//!
//! Tests cover:
//! - Pool creation and configuration
//! - Permit acquisition and release
//! - Concurrent access patterns
//! - Timeout handling
//! - Statistics tracking
//! - Background task spawning
//! - Health checks
//! - Edge cases and stress tests

use apex_core::error::ApexError;
use apex_core::orchestrator::worker_pool::{WorkerPool, WorkerPoolConfig, WorkerPoolStats};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Pool Configuration Tests
// ============================================================================

#[test]
fn test_pool_config_default() {
    let config = WorkerPoolConfig::default();

    assert_eq!(config.max_workers, 100);
    assert_eq!(config.acquire_timeout_ms, 30000);
    assert!(config.track_worker_stats);
    assert_eq!(config.name, "default");
}

#[test]
fn test_pool_config_small() {
    let config = WorkerPoolConfig::small();
    assert_eq!(config.max_workers, 10);
}

#[test]
fn test_pool_config_medium() {
    let config = WorkerPoolConfig::medium();
    assert_eq!(config.max_workers, 50);
}

#[test]
fn test_pool_config_large() {
    let config = WorkerPoolConfig::large();
    assert_eq!(config.max_workers, 200);
}

#[test]
fn test_pool_config_with_name() {
    let config = WorkerPoolConfig::small().with_name("test-pool");
    assert_eq!(config.name, "test-pool");
    assert_eq!(config.max_workers, 10);
}

#[test]
fn test_pool_config_with_string_name() {
    let config = WorkerPoolConfig::default().with_name(String::from("string-pool"));
    assert_eq!(config.name, "string-pool");
}

#[test]
fn test_pool_config_clone() {
    let config = WorkerPoolConfig {
        max_workers: 25,
        acquire_timeout_ms: 5000,
        track_worker_stats: false,
        name: "cloneable".to_string(),
    };

    let cloned = config.clone();
    assert_eq!(cloned.max_workers, 25);
    assert_eq!(cloned.acquire_timeout_ms, 5000);
    assert!(!cloned.track_worker_stats);
    assert_eq!(cloned.name, "cloneable");
}

#[test]
fn test_pool_config_debug() {
    let config = WorkerPoolConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("WorkerPoolConfig"));
}

// ============================================================================
// Pool Creation Tests
// ============================================================================

#[test]
fn test_pool_creation_default() {
    let pool = WorkerPool::with_defaults();

    assert_eq!(pool.max_workers(), 100);
    assert_eq!(pool.available_permits(), 100);
    assert_eq!(pool.active_workers(), 0);
    assert!(!pool.is_at_capacity());
}

#[test]
fn test_pool_creation_custom() {
    let config = WorkerPoolConfig {
        max_workers: 5,
        acquire_timeout_ms: 1000,
        track_worker_stats: true,
        name: "custom".to_string(),
    };

    let pool = WorkerPool::new(config);

    assert_eq!(pool.max_workers(), 5);
    assert_eq!(pool.available_permits(), 5);
    assert_eq!(pool.name(), "custom");
}

#[test]
fn test_pool_creation_single_worker() {
    let config = WorkerPoolConfig {
        max_workers: 1,
        ..Default::default()
    };

    let pool = WorkerPool::new(config);
    assert_eq!(pool.max_workers(), 1);
    assert_eq!(pool.available_permits(), 1);
}

#[test]
fn test_pool_default_implementation() {
    let pool = WorkerPool::default();
    assert_eq!(pool.max_workers(), 100);
}

// ============================================================================
// Permit Acquisition Tests (Async)
// ============================================================================

#[tokio::test]
async fn test_acquire_single_permit() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    });

    let permit = pool.acquire().await.unwrap();

    assert_eq!(pool.available_permits(), 4);
    assert_eq!(pool.active_workers(), 1);

    drop(permit);
}

#[tokio::test]
async fn test_acquire_all_permits() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 3,
        ..Default::default()
    });

    let p1 = pool.acquire().await.unwrap();
    let p2 = pool.acquire().await.unwrap();
    let p3 = pool.acquire().await.unwrap();

    assert_eq!(pool.available_permits(), 0);
    assert_eq!(pool.active_workers(), 3);
    assert!(pool.is_at_capacity());

    drop(p1);
    drop(p2);
    drop(p3);
}

#[tokio::test]
async fn test_acquire_and_release() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 2,
        ..Default::default()
    });

    assert_eq!(pool.available_permits(), 2);

    let permit1 = pool.acquire().await.unwrap();
    assert_eq!(pool.available_permits(), 1);

    let permit2 = pool.acquire().await.unwrap();
    assert_eq!(pool.available_permits(), 0);

    permit1.mark_success();
    assert_eq!(pool.available_permits(), 1);

    permit2.mark_failure();
    assert_eq!(pool.available_permits(), 2);
}

#[tokio::test]
async fn test_permit_drop_releases_slot() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 2,
        ..Default::default()
    });

    {
        let _permit = pool.acquire().await.unwrap();
        assert_eq!(pool.available_permits(), 1);
    }

    // Permit dropped but not marked - should still release
    // Note: this records as "unknown" completion
    assert_eq!(pool.available_permits(), 2);
}

// ============================================================================
// Try Acquire Tests
// ============================================================================

#[tokio::test]
async fn test_try_acquire_available() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 2,
        ..Default::default()
    });

    let permit = pool.try_acquire();
    assert!(permit.is_some());
    assert_eq!(pool.available_permits(), 1);
}

#[tokio::test]
async fn test_try_acquire_at_capacity() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        ..Default::default()
    });

    let permit1 = pool.try_acquire();
    assert!(permit1.is_some());

    let permit2 = pool.try_acquire();
    assert!(permit2.is_none());
}

#[tokio::test]
async fn test_try_acquire_after_release() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        ..Default::default()
    });

    let permit1 = pool.try_acquire().unwrap();
    assert!(pool.try_acquire().is_none());

    drop(permit1);

    let permit2 = pool.try_acquire();
    assert!(permit2.is_some());
}

// ============================================================================
// Timeout Tests
// ============================================================================

#[tokio::test]
async fn test_acquire_timeout() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        acquire_timeout_ms: 50,
        ..Default::default()
    });

    // Hold the only permit
    let _permit = pool.acquire().await.unwrap();

    // Try to acquire another - should timeout
    let result = pool.acquire().await;
    assert!(result.is_err());

    // Verify it's an error (timeout)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_acquire_succeeds_before_timeout() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        acquire_timeout_ms: 1000, // Long timeout
        ..Default::default()
    }));

    let pool_clone = pool.clone();

    // Hold permit briefly then release
    let handle = tokio::spawn(async move {
        let permit = pool_clone.acquire().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        permit.mark_success();
    });

    // Wait a bit for first acquire
    tokio::time::sleep(Duration::from_millis(10)).await;

    // This should succeed after first permit is released
    let result = pool.acquire().await;
    assert!(result.is_ok());

    handle.await.unwrap();
}

// ============================================================================
// Spawn Tests
// ============================================================================

#[tokio::test]
async fn test_spawn_success() {
    let pool = WorkerPool::with_defaults();

    let result = pool
        .spawn(|| async { Ok::<i32, ApexError>(42) })
        .await
        .unwrap();

    assert_eq!(result, 42);

    let stats = pool.stats();
    assert_eq!(stats.tasks_succeeded, 1);
}

#[tokio::test]
async fn test_spawn_failure() {
    let pool = WorkerPool::with_defaults();

    let result: Result<i32, ApexError> = pool
        .spawn(|| async { Err(ApexError::internal("test error")) })
        .await;

    assert!(result.is_err());

    let stats = pool.stats();
    assert_eq!(stats.tasks_failed, 1);
}

#[tokio::test]
async fn test_spawn_multiple_tasks() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        ..Default::default()
    });

    let mut handles = vec![];

    for i in 0..5 {
        let result = pool
            .spawn(move || async move { Ok::<i32, ApexError>(i) })
            .await;
        handles.push(result);
    }

    for (i, result) in handles.iter().enumerate() {
        assert_eq!(*result.as_ref().unwrap(), i as i32);
    }

    let stats = pool.stats();
    assert_eq!(stats.tasks_succeeded, 5);
}

// ============================================================================
// Background Spawn Tests
// ============================================================================

#[tokio::test]
async fn test_spawn_background() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 2,
        ..Default::default()
    }));

    let counter = Arc::new(AtomicU64::new(0));
    let counter_clone = counter.clone();

    pool.spawn_background(move || async move {
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    pool.join_all().await;

    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_spawn_multiple_background_tasks() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    }));

    let counter = Arc::new(AtomicU64::new(0));

    for _ in 0..10 {
        let counter_clone = counter.clone();
        pool.spawn_background(move || async move {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
    }

    pool.join_all().await;

    assert_eq!(counter.load(Ordering::Relaxed), 10);
}

#[tokio::test]
async fn test_cancel_all_background_tasks() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 2,
        ..Default::default()
    });

    // Spawn long-running tasks
    pool.spawn_background(|| async {
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    pool.spawn_background(|| async {
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    // Give tasks time to start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Cancel should not hang
    pool.cancel_all();
}

// ============================================================================
// Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_stats_initial() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        name: "stats-test".to_string(),
        ..Default::default()
    });

    let stats = pool.stats();

    assert_eq!(stats.name, "stats-test");
    assert_eq!(stats.max_workers, 10);
    assert_eq!(stats.available_permits, 10);
    assert_eq!(stats.active_workers, 0);
    assert_eq!(stats.tasks_submitted, 0);
    assert_eq!(stats.tasks_succeeded, 0);
    assert_eq!(stats.tasks_failed, 0);
    assert_eq!(stats.tasks_unknown, 0);
    assert_eq!(stats.acquire_timeouts, 0);
    assert_eq!(stats.peak_concurrent, 0);
}

#[tokio::test]
async fn test_stats_after_operations() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    });

    // 3 successes
    for _ in 0..3 {
        let permit = pool.acquire().await.unwrap();
        permit.mark_success();
    }

    // 1 failure
    let permit = pool.acquire().await.unwrap();
    permit.mark_failure();

    let stats = pool.stats();
    assert_eq!(stats.tasks_submitted, 4);
    assert_eq!(stats.tasks_succeeded, 3);
    assert_eq!(stats.tasks_failed, 1);
    assert_eq!(stats.success_rate(), 75.0);
}

#[tokio::test]
async fn test_stats_unknown_completions() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    });

    // Drop permit without marking
    {
        let _permit = pool.acquire().await.unwrap();
    }

    let stats = pool.stats();
    assert_eq!(stats.tasks_unknown, 1);
}

#[tokio::test]
async fn test_stats_timeout_tracking() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        acquire_timeout_ms: 10,
        ..Default::default()
    });

    let _permit = pool.acquire().await.unwrap();

    // This should timeout
    let _ = pool.acquire().await;

    let stats = pool.stats();
    assert_eq!(stats.acquire_timeouts, 1);
}

#[tokio::test]
async fn test_stats_peak_concurrent() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        ..Default::default()
    });

    // Acquire 5 permits at once
    let p1 = pool.acquire().await.unwrap();
    let p2 = pool.acquire().await.unwrap();
    let p3 = pool.acquire().await.unwrap();
    let p4 = pool.acquire().await.unwrap();
    let p5 = pool.acquire().await.unwrap();

    let stats = pool.stats();
    assert_eq!(stats.peak_concurrent, 5);

    // Release all
    p1.mark_success();
    p2.mark_success();
    p3.mark_success();
    p4.mark_success();
    p5.mark_success();

    // Peak should still be 5
    let stats = pool.stats();
    assert_eq!(stats.peak_concurrent, 5);
}

// ============================================================================
// WorkerPoolStats Methods Tests
// ============================================================================

#[test]
fn test_stats_success_rate_all_success() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 100,
        tasks_succeeded: 100,
        tasks_failed: 0,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 5,
        avg_wait_time_us: 100,
        avg_exec_time_us: 1000,
        uptime_secs: 60,
    };

    assert_eq!(stats.success_rate(), 100.0);
}

#[test]
fn test_stats_success_rate_all_failure() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 100,
        tasks_succeeded: 0,
        tasks_failed: 100,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 5,
        avg_wait_time_us: 100,
        avg_exec_time_us: 1000,
        uptime_secs: 60,
    };

    assert_eq!(stats.success_rate(), 0.0);
}

#[test]
fn test_stats_success_rate_no_tasks() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 0,
        tasks_succeeded: 0,
        tasks_failed: 0,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 0,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 60,
    };

    assert_eq!(stats.success_rate(), 100.0); // Default to 100% with no data
}

#[test]
fn test_stats_utilization() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 3,
        active_workers: 7,
        tasks_submitted: 0,
        tasks_succeeded: 0,
        tasks_failed: 0,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 7,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 60,
    };

    assert_eq!(stats.utilization(), 70.0);
}

#[test]
fn test_stats_utilization_zero() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 0,
        tasks_succeeded: 0,
        tasks_failed: 0,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 0,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 60,
    };

    assert_eq!(stats.utilization(), 0.0);
}

#[test]
fn test_stats_utilization_full() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 0,
        active_workers: 10,
        tasks_submitted: 0,
        tasks_succeeded: 0,
        tasks_failed: 0,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 10,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 60,
    };

    assert_eq!(stats.utilization(), 100.0);
}

#[test]
fn test_stats_throughput() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 100,
        tasks_succeeded: 80,
        tasks_failed: 20,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 10,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 60,
    };

    assert!((stats.throughput() - 1.667).abs() < 0.01);
}

#[test]
fn test_stats_throughput_zero_uptime() {
    let stats = WorkerPoolStats {
        name: "test".to_string(),
        max_workers: 10,
        available_permits: 10,
        active_workers: 0,
        tasks_submitted: 100,
        tasks_succeeded: 80,
        tasks_failed: 20,
        tasks_unknown: 0,
        acquire_timeouts: 0,
        peak_concurrent: 10,
        avg_wait_time_us: 0,
        avg_exec_time_us: 0,
        uptime_secs: 0,
    };

    assert_eq!(stats.throughput(), 0.0);
}

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_is_healthy_initial() {
    let pool = WorkerPool::with_defaults();
    assert!(pool.is_healthy());
}

#[tokio::test]
async fn test_is_healthy_after_successes() {
    let pool = WorkerPool::with_defaults();

    for _ in 0..10 {
        let permit = pool.acquire().await.unwrap();
        permit.mark_success();
    }

    assert!(pool.is_healthy());
}

#[tokio::test]
async fn test_health_with_high_failure_rate() {
    let pool = WorkerPool::with_defaults();

    // Create a scenario with high failure rate
    for _ in 0..10 {
        let permit = pool.acquire().await.unwrap();
        permit.mark_failure();
    }

    // With >50% failure rate, should be unhealthy
    // But we need some successes too for comparison
    for _ in 0..3 {
        let permit = pool.acquire().await.unwrap();
        permit.mark_success();
    }

    // 10 failures / 13 total = ~77% failure rate
    assert!(!pool.is_healthy());
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_acquire_release() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        ..Default::default()
    }));

    let mut handles = vec![];

    for _ in 0..50 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let permit = pool_clone.acquire().await.unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
            permit.mark_success();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let stats = pool.stats();
    assert_eq!(stats.tasks_submitted, 50);
    assert_eq!(stats.tasks_succeeded, 50);
    assert!(stats.peak_concurrent <= 10);
}

#[tokio::test]
async fn test_concurrent_try_acquire() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    }));

    let acquired = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..20 {
        let pool_clone = pool.clone();
        let acquired_clone = acquired.clone();
        let handle = tokio::spawn(async move {
            if let Some(permit) = pool_clone.try_acquire() {
                acquired_clone.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(10)).await;
                permit.mark_success();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Should have acquired some permits
    assert!(acquired.load(Ordering::Relaxed) > 0);
}

#[tokio::test]
async fn test_pool_under_load() {
    let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        acquire_timeout_ms: 5000,
        ..Default::default()
    }));

    let completed = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Submit 100 tasks to pool of 10
    for _ in 0..100 {
        let pool_clone = pool.clone();
        let completed_clone = completed.clone();
        let handle = tokio::spawn(async move {
            if let Ok(permit) = pool_clone.acquire().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
                permit.mark_success();
                completed_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(completed.load(Ordering::Relaxed), 100);
}

// ============================================================================
// Resize Tests
// ============================================================================

#[test]
fn test_resize_increase() {
    let mut pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 5,
        ..Default::default()
    });

    assert_eq!(pool.max_workers(), 5);

    pool.resize(10);

    assert_eq!(pool.max_workers(), 10);
    assert_eq!(pool.available_permits(), 10);
}

#[test]
fn test_resize_decrease() {
    let mut pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 10,
        ..Default::default()
    });

    pool.resize(5);

    // Max workers updated
    assert_eq!(pool.max_workers(), 5);
}

// ============================================================================
// Permit ID Tests
// ============================================================================

#[tokio::test]
async fn test_permit_id_unique() {
    let pool = WorkerPool::with_defaults();

    let permit1 = pool.acquire().await.unwrap();
    let permit2 = pool.acquire().await.unwrap();

    assert_ne!(permit1.id(), permit2.id());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_zero_timeout_immediate_failure() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        acquire_timeout_ms: 0, // Zero timeout
        ..Default::default()
    });

    let _permit = pool.acquire().await.unwrap();

    // With zero timeout, should fail immediately
    let result = pool.acquire().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_large_pool() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1000,
        ..Default::default()
    });

    assert_eq!(pool.max_workers(), 1000);
    assert_eq!(pool.available_permits(), 1000);
}

#[tokio::test]
async fn test_rapid_acquire_release() {
    let pool = WorkerPool::new(WorkerPoolConfig {
        max_workers: 1,
        ..Default::default()
    });

    for _ in 0..100 {
        let permit = pool.acquire().await.unwrap();
        permit.mark_success();
    }

    let stats = pool.stats();
    assert_eq!(stats.tasks_succeeded, 100);
}

#[tokio::test]
async fn test_stats_clone() {
    let pool = WorkerPool::with_defaults();
    let permit = pool.acquire().await.unwrap();
    permit.mark_success();

    let stats = pool.stats();
    let cloned = stats.clone();

    assert_eq!(stats.tasks_succeeded, cloned.tasks_succeeded);
    assert_eq!(stats.name, cloned.name);
}

#[test]
fn test_stats_debug() {
    let stats = WorkerPoolStats {
        name: "debug-test".to_string(),
        max_workers: 10,
        available_permits: 5,
        active_workers: 5,
        tasks_submitted: 100,
        tasks_succeeded: 90,
        tasks_failed: 10,
        tasks_unknown: 0,
        acquire_timeouts: 2,
        peak_concurrent: 8,
        avg_wait_time_us: 500,
        avg_exec_time_us: 2000,
        uptime_secs: 120,
    };

    let debug_str = format!("{:?}", stats);
    assert!(debug_str.contains("WorkerPoolStats"));
    assert!(debug_str.contains("debug-test"));
}

// ============================================================================
// WorkerExecution Tests
// ============================================================================

#[test]
fn test_worker_execution_default() {
    use apex_core::orchestrator::worker_pool::WorkerExecution;

    let exec = WorkerExecution::new();

    assert!(exec.finished_at.is_none());
    assert!(exec.succeeded.is_none());
    assert!(exec.duration().is_none());
}

#[test]
fn test_worker_execution_complete() {
    use apex_core::orchestrator::worker_pool::WorkerExecution;

    let mut exec = WorkerExecution::new();
    exec.complete(true);

    assert!(exec.finished_at.is_some());
    assert_eq!(exec.succeeded, Some(true));
    assert!(exec.duration().is_some());
}

#[test]
fn test_worker_execution_complete_failure() {
    use apex_core::orchestrator::worker_pool::WorkerExecution;

    let mut exec = WorkerExecution::new();
    exec.complete(false);

    assert_eq!(exec.succeeded, Some(false));
}

#[test]
fn test_worker_execution_clone() {
    use apex_core::orchestrator::worker_pool::WorkerExecution;

    let exec = WorkerExecution::new();
    let cloned = exec.clone();

    assert_eq!(exec.id, cloned.id);
}
