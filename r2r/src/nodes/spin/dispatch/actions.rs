use super::*;

#[derive(Default, Clone, Copy)]
pub(super) struct ClientReady {
    pub(super) feedback: bool,
    pub(super) status: bool,
    pub(super) goal_response: bool,
    pub(super) cancel_response: bool,
    pub(super) result_response: bool,
}

/// Readiness sampling stays on the native owner; channel takes retain their own bounded
/// budgets and all constructed completions run through the existing post-lock boundary.
pub(super) fn dispatch_client_ready(
    ac: &Arc<Mutex<dyn ActionClient_>>, ready: ClientReady, errors: &mut DispatchErrors,
) {
    let ac_ref = Arc::clone(ac);
    if ready.feedback {
        errors.record(complete_action_feedback_batch(
            ac,
            |client, completions| client.handle_feedback_msg(ac_ref.clone(), completions),
            |client, uuid| client.forget_goal(uuid),
        ));
    }
    if ready.status {
        errors.record(complete_action_client_operation(
            ac,
            |client, completions| {
                client
                    .handle_status_msg(ac_ref.clone())
                    .map(|ready| completions.extend(ready))
            },
            |client, uuid| client.forget_goal(uuid),
        ));
    }
    if ready.goal_response {
        errors.record(complete_action_client_response_batch(
            ac,
            |client, completions| client.handle_goal_response(ac_ref.clone(), completions),
            |client, uuid| client.forget_goal(uuid),
        ));
    }
    if ready.cancel_response {
        errors.record(complete_action_client_response_batch(
            ac,
            ActionClient_::handle_cancel_response,
            |client, uuid| client.forget_goal(uuid),
        ));
    }
    if ready.result_response {
        errors.record(complete_action_client_response_batch(
            ac,
            |client, completions| client.handle_result_response(ac_ref.clone(), completions),
            |client, uuid| client.forget_goal(uuid),
        ));
    }
}

impl WaitEntities<'_> {
    pub(super) fn dispatch_actions(&mut self, ws: &rcl_wait_set_t, errors: &mut DispatchErrors) {
        for ac in self.action_clients.iter() {
            let ready = {
                let ac = ac.lock();
                let mut ready = ClientReady::default();
                let ret = unsafe {
                    rcl_action_client_wait_set_get_entities_ready(
                        ws,
                        ac.handle(),
                        &mut ready.feedback,
                        &mut ready.status,
                        &mut ready.goal_response,
                        &mut ready.cancel_response,
                        &mut ready.result_response,
                    )
                };
                if ret != RCL_RET_OK as i32 {
                    errors.error(Error::from_rcl_error(ret));
                    continue;
                }
                ready
            };

            dispatch_client_ready(ac, ready, errors);
        }

        for s in self.action_servers.iter() {
            let (
                server_id,
                is_goal_request_ready,
                is_cancel_request_ready,
                is_result_request_ready,
                is_goal_expired,
            ) = {
                let server = s.lock();
                let mut is_goal_request_ready = false;
                let mut is_cancel_request_ready = false;
                let mut is_result_request_ready = false;
                let mut is_goal_expired = false;

                let ret = unsafe {
                    rcl_action_server_wait_set_get_entities_ready(
                        ws,
                        server.handle(),
                        &mut is_goal_request_ready,
                        &mut is_cancel_request_ready,
                        &mut is_result_request_ready,
                        &mut is_goal_expired,
                    )
                };

                if ret != RCL_RET_OK as i32 {
                    errors.error(Error::from_rcl_error(ret));
                    continue;
                }
                (
                    server.id(),
                    is_goal_request_ready,
                    is_cancel_request_ready,
                    is_result_request_ready,
                    is_goal_expired,
                )
            };

            if is_goal_request_ready {
                errors.record(complete_action_server_request_batch(s, |server| {
                    server.handle_goal_request(s.clone())
                }));
            }
            if is_cancel_request_ready {
                let cancel_result = complete_action_server_delivery_batch(
                    s,
                    |server| server.handle_cancel_request(s.clone()),
                    ActionServer_::has_pending_cancel_delivery,
                );
                if s.lock().has_pending_cancel_work() {
                    crate::action_servers::notifications::enqueue_ready(
                        self.action_server_notifications,
                        server_id,
                    );
                }
                errors.record(cancel_result);
            }
            if is_result_request_ready {
                errors.record(complete_action_server_take_batch(s, |server| {
                    server.handle_result_request(s.clone())
                }));
            }
            errors.record(complete_action_server_operation(s, |server, completions| {
                if is_goal_expired {
                    server.handle_goal_expired(completions)?;
                }
                Ok(())
            }));
        }
    }
}
