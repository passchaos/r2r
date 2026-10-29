use super::super::actions::{dispatch_client_ready, ClientReady};
use super::*;
use crate::{
    action_clients::{
        action_stateful_response_completion, ActionClientCompletion, ActionClientTakeResult,
    },
    GoalStatus,
};
use futures::channel::oneshot;
use std::{any::Any, sync::Weak};

struct ScriptClient {
    owner: Option<Weak<Mutex<ScriptClient>>>,
    calls: [usize; 5],
    quota: [usize; 5],
    failures: [bool; 5],
    receivers: Vec<oneshot::Receiver<usize>>,
    delivered: Arc<Mutex<Vec<usize>>>,
}

impl ScriptClient {
    fn take(
        &mut self, channel: usize, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        self.calls[channel] += 1;
        if self.failures[channel] {
            return Err(match channel {
                0 => Error::RCL_RET_INVALID_ARGUMENT,
                1 => Error::RCL_RET_ERROR,
                _ => Error::RCL_RET_BAD_ALLOC,
            });
        }
        if self.calls[channel] > self.quota[channel] {
            return Ok(ActionClientTakeResult::Idle);
        }
        let value = channel * 100 + self.calls[channel];
        let owner = self.owner.clone().unwrap();
        let delivered = self.delivered.clone();
        let (sender, receiver) = oneshot::channel();
        self.receivers.push(receiver);
        completions.push(action_stateful_response_completion(
            sender,
            value,
            move |value| {
                let client = owner.upgrade().unwrap();
                assert!(client.try_lock().is_some(), "action completion held its client lock");
                delivered.lock().push(value);
                (value, Vec::new())
            },
            "ready dispatch script",
            None,
        ));
        Ok(ActionClientTakeResult::Taken)
    }
}

impl ActionClient_ for ScriptClient {
    fn handle(&self) -> &rcl_action_client_t {
        panic!("channel dispatch sampled native readiness")
    }
    fn destroy(&mut self, _: &mut rcl_node_t) {}
    fn has_pending_work(&mut self) -> bool {
        false
    }
    fn forget_goal(&mut self, _: &uuid::Uuid) -> Vec<ActionClientCompletion> {
        Vec::new()
    }
    fn finish_goal(&mut self, _: &uuid::Uuid, _: GoalStatus) -> Vec<ActionClientCompletion> {
        Vec::new()
    }
    fn apply_status_updates(
        &mut self, _: &[(uuid::Uuid, GoalStatus)],
    ) -> Vec<ActionClientCompletion> {
        Vec::new()
    }
    fn route_feedback(
        &mut self, _: &uuid::Uuid, _: Box<dyn Any>,
    ) -> Result<Option<ActionClientCompletion>> {
        panic!("script feedback routing")
    }
    fn handle_goal_response(
        &mut self, _: Arc<Mutex<dyn ActionClient_>>, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        self.take(2, completions)
    }
    fn handle_cancel_response(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        self.take(3, completions)
    }
    fn handle_feedback_msg(
        &mut self, _: Arc<Mutex<dyn ActionClient_>>, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        self.take(0, completions)
    }
    fn handle_status_msg(
        &mut self, _: Arc<Mutex<dyn ActionClient_>>,
    ) -> Result<Vec<ActionClientCompletion>> {
        let mut completions = Vec::new();
        self.take(1, &mut completions)?;
        Ok(completions)
    }
    fn handle_result_response(
        &mut self, _: Arc<Mutex<dyn ActionClient_>>, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        self.take(4, completions)
    }
    fn register_poll_available(&mut self, _: oneshot::Sender<()>) -> bool {
        false
    }
    fn poll_maintenance(&mut self) -> (Vec<ActionClientCompletion>, bool) {
        (Vec::new(), false)
    }
    fn has_pending_maintenance(&self) -> bool {
        false
    }
    fn disable_maintenance_notifications(&self) {}
    fn poll_available(&mut self, _: &mut rcl_node_t) -> Vec<ActionClientCompletion> {
        Vec::new()
    }
    fn has_pending_availability(&mut self) -> bool {
        false
    }
}

fn run(
    ready: ClientReady, quota: [usize; 5], failures: [bool; 5],
) -> (Result<()>, [usize; 5], Vec<usize>) {
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let script = Arc::new(Mutex::new(ScriptClient {
        owner: None,
        calls: [0; 5],
        quota,
        failures,
        receivers: Vec::new(),
        delivered: delivered.clone(),
    }));
    let client: Arc<Mutex<dyn ActionClient_>> = script.clone();
    script.lock().owner = Some(Arc::downgrade(&script));
    let mut errors = DispatchErrors::default();
    dispatch_client_ready(&client, ready, &mut errors);
    let (calls, receivers) = {
        let mut script = script.lock();
        (script.calls, std::mem::take(&mut script.receivers))
    };
    let values = receivers
        .into_iter()
        .map(|receiver| receiver.now_or_never().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(*delivered.lock(), values);
    (errors.result(), calls, values)
}

fn all_ready() -> ClientReady {
    ClientReady {
        feedback: true,
        status: true,
        goal_response: true,
        cancel_response: true,
        result_response: true,
    }
}

#[test]
fn ready_dispatch_action_channel_errors_preserve_later_responses_and_post_lock_completions() {
    let (result, calls, delivered) =
        run(all_ready(), [20, 1, 20, 20, 3], [true, true, false, true, false]);
    assert!(matches!(result, Err(Error::RCL_RET_INVALID_ARGUMENT)));
    assert_eq!(calls, [1, 1, 16, 1, 4]);
    assert_eq!(delivered, (201..=216).chain(401..=403).collect::<Vec<_>>());
}

#[test]
fn ready_dispatch_action_channels_keep_independent_sixteen_take_budgets() {
    let (result, calls, delivered) = run(all_ready(), [20, 1, 20, 20, 20], [false; 5]);
    result.unwrap();
    assert_eq!(calls, [16, 1, 16, 16, 16]);
    assert_eq!(
        delivered,
        (1..=16)
            .chain([101])
            .chain(201..=216)
            .chain(301..=316)
            .chain(401..=416)
            .collect::<Vec<_>>()
    );
}

#[test]
fn ready_dispatch_action_channels_skip_unready_sources() {
    let (result, calls, delivered) = run(
        ClientReady {
            result_response: true,
            ..Default::default()
        },
        [1; 5],
        [false; 5],
    );
    result.unwrap();
    assert_eq!(calls, [0, 0, 0, 0, 2]);
    assert_eq!(delivered, [401]);
}
