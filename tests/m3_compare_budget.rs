use viewer_domain::{
    EntityId,
    image::{CompareImageBudgetRequest, DecodeBudget},
};

fn request(id: u128, proxy: (u32, u32), original: Option<(u32, u32)>) -> CompareImageBudgetRequest {
    CompareImageBudgetRequest {
        entity_id: EntityId::from_u128(id),
        proxy_width: proxy.0,
        proxy_height: proxy.1,
        requested_original: original,
    }
}

#[test]
fn four_viewport_proxies_are_reserved_before_any_original() {
    let budget = DecodeBudget::new(300_000_000, 100_000_000);
    let requests = (1..=4)
        .map(|id| request(id, (2_000, 1_500), None))
        .collect::<Vec<_>>();

    let plan = budget.plan_compare(&requests, 4).expect("proxy plan");

    assert_eq!(plan.proxy_panes().len(), 4);
    assert!(plan.original_panes().is_empty());
    assert!(plan.downgraded_panes().is_empty());
    assert_eq!(plan.reserved_bytes(), 48_000_000);
}

#[test]
fn unsafe_simultaneous_originals_are_deterministically_degraded_to_proxies() {
    let budget = DecodeBudget::new(300_000_000, 100_000_000);
    let requests = (1..=4)
        .map(|id| request(id, (2_000, 1_500), Some((6_000, 4_000))))
        .collect::<Vec<_>>();

    let plan = budget.plan_compare(&requests, 4).expect("bounded plan");

    assert_eq!(
        plan.original_panes(),
        &[EntityId::from_u128(1), EntityId::from_u128(2)]
    );
    assert_eq!(
        plan.downgraded_panes(),
        &[EntityId::from_u128(3), EntityId::from_u128(4)]
    );
    assert_eq!(plan.reserved_bytes(), 240_000_000);
}

#[test]
fn hard_pixel_limit_downgrades_an_original_even_when_bytes_are_available() {
    let budget = DecodeBudget::new(700_000_000, 100_000_000);
    let pane = request(7, (2_000, 1_500), Some((20_000, 6_000)));

    let plan = budget.plan_compare(&[pane], 4).expect("proxy fallback");

    assert!(plan.original_panes().is_empty());
    assert_eq!(plan.downgraded_panes(), &[EntityId::from_u128(7)]);
}

#[test]
fn invalid_or_overflowing_original_dimensions_keep_the_proxy_plan() {
    let budget = DecodeBudget::new(u64::MAX, u64::MAX);
    let plan = budget
        .plan_compare(
            &[
                request(1, (100, 100), Some((0, 200))),
                request(2, (100, 100), Some((u32::MAX, u32::MAX))),
            ],
            u8::MAX,
        )
        .expect("valid proxies must survive unsafe originals");

    assert!(plan.original_panes().is_empty());
    assert_eq!(
        plan.downgraded_panes(),
        &[EntityId::from_u128(1), EntityId::from_u128(2)]
    );
}

#[test]
fn replanning_exposes_removed_panes_for_request_cancellation() {
    let budget = DecodeBudget::new(700_000_000, 100_000_000);
    let initial = budget
        .plan_compare(
            &[
                request(1, (2_000, 1_500), None),
                request(2, (2_000, 1_500), None),
                request(3, (2_000, 1_500), None),
            ],
            4,
        )
        .unwrap();
    let next = budget
        .plan_compare(
            &[
                request(1, (2_000, 1_500), None),
                request(3, (2_000, 1_500), None),
            ],
            4,
        )
        .unwrap();

    assert_eq!(initial.cancelled_panes(&next), vec![EntityId::from_u128(2)]);
    let empty = budget.plan_compare(&[], 4).expect("empty release plan");
    assert_eq!(
        initial.cancelled_panes(&empty),
        vec![
            EntityId::from_u128(1),
            EntityId::from_u128(2),
            EntityId::from_u128(3),
        ]
    );
}

#[test]
fn invalid_or_unbounded_proxy_sets_are_rejected_without_a_partial_plan() {
    let budget = DecodeBudget::new(32_000_000, 100_000_000);
    assert!(
        budget
            .plan_compare(&[request(1, (4_000, 4_000), None)], 4)
            .is_none()
    );
    assert!(
        budget
            .plan_compare(
                &[request(1, (100, 100), None), request(1, (100, 100), None),],
                4,
            )
            .is_none()
    );
    assert!(
        budget
            .plan_compare(&[request(1, (0, 100), None)], 4)
            .is_none()
    );
    assert!(
        budget
            .plan_compare(&[request(1, (100, 100), None)], 0)
            .is_none()
    );
    assert!(
        budget
            .plan_compare(
                &(1..=5)
                    .map(|id| request(id, (100, 100), None))
                    .collect::<Vec<_>>(),
                4,
            )
            .is_none()
    );

    let exact = DecodeBudget::new(40_000, 100_000_000);
    assert_eq!(
        exact
            .plan_compare(&[request(9, (100, 100), None)], 4)
            .unwrap()
            .reserved_bytes(),
        40_000
    );
}
