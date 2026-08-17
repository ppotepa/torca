use std::time::{Duration, Instant};

use torca_runtime_policy::{AttentionContext, PolicyEvent, RuntimeGovernor};

#[test]
fn six_hours_of_background_attention_stay_completely_idle() {
    let start = Instant::now();
    let mut governor = RuntimeGovernor::new(start);

    for hour in 0_u64..=6 {
        let now = start + Duration::from_secs(hour * 60 * 60);
        let attention = AttentionContext { generation: hour + 1, ..AttentionContext::default() };
        let delta = governor.apply(PolicyEvent::Attention(attention), now);
        assert!(delta.permits_due.is_empty());
        assert!(governor.take_due(now).is_empty());

        let snapshot = governor.snapshot(now);
        assert_eq!(snapshot.active_leases, 0);
        assert_eq!(snapshot.active_demands, 0);
        assert_eq!(snapshot.scheduled_deadlines, 0);
        assert_eq!(snapshot.next_deadline_in_ms, None);
        assert_eq!(snapshot.stats.scheduler_wakeups, 0);
        assert_eq!(snapshot.stats.permits_issued, 0);
    }
}

#[test]
fn network_generation_changes_do_not_create_work_without_demand() {
    let start = Instant::now();
    let mut governor = RuntimeGovernor::new(start);

    for generation in 1_u64..=32 {
        let now = start + Duration::from_secs(generation);
        let delta = governor.apply(PolicyEvent::NetworkChanged { generation }, now);
        assert!(delta.network_changed);
        assert!(delta.permits_due.is_empty());
        assert_eq!(governor.next_deadline(), None);
        assert_eq!(governor.next_lease_expiry(), None);
    }

    let snapshot = governor.snapshot(start + Duration::from_secs(33));
    assert_eq!(snapshot.active_leases, 0);
    assert_eq!(snapshot.active_demands, 0);
    assert_eq!(snapshot.scheduled_deadlines, 0);
    assert_eq!(snapshot.stats.scheduler_wakeups, 0);
}
