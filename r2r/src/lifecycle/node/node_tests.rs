//! Managed node tests driven over native r2r service clients and subscribers.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};

use super::*;
use crate::lifecycle_msgs::msg::{State, Transition, TransitionEvent};
use crate::lifecycle_msgs::srv::{
    ChangeState, GetAvailableStates, GetAvailableTransitions, GetState,
};
use crate::{Context, Executor, QosProfile};

/// Spin both nodes until the service future resolves or the deadline passes.
fn drive<T, F>(
    managed: &mut LifecycleNode, client_node: &mut crate::Node, future: crate::Result<F>,
) -> T
where
    F: std::future::Future<Output = crate::Result<T>>,
{
    let future = future.unwrap();
    futures::pin_mut!(future);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = future.as_mut().now_or_never() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline, "lifecycle service call timed out");
        managed.try_spin_once(Duration::ZERO).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
        std::thread::yield_now();
    }
}

/// Spin an executor-owned managed node and a client node until the future
/// resolves.
fn drive_executor<T, F>(
    executor: &mut Executor, client_node: &mut crate::Node, future: crate::Result<F>,
) -> T
where
    F: std::future::Future<Output = crate::Result<T>>,
{
    let future = future.unwrap();
    futures::pin_mut!(future);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = future.as_mut().now_or_never() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline, "executor lifecycle call timed out");
        executor.spin_once(Duration::from_millis(5)).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
        std::thread::yield_now();
    }
}

fn unique(name: &str) -> String {
    format!("{name}_{}", uuid::Uuid::new_v4().simple())
}

#[test]
fn get_state_reports_unconfigured_after_creation() {
    let context = Context::create().unwrap();
    let name = unique("lc_get_state");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut client_node = crate::Node::create(context, "lc_get_state_client", "").unwrap();

    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);

    let client = client_node
        .create_client::<GetState::Service>(&format!("/{name}/get_state"), QosProfile::default())
        .unwrap();
    let response = drive(&mut managed, &mut client_node, client.request(&GetState::Request {}));
    assert_eq!(response.current_state.id, State::PRIMARY_STATE_UNCONFIGURED as u8);
    assert_eq!(response.current_state.label.as_bytes(), b"unconfigured");
}

#[test]
fn available_states_list_all_registered_states() {
    let context = Context::create().unwrap();
    let name = unique("lc_states");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut client_node = crate::Node::create(context, "lc_states_client", "").unwrap();

    let client = client_node
        .create_client::<GetAvailableStates::Service>(
            &format!("/{name}/get_available_states"),
            QosProfile::default(),
        )
        .unwrap();
    let response =
        drive(&mut managed, &mut client_node, client.request(&GetAvailableStates::Request {}));
    let ids: Vec<u8> = response.available_states.iter().map(|s| s.id).collect();
    // rclcpp returns every map state, including unknown and intermediates.
    assert_eq!(ids, vec![0, 1, 2, 3, 4, 10, 11, 12, 13, 14, 15]);
}

#[test]
fn available_transitions_and_graph_match_current_and_full_maps() {
    let context = Context::create().unwrap();
    let name = unique("lc_graph_srv");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut client_node = crate::Node::create(context, "lc_graph_client", "").unwrap();

    let available = client_node
        .create_client::<GetAvailableTransitions::Service>(
            &format!("/{name}/get_available_transitions"),
            QosProfile::default(),
        )
        .unwrap();
    let response = drive(
        &mut managed,
        &mut client_node,
        available.request(&GetAvailableTransitions::Request {}),
    );
    let ids: Vec<u8> = response
        .available_transitions
        .iter()
        .map(|d| d.transition.id)
        .collect();
    assert_eq!(ids, vec![1, 5]);
    let configure = &response.available_transitions[0];
    assert_eq!(configure.transition.label.as_bytes(), b"configure");
    assert_eq!(configure.start_state.id, State::PRIMARY_STATE_UNCONFIGURED as u8);
    assert_eq!(configure.goal_state.id, State::TRANSITION_STATE_CONFIGURING as u8);

    let graph = client_node
        .create_client::<GetAvailableTransitions::Service>(
            &format!("/{name}/get_transition_graph"),
            QosProfile::default(),
        )
        .unwrap();
    let response = drive(
        &mut managed,
        &mut client_node,
        graph.request(&GetAvailableTransitions::Request {}),
    );
    assert_eq!(response.available_transitions.len(), 25);
}

#[test]
fn configure_service_runs_callback_and_reports_inactive() {
    let context = Context::create().unwrap();
    let name = unique("lc_configure_srv");
    let observed: Arc<Mutex<Vec<LifecycleState>>> = Arc::new(Mutex::new(Vec::new()));
    let observed_cb = observed.clone();
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed.on_configure(Box::new(move |previous| {
        observed_cb.lock().unwrap().push(previous);
        CallbackReturn::Success
    }));
    let mut client_node = crate::Node::create(context, "lc_configure_client", "").unwrap();

    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();
    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Inactive);
    assert_eq!(*observed.lock().unwrap(), vec![LifecycleState::Unconfigured]);

    let get_state = client_node
        .create_client::<GetState::Service>(&format!("/{name}/get_state"), QosProfile::default())
        .unwrap();
    let response = drive(&mut managed, &mut client_node, get_state.request(&GetState::Request {}));
    assert_eq!(response.current_state.id, State::PRIMARY_STATE_INACTIVE as u8);
}

#[test]
fn label_based_change_takes_precedence_over_id() {
    let context = Context::create().unwrap();
    let name = unique("lc_label");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut client_node = crate::Node::create(context, "lc_label_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    // id stays zero (as ros2 service call defaults) but the label resolves.
    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: 0,
                label: "configure".into(),
            },
        }),
    );
    assert!(response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Inactive);
}

#[test]
fn invalid_transition_is_rejected_without_moving_state() {
    let context = Context::create().unwrap();
    let name = unique("lc_invalid_srv");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut client_node = crate::Node::create(context, "lc_invalid_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    // activate is not available from unconfigured.
    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_ACTIVATE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(!response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);
}

#[test]
fn configure_failure_returns_to_unconfigured() {
    let context = Context::create().unwrap();
    let name = unique("lc_failure_srv");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed.on_configure(Box::new(|_| CallbackReturn::failure("nope")));
    let mut client_node = crate::Node::create(context, "lc_failure_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(!response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);
}

#[test]
fn configure_error_invokes_error_callback_and_recovers() {
    let context = Context::create().unwrap();
    let name = unique("lc_error_srv");
    let calls: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let configure_calls = calls.clone();
    let error_calls = calls.clone();
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed
        .on_configure(Box::new(move |_| {
            configure_calls.lock().unwrap().push("configure");
            CallbackReturn::error("boom")
        }))
        .on_error(Box::new(move |_| {
            error_calls.lock().unwrap().push("error");
            CallbackReturn::Success
        }));
    let mut client_node = crate::Node::create(context, "lc_error_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    // Primary callback errored, so success stays false even after recovery.
    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(!response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);
    assert_eq!(*calls.lock().unwrap(), vec!["configure", "error"]);
}

#[test]
fn fatal_error_finalizes_the_node() {
    let context = Context::create().unwrap();
    let name = unique("lc_fatal_srv");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed
        .on_configure(Box::new(|_| CallbackReturn::Error("boom".into())))
        .on_error(Box::new(|_| CallbackReturn::failure("cannot recover")));
    let mut client_node = crate::Node::create(context, "lc_fatal_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    let response = drive(
        &mut managed,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(!response.success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Finalized);
}

#[test]
fn direct_rust_transitions_run_full_happy_path() {
    let context = Context::create().unwrap();
    let managed = LifecycleNode::create(context, &unique("lc_direct_happy"), "").unwrap();

    assert_eq!(managed.configure().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Inactive);
    assert_eq!(managed.activate().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Active);
    assert_eq!(managed.deactivate().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Inactive);
    assert_eq!(managed.cleanup().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);
    assert_eq!(managed.shutdown().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Finalized);

    // finalized has no outgoing edges: configure reports failure and does
    // not move the state.
    assert!(matches!(managed.configure(), Ok(CallbackReturn::Failure(_))));
    assert_eq!(managed.lifecycle_state(), LifecycleState::Finalized);
}

#[test]
fn transition_events_are_published_for_each_state_change() {
    let context = Context::create().unwrap();
    let name = unique("lc_events");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut subscriber_node = crate::Node::create(context, "lc_events_sub", "").unwrap();
    let mut events = subscriber_node
        .subscribe::<TransitionEvent>(&format!("/{name}/transition_event"), QosProfile::default())
        .unwrap();

    // Give discovery time to connect before triggering transitions.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        subscriber_node
            .try_spin_once(Duration::from_millis(5))
            .unwrap();
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    managed.configure().unwrap();

    // configure publishes the external edge and the success edge.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received = Vec::new();
    while received.len() < 2 && Instant::now() < deadline {
        managed.try_spin_once(Duration::from_millis(5)).unwrap();
        subscriber_node
            .try_spin_once(Duration::from_millis(5))
            .unwrap();
        if let Some(event) = events.next().now_or_never().flatten() {
            received.push(event);
        }
    }
    assert_eq!(received.len(), 2, "expected configure + success events");
    assert_eq!(received[0].transition.id, Transition::TRANSITION_CONFIGURE as u8);
    assert_eq!(received[0].start_state.id, State::PRIMARY_STATE_UNCONFIGURED as u8);
    assert_eq!(received[0].goal_state.id, State::TRANSITION_STATE_CONFIGURING as u8);
    assert_eq!(received[1].transition.id, Transition::TRANSITION_ON_CONFIGURE_SUCCESS as u8);
    assert_eq!(received[1].start_state.id, State::TRANSITION_STATE_CONFIGURING as u8);
    assert_eq!(received[1].goal_state.id, State::PRIMARY_STATE_INACTIVE as u8);
}

#[test]
fn lifecycle_services_work_through_unified_executor() {
    let context = Context::create().unwrap();
    let name = unique("lc_executor");
    let managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed.on_configure(Box::new(|_| CallbackReturn::Success));

    let mut client_node = crate::Node::create(context.clone(), "lc_executor_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    let mut executor = Executor::new(context);
    let managed_id = executor.add_node(managed.into_node()).unwrap();

    let response = drive_executor(
        &mut executor,
        &mut client_node,
        client.request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        }),
    );
    assert!(response.success);

    let get_state = client_node
        .create_client::<GetState::Service>(&format!("/{name}/get_state"), QosProfile::default())
        .unwrap();
    let response =
        drive_executor(&mut executor, &mut client_node, get_state.request(&GetState::Request {}));
    assert_eq!(response.current_state.id, State::PRIMARY_STATE_INACTIVE as u8);

    // The extracted runtime is torn down with the node at executor removal.
    let extracted = executor.remove_node(managed_id).unwrap();
    drop(extracted);
}
