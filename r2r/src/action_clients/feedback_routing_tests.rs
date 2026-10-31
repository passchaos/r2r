//! Rejected payload ownership must leave the native client lock before destruction.

use super::*;
use crate::{
    action_clients_untyped::WrappedActionClientUntyped, example_interfaces::action::Fibonacci,
};

fn typed_client() -> Arc<Mutex<dyn ActionClient_>> {
    typed_state(HashMap::new(), Arc::new(Mutex::new(Default::default())))
}

pub(super) fn typed_state(
    feedback_senders: HashMap<uuid::Uuid, FeedbackSender<Fibonacci::Feedback>>,
    pending: Arc<Mutex<PendingFeedbackQueue>>,
) -> Arc<Mutex<dyn ActionClient_>> {
    Arc::new(Mutex::new(WrappedActionClient::<Fibonacci::Action> {
        rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
        goal_response_channels: HashMap::new(),
        goal_response_sequences: HashMap::new(),
        cancel_response_channels: HashMap::new(),
        canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
        feedback_senders,
        canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
        pending_feedback_goals: pending,
        native_feedback_senders: HashMap::new(),
        canceled_native_feedback_goals: Arc::new(Mutex::new(Default::default())),
        pending_native_feedback_goals: Arc::new(Mutex::new(Default::default())),
        result_requests: HashMap::new(),
        result_request_sequences: HashMap::new(),
        result_senders: HashMap::new(),
        canceled_result_goals: Arc::new(Mutex::new(Default::default())),
        goal_status: HashMap::new(),
        feedback_channel_capacity: 0,
        feedback_native_buffer: Default::default(),
        goal_response_native_buffer: Default::default(),
        status_native_buffer: Default::default(),
        result_response_native_buffer: Default::default(),
        poll_available_channels: AvailabilityWaiters::default(),
    }))
}

fn untyped_client() -> Arc<Mutex<dyn ActionClient_>> {
    Arc::new(Mutex::new(WrappedActionClientUntyped {
        action_type_support: UntypedActionSupport::new_from("example_interfaces/action/Fibonacci")
            .unwrap(),
        rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
        goal_response_channels: HashMap::new(),
        goal_response_sequences: HashMap::new(),
        cancel_response_channels: HashMap::new(),
        canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
        feedback_senders: HashMap::new(),
        canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
        pending_feedback_goals: Arc::new(Mutex::new(Default::default())),
        result_requests: HashMap::new(),
        result_request_sequences: HashMap::new(),
        result_senders: HashMap::new(),
        canceled_result_goals: Arc::new(Mutex::new(Default::default())),
        goal_status: HashMap::new(),
        feedback_channel_capacity: 0,
        feedback_native_buffer: Default::default(),
        poll_available_channels: AvailabilityWaiters::default(),
    }))
}

struct DropProbe {
    client: Weak<Mutex<dyn ActionClient_>>,
    drops: Arc<AtomicUsize>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        let client = self.client.upgrade().unwrap();
        assert!(
            client.try_lock().is_some(),
            "rejected feedback payload destroyed under native client lock"
        );
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

fn rejected(client: Arc<Mutex<dyn ActionClient_>>) {
    let drops = Arc::new(AtomicUsize::new(0));
    let probe = DropProbe {
        client: Arc::downgrade(&client),
        drops: drops.clone(),
    };
    let completion = action_feedback_decode_completion(client, (), move |_| {
        Ok((uuid::Uuid::from_u128(1), Box::new(probe)))
    });
    assert!(matches!((completion.notify)(), Err(Error::RCL_RET_ACTION_CLIENT_INVALID)));
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[test]
fn typed_rejected_feedback_payload_drops_after_client_unlock() {
    rejected(typed_client());
}

#[test]
fn untyped_rejected_feedback_payload_drops_after_client_unlock() {
    rejected(untyped_client());
}
