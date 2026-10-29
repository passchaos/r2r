use super::*;

#[test]
fn callback_service_options_reject_zero_before_native_endpoint_creation() {
    assert_eq!(CallbackServiceOptions::default().max_requests_per_spin, 16);
    assert!(matches!(
        CallbackServiceOptions::default()
            .max_requests_per_spin(0)
            .validate(),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}

#[test]
fn callback_service_token_drop_closes_only_its_take_flag_and_queues_targeted_cleanup() {
    let id = uuid::Uuid::new_v4();
    let alive = Arc::new(AtomicBool::new(true));
    let canceled = Arc::new(Mutex::new(Default::default()));
    let token = CallbackService::new(id, alive.clone(), canceled.clone());
    drop(token);
    assert!(!alive.load(Ordering::Acquire));
    assert_eq!(canceled.lock().pop_front(), Some(id));
    assert!(canceled.lock().is_empty());
}
